use core::iter::FromFn;

use crate::core::sync::Arc;
use crate::core::sync::atomic::AtomicBool;
use crate::core::{vec, vec::Vec};
use crate::dynugen::UGenEnum;
use alloc::{boxed::Box, string::String};

use ecow::EcoString;
use knaster_core::{AudioCtx, Float, ParameterHint};

use crate::graph::{GraphId, NodeKey};
use crate::{buffer_allocator::BufferAllocator, dynugen::DynUGen, task::Task};

#[derive(Clone, Copy, Debug)]
pub struct NodeData {
    pub(crate) parameter_descriptions_fn: fn(usize) -> Option<&'static str>,
    pub(crate) parameter_hints_fn: fn(usize) -> Option<ParameterHint>,
    pub(crate) inputs: u16,
    pub(crate) outputs: u16,
    pub(crate) parameters: u16,
}
impl NodeData {
    pub fn parameter_descriptions(&self) -> impl Iterator<Item = &'static str> {
        let mut i = 0;
        crate::core::iter::from_fn(move || {
            let s = (self.parameter_descriptions_fn)(i);
            i += 1;
            s
        })
    }
    pub fn parameter_hints(&self) -> impl Iterator<Item = ParameterHint> {
        let mut i = 0;
        crate::core::iter::from_fn(move || {
            let s = (self.parameter_hints_fn)(i);
            i += 1;
            s
        })
    }
}

enum NodeUGen<F: Float> {
    /// The UGen is stored here and has not yet been moved to the audio thread.
    Local(UGenEnum<F>),
    /// The UGen is stored on the audio thread at the given index (updated after task generation).
    Live(usize),
}

/// `Node` wraps a [`DynUGen`] for storage in a graph. It is used to hold a pointer to the
/// UGen allocation and some metadata about it.
///
/// Safety:
/// - `Node` should not be used outside the graph context.
/// - The Node may not be dropped while its gen pointer is accessible on the graph, e.g. via a Task
/// - Dereferencing the gen pointer from a thread other than the audio thread is a data race.
/// - Every other field of this struct can be accessed from the Graph directly.
pub(crate) struct Node<F: Float> {
    /// ACCESSIBILITY AND QOL
    // TODO: option to disable this and other optional QOL features in shipped builds
    pub(crate) name: EcoString,
    pub(crate) is_graph: Option<GraphId>,

    /// STATIC DATA (won't change after the node has been created)
    pub(crate) data: NodeData,
    /// true if the node was not pushed manually to the Graph. Such nodes may
    /// also be removed automatically when no longer needed.
    pub(crate) auto_math_node: bool,
    /// If the node is left unconnected to any other node, remove it
    pub(crate) auto_free_when_unconnected: bool,
    /// A node that is strongly connected to this one. When this one is removed, remove the other
    /// one as well. Used for feedback nodes
    pub(crate) strong_dependent: Option<NodeKey>,

    /// STATE FOR TASK GENERATION etc.
    pub(crate) ugen: NodeUGen<F>,
    pub(crate) node_inputs: Vec<*const F>,
    pub(crate) node_output: NodeOutput<F>,
    /// The number of channels in potentially different nodes that depend
    /// on the output of this node. We are counting channels because that's how
    /// the input edges are stored, thereby avoiding additional bookkeeping when
    /// allocating buffers.
    pub(crate) num_output_dependents: usize,
    /// If this node can signal its own removal from the audio thread, it will
    /// do so by setting this AtomicBool to true.
    pub(crate) remove_me: Option<Arc<AtomicBool>>,
}
impl<F: Float> Node<F> {
    pub fn new<T: DynUGen<F> + 'static>(name: EcoString, ugen: T) -> Self {
        let parameter_descriptions_fn = ugen.param_description_fn();
        let parameter_hints_fn = ugen.param_hints_fn();
        let parameters = ugen.parameters();
        let inputs = ugen.inputs();
        let outputs = ugen.outputs();
        let ugen = NodeUGen::Local(UGenEnum::from_ugen(ugen));
        // let boxed_gen = Box::new(ugen);
        // let ptr = Box::into_raw(boxed_gen);

        Self {
            name,
            data: NodeData {
                parameter_descriptions_fn,
                parameter_hints_fn,
                inputs,
                outputs,
                parameters,
            },
            ugen,
            node_inputs: vec![crate::core::ptr::null_mut(); inputs as usize],
            node_output: NodeOutput::Offset(0),
            remove_me: None,
            auto_math_node: false,
            is_graph: None,
            num_output_dependents: 0,
            auto_free_when_unconnected: false,
            strong_dependent: None,
        }
    }
    pub fn init(&mut self, sample_rate: u32, block_size: usize) {
        if let NodeUGen::Local(ugen) = &mut self.ugen {
            ugen.init(sample_rate, block_size);
        }
    }
    pub fn ugen(&mut self) -> Option<&mut UGenEnum<F>> {
        match &mut self.ugen {
            NodeUGen::Local(ugen) => Some(ugen),
            NodeUGen::Live(_) => None,
        }
    }
    /// Generates a Task from this Node. The TaskData is generated in the order of the Graph, so
    /// `node_order_index` is the index of this node in the order of the TaskData that is currently
    /// generated when this function is called.
    ///
    /// The [`Node`] will save the index in order to transfer the UGen during the next update.
    pub(super) fn to_task(&mut self, node_order_index: usize) -> Task<F> {
        let in_buffers = self.node_inputs.clone();
        let out_buffer = self
            .node_output_ptr()
            .expect("The node output ptr should have been created at this point");
        let new_ugen = NodeUGen::Live(node_order_index);
        let ugen = std::mem::replace(&mut self.ugen, new_ugen);
        let ugen = match ugen {
            NodeUGen::Local(ugen) => ugen,
            NodeUGen::Live(index) => UGenEnum::TakeFromTask(index),
        };

        Task {
            in_buffers,
            out_buffer,
            ugen,
            output_channels: self.data.outputs as usize,
        }
    }
    pub fn node_output_ptr(&self) -> Option<*mut F> {
        if let NodeOutput::Pointer(ptr) = self.node_output {
            Some(ptr)
        } else {
            None
        }
    }
    pub fn assign_inputs(&mut self, inputs: Vec<*const F>) {
        self.node_inputs = inputs;
    }
    pub fn assign_output_offset(&mut self, output_offset: usize) {
        self.node_output = NodeOutput::Offset(output_offset);
    }
    pub fn swap_offset_to_ptr(&mut self, b: &BufferAllocator<F>) {
        if let NodeOutput::Offset(offset) = self.node_output {
            if let Some(ptr) = b.offset_to_ptr(offset) {
                self.node_output = NodeOutput::Pointer(ptr);
            } else {
                log::error!("Error: Unable to convert offset to pointer!");
            }
        } else {
            log::error!("Error: Tried to convert node offset to ptr, but the node had no offset!");
        }
    }
    pub fn parameter_descriptions(&self) -> impl Iterator<Item = &'static str> {
        self.data.parameter_descriptions()
    }
    pub fn parameter_hints(&self) -> impl Iterator<Item = ParameterHint> {
        self.data.parameter_hints()
    }
}

#[derive(Copy, Clone)]
pub(crate) enum NodeOutput<F> {
    Pointer(*mut F),
    Offset(usize),
}

/// # Safety
///
/// Nodes are only accessed from Graph which maintains the pointers within. Nodes own their DynGen
/// and an atomic flag is used to make sure nothing else points to the same allocation before it is
/// dropped. See Graph::node_keys_to_free_when_safe and references to it for implementation info.
unsafe impl<F: Float> Send for Node<F> {}
