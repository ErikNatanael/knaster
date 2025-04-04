use crate::{
    SchedulingEvent,
    core::{
        cell::UnsafeCell,
        marker::PhantomData,
        slice,
        sync::atomic::{AtomicBool, Ordering},
    },
    dyngen::DynUGen,
};
use std::collections::VecDeque;
use std::sync::Arc;

use knaster_core::{
    numeric_array::NumericArray, rt_log, typenum::U0, AudioCtx, Float, Size, UGen, UGenFlags
};
use slotmap::SlotMap;

use crate::{
    SchedulingChannelConsumer,
    graph::{NodeKey, OwnedRawBuffer},
    node::Node,
    task::TaskData,
};

/// This gets placed as a dyn UGen in a Node in a Graph. It's how the Graph gets
/// run. The Graph communicates with the GraphGen in a thread safe way.
///
/// # Safety
/// Using this struct is safe only if used in conjunction with the
/// Graph. The Graph owns nodes and gives its corresponding GraphGen raw
/// pointers to them through Tasks, but it never accesses or deallocates a node
/// while it can be accessed by the [`GraphGen`] through a Task. The [`GraphGen`]
/// mustn't use the _arc_nodes field; it is only there to make sure the nodes
/// don't get dropped.
pub(super) struct GraphGen<F: Float, Inputs: Size, Outputs: Size> {
    // block_size with oversampling applied
    pub(super) block_size: usize,
    // sample_rate with oversampling applied
    pub(super) sample_rate: u32,
    // If the graph has been freed and is waiting to be removed through an updated TaskData
    pub(super) freed: bool,

    // The parameter changes that haven't been applied yet, and the number of
    // blocks it has stayed here. After a large number of blocks it is removed
    // to avoid filling up the queue with parameter changes for nodes that don't
    // exist anymore.
    pub(super) waiting_parameter_changes: VecDeque<(SchedulingEvent, u32)>,

    pub(super) current_task_data: TaskData<F>,
    // This Arc is cloned from the Graph and exists so that if the Graph gets
    // dropped, the GraphGen can continue on without segfaulting. Pointers to
    // the Gens inside Nodes also exist in TaskData where they are called. The
    // Node may not be accessed from within GraphGen at all.
    pub(super) _arc_nodes: Arc<UnsafeCell<SlotMap<NodeKey, Node<F>>>>,
    // This Arc makes sure the buffer allocation is valid for as long as it needs to be
    pub(super) _arc_buffer_allocation_ptr: Arc<OwnedRawBuffer<F>>,
    /// Stores the number of completed samples, updated at the end of a block
    pub(super) scheduling_event_receiver: SchedulingChannelConsumer,
    pub(super) task_data_to_be_dropped_producer: rtrb::Producer<TaskData<F>>,
    pub(super) new_task_data_consumer: rtrb::Consumer<TaskData<F>>,
    pub(super) remove_me_flag: Arc<AtomicBool>,
    pub(super) _channels: PhantomData<(NumericArray<(), Inputs>, NumericArray<(), Outputs>)>,
    pub(super) blocks_to_keep_scheduled_changes: u32,
}

impl<F: Float, Inputs: Size, Outputs: Size> UGen for GraphGen<F, Inputs, Outputs> {
    type Sample = F;
    type Inputs = Inputs;
    type Outputs = Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.blocks_to_keep_scheduled_changes = self.sample_rate / self.block_size as u32;
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        if self.freed {
            for output_channel in output.iter_mut() {
                output_channel.fill(F::ZERO);
            }
            return;
        }
        let num_new_task_data = self.new_task_data_consumer.slots();
        if num_new_task_data > 0 {
            if let Ok(td_chunk) = self.new_task_data_consumer.read_chunk(num_new_task_data) {
                for mut td in td_chunk {
                    td.apply_self_on_audio_thread(ctx);
                    let old_td = std::mem::replace(&mut self.current_task_data, td);
                    match self.task_data_to_be_dropped_producer.push(old_td) {
                        Ok(_) => (),
                        Err(e) => {
                        rt_log!(ctx.logger(); "RingBuffer for TaskData to be dropped was full. Please increase the size of the RingBuffer. The GraphGen will drop the TaskData here instead. e: {e}");
                        }
                    }
                }
            }
        }
        // Apply parameter changes
        if !self.waiting_parameter_changes.is_empty() {
            let num_waiting_parameter_changes = self.waiting_parameter_changes.len();
            let gens = &mut self.current_task_data.gens;
            for _i in 0..num_waiting_parameter_changes {
                let (event, num_blocks_waiting) = self
                    .waiting_parameter_changes
                    .pop_front()
                    .expect(
                    "There should be at least waiting_parameter_changes elements in the vecdeque",
                );
                // Remove old changes that aren't applied in time. When a Gen is removed, but has parameter changes queued, they would otherwise pile up.
                if num_blocks_waiting > self.blocks_to_keep_scheduled_changes {
                    // By not pushing it back to the vecdeque, this change is removed
                    continue;
                }

                if let Some(unapplied) = apply_parameter_change(
                    event,
                    ctx.block_size() as u64,
                    ctx.sample_rate() as u64,
                    ctx,
                    gens,
                ) {
                    self.waiting_parameter_changes
                        .push_back((unapplied, num_blocks_waiting + 1));
                }
            }
        }

        // Get new parameter changes
        let parameter_changes_waiting = self.scheduling_event_receiver.slots();
        if let Ok(pm_chunk) = self
            .scheduling_event_receiver
            .read_chunk(parameter_changes_waiting)
        {
            let gens = &mut self.current_task_data.gens;
            for event in pm_chunk {
                if let Some(event) = apply_parameter_change(
                    event,
                    ctx.block_size() as u64,
                    ctx.sample_rate() as u64,
                    ctx,
                    gens,
                ) {
                    if self.waiting_parameter_changes.len()
                        < self.waiting_parameter_changes.capacity()
                    {
                        self.waiting_parameter_changes.push_back((event, 0));
                    }
                }
            }
        }
        // TODO: Remove parameter changes that have expired for tasks
        // that don't exist (anymore, removed nodes). Otherwise they
        // will accumulate until there is a crash.

        let task_data = &mut self.current_task_data;
        let TaskData {
            tasks,
            output_task,
            current_buffer_allocation: new_buffer_allocation,
            graph_input_channels_to_nodes,
            applied: _,
            ar_parameter_changes: _,
            gens: _,
        } = task_data;

        if let Some(buffer_allocation) = new_buffer_allocation.take() {
            // The old buffers will be kept alive until the Arc has been dropped in the GraphGen
            self._arc_buffer_allocation_ptr = buffer_allocation;
        }

        for (node_index, graph_input_indices) in graph_input_channels_to_nodes {
            let node_in_buffers = &mut tasks[*node_index].in_buffers;
            for (graph_input, node_input) in graph_input_indices {
                let channel = input.channel_as_slice(*graph_input);
                let graph_input_ptr = channel.as_ptr();
                node_in_buffers[*node_input] = graph_input_ptr;
            }
        }

        let mut new_flags = UGenFlags::default();
        // Run the tasks
        for task in tasks.iter_mut() {
            task.run(ctx, &mut new_flags);
        }

        // Set the output of the graph
        // Zero the output buffer.
        debug_assert_eq!(output_task.channels.len(), Outputs::USIZE);
        for (in_channel, out_channel) in output_task.channels.iter().zip(output.iter_mut()) {
            if let Some(channel_input) = in_channel {
                match channel_input {
                    crate::task::BlockOrGraphInput::Block(ptr) => {
                        // Safety:
                        //
                        // We only provide allocations of the correct size from
                        // the Graph. Anything else is a bug.
                        let s = unsafe { slice::from_raw_parts(*ptr, self.block_size) };
                        out_channel.copy_from_slice(s);
                    }
                    crate::task::BlockOrGraphInput::GraphInput(channel) => {
                        let s = input.channel_as_slice(*channel);
                        out_channel.copy_from_slice(s);
                    }
                }
            } else {
                out_channel.fill(F::ZERO);
            }
        }

        // Check if the free graph flag has been set
        if let Some(frame_num) = new_flags.remove_graph() {
            if (frame_num as usize) < self.block_size {
                // output zeroes from the frame it's supposed to be freed
                for channel in output.iter_mut() {
                    for sample in &mut channel[frame_num as usize..] {
                        *sample = F::ZERO;
                    }
                }
            }
            self.freed = true;
            self.remove_me_flag.store(true, Ordering::SeqCst);
        }
    }

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        _input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        unreachable!()
    }
    type Parameters = U0;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        [].into()
    }

    fn param_hints() -> NumericArray<knaster_core::ParameterHint, Self::Parameters> {
        [].into()
    }

    fn param_apply(
        &mut self,
        _ctx: &mut AudioCtx,
        _index: usize,
        _value: knaster_core::ParameterValue,
    ) {
    }
}

#[inline]
fn apply_parameter_change<'a, 'b, F: Float>(
    mut event: SchedulingEvent,
    block_size: u64,
    sample_rate: u64,
    ctx: &mut AudioCtx,
    // replace implicit 'static with 'b
    gens: &'a mut [(NodeKey, *mut (dyn DynUGen<F> + 'b))],
) -> Option<SchedulingEvent> {
    let mut ready_to_apply = event.token.as_ref().is_none_or(|t| t.ready());
    let mut delay_in_block = 0;
    if let Some(time) = &mut event.time {
        delay_in_block = time.to_samples_until_due(block_size, sample_rate, ctx.frame_clock());
        ready_to_apply = ready_to_apply && (delay_in_block < block_size);
    }

    let node_key = event.node_key;
    if ready_to_apply {
        for (key, ugen) in gens.iter_mut() {
            if *key == node_key {
                let g = unsafe { &mut (**ugen) };
                if delay_in_block > 0 {
                    g.set_delay_within_block_for_param(ctx, event.parameter, delay_in_block as u16);
                }
                if let Some(smoothing) = event.smoothing {
                    g.param_apply(ctx.into(), event.parameter, smoothing.into());
                }
                if let Some(value) = event.value {
                    g.param_apply(ctx.into(), event.parameter, value);
                }
                return None;
            }
        }
    }
    Some(event)
}

/// Safety: This impl of Send is required because of the Arc<UnsafeCell<...>> in
/// GraphGen. The _arc_nodes field of GraphGen exists only so that the nodes
/// won't get dropped if the Graph is dropped. The UnsafeCell will never be used
/// to access the data from within GraphGen.
unsafe impl<F: Float, Inputs: Size, Outputs: Size> Send for GraphGen<F, Inputs, Outputs> {}
