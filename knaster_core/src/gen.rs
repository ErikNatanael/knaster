pub mod math;
pub mod osc;

use knaster_primitives::{typenum::*, Block, BlockRead, Float, Frame, Size};

/// Contains basic metadata about the context in which an audio process is
/// running which is often necessary for correct calculation, initialisation etc.
#[derive(Clone, Copy, Debug)]
pub struct AudioCtx {
    sample_rate: u32,
    block_size: usize,
    flags: GenFlags,
}
impl AudioCtx {
    pub fn new(sample_rate: u32, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            flags: GenFlags::default(),
        }
    }
    pub fn block_size(&self) -> usize {
        self.block_size
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    /// Set the flag to remove this node. Returns true if freeing self is
    /// supported by the node tree and false if it isn't. If this returns false,
    /// the node will not be removed.
    pub fn remove_self(&mut self) -> bool {
        self.flags.remove_self = true;
        self.flags.remove_self_supported
    }
    /// Set the flag to remove the graph that owns this node and from which frame.
    ///
    /// From and including the frame number specified, the graph will output 0.0
    /// on all channels until removed. To output the entire block, set
    /// `from_frame_in_block` to the current block size.
    pub fn remove_graph(&mut self, from_frame_in_block: u32) {
        self.flags.remove_graph = true;
        self.flags.remove_graph_from_frame_in_block = from_frame_in_block;
    }
    pub fn combine_flag_state(&mut self, other: &mut AudioCtx) {
        self.flags.remove_graph = self.flags.remove_graph || other.flags.remove_graph;
        if self.flags.remove_graph {
            self.flags.remove_graph_from_frame_in_block = self
                .flags
                .remove_graph_from_frame_in_block
                .min(other.flags.remove_graph_from_frame_in_block);
        }
        self.flags.remove_self = self.flags.remove_self || other.flags.remove_self;
    }
    pub fn flags_mut(&mut self) -> &mut GenFlags {
        &mut self.flags
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
pub struct BlockAudioCtx {
    audio_ctx: AudioCtx,
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
impl BlockAudioCtx {
    pub fn new(sample_rate: u32, block_size: usize) -> Self {
        Self {
            audio_ctx: AudioCtx::new(sample_rate, block_size),
            block_start_offset: 0,
            frames_to_process: block_size,
            frame_clock: 0,
        }
    }
    pub fn make_partial(&self, start_offset: usize, length: usize) -> BlockAudioCtx {
        let mut new = *self;
        new.block_start_offset += start_offset;
        new.frames_to_process = length;
        new.frame_clock += start_offset as u64;
        new
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
    pub fn block_size(&self) -> usize {
        self.audio_ctx.block_size
    }
    pub fn sample_rate(&self) -> u32 {
        self.audio_ctx.sample_rate
    }
    /// Set the flag to remove this node. Returns true if freeing self is
    /// supported by the node tree and false if it isn't. If this returns false,
    /// the node will not be removed.
    pub fn remove_self(&mut self) -> bool {
        self.audio_ctx.remove_self()
    }
    /// Set the flag to remove the graph that owns this node and from which frame.
    ///
    /// From and including the frame number specified, the graph will output 0.0
    /// on all channels until removed. To output the entire block, set
    /// `from_frame_in_block` to the current block size.
    pub fn remove_graph(&mut self, from_frame_in_block: u32) {
        self.audio_ctx.remove_graph(from_frame_in_block);
    }
    pub fn combine_flag_state(&mut self, other: &mut BlockAudioCtx) {
        self.audio_ctx.combine_flag_state(other.into())
    }
    pub fn flags_mut(&mut self) -> &mut GenFlags {
        &mut self.audio_ctx.flags
    }
}
impl<'a> From<&'a mut BlockAudioCtx> for &'a mut AudioCtx {
    fn from(val: &'a mut BlockAudioCtx) -> Self {
        &mut val.audio_ctx
    }
}
impl<'a> From<&'a BlockAudioCtx> for &'a AudioCtx {
    fn from(val: &'a BlockAudioCtx) -> Self {
        &val.audio_ctx
    }
}
// Why do we need this? But it works.
impl<'a> From<&'a mut AudioCtx> for &'a AudioCtx {
    fn from(val: &'a mut AudioCtx) -> Self {
        val
    }
}
impl<'a> From<&'a mut BlockAudioCtx> for &'a AudioCtx {
    fn from(val: &'a mut BlockAudioCtx) -> Self {
        &val.audio_ctx
    }
}
/// Used for carrying some basic state up through the tree of wrappers.
/// Currently only used for freeing nodes.
#[derive(Copy, Clone, Debug)]
pub struct GenFlags {
    /// Will be set to true by a wrapper if self freeing it supported on the
    /// current node, otherwise it will be false. This is purely diagnostic to
    /// display an error if you try to free a node that cannot be freed.
    ///
    /// TODO: Make it part of a diagnostics/debug feature
    remove_self_supported: bool,
    /// If the local node should be freed. Requires a wrapper to free it.
    remove_self: bool,
    /// Set to true if the graph should be freed within this block
    remove_graph: bool,
    /// The frame at which the graph should be freed. From (including) that
    /// frame, the graph output will be 0 until it is removed.
    remove_graph_from_frame_in_block: u32,
}
impl GenFlags {
    /// If the graph should be removed this returns Some(u32) where the u32 is
    /// the frame in the current block from which the graph should output 0. The
    /// frame number may be larger than the current block, in which case the
    /// whole block should be output as usual.
    pub fn remove_graph(&self) -> Option<u32> {
        if self.remove_graph {
            Some(self.remove_graph_from_frame_in_block)
        } else {
            None
        }
    }
    pub fn remove_self(&self) -> bool {
        self.remove_self
    }
    pub fn set_remove_self(&mut self) {
        if self.remove_self_supported {
            self.remove_self = true;
        } else {
            // TODO: report error
        }
    }
    pub fn set_remove_graph(&mut self, from_frame: u32) {
        self.remove_graph = true;
        self.remove_graph_from_frame_in_block = from_frame;
    }
    pub fn clear_graph_flags(&mut self) {
        self.remove_graph = false;
        self.remove_graph_from_frame_in_block = u32::MAX;
    }
    pub fn clear_node_flags(&mut self) {
        self.remove_self = false;
        self.remove_self_supported = false;
    }
}
impl Default for GenFlags {
    fn default() -> Self {
        Self {
            remove_self_supported: false,
            remove_self: false,
            remove_graph: false,
            remove_graph_from_frame_in_block: u32::MAX,
        }
    }
}
pub trait Gen {
    /// The type of float (f32 or f64) that this Gen is implemented for. It is
    /// recommended to implement your types be generic over [`Float`].
    type Sample: Float;
    /// The number of input audio channels.
    type Inputs: Size;
    /// The number of output audio channels.
    type Outputs: Size;
    /// `init()` is the place where you should implement initialisation of
    /// internal state with knowledge of the sample rate and the block size. It
    /// is safe to allocate here.
    #[allow(unused)]
    fn init(&mut self, ctx: &AudioCtx) {}
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs>;

    /// Processes one block of frames
    ///
    /// Knaster supports adaptive partial blocks. This means that block
    /// the `Gen::process_block` function may run more than once per global block
    /// size number of frames.
    ///
    /// `process_block` will be run at least once per global block.
    ///
    /// The information about partial blocks is available in [`BlockAudioCtx`]
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut BlockAudioCtx,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: BlockRead<Sample = Self::Sample>,
        OutBlock: Block<Sample = Self::Sample>,
    {
        for frame in 0..ctx.frames_to_process {
            // This is potentially a tiny bit inefficient because it initialises the memory before overwriting it.
            let mut in_frame = Frame::default();
            for i in 0..Self::Inputs::USIZE {
                in_frame[i] = input.read(i, frame);
            }
            let out_frame = self.process(ctx.into(), in_frame);
            for i in 0..Self::Outputs::USIZE {
                output.write(out_frame[i], i, frame);
            }
        }
    }
}
