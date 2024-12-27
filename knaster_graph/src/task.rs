use crate::block::AggregateBlockRead;
use crate::core::sync::atomic::Ordering;

use crate::core::sync::atomic::AtomicBool;
use crate::core::sync::Arc;

use knaster_core::GenFlags;
use knaster_core::{BlockAudioCtx, Float};

use crate::block::RawBlock;
use crate::graph::{NodeKey, OwnedRawBuffer};
use crate::dyngen::DynGen;

pub struct Task<F> {
    pub(crate) gen: *mut dyn DynGen<F>,
    // Sequential buffers of a certain channel count
    pub(crate) in_buffers: Vec<*const F>,
    pub(crate) out_buffer: *mut F,
    pub(crate) output_channels: usize,
}
impl<F: Float> Task<F> {
    pub fn run(&mut self, ctx: BlockAudioCtx, flags: &mut GenFlags) {
        let input = unsafe { AggregateBlockRead::new(&self.in_buffers, ctx.block_size()) };
        let mut output =
            unsafe { RawBlock::new(self.out_buffer, self.output_channels, ctx.block_size()) };
        unsafe {
            (*self.gen).process_block(ctx, flags, &&input, &mut output);
        }
    }
}

pub(crate) struct InputToOutputTask {
    pub(crate) graph_input_index: usize,
    pub(crate) graph_output_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BlockOrGraphInput<F> {
    Block(*mut F),
    GraphInput(usize),
}

/// The buffers to be copied to the GraphGen output.
#[derive(Debug)]
pub(crate) struct OutputTask<F> {
    /// Pointers to buffers that are guaranteed to be sufficiently large for
    /// the current block size. One optional buffer per output.
    pub(crate) channels: Box<[Option<BlockOrGraphInput<F>>]>,
}
// impl<F: Float> std::fmt::Debug for OutputTask<F> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let f = f.debug_struct("OutputTask");
//         for b in &self.input_buffers {
//             f.field("", &self.input_index);
//         }
//         .field("input_index", &self.input_index)
//         .field("graph_output_index", &self.graph_output_index)
//         .finish()
//     }
// }
unsafe impl<F: Float> Send for OutputTask<F> {}

/// This data is sent via a boxed TaskData converted to a raw pointer.
///
/// Safety: The tasks or output_tasks may not be moved while there is a raw
/// pointer to the TaskData. If there is a problem, the Boxes in TaskData may
/// need to be raw pointers.
pub(crate) struct TaskData<F: Float> {
    // `applied` must be set to true when the running GraphGen receives it. This
    // signals that the changes in this TaskData have been applied and certain
    // Nodes may be dropped.
    pub(crate) applied: Arc<AtomicBool>,
    // Tasks run Gens
    pub(crate) tasks: Box<[Task<F>]>,
    pub(crate) output_task: OutputTask<F>,
    // if the buffer allocation has been replaced, replace the Arc to them in
    // the GraphGen as well. This keeps the buffer allocation alive even if the
    // `Graph` is dropped.
    pub(crate) current_buffer_allocation: Option<Arc<OwnedRawBuffer<F>>>,
    /// Audio rate parameter changes are tied to the graph structure just like
    /// audio passing through the graph. Changes get applied from this list at
    /// the point where the new schedule is received.
    pub(crate) ar_parameter_changes: Vec<ArParameterChange<F>>,
    /// Direct pointers to all the gens used in `tasks` in node execution order,
    /// and to the NodeKey that points to them in the Graph. This is used to
    /// apply parameter changes directly by function calls before any tasks are
    /// applied.
    pub(crate) gens: Vec<(NodeKey, *mut dyn DynGen<F>)>,
    /// (node_index_in_order, Vec<(graph_input_channel, node_input_channel))
    pub(crate) graph_input_channels_to_nodes: Vec<(usize, Vec<(usize, usize)>)>,
}

impl<F: Float> TaskData<F> {
    /// Run this when the TaskData is received on the audio thread and is
    /// applied to be the new current TaskData.
    pub fn apply_self_on_audio_thread(&mut self) {
        // Setting `applied` to true signals that the new
        // TaskData have been received and old data can be
        // dropped. It is necessary to set it for each
        // TaskData in order not to leak memory on the other
        // thread.
        self.applied.store(true, Ordering::SeqCst);
        for apc in &self.ar_parameter_changes {
            unsafe {
                (&mut *self.gens[apc.node].1).set_ar_param_buffer(apc.parameter_index, apc.buffer)
            };
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ArParameterChange<F> {
    /// Node index already converted to the index into `gens` in TaskData
    pub(crate) node: usize,
    pub(crate) parameter_index: usize,
    pub(crate) buffer: *const F,
}
