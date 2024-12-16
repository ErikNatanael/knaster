use knaster_core::{typenum::NonZero, BlockAudioCtx, Float, Size};

use crate::{
    block::{AggregateBlockRead, RawBlock},
    graph::{Graph, GraphSettings, OwnedRawBuffer},
    node::Node,
};

/// Top level runner for Knaster. Put this on the audio thread and run it.
pub struct Runner<F: Float> {
    // The Node contains sapce for metadata for the owning Graph that we don't need, but
    // it was convenient to reuse it here.
    graph_node: Node<F>,
    sample_rate: u32,
    block_size: usize,
    _output_buffer: OwnedRawBuffer<F>,
    output_block: RawBlock<F>,
    frame_clock: u64,
}
impl<F: Float> Runner<F> {
    pub fn new<Inputs: Size, Outputs: Size>(options: GraphSettings) -> (Graph<F>, Runner<F>)
    where
        Outputs: NonZero,
    {
        let block_size = options.block_size;
        let sample_rate = options.sample_rate;
        assert!(block_size != 0, "The block size must not be 0");
        let output_buffer = OwnedRawBuffer::new(options.block_size * Outputs::USIZE);
        let (graph, node) = Graph::new::<Inputs, Outputs>(options);
        let runner = Runner {
            graph_node: node,
            output_block: unsafe {
                RawBlock::new(
                    output_buffer.add(0).expect("This is infallible"),
                    Outputs::USIZE,
                    block_size,
                )
            },
            _output_buffer: output_buffer,
            sample_rate,
            block_size,
            frame_clock: 0,
        };
        (graph, runner)
    }

    /// Produce one block of audio
    ///
    /// Safety:
    ///
    /// The pointers provided to the input buffer must be valid for the duration
    /// of this function call. Each pointer must point to an allocation of at
    /// least `self.block_size()`. The pointers must be of the same number as
    /// the number of inputs to the top level Graph inside. There must be no
    /// mutable references to the allocations they point to until this function
    /// returns. The pointers will not be stored past this function call.
    pub unsafe fn run(&mut self, input_pointers: &[*const F]) {
        assert!(input_pointers.len() == self.inputs());
        let mut ctx = BlockAudioCtx::new(self.sample_rate, self.block_size);
        ctx.set_frame_clock(self.frame_clock);
        let gen = self.graph_node.gen;
        let input = unsafe { AggregateBlockRead::new(input_pointers, self.block_size) };
        unsafe { &mut (*gen) }.process_block(&mut ctx, &input, &mut self.output_block);
        self.frame_clock += self.block_size as u64;
    }
    pub fn output_block(&mut self) -> &mut RawBlock<F> {
        &mut self.output_block
    }
    pub fn block_size(&self) -> usize {
        self.block_size
    }
    pub fn inputs(&self) -> usize {
        self.graph_node.inputs
    }
    pub fn outputs(&self) -> usize {
        self.graph_node.outputs
    }
}

// Safety:
//
//
unsafe impl<F: Float> Send for Runner<F> {}
