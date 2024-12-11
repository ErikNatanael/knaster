use crate::{
    core::{
        cell::UnsafeCell,
        marker::PhantomData,
        slice,
        sync::atomic::{AtomicBool, Ordering},
    },
    SchedulingEvent,
};
use std::sync::Arc;

use knaster_core::{numeric_array::NumericArray, typenum::U0, Float, Gen, Parameterable, Size};
use slotmap::SlotMap;

use crate::{
    graph::{NodeKey, OwnedRawBuffer},
    node::Node,
    task::TaskData,
    SchedulingChannelConsumer,
};

/// This gets placed as a dyn Gen in a Node in a Graph. It's how the Graph gets
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
    pub(super) waiting_parameter_changes: Vec<(SchedulingEvent, usize)>,

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
}

impl<F: Float, Inputs: Size, Outputs: Size> Gen for GraphGen<F, Inputs, Outputs> {
    type Sample = F;
    type Inputs = Inputs;
    type Outputs = Outputs;

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut knaster_core::BlockAudioCtx,
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
                    td.apply_self_on_audio_thread();
                    let old_td = std::mem::replace(&mut self.current_task_data, td);
                    match self.task_data_to_be_dropped_producer.push(old_td) {
                          Ok(_) => (),
                          Err(e) => eprintln!("RingBuffer for TaskData to be dropped was full. Please increase the size of the RingBuffer. The GraphGen will drop the TaskData here instead. e: {e}"),
                      }
                }
            }
        }

        // Get new parameter changes
        let parameter_changes_waiting = self.scheduling_event_receiver.slots();
        if let Ok(pm_chunk) = self
            .scheduling_event_receiver
            .read_chunk(parameter_changes_waiting)
        {
            for event in pm_chunk {
                if self.waiting_parameter_changes.len() < self.waiting_parameter_changes.capacity()
                {
                    self.waiting_parameter_changes.push((event, 0));
                }
            }
        }
        // Apply parameter changes
        if !self.waiting_parameter_changes.is_empty() {
            let mut i = self.waiting_parameter_changes.len() - 1;
            loop {
                let (event, num_blocks_waiting) = &self.waiting_parameter_changes[i];
                let ready_to_apply = event.token.as_ref().map_or(true, |t| t.ready());
                let node_key = event.node_key;

                if ready_to_apply {
                    for (key, gen) in &mut self.current_task_data.gens {
                        if *key == node_key {
                            let g = unsafe { &mut (**gen) };
                            if let Some(smoothing) = event.smoothing {
                                g.param_apply(ctx.into(), event.parameter, smoothing.into());
                            }
                            if let Some(value) = event.value {
                                g.param_apply(ctx.into(), event.parameter, value.into());
                            }
                            self.waiting_parameter_changes.swap_remove(i);
                            break;
                        }
                    }
                }
                // Since we are using an usize it can't go into the negative so this
                // conditional is used instead of a while loop. We need the i == 0
                // iteration to run before breaking out.
                if i == 0 {
                    break;
                }
                i -= 1;
            }
        }
        // TODO: Remove parameter changes that have expired for tasks
        // that don't exist (anymore, removed nodes). Otherwise they
        // will accumulate until there is a crash.

        let task_data = &mut self.current_task_data;
        let TaskData {
            applied: _,
            tasks,
            output_task: output_tasks,
            current_buffer_allocation: new_buffer_allocation,
            input_to_output_tasks,
            ar_parameter_changes,
            gens,
        } = task_data;

        if let Some(buffer_allocation) = new_buffer_allocation.take() {
            // The old buffers will be kept alive until the Arc has been dropped in the GraphGen
            self._arc_buffer_allocation_ptr = buffer_allocation;
        }

        // Run the tasks
        for task in tasks.iter_mut() {
            task.run(ctx);
        }

        // Set the output of the graph
        // Zero the output buffer.
        assert_eq!(output_tasks.channels.len(), Outputs::USIZE);
        for (in_channel, out_channel) in output_tasks.channels.iter().zip(output.iter_mut()) {
            if let Some(ptr) = in_channel {
                // Safety:
                //
                // We only provide allocations of the correct size from
                // the Graph. Anything else is a bug.
                let s = unsafe { slice::from_raw_parts(*ptr, self.block_size) };
                out_channel.copy_from_slice(s);
            } else {
                out_channel.fill(F::ZERO);
            }
        }

        // Check if the free graph flag has been set
        if let Some(frame_num) = ctx.flags_mut().remove_graph() {
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

        // Copy from inputs to outputs of the graph
        for in_to_out_task in input_to_output_tasks.iter() {
            // Safety: We always drop the&mut refernce before requesting
            // another one so we cannot hold mutliple references to the
            // same channnel.
            unsafe { output.channel_as_slice_mut(in_to_out_task.graph_output_index) }
                .copy_from_slice(input.channel_as_slice(in_to_out_task.graph_input_index));
        }
    }

    fn process(
        &mut self,
        ctx: &mut knaster_core::AudioCtx,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        todo!()
    }
}

impl<F: Float, Inputs: Size, Outputs: Size> Parameterable<F> for GraphGen<F, Inputs, Outputs> {
    type Parameters = U0;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        todo!()
    }

    fn param_default_values() -> NumericArray<knaster_core::ParameterValue, Self::Parameters> {
        todo!()
    }

    fn param_range() -> NumericArray<knaster_core::ParameterRange, Self::Parameters> {
        todo!()
    }

    fn param_apply(
        &mut self,
        ctx: &knaster_core::AudioCtx,
        index: usize,
        value: knaster_core::ParameterValue,
    ) {
        todo!()
    }
}

/// Safety: This impl of Send is required because of the Arc<UnsafeCell<...>> in
/// GraphGen. The _arc_nodes field of GraphGen exists only so that the nodes
/// won't get dropped if the Graph is dropped. The UnsafeCell will never be used
/// to access the data from within GraphGen.
unsafe impl<F: Float, Inputs: Size, Outputs: Size> Send for GraphGen<F, Inputs, Outputs> {}
