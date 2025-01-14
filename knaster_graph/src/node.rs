use crate::core::sync::atomic::AtomicBool;
use crate::core::sync::Arc;
use crate::core::{eprintln, vec, vec::Vec};
use alloc::{boxed::Box, string::String, string::ToString};

use knaster_core::{AudioCtx, Float};

use crate::graph::GraphId;
use crate::{buffer_allocator::BufferAllocator, dyngen::DynGen, task::Task};

/// `Node` wraps a [`DynGen`] in a graph. It is used to hold a pointer to the
/// Gen allocation and some metadata about it.
///
/// Safety:
/// - `Node` should not be used outside the graph context.
/// - The Node may not be dropped while its gen pointer is accessible on the graph, e.g. via a Task
/// - Dereferencing the gen pointer from a thread other than the audio thread is a data race.
/// - Every other field of this struct can be accessed from the Graph directly.
pub(crate) struct Node<F> {
    /// ACCESSIBILITY AND QOL
    // TODO: option to disable this and other optional QOL features in shipped builds
    pub(crate) name: String,
    pub(crate) parameter_descriptions: Vec<String>,
    pub(crate) is_graph: Option<GraphId>,

    /// STATIC DATA (won't change after the node has been created)
    pub(crate) gen: *mut dyn DynGen<F>,
    pub(crate) inputs: usize,
    pub(crate) outputs: usize,
    /// true if the node was not pushed manually to the Graph. Such nodes may
    /// also be removed automatically.
    pub(crate) auto_added: bool,

    /// STATE FOR TASK GENERATION etc.
    // TODO: Should this be NonNull<*const F> ??
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
    pub fn new<T: DynGen<F> + 'static>(name: String, gen: T) -> Self {
        let parameter_descriptions = gen
            .param_descriptions()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let inputs = gen.inputs();
        let outputs = gen.outputs();
        let boxed_gen = Box::new(gen);
        let ptr = Box::into_raw(boxed_gen);

        Self {
            name,
            parameter_descriptions,
            gen: ptr,
            inputs,
            outputs,
            node_inputs: vec![crate::core::ptr::null_mut(); inputs],
            node_output: NodeOutput::Offset(0),
            remove_me: None,
            auto_added: false,
            is_graph: None,
            num_output_dependents: 0,
        }
    }
    pub fn init(&mut self, ctx: &AudioCtx) {
        unsafe { &mut *(self.gen) }.init(ctx);
    }
    pub(super) fn to_task(&self) -> Task<F> {
        let in_buffers = self.node_inputs.clone();
        let out_buffer = self
            .node_output_ptr()
            .expect("The node output ptr should have been created at this point");

        Task {
            in_buffers,
            out_buffer,
            gen: self.gen,
            output_channels: self.outputs,
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
                eprintln!("Error: Unable to convert offset to pointer!");
            }
        } else {
            eprintln!("Error: Tried to convert node offset to ptr, but the node had no offset!");
        }
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
unsafe impl<F> Send for Node<F> {}
