#[cfg(any(feature = "std", feature = "alloc"))]
pub mod buffer;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod delay;

pub mod envelopes;
pub mod math;
pub mod noise;
pub mod onepole;
pub mod osc;
pub mod pan;
pub mod polyblep;
pub mod svf;
pub mod util;

#[cfg(feature = "std")]
use crate::core::eprintln;
use crate::log::ArLogSender;
use crate::numeric_array::NumericArray;
use crate::{rt_log, Param, ParameterError, ParameterHint, ParameterType, ParameterValue};
use knaster_primitives::{Block, BlockRead, Float, Frame, Size, typenum::*};

/// Contains basic metadata about the context in which an audio process is
/// running which is often necessary for correct calculation, initialisation etc.
pub struct AudioCtx {
    sample_rate: u32,
    block_size: usize,
    logger: ArLogSender,
    pub block: BlockMetadata,
}
impl AudioCtx {
    pub fn new(sample_rate: u32, block_size: usize, logger: ArLogSender) -> Self {
        Self {
            sample_rate,
            block_size,
            logger,
            block: BlockMetadata::new(block_size),
        }
    }
    pub fn block_size(&self) -> usize {
        self.block_size
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn logger(&mut self) -> &mut ArLogSender {
        &mut self.logger
    }
    pub fn frames_to_process(&self) -> usize {
        self.block.frames_to_process
    }
    pub fn block_start_offset(&self) -> usize {
        self.block.block_start_offset
    }
    pub fn frame_clock(&self) -> u64 {
        self.block.frame_clock
    }
}
/// [`AudioCtx`] + metadata about the current context of block processing.
/// Blocks can be divided up into multiple smaller blocks. If so, this ctx
/// provides that information.
///
/// Most [`Gen`]s don't need that info and can work straight off of the buffers
/// given, but some do need to know where they are within a standard block.
// TODO: Make BlockAudioCtx mutably borrow an AudioCtx to make partial blocks
// more efficient. This would avoid having to copy flags and parameters that
// don't change when a partial block is created.
#[derive(Clone, Copy, Debug)]
pub struct BlockMetadata {
    /// The offset of the current processing block from the start of any
    /// buffers. Does not need to be applied to a [`Block`] since they have
    /// already been adapted to the new block size. However, if you maintain
    /// some internal block sized data, this information can be useful.
    block_start_offset: usize,
    /// The size of the current processing block. Does not take offset into account.
    ///
    /// Example: We want to process 7 frames starting from frame 2.
    /// `frames_to_process = 7`, `block_start_offset = 2`.
    frames_to_process: usize,
    /// The current moment in time in frames since the process was started on
    /// the audio thread.
    ///
    /// Can be used to schedule changes such as when a node should start processing.
    frame_clock: u64,
}
impl BlockMetadata {
    pub fn new(block_size: usize) -> Self {
        Self {
            frames_to_process: block_size,
            block_start_offset: 0,
            frame_clock: 0,
        }
    }
    pub fn make_partial(&self, start_offset: usize, length: usize) -> BlockMetadata {
        Self {
            block_start_offset: self.block_start_offset + start_offset,
            frames_to_process: length,
            frame_clock: self.frame_clock + start_offset as u64,
        }
    }
    /// Substitute the frame clock time with your own. You almost never want to
    /// do this inside the graph.
    pub fn set_frame_clock(&mut self, new_frame_time: u64) {
        self.frame_clock = new_frame_time
    }
    pub fn frame_clock(&self) -> u64 {
        self.frame_clock
    }
    pub fn frames_to_process(&self) -> usize {
        self.frames_to_process
    }
    pub fn block_start_offset(&self) -> usize {
        self.block_start_offset
    }
}
/// Output state used for carrying some basic state up through the tree of Gens and wrappers_graph.
/// Currently only used for freeing nodes.
///
/// When a node wants to signal that it should be freed, it will set the flag
/// `remove_self`. When a node wants to signal that its parent should be freed, it will set
/// the flag `remove_container`. It is up to the graph implementations how the node will be
/// freed. In knaster_graph, `remove_self` requires a wrapper while `remove_parent` is built in.
#[derive(Copy, Clone, Debug)]
pub struct UGenFlags {
    /// Will be set to true by a wrapper if self freeing it supported on the
    /// current node, otherwise it will be false. This is purely diagnostic to
    /// display an error if you try to free a node that cannot be freed.
    ///
    /// TODO: Make it part of a diagnostics/debug feature
    remove_self_supported: bool,
    /// The [`Gen`] that is done sets this to true when done through [`Self::mark_done`]
    done: bool,
    /// What frame in the current block the node was done.
    done_frame_in_block: u32,
    /// If the local node should be freed. Requires a wrapper to free it.
    remove_self: bool,
    /// Set to true if the parent of this node should be freed within this block
    remove_parent: bool,
    /// The frame at which the parent should be freed. From (including) that
    /// frame, the graph output will be 0 until it is removed.
    remove_parent_from_frame_in_block: u32,
}
impl UGenFlags {
    pub fn new() -> Self {
        Self {
            remove_self_supported: false,
            done: false,
            done_frame_in_block: u32::MAX,
            remove_self: false,
            remove_parent: false,
            remove_parent_from_frame_in_block: u32::MAX,
        }
    }
    /// Returns Some(u32) if the graph should be removed, where the u32 is
    /// the frame in the current block from which the graph should output 0. The
    /// frame number may be larger than the current block, in which case the
    /// whole block should be output as usual.
    pub fn remove_graph(&self) -> Option<u32> {
        if self.remove_parent {
            Some(self.remove_parent_from_frame_in_block)
        } else {
            None
        }
    }
    pub fn remove_self(&self) -> bool {
        self.remove_self
    }
    pub fn done(&self) -> Option<u32> {
        if self.done {
            Some(self.done_frame_in_block)
        } else {
            None
        }
    }
    /// Set the flag to remove this node. Returns true if freeing self is
    /// supported by the node tree and false if it isn't. If this returns false,
    /// the node will not be removed.
    pub fn mark_remove_self(&mut self, ctx: &mut AudioCtx) {
        if self.remove_self_supported {
            self.remove_self = true;
        } else {
            rt_log!(ctx.logger(); "Warning: Remove self flag set, but not supported.");
        }
    }
    /// Set the flag to remove the graph that owns this node and from which frame.
    ///
    /// From and including the frame number specified, the graph will output 0.0
    /// on all channels until removed. To output the entire block, set
    /// `from_frame_in_block` to the current block size.
    pub fn mark_remove_parent(&mut self, from_frame: u32) {
        self.remove_parent = true;
        self.remove_parent_from_frame_in_block = from_frame;
    }
    /// Mark this node as done.
    ///
    /// The property can be read by a wrapper or the container/graph, which can take some action in
    /// response.
    pub fn mark_done(&mut self, from_frame: u32) {
        self.done = true;
        self.done_frame_in_block = from_frame;
    }
    pub fn mark_remove_self_supported(&mut self) {
        self.remove_self_supported = true;
    }
    pub fn clear_parent_flags(&mut self) {
        self.remove_parent = false;
        self.remove_parent_from_frame_in_block = u32::MAX;
    }
    pub fn clear_node_flags(&mut self) {
        self.remove_self = false;
        self.remove_self_supported = false;
        self.done = false;
        self.done_frame_in_block = u32::MAX;
    }
}
impl Default for UGenFlags {
    fn default() -> Self {
        Self::new()
    }
}
/// Defines a unit that can generate and/or process sound.
///
/// The UGen provides associated types for the number of inputs, outputs and parameters that the
/// UGen provides. These are given in `typenum` numbers, because of the limitations of const
/// generics in Rust at the time of writing. These numbers look like `U0`, `U1`, `U2` etc.
///
/// The name UGen stands for "unit generator" and originally comes from the MUSIC-N family of audio programming environments.
pub trait UGen {
    /// The type of float (f32 or f64) that this Gen is implemented for. It is
    /// recommended to implement your types be generic over [`Float`].
    type Sample: Float;
    /// The number of input audio channels.
    type Inputs: Size;
    /// The number of output audio channels.
    type Outputs: Size;
    /// The number of parameters this Gen has.
    type Parameters: Size;
    /// `init()` is the place where you should implement initialisation of
    /// internal state with knowledge of the sample rate and the block size. It
    /// is safe to allocate here.
    #[allow(unused)]
    fn init(&mut self, sample_rate: u32, block_size: usize) {}
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs>;

    /// Processes one block of frames
    ///
    /// Knaster supports adaptive partial blocks. This means that block
    /// the `Gen::process_block` function may run more than once per global block.
    ///
    /// `process_block` will be run at least once per global block.
    ///
    /// The information about partial blocks is available in [`BlockAudioCtx`]
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: BlockRead<Sample = Self::Sample>,
        OutBlock: Block<Sample = Self::Sample>,
    {
        for frame in 0..ctx.block.frames_to_process {
            // This is potentially a tiny bit inefficient because it initialises the memory before overwriting it.
            let mut in_frame = Frame::default();
            for i in 0..Self::Inputs::USIZE {
                in_frame[i] = input.read(i, frame);
            }
            let out_frame = self.process(ctx, flags, in_frame);
            for i in 0..Self::Outputs::USIZE {
                output.write(out_frame[i], i, frame);
            }
        }
    }

    /// Specifies which [`ParameterType`] each parameter is. If the types given to [`Gen::param_apply`] match
    /// these types, it is assumed that the parameter will be correctly applied by the Gen. It
    /// is also used by some wrappers_graph to implement type specific functionality.
    ///
    /// If not manually implemented, types are inferred from [`Gen::param_range`]
    fn param_types() -> NumericArray<ParameterType, Self::Parameters> {
        Self::param_hints().into_iter().map(|r| r.ty()).collect()
    }
    /// Specifies a name per parameter which can be used to refer to that parameter
    /// when calling [`Gen::param`].
    ///
    /// Not required to be implemented, but provides a better developer experience.
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        NumericArray::default()
    }
    /// Specifies the range that is valid for each parameter.
    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters>;
    /// Set the parameter value without range or type checks.
    /// See Parameterable::param for a safer and more ergonomic interface.
    ///
    /// Tries to apply the parameter change without checking the validity of the
    /// values. May panic or do nothing given unexpected values.
    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue);
    /// Set an audio buffer to control a parameter. Does nothing unless an
    /// `ArParams` wrapper or alternative wrapper making use of this value wraps the Gen.
    ///
    /// There is little use in calling this directly unless you are implementing
    /// a graph. If you are not using a `Graph`, using the ArParams wrapper and
    /// this function is equivalent to running frame-by-frame and setting the
    /// value every frame.
    ///
    /// # Safety
    /// The caller guarantees that the pointer will point to a
    /// contiguous allocation of at least block size until it is replaced,
    /// disabled, or the inner struct is dropped.
    #[allow(unused)]
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const Self::Sample,
    ) {
        rt_log!(ctx.logger(); "Warning: Audio rate parameter buffer set, but did not reach a WrArParams and will have no effect.");
    }
    /// Sets a delay to what frame within the next block the next parameter
    /// change should take effect.
    ///
    /// This will not have any effect unless a [`WrHiResParams`] wrapper is
    /// used, or the [`UGen`] supports it internally (none of the Knaster proper
    /// Gens do).
    ///
    /// Wrappers must propagagte this call.
    #[allow(unused)]
    fn set_delay_within_block_for_param(&mut self,ctx: &mut AudioCtx, index: usize, delay: u16) {
        rt_log!(ctx.logger(); "Warning: Parameter delay set, but did not reach a WrHiResParams and will have no effect.");
    }
    /// Apply a parameter change. Typechecks and bounds checks the arguments and
    /// provides sensible errors. Calls [`UGen::param_apply`] under the hood.
    fn param(
        &mut self,
        ctx: &mut AudioCtx,
        param: impl Into<Param>,
        value: impl Into<ParameterValue>,
    ) -> Result<(), ParameterError> {
        match param.into() {
            Param::Index(i) => {
                if i >= Self::Parameters::USIZE {
                    return Err(ParameterError::ParameterIndexOutOfBounds);
                }
                self.param_apply(ctx, i, value.into());
                Ok(())
            }
            Param::Desc(desc) => {
                for (i, d) in Self::param_descriptions().into_iter().enumerate() {
                    if d == desc {
                        self.param_apply(ctx, i, value.into());
                        return Ok(());
                    }
                }
                Err(ParameterError::DescriptionNotFound(desc))
            }
        }
    }
}
