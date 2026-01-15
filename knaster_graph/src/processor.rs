//! # Audio processor
//!
//! This module contains the [`AudioProcessor`] which is the main entry point for processing audio. It can either be placed in
//! in an [`AudioBackend`], placed in a different audio callback to produce blocks of audio, or manually called in a non-realtime
//! context for non-realtime processing.

use knaster_core::Seconds;
use knaster_core::log::ArLogReceiver;
use knaster_core::typenum::U1;
use knaster_core::{AudioCtx, Float, Size, UGenFlags, typenum::NonZero};

use crate::SharedFrameClock;
use crate::dynugen::DynUGen;
use crate::graph::NodeId;
use crate::{
    block::{RawAggregateBlockRead, RawContiguousBlock},
    graph::{Graph, GraphOptions, OwnedRawBuffer},
    node::Node,
};

/// Options for creating a new [`AudioProcessor`].
#[derive(Clone, Debug)]
pub struct AudioProcessorOptions {
    /// The block size this Graph uses for processing.
    pub block_size: usize,
    /// The sample rate this Graph uses for processing.
    pub sample_rate: u32,
    /// The number of messages that can be sent through any of the ring buffers.
    /// Ring buffers are used pass information back and forth between the audio
    /// thread (GraphGen) and the Graph.
    pub ring_buffer_size: usize,
    /// Log channel capacity for `ArLogMessage`s, i.e. those sent using the `rt_log` macro from the
    /// audio thread.
    pub log_channel_capacity: usize,
}
impl Default for AudioProcessorOptions {
    fn default() -> Self {
        Self {
            block_size: 64,
            sample_rate: 48000,
            ring_buffer_size: 1000,
            log_channel_capacity: 100,
        }
    }
}
/// Top level runner for Knaster. Put this on the audio thread and run it.
pub struct AudioProcessor<F: Float> {
    // The Node contains sapce for metadata for the owning Graph that we don't need, but
    // it was convenient to reuse it here.
    graph_node: Node<F>,
    sample_rate: u32,
    block_size: usize,
    input_pointers: crate::core::vec::Vec<*const F>,
    // The buffer referenced by `output_block`. OwnedRawBuffer will drop the allocation when dropped.
    _output_buffer: OwnedRawBuffer<F>,
    output_block: RawContiguousBlock<F>,
    frame_clock: u64,
    // The frame clock available on other threads
    shared_frame_clock: SharedFrameClock,
    ctx: AudioCtx,
}
impl<F: Float> AudioProcessor<F> {
    /// Create a new [`AudioProcessor`] with the given [`AudioProcessorOptions`].
    ///
    /// Returns a tuple of
    /// - the [`Graph`], which is where you add audio processing and generating nodes,
    /// - the [`AudioProcessor`], which is how you run the graph to produce audio, and
    /// - the [`ArLogReceiver`], which is how you receive logs from the audio thread.
    pub fn new<Inputs: Size, Outputs: Size + NonZero>(
        options: AudioProcessorOptions,
    ) -> (Graph<F>, AudioProcessor<F>, ArLogReceiver<U1>) {
        let block_size = options.block_size;
        let sample_rate = options.sample_rate;
        assert!(block_size != 0, "The block size must not be 0");
        let output_buffer = OwnedRawBuffer::new(options.block_size * Outputs::USIZE);
        let invalid_node_id = NodeId::invalid();
        let shared_frame_clock = SharedFrameClock::new();
        let graph_options = GraphOptions {
            name: "OuterGraph".into(),
            ring_buffer_size: options.ring_buffer_size,
        };
        let (graph, node) = Graph::new::<Inputs, Outputs>(
            graph_options,
            invalid_node_id,
            shared_frame_clock.clone(),
            block_size,
            sample_rate,
            |_| {},
        );
        let log_receiver = ArLogReceiver::new();
        let (log_sender, log_receiver) = log_receiver.sender(options.log_channel_capacity);
        let ctx = AudioCtx::new(sample_rate, block_size, log_sender);

        let mut input_pointers = crate::core::vec::Vec::with_capacity(Inputs::USIZE);
        for _ in 0..Inputs::USIZE {
            input_pointers.push(crate::core::ptr::null());
        }
        let audio_processor = AudioProcessor {
            graph_node: node,
            input_pointers,
            output_block: unsafe {
                RawContiguousBlock::new(
                    output_buffer.add(0).expect("This is infallible"),
                    Outputs::USIZE,
                    block_size,
                )
            },
            _output_buffer: output_buffer,
            sample_rate,
            block_size,
            frame_clock: 0,
            shared_frame_clock,
            ctx,
        };
        (graph, audio_processor, log_receiver)
    }

    /// Produce one block of audio with the given inputs
    pub fn run(&mut self, inputs: &[&[F]]) {
        assert_eq!(inputs.len(), self.input_pointers.len());
        for (slice, ptr) in inputs.iter().zip(self.input_pointers.iter_mut()) {
            *ptr = slice.as_ptr();
        }
        self.ctx.block.set_frame_clock(self.frame_clock);
        let mut flags = UGenFlags::new();
        let ugen = self
            .graph_node
            .ugen()
            .expect("The top level graph should be guaranteed to be local to its node");
        // SAFETY: The input pointers were just created from shared references
        let input = unsafe { RawAggregateBlockRead::new(&self.input_pointers, self.block_size) };
        ugen.process_block(&mut self.ctx, &mut flags, &input, &mut self.output_block);
        self.frame_clock += self.block_size as u64;
        self.shared_frame_clock
            .store_new_time(Seconds::from_samples(
                self.frame_clock,
                self.sample_rate as u64,
            ));
    }

    /// Produce one block of audio for a graph with no inputs
    pub fn run_without_inputs(&mut self) {
        assert_eq!(self.inputs(), 0);
        // SAFETY: The input pointer slice is empty
        unsafe {
            self.run_raw_ptr_inputs(&[]);
        }
    }

    /// Produce one block of audio
    ///
    /// Prefer using [`run`] instead, unless you already need to store the raw pointers,
    /// or creating the slice of slices is not feasible.
    ///
    /// # Safety
    ///
    /// The pointers provided to the input buffer must be valid for the duration
    /// of this function call. Each pointer must point to an allocation of at
    /// least `self.block_size()`. The pointers must be of the same number as
    /// the number of inputs to the top level Graph inside. There must be no
    /// mutable references to the allocations they point to until this function
    /// returns. The pointers will not be stored past this function call.
    pub unsafe fn run_raw_ptr_inputs(&mut self, input_pointers: &[*const F]) {
        assert!(input_pointers.len() == self.inputs() as usize);
        self.ctx.block.set_frame_clock(self.frame_clock);
        let mut flags = UGenFlags::new();
        let ugen = self
            .graph_node
            .ugen()
            .expect("The top level graph should be guaranteed to be local to its node");
        let input = unsafe { RawAggregateBlockRead::new(input_pointers, self.block_size) };
        ugen.process_block(&mut self.ctx, &mut flags, &input, &mut self.output_block);
        self.frame_clock += self.block_size as u64;
        self.shared_frame_clock
            .store_new_time(Seconds::from_samples(
                self.frame_clock,
                self.sample_rate as u64,
            ));
    }
    /// Get a mutable reference to the output block. This block holds the output of the last
    /// processed block.
    pub fn output_block(&mut self) -> &mut RawContiguousBlock<F> {
        &mut self.output_block
    }
    /// Get the block size, i.e. how many frames are produced each time [`Self::run`] is called.
    pub fn block_size(&self) -> usize {
        self.block_size
    }
    /// Get the number of inputs to the top level graph.
    pub fn inputs(&self) -> u16 {
        self.graph_node.data.inputs
    }
    /// Get the number of outputs from the top level graph.
    pub fn outputs(&self) -> u16 {
        self.graph_node.data.outputs
    }
}

// # Safety
//
// Synchronisation with the Graph happens through safe means. See Graph for more info.
unsafe impl<F: Float> Send for AudioProcessor<F> {}
// # Safety
//
// Synchronisation with the Graph happens through safe means. See Graph for more info.
unsafe impl<F: Float> Sync for AudioProcessor<F> {}
