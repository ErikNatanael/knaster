//! # Graph
//!
//! This module contains the [`Graph`] struct, which is a dynamically editable audio graph.
use crate::core::collections::VecDeque;
use crate::core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::core::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::{
    SharedFrameClock, Time,
    buffer_allocator::BufferAllocator,
    core::sync::atomic::AtomicU64,
    edge::{Edge, NodeKeyOrGraph, ParameterEdge},
    graph_edit::GraphEdit,
    graph_gen::GraphGen,
    handle::{Handle, RawHandle, SchedulingChannelSender},
    node::{Node, NodeData},
    task::{ArParameterChange, BlockOrGraphInput, OutputTask, Task, TaskData},
};
use core::cell::Cell;
use ecow::EcoString;
use knaster_core::numeric_array::NumericArray;
/// no_std_compat prelude import, supporting both std and no_std
use std::prelude::v1::*;

use crate::inspection::{EdgeInspection, EdgeSource, GraphInspection, NodeInspection};
use crate::wrappers_graph::done::WrDone;
use knaster_core::{
    AudioCtx, Done, Float, Param, ParameterError, ParameterValue, Size, UGen,
    log::ArLogSender,
    math::{Add, MathUGen},
    typenum::*,
};
use rtrb::RingBuffer;
use slotmap::{SecondaryMap, SlotMap, new_key_type};

/// Unique id identifying a [`Graph`]. Is set from an atomic any time a [`Graph`] is created.
///
/// u64 should be sufficient as a total number of Graphs created during one run.
/// Creating 200 Graphs a second (which is not efficient and you should look at
/// alternative solutions), it would take almost 3 billion years to run out of
/// IDs. If this is an issue for your use case, please file a bug report.
pub type GraphId = u64;

/// Get a unique id for a Graph from this by using `fetch_add`
static NEXT_GRAPH_ID: AtomicU64 = AtomicU64::new(0);

new_key_type! {
    /// Node identifier in a specific Graph. For referring to a Node outside the context of a Graph, use NodeId instead.
    pub struct NodeKey;
}

/// Unique identifier for a specific Node
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    /// The key is only unique within the specific Graph
    pub(crate) key: NodeKey,
    pub(crate) graph: GraphId,
}
impl NodeId {
    /// Returns a NodeId that doesn't point to any node. Should only be used for nodes which are
    /// not in a [`Graph`], e.g. the top level [`Graph`].
    pub fn invalid() -> Self {
        Self {
            key: NodeKey::default(),
            // We should never reach the max of a u64, see comment for GraphId
            graph: GraphId::MAX,
        }
    }
    /// Returns the [`NodeKey`], i.e. the [`Graph`] internal node address, of this NodeId
    pub fn key(&self) -> NodeKey {
        self.key
    }
}

/// Options for a new [`Graph`]
#[derive(Clone, Debug)]
pub struct GraphOptions {
    /// The name of the Graph
    pub name: EcoString,
    /// The number of messages that can be sent through any of the ring buffers.
    /// Ring buffers are used pass information back and forth between the audio
    /// thread (GraphGen) and the Graph.
    pub ring_buffer_size: usize,
}
impl GraphOptions {
    /// Set the name of the new [`Graph`]
    pub fn name(mut self, n: impl AsRef<str>) -> Self {
        self.name = EcoString::from(n.as_ref());
        self
    }
    /// Set the number of messages that can be in transit through any of the ring buffers.
    /// Ring buffers are used pass information back and forth between the audio
    /// thread (GraphGen) and the Graph, mainly scheduled parameter changes.
    pub fn ring_buffer_size(mut self, n: usize) -> Self {
        self.ring_buffer_size = n;
        self
    }
}

impl Default for GraphOptions {
    fn default() -> Self {
        GraphOptions {
            name: EcoString::new(),
            ring_buffer_size: 1000,
        }
    }
}

/// Hold on to an allocation and drop it when we're done. Can be easily wrapped
/// in an Arc. This ensures we free the memory.
pub(crate) struct OwnedRawBuffer<F: Float> {
    pub(crate) ptr: *mut [F],
}
impl<F: Float> OwnedRawBuffer<F> {
    pub fn new(len: usize) -> Self {
        // TODO: Technically, it would be possible to make these blocks
        // MaybeUninit, but in practice it would probably lead to a costlier API
        // on the audio thread?
        let ptr = Box::<[F]>::into_raw(vec![F::ZERO; len].into_boxed_slice());
        Self { ptr }
    }
    pub fn add(&self, add: usize) -> Option<*mut F> {
        if add < self.ptr.len() {
            Some(unsafe { self.ptr.cast::<F>().add(add) })
        } else {
            None
        }
    }
    /// # Safety:
    /// Only call if you are certain that no other mutable reference to this buffer exists. Destroy
    /// the mutable reference as soon as possible to allow other call sites to call this function
    /// for the same OwnedRawBuffer.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn as_slice_mut(&self) -> &mut [F] {
        unsafe { crate::core::slice::from_raw_parts_mut(self.ptr.cast::<F>(), self.ptr.len()) }
    }
}
impl<F: Float> Drop for OwnedRawBuffer<F> {
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.ptr)) }
    }
}

/// Dynamically editable audio graph, consisting of nodes, which implement [`UGen`], and edges
/// between these nodes and graph inputs and outputs.
///
/// Edges can be direct or feedback edges. Direct edges determine node order and cannot be added if they create a circular
/// dependency. Feedback edges always buffer the output of a node to become the input of another
/// node one block later, and have no impact on node ordering.
///
/// Use [`Graph::edit`] to make changes to a [`Graph`] at any point. When the edit closure returns,
/// all changes will be sent to the audio thread and applied at the start of the next block.
pub struct Graph<F: Float> {
    graph_id: GraphId,
    name: EcoString,
    // The reason this is an Arc is for the GraphGen to hold a clone of the Arc
    // so that the data doesn't get dropped if the Graph is dropped while the
    // GraphGen is running.
    nodes: Arc<UnsafeCell<SlotMap<NodeKey, Node<F>>>>,
    node_keys_to_free_when_safe: Vec<(NodeKey, Arc<AtomicBool>)>,
    buffers_to_free_when_safe: Vec<Arc<OwnedRawBuffer<F>>>,
    /// Set of keys pending removal to easily check if a node is pending
    /// removal. TODO: Maybe it's actually faster and easier to just look
    /// through node_keys_to_free_when_safe than to bother with a HashSet since
    /// this list will almost always be tiny.
    // node_keys_pending_removal: HashSet<NodeKey>,
    /// A list of input edges for every node. The input channel is the index into the boxed slice
    node_input_edges: SecondaryMap<NodeKey, Box<[Option<Edge>]>>,
    /// Edges which control a parameter of a node through the output of another
    /// node. These can be in addition to audio input edges.
    node_parameter_edges: SecondaryMap<NodeKey, Vec<ParameterEdge>>,
    /// If a node can be freed or not. A node can be made immortal to avoid accidentally removing it.
    node_mortality: SecondaryMap<NodeKey, bool>,
    node_order: Vec<NodeKey>,
    disconnected_nodes: Vec<NodeKey>,
    /// The outputs of the Graph
    output_edges: Box<[Option<Edge>]>,
    /// If changes have been made that require recalculating the graph this will be set to true.
    recalculation_required: bool,
    num_inputs: u16,
    num_outputs: u16,
    block_size: usize,
    sample_rate: u32,
    /// Used for processing every node, index using \[input_num\]\[sample_in_block\]
    // inputs_buffers: Vec<Box<[Sample]>>,
    /// A pointer to an allocation that is being used for the inputs to nodes,
    /// and aliased in the inputs_buffers
    buffer_allocator: BufferAllocator<F>,
    graph_gen_communicator: GraphGenCommunicator<F>,
    /// The nodeId of the Graph node in the parent. Only the top level Graph has an invalid NodeId
    /// which will not allow any action.
    self_node_id: NodeId,
}

impl<F: Float> Graph<F> {
    /// Create a new empty [`Graph`] with a unique atomically generated [`GraphId`]
    pub(crate) fn new<Inputs: Size, Outputs: Size>(
        options: GraphOptions,
        node_id: NodeId,
        shared_frame_clock: SharedFrameClock,
        block_size: usize,
        sample_rate: u32,
        init_callback: impl FnOnce(GraphEdit<F>),
    ) -> (Self, Node<F>) {
        let GraphOptions {
            name,
            ring_buffer_size,
        } = options;
        const DEFAULT_NUM_NODES: usize = 4;
        let graph_id = NEXT_GRAPH_ID.fetch_add(1, crate::core::sync::atomic::Ordering::SeqCst);
        let nodes = Arc::new(UnsafeCell::new(SlotMap::with_capacity_and_key(
            DEFAULT_NUM_NODES,
        )));
        let node_input_edges = SecondaryMap::with_capacity(DEFAULT_NUM_NODES);
        // let node_feedback_edges = SecondaryMap::with_capacity(DEFAULT_NUM_NODES);
        let node_parameter_edges = SecondaryMap::with_capacity(DEFAULT_NUM_NODES);
        let buffer_allocator = BufferAllocator::new(block_size * 4);

        let (new_task_data_producer, new_task_data_consumer) =
            RingBuffer::<TaskData<F>>::new(ring_buffer_size);
        let (task_data_to_be_dropped_producer, task_data_to_be_dropped_consumer) =
            RingBuffer::<TaskData<F>>::new(ring_buffer_size);
        let (scheduling_event_producer, scheduling_event_receiver) =
            rtrb::RingBuffer::new(ring_buffer_size);

        let graph_gen_communicator = GraphGenCommunicator {
            scheduling_event_producer: SchedulingChannelSender(Arc::new(Mutex::new(
                scheduling_event_producer,
            ))),
            task_data_to_be_dropped_consumer,
            new_task_data_producer,
            next_change_flag: Arc::new(AtomicBool::new(false)),
            shared_frame_clock,
        };
        let remove_me = Arc::new(AtomicBool::new(false));
        let mut graph = Self {
            graph_id,
            name,
            nodes,
            node_input_edges,
            node_parameter_edges,
            node_mortality: SecondaryMap::with_capacity(DEFAULT_NUM_NODES),
            node_order: Vec::with_capacity(DEFAULT_NUM_NODES),
            disconnected_nodes: vec![],
            node_keys_to_free_when_safe: vec![],
            output_edges: vec![None; Outputs::USIZE].into(),
            num_inputs: Inputs::U16,
            num_outputs: Outputs::U16,
            block_size,
            sample_rate,
            graph_gen_communicator,
            recalculation_required: false,
            buffers_to_free_when_safe: vec![],
            buffer_allocator,
            self_node_id: node_id,
        };
        (init_callback)(GraphEdit::new(&mut graph));
        // graph_gen
        let task_data = graph.generate_task_data(Arc::new(AtomicBool::new(false)), Vec::new());

        //         use paste::paste;
        //         macro_rules! graph_gen_channels {
        //     (
        //         // Start a repetition:
        //         $(
        //             // Each repeat must contain an expression...
        //             $input_usize:expr,
        //             $output_usize:expr
        //         )
        //         // ...separated by commas...
        //         ;
        //         // ...zero or more times.
        //         *
        //     ) => {
        //         // Enclose the expansion in a block so that we can use
        //         // multiple statements.
        //         {
        //             match (graph.num_inputs, graph.num_outputs) {
        //             // Start a repetition:
        //             $(
        //                 ($input_usize, $output_usize) => Node::new(String::from("GraphGen"), GraphGen::<F, paste! {[<U $input_usize>]}, paste! {[<U $output_usize>]}> {
        //                     sample_rate: graph.sample_rate,
        //                     current_task_data: task_data,
        //                     block_size: graph.block_size,
        //                     scheduling_event_receiver,
        //                     task_data_to_be_dropped_producer,
        //                     new_task_data_consumer,
        //                     freed: false,
        //                     _arc_nodes: graph.nodes.clone(),
        //                     _arc_buffer_allocation_ptr: graph.buffer_allocator.buffer(),
        //                     _channels: core::marker::PhantomData,
        //                     remove_me_flag: remove_me.clone(),
        //             }),
        //             )*
        //                 _ => panic!("Unsupported graph input/output channel configuration. Configuration cannot be generated for GraphGen.")
        //         }

        //         }
        //     };
        // }
        //         // TODO: Create a beutiful Cartesian product macro instead and do many more channel combinations
        //         let mut graph_gen: Node<F> =
        //             graph_gen_channels!(0,1;1,1;0,2;1,2;2,2;0,3;1,3;2,3;3,3;0,4;1,4;2,4;3,4;4,4);

        let mut graph_gen = Node::new(
            graph.name.clone(),
            GraphGen::<F, Inputs, Outputs> {
                sample_rate: graph.sample_rate,
                current_task_data: task_data,
                block_size: graph.block_size,
                scheduling_event_receiver,
                task_data_to_be_dropped_producer,
                new_task_data_consumer,
                freed: false,
                waiting_parameter_changes: VecDeque::with_capacity(ring_buffer_size),
                _arc_nodes: graph.nodes.clone(),
                _arc_buffer_allocation_ptr: graph.buffer_allocator.buffer(),
                _channels: core::marker::PhantomData,
                remove_me_flag: remove_me.clone(),
                blocks_to_keep_scheduled_changes: graph.sample_rate / graph.block_size as u32,
            },
        );

        graph_gen.remove_me = Some(remove_me);

        (graph, graph_gen)
    }

    /// Returns a clone of the shared frame clock for this [`Graph`].
    ///
    /// The frame clock holds a reference to the number of frames elapsed on the audio thread, and
    /// is shared among all subgraphs to any top level [`Graph`].
    pub fn shared_frame_clock(&self) -> SharedFrameClock {
        self.graph_gen_communicator.shared_frame_clock.clone()
    }

    /// Get a metadata for a node with the given [`NodeId`] if it exists.
    pub fn node_data(&self, id: impl Into<NodeId>) -> Option<NodeData> {
        let node_id = id.into();
        if node_id.graph != self.graph_id {
            return None;
        }
        self.get_nodes().get(node_id.key()).map(|node| node.data)
    }
    /// Get a metadata for a node with the given [`NodeId`] if it exists.
    pub fn node_data_from_name(&self, name: impl Into<EcoString>) -> Option<(NodeId, NodeData)> {
        let name = name.into();
        self.get_nodes()
            .iter()
            .find(|node| node.1.name == name)
            .map(|node| {
                (
                    NodeId {
                        key: node.0,
                        graph: self.graph_id,
                    },
                    node.1.data,
                )
            })
    }

    /// Push something implementing [`UGen`] to the graph.
    #[deprecated(note = "use `edit` instead")]
    pub fn push<T: UGen<Sample = F> + 'static>(&mut self, ugen: T) -> Handle<T> {
        self.push_internal(ugen)
    }
    pub(crate) fn push_internal<T: UGen<Sample = F> + 'static>(&mut self, ugen: T) -> Handle<T> {
        let name = crate::core::any::type_name::<T>();
        let name = shorten_name(name);
        let node = Node::new(name, ugen);
        let node_key = self.push_node(node);

        Handle::new(RawHandle::new(
            NodeId {
                key: node_key,
                graph: self.graph_id,
            },
            self.graph_gen_communicator
                .scheduling_event_producer
                .clone(),
            self.graph_gen_communicator.shared_frame_clock.clone(),
        ))
    }
    /// Push something implementing [`UGen`] to the graph, adding the [`WrDone`] wrapper. This
    /// enables the node to free itself if it marks itself as done or for removal using [`GenFlags`].
    pub(crate) fn push_with_done_action<T: UGen<Sample = F> + 'static>(
        &mut self,
        ugen: T,
        default_done_action: Done,
    ) -> Handle<WrDone<T>>
    where
        // Make sure we can add a parameter
        <T as UGen>::Parameters: crate::core::ops::Add<B1>,
        <<T as UGen>::Parameters as crate::core::ops::Add<B1>>::Output: Size,
    {
        let free_self_flag = Arc::new(AtomicBool::new(false));
        let ugen = WrDone {
            ugen,
            free_self_flag: free_self_flag.clone(),
            done_action: default_done_action,
        };
        let name = crate::core::any::type_name::<T>();
        let name = shorten_name(name);
        let mut node = Node::new(name, ugen);
        node.remove_me = Some(free_self_flag);
        let node_key = self.push_node(node);
        Handle::new(RawHandle::new(
            NodeId {
                key: node_key,
                graph: self.graph_id,
            },
            self.graph_gen_communicator
                .scheduling_event_producer
                .clone(),
            self.graph_gen_communicator.shared_frame_clock.clone(),
        ))
    }
    /// Get the channel to the GraphGen on the audio thread. Mainly used to schedule parameter
    /// changes.
    pub fn scheduling_channel_sender(&self) -> SchedulingChannelSender {
        self.graph_gen_communicator
            .scheduling_event_producer
            .clone()
    }

    /// Set the stored name for the given node.
    ///
    /// The name can be used to recover a handle to the node and will be displayed in any [`GraphInspection`]
    pub fn set_name(&mut self, node_id: NodeId, name: EcoString) {
        self.get_nodes_mut()[node_id.key()].name = name;
    }

    /// If any node matches `name`, return its [`NodeId`]
    pub fn node_id_with_name(&self, name: impl AsRef<str>) -> Option<NodeId> {
        let name = name.as_ref();
        for (id, node) in self.get_nodes() {
            if node.name == name {
                return Some(NodeId {
                    key: id,
                    graph: self.graph_id,
                });
            }
        }
        None
    }

    /// Returns an [`AudioCtx`] matching this [`Graph`] with a non real-time [`ArLogSender`]. This
    /// means that any log messages will be logged to the `log` crate scaffolding. See the `log`
    /// crate for more information on how to receive these log messages.
    pub fn ctx(&self) -> AudioCtx {
        AudioCtx::new(self.sample_rate, self.block_size, ArLogSender::non_rt())
    }

    /// Add a node to this Graph. The Node will be (re)initialised with the
    /// correct block size for this Graph.
    fn push_node(&mut self, mut node: Node<F>) -> NodeKey {
        self.recalculation_required = true;

        node.init(self.sample_rate, self.block_size);
        let node_inputs = node.data.inputs;
        let key = self.get_nodes_mut().insert(node);
        self.node_input_edges
            .insert(key, vec![None; node_inputs as usize].into_boxed_slice());
        // self.node_feedback_edges.insert(key, vec![]);
        self.node_mortality.insert(key, true);
        self.node_parameter_edges.insert(key, vec![]);

        key
    }

    // #[deprecated(since = "0.1.0", note = "Will become private")]
    // pub fn connect_nodes(
    //     &mut self,
    //     source: impl Into<NodeId>,
    //     sink: impl Into<NodeId>,
    //     source_channel: u16,
    //     sink_channel: u16,
    //     additive: bool,
    //     feedback: bool,
    // ) -> Result<(), GraphError> {
    //     self.connect_nodes_internal(
    //         source,
    //         sink,
    //         source_channel,
    //         sink_channel,
    //         additive,
    //         feedback,
    //     )
    // }
    fn connect_nodes_internal(
        &mut self,
        source: impl Into<NodeId>,
        sink: impl Into<NodeId>,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
        feedback: bool,
    ) -> Result<(), GraphError> {
        let source = source.into();
        let sink = sink.into();
        if !source.graph == self.graph_id {
            return Err(GraphError::WrongSourceNodeGraph {
                expected_graph: self.graph_id,
                found_graph: source.graph,
            });
        }
        if !sink.graph == self.graph_id {
            return Err(GraphError::WrongSinkNodeGraph {
                expected_graph: self.graph_id,
                found_graph: sink.graph,
            });
        }

        let nodes = self.get_nodes();
        if !nodes.contains_key(source.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if source_channel >= nodes[source.key()].data.outputs {
            return Err(GraphError::OutputOutOfBounds(source_channel));
        }
        if !nodes.contains_key(sink.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if sink_channel >= nodes[sink.key()].data.inputs {
            return Err(GraphError::InputOutOfBounds(sink_channel));
        }
        if !feedback && self.has_path(sink, source) {
            return Err(GraphError::CircularConnection);
        }
        self.connect_to_node_internal(
            NodeKeyOrGraph::Node(source.key()),
            sink.key(),
            source_channel,
            sink_channel,
            additive,
            feedback,
        );
        Ok(())
    }
    /// Connect a graph input directly to a graph output
    fn connect_input_to_output(
        &mut self,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
    ) -> Result<(), GraphError> {
        if source_channel >= self.num_inputs {
            return Err(GraphError::GraphInputOutOfBounds(source_channel));
        }
        if sink_channel >= self.num_outputs {
            return Err(GraphError::GraphOutputOutOfBounds(sink_channel));
        }
        self.connect_to_output_internal(
            NodeKeyOrGraph::Graph,
            source_channel,
            sink_channel,
            additive,
        );
        Ok(())
    }
    fn connect_node_to_output(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
    ) -> Result<(), GraphError> {
        let source = source.into();
        if !source.graph == self.graph_id {
            return Err(GraphError::WrongSourceNodeGraph {
                expected_graph: self.graph_id,
                found_graph: source.graph,
            });
        }
        if sink_channel >= self.num_outputs {
            return Err(GraphError::GraphOutputOutOfBounds(sink_channel));
        }
        let nodes = self.get_nodes();
        if !nodes.contains_key(source.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if source_channel >= nodes[source.key()].data.outputs {
            return Err(GraphError::OutputOutOfBounds(source_channel));
        }
        self.connect_to_output_internal(
            NodeKeyOrGraph::Node(source.key()),
            source_channel,
            sink_channel,
            additive,
        );
        Ok(())
    }
    /// Connecting a node output to a parameter input at audio rate, adding the source to any
    /// existing node input(s) to the parameter.
    ///
    /// Graph inputs are not currently supported as parameter inputs. As a workaround, connect the
    /// graph input to a node (e.g. an addition node with zero as the other input) and connect that
    /// node to the parameter.
    pub fn connect_to_parameter(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
    ) -> Result<(), GraphError> {
        self.connect_node_to_parameter(source, source_channel, parameter, sink, true)
    }
    /// Connecting a node output to a parameter input at audio rate, replacing any
    /// existing node input(s) to the parameter.
    ///
    /// Graph inputs are not currently supported as parameter inputs. As a workaround, connect the
    /// graph input to a node (e.g. an addition node with zero as the other input) and connect that
    /// node to the parameter.
    pub fn connect_replace_to_parameter(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
    ) -> Result<(), GraphError> {
        self.connect_node_to_parameter(source, source_channel, parameter, sink, false)
    }
    fn connect_node_to_parameter(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
        additive: bool,
    ) -> Result<(), GraphError> {
        let source = source.into();
        if !source.graph == self.graph_id {
            return Err(GraphError::WrongSourceNodeGraph {
                expected_graph: self.graph_id,
                found_graph: source.graph,
            });
        }
        let sink = sink.into();
        if !sink.graph == self.graph_id {
            return Err(GraphError::WrongSinkNodeGraph {
                expected_graph: self.graph_id,
                found_graph: sink.graph,
            });
        }
        let nodes = self.get_nodes();
        let sink_node = &nodes[sink.key()];
        let param = parameter.into();
        let param_index = match param {
            Param::Index(param_index) => param_index as u16,
            Param::Desc(desc) => {
                if let Some(index) = sink_node.parameter_descriptions().position(|s| s == desc) {
                    index as u16
                } else {
                    log::error!(
                        "Parameter description not found: {desc}, found instead {}",
                        sink_node
                            .parameter_descriptions()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    return Err(GraphError::ParameterDescriptionNotFound(desc.to_string()));
                }
            }
        };
        let nodes = self.get_nodes();
        if !nodes.contains_key(source.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if source_channel >= nodes[source.key()].data.outputs {
            return Err(GraphError::OutputOutOfBounds(source_channel));
        }

        let edges = self
            .node_parameter_edges
            .get_mut(sink.key())
            .expect("All nodes should have parameter edges");
        if additive {
            if let Some(pos) = edges
                .iter()
                .position(|pe| pe.parameter_index == param_index)
            {
                let existing_edge = edges.swap_remove(pos);
                let add_node = self.new_additive_node();
                self.node_input_edges[add_node][0] = Some(Edge {
                    source: existing_edge.source.into(),
                    channel_in_source: existing_edge.channel_in_source,
                    is_feedback: false,
                });
                self.node_input_edges[add_node][1] = Some(Edge {
                    source: source.key().into(),
                    channel_in_source: source_channel,
                    is_feedback: false,
                });
                // Need to fetch again not to borrow self twice
                let edges = self
                    .node_parameter_edges
                    .get_mut(sink.key())
                    .expect("All nodes should have parameter edges");
                edges.push(ParameterEdge {
                    source: add_node,
                    channel_in_source: 0,
                    parameter_index: param_index,
                });
            } else {
                edges.push(ParameterEdge {
                    source: source.key(),
                    channel_in_source: source_channel,
                    parameter_index: param_index,
                });
            }
        } else {
            edges.retain(|pe| pe.parameter_index != param_index);
            edges.push(ParameterEdge {
                source: source.key(),
                channel_in_source: source_channel,
                parameter_index: param_index,
            });
        }
        Ok(())
    }
    fn connect_input_to_node(
        &mut self,
        sink: impl Into<NodeId>,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
    ) -> Result<(), GraphError> {
        let sink = sink.into();
        if !sink.graph == self.graph_id {
            return Err(GraphError::WrongSinkNodeGraph {
                expected_graph: self.graph_id,
                found_graph: sink.graph,
            });
        }
        if source_channel >= self.num_inputs {
            return Err(GraphError::GraphInputOutOfBounds(source_channel));
        }
        let nodes = self.get_nodes();
        if !nodes.contains_key(sink.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if sink_channel >= nodes[sink.key()].data.inputs {
            return Err(GraphError::OutputOutOfBounds(sink_channel));
        }

        self.connect_to_node_internal(
            NodeKeyOrGraph::Graph,
            sink.key(),
            source_channel,
            sink_channel,
            additive,
            false,
        );
        Ok(())
    }

    /// Make a connection between two nodes in the Graph when it is certain that
    /// the NodeKeys are from this graph
    ///
    /// Assumes that the parameters are valid. Use the public functions for error checking. This
    /// enables a statically checked API.
    fn connect_to_node_internal(
        &mut self,
        mut source: NodeKeyOrGraph,
        sink: NodeKey,
        so_channel: u16,
        si_channel: u16,
        additive: bool,
        feedback: bool,
    ) {
        self.recalculation_required = true;
        if feedback {
            let (fb_source, fb_sink) = self.new_feedback_nodes();
            self.node_input_edges[fb_sink][0] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: true,
            });
            source = NodeKeyOrGraph::Node(fb_source);
        }
        // Fast and common path
        if !additive {
            self.node_input_edges[sink][si_channel as usize] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            return;
        }
        // Connect additively
        // If no input exists for the channel, connect directly.
        // If an input does exist, create a new add node and connect it up, replacing the current edge.

        if let Some(existing_edge) = self.node_input_edges[sink][si_channel as usize] {
            // Put an add node in between the input and the previous input,
            // adding the new source together with the old
            let add_node = self.new_additive_node();
            self.node_input_edges[add_node][0] = Some(existing_edge);
            self.node_input_edges[add_node][1] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            self.node_input_edges[sink][si_channel as usize] = Some(Edge {
                source: NodeKeyOrGraph::Node(add_node),
                channel_in_source: 0,
                is_feedback: false,
            });
        } else {
            self.node_input_edges[sink][si_channel as usize] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: feedback,
            });
        }
    }
    /// The internal function for connecting a node to the output
    ///
    /// Assumes that the parameters are valid. Use the public functions for error checking. This
    /// enables a statically checked API.
    fn connect_to_output_internal(
        &mut self,
        source: NodeKeyOrGraph,
        so_channel: u16,
        si_channel: u16,
        additive: bool,
    ) {
        // Only the pob functions do input checking on nodes and channels. This enables

        self.recalculation_required = true;
        // Fast and common path
        if !additive {
            self.output_edges[si_channel as usize] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            return;
        }
        // Connect additively
        // If no input exists for the channel, connect directly.
        // If an input does exist, create a new add node and connect it up, replacing the current edge.

        if let Some(existing_edge) = self.output_edges[si_channel as usize] {
            // Put an add node in between the input and the previous input,
            // adding the new source together with the old
            let add_node = self.new_additive_node();
            self.node_input_edges[add_node][0] = Some(existing_edge);
            self.node_input_edges[add_node][1] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            self.output_edges[si_channel as usize] = Some(Edge {
                source: NodeKeyOrGraph::Node(add_node),
                channel_in_source: 0,
                is_feedback: false,
            });
        } else {
            self.output_edges[si_channel as usize] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
        }
    }

    fn new_additive_node(&mut self) -> NodeKey {
        let add_gen = MathUGen::<F, U1, Add>::new();
        // TODO: We don't need a full handle here
        let add_handle = self.push_internal(add_gen);
        let add_node = add_handle.raw_handle.node.key;
        self.get_nodes_mut()[add_node].auto_math_node = true;
        add_node
    }
    fn new_feedback_nodes(&mut self) -> (NodeKey, NodeKey) {
        let buffers = Arc::new([
            OwnedRawBuffer::new(self.block_size),
            OwnedRawBuffer::new(self.block_size),
        ]);
        // We currently guarantee that the two instances of this Arc are on the same thread, and therefore
        // we can get away with using a `Cell` which is !Sync.
        #[allow(clippy::arc_with_non_send_sync)]
        let bufnum = Arc::new(Cell::new(0));
        let sink = FeedbackSink {
            buffers: buffers.clone(),
            bufnum: bufnum.clone(),
        };
        let source = FeedbackSink {
            buffers: buffers.clone(),
            bufnum: bufnum.clone(),
        };
        // TODO: We don't need a full handle here
        let sink_handle = self.push_internal(sink);
        let sink_node = sink_handle.raw_handle.node.key;
        let source_handle = self.push_internal(source);
        let source_node = source_handle.raw_handle.node.key;
        self.get_nodes_mut()[source_node].auto_free_when_unconnected = true;
        self.get_nodes_mut()[source_node].strong_dependent = Some(sink_node);
        self.get_nodes_mut()[sink_node].auto_free_when_unconnected = true;
        self.get_nodes_mut()[sink_node].strong_dependent = Some(source_node);
        (source_node, sink_node)
    }

    // /// Connect a source to a sink with the designated channels replacing any existing connections to the sink at those channels. If you want to add to any
    // /// existing inputs to the sink, use [`Graph::connect`]
    // ///
    // /// # Example
    // /// ```rust,ignore
    // /// // Connect `sine` to `lpf`, channel 0 to 0
    // /// graph.connect_replace(&sine, 0, 0, &lpf)?;
    // /// // Connect `multi_oscillator` to the graph outputs, channels 1 to 0, 2, 1
    // /// // and 0 to 3.
    // /// graph.connect_replace(&multi_oscillator, [1, 2, 0], [0, 1, 2], Sink::Graph)?;
    // /// ```
    // #[deprecated(since = "0.1.0", note = "Deprecated in favour of the GraphEdit API")]
    // fn connect_replace<N: Size>(
    //     &mut self,
    //     source: impl Into<Connectable>,
    //     source_channels: impl Into<Channels<N>>,
    //     sink_channels: impl Into<Channels<N>>,
    //     sink: impl Into<Connectable>,
    // ) -> Result<(), GraphError> {
    //     let source = source.into();
    //     let sink = sink.into();
    //     for (so_chan, si_chan) in source_channels
    //         .into()
    //         .into_iter()
    //         .zip(sink_channels.into().into_iter())
    //     {
    //         if let Some((source, so_chan)) = source.for_output_channel(so_chan) {
    //             if let Some((sink, si_chan)) = sink.for_input_channel(si_chan) {
    //                 match (source, sink) {
    //                     (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
    //                         self.connect_input_to_node(sink, so_chan, si_chan, false)?;
    //                     }
    //                     (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
    //                         self.connect_nodes(source, sink, so_chan, si_chan, false, false)?;
    //                     }
    //                     (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
    //                         self.connect_node_to_output(source, so_chan, si_chan, false)?;
    //                     }
    //                     (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
    //                         self.connect_input_to_output(so_chan, si_chan, false)?;
    //                     }
    //                 }
    //             }
    //         }
    //     }
    //     Ok(())
    // }
    // /// Connect a source to a sink with the designated channels, addin it to any existing connections to the sink at those channels. If you want to replace
    // /// existing inputs to the sink, use [`Graph::connect_replace`]
    // #[deprecated(since = "0.1.0", note = "Deprecated in favour of the GraphEdit API")]
    // fn connect<N: Size>(
    //     &mut self,
    //     source: impl Into<Connectable>,
    //     source_channels: impl Into<Channels<N>>,
    //     sink_channels: impl Into<Channels<N>>,
    //     sink: impl Into<Connectable>,
    // ) -> Result<(), GraphError> {
    //     let source = source.into();
    //     let sink = sink.into();
    //     for (so_chan, si_chan) in source_channels
    //         .into()
    //         .into_iter()
    //         .zip(sink_channels.into().into_iter())
    //     {
    //         if let Some((source, so_chan)) = source.for_output_channel(so_chan) {
    //             if let Some((sink, si_chan)) = sink.for_input_channel(si_chan) {
    //                 match (source, sink) {
    //                     (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
    //                         self.connect_input_to_node(sink, so_chan, si_chan, true)?;
    //                     }
    //                     (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
    //                         self.connect_nodes(source, sink, so_chan, si_chan, true, false)?;
    //                     }
    //                     (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
    //                         self.connect_node_to_output(source, so_chan, si_chan, true)?;
    //                     }
    //                     (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
    //                         self.connect_input_to_output(so_chan, si_chan, true)?;
    //                     }
    //                 }
    //             }
    //         }
    //     }
    //     Ok(())
    // }
    /// Disconnect all edges from a specific output channel of a node or a graph input.
    pub fn disconnect_output_from_source(
        &mut self,
        source: impl Into<NodeOrGraph>,
        source_channel: u16,
    ) -> Result<(), GraphError> {
        let source = source.into();
        match source {
            NodeOrGraph::Node(source_node) => {
                if source_node.graph != self.graph_id {
                    return Err(GraphError::WrongSinkNodeGraph {
                        expected_graph: source_node.graph,
                        found_graph: self.graph_id,
                    });
                }
                let nodes = self.get_nodes();
                if !nodes.contains_key(source_node.key()) {
                    return Err(GraphError::NodeNotFound);
                }
                let key = source_node.key();
                let node = &nodes[key];
                if source_channel >= node.data.outputs {
                    return Err(GraphError::OutputOutOfBounds(source_channel));
                }
            }
            NodeOrGraph::Graph => {
                if source_channel >= self.num_inputs {
                    return Err(GraphError::GraphInputOutOfBounds(source_channel));
                }
                if let Some(edge) = self.output_edges[source_channel as usize].take() {
                    if let NodeKeyOrGraph::Node(source_node) = edge.source {
                        self.evaluate_if_node_should_be_removed(source_node);
                    }
                }
            }
        }
        let source: NodeKeyOrGraph = source.into();
        // We have to go through all edges
        for (_k, input_edges) in &mut self.node_input_edges {
            for edge in input_edges.iter_mut().filter(|e| e.is_some()) {
                let edge_unwraped = edge.as_mut().unwrap();
                if edge_unwraped.channel_in_source == source_channel
                    && source == edge_unwraped.source
                {
                    *edge = None;
                }
            }
        }
        for edge in self.output_edges.iter_mut().filter(|e| e.is_some()) {
            let edge_unwraped = edge.as_mut().unwrap();
            if edge_unwraped.channel_in_source == source_channel && source == edge_unwraped.source {
                *edge = None;
            }
        }

        self.recalculation_required = true;
        Ok(())
    }
    /// Disconnect any input from a specific channel on a node or an internal graph output.
    pub fn disconnect_input_to_sink(
        &mut self,
        sink_channel: u16,
        sink: impl Into<NodeOrGraph>,
    ) -> Result<(), GraphError> {
        match sink.into() {
            NodeOrGraph::Node(sink_node) => {
                if sink_node.graph != self.graph_id {
                    return Err(GraphError::WrongSinkNodeGraph {
                        expected_graph: sink_node.graph,
                        found_graph: self.graph_id,
                    });
                }
                let nodes = self.get_nodes();
                if !nodes.contains_key(sink_node.key()) {
                    return Err(GraphError::NodeNotFound);
                }
                let edges = &mut self.node_input_edges[sink_node.key()];
                if sink_channel as usize >= edges.len() {
                    return Err(GraphError::InputOutOfBounds(sink_channel));
                }
                if let Some(edge) = edges[sink_channel as usize].take() {
                    self.evaluate_if_node_should_be_removed(sink_node.key());
                    if let NodeKeyOrGraph::Node(source_node) = edge.source {
                        self.evaluate_if_node_should_be_removed(source_node);
                    }
                }
                self.recalculation_required = true;
            }
            NodeOrGraph::Graph => {
                if sink_channel >= self.num_outputs {
                    return Err(GraphError::GraphOutputOutOfBounds(sink_channel));
                }
                if let Some(edge) = self.output_edges[sink_channel as usize].take() {
                    if let NodeKeyOrGraph::Node(source_node) = edge.source {
                        self.evaluate_if_node_should_be_removed(source_node);
                    }
                }
            }
        }
        Ok(())
    }
    /// If the node should be automatically removed, this removes it.
    fn evaluate_if_node_should_be_removed(&mut self, key: NodeKey) {
        let node = &mut self.get_nodes_mut()[key];
        if node.auto_math_node {
            // We assume math nodes have two inputs and one output
            assert_eq!(node.data.inputs, 2);
            assert_eq!(node.data.outputs, 1);
            if let Some(edges) = self.node_input_edges.get(key) {
                match (edges[0], edges[1]) {
                    (Some(_), Some(_)) => (),
                    (None, Some(input_edge)) | (Some(input_edge), None) => {
                        // Find out where this math node is pointing and replace the node
                        for (_k, input_edges) in &mut self.node_input_edges {
                            for edge in input_edges.iter_mut().filter(|e| e.is_some()) {
                                let edge = edge.as_mut().unwrap();
                                if let NodeKeyOrGraph::Node(source) = &edge.source {
                                    if *source == key {
                                        *edge = input_edge;
                                    }
                                }
                            }
                        }
                        for edge in &mut self.output_edges {
                            let edge = edge.as_mut().unwrap();
                            if let NodeKeyOrGraph::Node(source) = &edge.source {
                                if *source == key {
                                    *edge = input_edge;
                                }
                            }
                        }
                        self.free_node_from_key(key).ok();
                    }
                    (None, None) => {
                        // No inputs, simply remove
                        self.free_node_from_key(key).ok();
                    }
                }
            }
        }
        let node = &mut self.get_nodes_mut()[key];
        if node.auto_free_when_unconnected {
            let mut unconnected = true;
            'unconnected_block: {
                if let Some(edges) = self.node_input_edges.get(key) {
                    if edges.iter().flatten().peekable().peek().is_some() {
                        unconnected = false;
                        break 'unconnected_block;
                    }
                }
                for (_k, input_edges) in &mut self.node_input_edges {
                    for edge in input_edges.iter().flatten() {
                        if let NodeKeyOrGraph::Node(source) = &edge.source {
                            if *source == key {
                                unconnected = false;
                                break 'unconnected_block;
                            }
                        }
                    }
                }
            }
            if unconnected {
                self.free_node_from_key(key).ok();
            }
        }
    }
    /// Connect a source to a sink with the designated channels, addin it to any existing connections to the sink at those channels. If you want to replace
    /// existing inputs to the sink, use [`Graph::connect2_replace`]
    pub fn connect2(
        &mut self,
        source: impl Into<NodeOrGraph>,
        source_channel: u16,
        sink_channel: u16,
        sink: impl Into<NodeOrGraph>,
    ) -> Result<(), GraphError> {
        match (source.into(), sink.into()) {
            (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
                self.connect_input_to_node(sink, source_channel, sink_channel, true)?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
                self.connect_nodes_internal(
                    source,
                    sink,
                    source_channel,
                    sink_channel,
                    true,
                    false,
                )?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
                self.connect_node_to_output(source, source_channel, sink_channel, true)?;
            }
            (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
                self.connect_input_to_output(source_channel, sink_channel, true)?;
            }
        }
        Ok(())
    }
    /// Connect a source to a sink with the designated channels, replacing any existing connections to the sink at those channels.
    pub fn connect2_replace(
        &mut self,
        source: NodeOrGraph,
        source_channel: u16,
        sink_channel: u16,
        sink: NodeOrGraph,
    ) -> Result<(), GraphError> {
        let so_chan = source_channel;
        let si_chan = sink_channel;
        match (source, sink) {
            (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
                self.connect_input_to_node(sink, so_chan, si_chan, false)?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
                self.connect_nodes_internal(source, sink, so_chan, si_chan, false, false)?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
                self.connect_node_to_output(source, so_chan, si_chan, false)?;
            }
            (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
                self.connect_input_to_output(so_chan, si_chan, false)?;
            }
        }
        Ok(())
    }
    /// Connect a source to a sink with the designated channels with feedback, adding to any existing connections to the sink at those channels. Feedback means that the signal data will be delayed by one block, breaking potential cycles in the graph.
    pub fn connect2_feedback(
        &mut self,
        source: NodeOrGraph,
        source_channel: u16,
        sink_channel: u16,
        sink: NodeOrGraph,
    ) -> Result<(), GraphError> {
        let so_chan = source_channel;
        let si_chan = sink_channel;
        match (source, sink) {
            (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
                log::error!(
                    "There was an attempt to connect a graph input to a node via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
                );
                self.connect_input_to_node(sink, so_chan, si_chan, true)?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
                self.connect_nodes_internal(source, sink, so_chan, si_chan, true, true)?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
                log::error!(
                    "There was an attempt to connect a node to a graph output via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
                );
                self.connect_node_to_output(source, so_chan, si_chan, true)?;
            }
            (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
                log::error!(
                    "There was an attempt to connect to a graph input to a  graph output via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
                );
                self.connect_input_to_output(so_chan, si_chan, true)?;
            }
        }
        Ok(())
    }
    /// Connect a source to a sink with the designated channels with feedback, adding to any existing connections to the sink at those channels. Feedback means that the signal data will be delayed by one block, breaking potential cycles in the graph.
    pub fn connect2_feedback_replace(
        &mut self,
        source: NodeOrGraph,
        source_channel: u16,
        sink_channel: u16,
        sink: NodeOrGraph,
    ) -> Result<(), GraphError> {
        let so_chan = source_channel;
        let si_chan = sink_channel;
        match (source, sink) {
            (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
                log::error!(
                    "There was an attempt to connect a graph input to a node via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
                );
                self.connect_input_to_node(sink, so_chan, si_chan, false)?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
                self.connect_nodes_internal(source, sink, so_chan, si_chan, false, true)?;
            }
            (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
                log::error!(
                    "There was an attempt to connect a node to a graph output via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
                );
                self.connect_node_to_output(source, so_chan, si_chan, false)?;
            }
            (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
                log::error!(
                    "There was an attempt to connect to a graph input to a  graph output via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
                );
                self.connect_input_to_output(so_chan, si_chan, false)?;
            }
        }
        Ok(())
    }
    // /// Connect a source to a sink via a feedback edge with the designated channels, adding it to any existing connections to the sink at those channels. If you want to replace
    // /// existing inputs to the sink, use [`Graph::connect_replace_feedback`]
    // ///
    // /// A feedback edge is used to break cycles in a graph and causes a one block delay of the
    // /// signal data.
    // #[deprecated(since = "0.1.0", note = "Deprecated in favour of the GraphEdit API")]
    // pub fn connect_feedback<N: Size>(
    //     &mut self,
    //     source: impl Into<Connectable>,
    //     source_channels: impl Into<Channels<N>>,
    //     sink_channels: impl Into<Channels<N>>,
    //     sink: impl Into<Connectable>,
    // ) -> Result<(), GraphError> {
    //     let source = source.into();
    //     let sink = sink.into();
    //     for (so_chan, si_chan) in source_channels
    //         .into()
    //         .into_iter()
    //         .zip(sink_channels.into().into_iter())
    //     {
    //         if let Some((source, so_chan)) = source.for_output_channel(so_chan) {
    //             if let Some((sink, si_chan)) = sink.for_input_channel(si_chan) {
    //                 match (source, sink) {
    //                     (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
    //                         log::error!(
    //                             "There was an attempt to connect a graph input to a node via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
    //                         );
    //                         self.connect_input_to_node(sink, so_chan, si_chan, true)?;
    //                     }
    //                     (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
    //                         self.connect_nodes_internal(
    //                             source, sink, so_chan, si_chan, true, true,
    //                         )?;
    //                     }
    //                     (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
    //                         log::error!(
    //                             "There was an attempt to connect a node to a graph output via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
    //                         );
    //                         self.connect_node_to_output(source, so_chan, si_chan, true)?;
    //                     }
    //                     (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
    //                         log::error!(
    //                             "There was an attempt to connect to a graph input to a  graph output via a feedback node. Feedback to or from graph outputs is non-sensical. Connection will be made without feedback."
    //                         );
    //                         self.connect_input_to_output(so_chan, si_chan, false)?;
    //                     }
    //                 }
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    /// Set a parameter value on a node.
    ///
    /// # Errors
    ///
    /// Returns an error if the node is not found in the graph or the `param` is not a valid parameter for the node.
    pub fn set(
        &self,
        node: impl Into<NodeId>,
        param: impl Into<Param>,
        value: impl Into<ParameterValue>,
        t: impl Into<Time>,
    ) -> Result<(), GraphError> {
        let node_id = node.into();
        if !node_id.graph == self.graph_id {
            return Err(GraphError::WrongSinkNodeGraph {
                expected_graph: self.graph_id,
                found_graph: node_id.graph,
            });
        }
        let nodes = self.get_nodes();
        let Some(node) = nodes.get(node_id.key()) else {
            return Err(GraphError::NodeNotFound);
        };
        let param_index = match param.into() {
            knaster_core::Param::Index(param_i) => {
                if param_i as u16 >= node.data.parameters {
                    return Err(ParameterError::ParameterIndexOutOfBounds.into());
                } else {
                    param_i
                }
            }
            knaster_core::Param::Desc(desc) => {
                match node.parameter_descriptions().position(|d| d == desc) {
                    Some(param_i) => param_i,
                    _ => {
                        // Fail
                        return Err(ParameterError::DescriptionNotFound(desc).into());
                    }
                }
            }
        };
        let value = value.into();
        self.graph_gen_communicator
            .scheduling_event_producer
            .send(crate::SchedulingEvent {
                node_key: node_id.key(),
                parameter: param_index,
                value: Some(value),
                smoothing: None,
                token: None,
                time: Some(t.into()),
            })?;
        Ok(())
    }
    /// Set many parameters at once, logging any errors encountered. Even if an error is encountered, the rest of the changes will be applied.
    pub fn set_many(&self, changes: &[(NodeId, Param, ParameterValue)], time: Time) {
        for (node, param, value) in changes {
            if let Err(e) = self.set(*node, *param, *value, time) {
                log::error!(
                    "Error setting parameter {:?} on node {:?}: {:?}",
                    param,
                    node,
                    e
                );
            }
        }
    }
    /// Perform edits on the graph structure, such as adding nodes, removing nodes, connecting nodes.
    /// The edits are committed in a single step when the [`GraphEdit`] is dropped.
    ///
    /// Note that parameter changes don't require editing the structure of the graph, and therefore
    /// don't require this method. See [`Parameter`], [`Graph::set`] and [`Graph::set_many`].
    pub fn edit<T>(&mut self, c: impl FnOnce(GraphEdit<F>) -> T) -> T {
        c(GraphEdit::new(self))
    }

    #[deprecated(note = "Deprecated in favour or GraphEdit")]
    /// Create a subgraph as a new node in this graph.
    pub fn subgraph<Inputs: Size, Outputs: Size>(&mut self, options: GraphOptions) -> Self {
        let temporary_invalid_node_id = NodeId::invalid();
        let (mut subgraph, graph_gen) = Self::new::<Inputs, Outputs>(
            options,
            temporary_invalid_node_id,
            self.graph_gen_communicator.shared_frame_clock.clone(),
            self.block_size,
            self.sample_rate,
            |_| {},
        );
        let node_key = self.push_node(graph_gen);
        self.get_nodes_mut()[node_key].is_graph = Some(subgraph.graph_id);
        // Set the real NodeId of the Graph
        subgraph.self_node_id = NodeId {
            key: node_key,
            graph: self.graph_id,
        };

        subgraph
    }
    pub(crate) fn subgraph_init<Inputs: Size, Outputs: Size>(
        &mut self,
        options: GraphOptions,
        init_callback: impl FnOnce(GraphEdit<F>),
    ) -> Self {
        let temporary_invalid_node_id = NodeId::invalid();
        let (mut subgraph, graph_gen) = Self::new::<Inputs, Outputs>(
            options,
            temporary_invalid_node_id,
            self.graph_gen_communicator.shared_frame_clock.clone(),
            self.block_size,
            self.sample_rate,
            init_callback,
        );
        let node_key = self.push_node(graph_gen);
        self.get_nodes_mut()[node_key].is_graph = Some(subgraph.graph_id);
        // Set the real NodeId of the Graph
        subgraph.self_node_id = NodeId {
            key: node_key,
            graph: self.graph_id,
        };

        subgraph
    }

    /// Returns true if there is a path from `from` to `to` in the graph.
    fn has_path(&self, from: NodeId, to: NodeId) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![to.key];
        let from = from.key;

        while let Some(node) = stack.pop() {
            if node == from {
                return true;
            }
            if visited.insert(node) {
                if let Some(edges) = self.node_input_edges.get(node) {
                    for &parent in edges.iter().flatten() {
                        if let NodeKeyOrGraph::Node(parent) = parent.source {
                            stack.push(parent);
                        }
                    }
                }
            }
        }

        false
    }
    /// Create the list of node executions, with all the data they need to be
    /// run, in the correct order.
    fn generate_tasks(&mut self) -> Vec<Task<F>> {
        let mut tasks = vec![];
        // Safety: No other thread will access the SlotMap. All we're doing with the buffers is taking pointers; there's no manipulation.
        let nodes = unsafe { &mut *self.nodes.get() };
        for (i, &node_key) in self.node_order.iter().enumerate() {
            let node = &mut nodes[node_key];
            tasks.push(node.make_task(i));
        }
        tasks
    }
    fn generate_output_tasks(&mut self) -> OutputTask<F> {
        let mut output_task = OutputTask {
            channels: vec![None; self.num_outputs as usize].into_boxed_slice(),
        };
        let block_size = self.block_size;
        for (sink_channel, output_edge) in self
            .output_edges
            .iter()
            .enumerate()
            // Return only the channels that are Some
            .filter_map(|(i, e)| e.map(|e| (i, e)))
        {
            match output_edge.source {
                NodeKeyOrGraph::Node(source_key) => {
                    let source = &self.get_nodes()[source_key];
                    let source_ptr = source
                        .node_output_ptr()
                        .expect("Node output should be ptr at this point");
                    assert!(output_edge.channel_in_source < source.data.outputs);
                    output_task.channels[sink_channel] = Some(BlockOrGraphInput::Block(unsafe {
                        source_ptr.add(block_size * (output_edge.channel_in_source as usize))
                    }));
                }
                NodeKeyOrGraph::Graph => {
                    output_task.channels[sink_channel] = Some(BlockOrGraphInput::GraphInput(
                        output_edge.channel_in_source as usize,
                    ));
                }
            }
        }
        output_task
    }
    /// Looking at the parameter edges in the graph, this function generates a
    /// list of all the buffer to parameter mappings for the current Graph.
    /// Since buffers may reallocate, we set all of the parameter buffers every
    /// time the schedule is updated.
    fn generate_ar_parameter_changes(&mut self) -> Vec<ArParameterChange<F>> {
        let mut apc = Vec::new();
        for (node_key, edges) in &self.node_parameter_edges {
            for edge in edges {
                let ParameterEdge {
                    source,
                    channel_in_source,
                    parameter_index,
                } = *edge;
                {
                    if let Some(node_index) = self.node_order.iter().position(|k| *k == node_key) {
                        let source_node = &self.get_nodes()[source];
                        let buffer = source_node
                            .node_output_ptr()
                            .expect("Node output ptr should be available when generating tasks");
                        assert!(channel_in_source < source_node.data.outputs);
                        // Safety: The buffer has at least `source_node.outputs`
                        // channels of data of size `self.block_size`.
                        let buffer =
                            unsafe { buffer.add(channel_in_source as usize * self.block_size) };
                        apc.push(ArParameterChange {
                            node: node_index,
                            parameter_index: parameter_index as usize,
                            buffer,
                        })
                    }
                }
            }
        }
        apc
    }

    /// `graph_inputs_to_nodes`: (node_index_in_order, Vec<(graph_input_channel, node_input_channel))
    fn generate_task_data(
        &mut self,
        applied_flag: Arc<AtomicBool>,
        graph_input_channels_to_nodes: Vec<(usize, Vec<(usize, usize)>)>,
    ) -> TaskData<F> {
        let tasks = self.generate_tasks().into_boxed_slice();
        let output_task = self.generate_output_tasks();
        let ar_parameter_changes = self.generate_ar_parameter_changes();
        TaskData {
            applied: applied_flag,
            tasks,
            output_task,
            current_buffer_allocation: Some(self.buffer_allocator.buffer()),
            ar_parameter_changes,
            graph_input_channels_to_nodes,
            node_task_order: self.node_order.clone(),
        }
    }
    /// Assign buffers to nodes maximizing buffer reuse and cache locality
    /// (ideally, there are surely optimisations left)
    ///
    /// Returns (node_index_in_order, Vec<(graph_input_channel, node_input_channel))
    #[must_use]
    fn allocate_node_buffers(&mut self) -> Vec<(usize, Vec<(usize, usize)>)> {
        // Recalculate the number of dependent channels of a node
        // TODO: This makes a lot of node lookups. Optimise?
        for (_key, node) in self.get_nodes_mut() {
            node.num_output_dependents = 0;
        }
        for (_key, edges) in &self.node_input_edges {
            for edge in edges.iter().filter_map(|e| *e) {
                match edge.source {
                    NodeKeyOrGraph::Node(source_key) => {
                        // Safety:
                        //
                        // Accessing self.nodes is always safe because the
                        // Arc owned by the GraphGen will never touch it, it just
                        // guarantees that the allocation stays valid.
                        (unsafe { &mut *self.nodes.get() })[source_key].num_output_dependents += 1;
                    }
                    NodeKeyOrGraph::Graph => {}
                }
            }
        }
        // Add parameter edges as dependents
        for (_key, edges) in &self.node_parameter_edges {
            for edge in edges {
                (unsafe { &mut *self.nodes.get() })[edge.source].num_output_dependents += 1;
            }
        }

        let mut graph_input_pointers_to_nodes = Vec::new();
        // Assign buffers
        self.buffer_allocator.reset(self.block_size);
        // TODO: Iterate by index instead
        let node_order = self.node_order.clone();
        for (node_order_index, &key) in node_order.iter().enumerate() {
            let outputs = self.get_nodes()[key].data.outputs as usize;
            let num_borrows = self.get_nodes()[key].num_output_dependents;
            let offset = self
                .buffer_allocator
                .get_block(outputs, self.block_size, num_borrows);
            self.get_nodes_mut()[key].assign_output_offset(offset);
            let mut input_pointers_to_node = None;
            // Return every block that is used as an input
            for (channel_index, edge) in self.node_input_edges[key]
                .iter()
                .enumerate()
                .filter_map(|(i, e)| e.map(|e| (i, e)))
            {
                match edge.source {
                    NodeKeyOrGraph::Node(source_key) => {
                        let block = self.get_nodes()[source_key].node_output;
                        if let crate::node::NodeOutput::Offset(block) = block {
                            self.buffer_allocator.return_block(block);
                        }
                    }
                    NodeKeyOrGraph::Graph => {
                        if input_pointers_to_node.is_none() {
                            input_pointers_to_node = Some(Vec::new());
                        }
                        input_pointers_to_node
                            .as_mut()
                            .unwrap()
                            .push((edge.channel_in_source as usize, channel_index));
                    }
                }
            }
            // Also parameter inputs
            let param_edges = &self.node_parameter_edges[key];
            for edge in param_edges {
                let block = self.get_nodes()[edge.source].node_output;
                if let crate::node::NodeOutput::Offset(block) = block {
                    self.buffer_allocator.return_block(block);
                }
            }
            if let Some(graph_inputs_to_node) = input_pointers_to_node {
                graph_input_pointers_to_nodes.push((node_order_index, graph_inputs_to_node));
            }
        }
        if let Some(discarded_allocation) =
            self.buffer_allocator.finished_assigning_make_allocation()
        {
            self.buffers_to_free_when_safe.push(discarded_allocation);
        }
        // Convert offsets to pointers
        for &key in &node_order {
            unsafe { (&mut *self.nodes.get())[key].swap_offset_to_ptr(&self.buffer_allocator) };
        }
        // Assign input pointers
        for (node_key, edges) in &self.node_input_edges {
            let num_inputs = self.get_nodes()[node_key].data.inputs;
            let mut inputs = vec![crate::core::ptr::null(); num_inputs as usize];

            for (sink_channel, edge) in edges
                .iter()
                .enumerate()
                // Return only the channels that are Some
                .filter_map(|(i, e)| e.map(|e| (i, e)))
            {
                if let NodeKeyOrGraph::Node(source_key) = edge.source {
                    let source_output_ptr = self.get_nodes()[source_key]
                        .node_output_ptr()
                        .expect("real buffer was just assigned");
                    inputs[sink_channel] = unsafe {
                        source_output_ptr.add(edge.channel_in_source as usize * self.block_size)
                    };
                }
            }
            // If any input hasn't been set, give it a cleared zero buffer.
            // This is important for soundness.
            for input in &mut inputs {
                if input.is_null() {
                    *input = self.buffer_allocator.empty_channel();
                }
            }
            unsafe { (&mut *self.nodes.get())[node_key].assign_inputs(inputs) };
        }
        graph_input_pointers_to_nodes
    }

    /// Applies the latest changes to connections and added nodes in the graph on the audio thread and updates the scheduler.
    pub(crate) fn commit_changes(&mut self) -> Result<(), GraphError> {
        // We need to run free_old to know if there are nodes to free and hence a recalculation required.
        self.free_old();
        self.graph_gen_communicator.free_old_task_data();
        if self.recalculation_required {
            self.calculate_node_order();
            let graph_input_pointers_to_nodes = self.allocate_node_buffers();

            let ggc = &mut self.graph_gen_communicator;
            let current_change_flag = crate::core::mem::replace(
                &mut ggc.next_change_flag,
                Arc::new(AtomicBool::new(false)),
            );
            let task_data =
                self.generate_task_data(current_change_flag, graph_input_pointers_to_nodes);
            self.graph_gen_communicator.send_updated_tasks(task_data)?;
            self.recalculation_required = false;
        }
        Ok(())
    }
    /// Returns the [`GraphId`] of this graph.
    pub fn graph_id(&self) -> GraphId {
        self.graph_id
    }

    pub(crate) fn free_node_from_key(&mut self, node_key: NodeKey) -> Result<(), FreeError> {
        // Does the Node exist?
        if !self.get_nodes_mut().contains_key(node_key) {
            return Err(FreeError::NodeNotFound);
        }
        if !self.node_mortality[node_key] {
            return Err(FreeError::ImmortalNode);
        }
        if self
            .node_keys_to_free_when_safe
            .iter()
            .any(|(key, _)| *key == node_key)
        {
            return Ok(());
        }
        //
        self.recalculation_required = true;

        // Remove all edges leading to the node
        if let Some(_edges) = self.node_input_edges.remove(node_key) {}
        self.node_parameter_edges.remove(node_key);
        // Remove all edges leading from the node to other nodes
        for (_k, input_edges) in &mut self.node_input_edges {
            let mut i = 0;
            while i < input_edges.len() {
                if let Some(edge) = input_edges[i] {
                    if let NodeKeyOrGraph::Node(source) = edge.source {
                        if source == node_key {
                            input_edges[i] = None;
                        } else {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
        // Remove all edges leading from the node to the Graph output
        {
            let mut i = 0;
            while i < self.output_edges.len() {
                if let Some(edge) = self.output_edges[i] {
                    if let NodeKeyOrGraph::Node(source) = edge.source {
                        if source == node_key {
                            self.output_edges[i] = None;
                        } else {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
        let ggc = &mut self.graph_gen_communicator;
        self.node_keys_to_free_when_safe
            .push((node_key, ggc.next_change_flag.clone()));
        if let Some(dep) = self.get_nodes()[node_key].strong_dependent {
            self.free_node_from_key(dep).ok();
        }
        // self.node_keys_pending_removal.insert(node_key);
        Ok(())
    }

    /// Generate inspection metadata for this graph. Intended for
    /// generating static or dynamic inspection and graph manipulation tools.
    pub fn inspection(&self) -> GraphInspection {
        let real_nodes = self.get_nodes();
        // Maps a node key to the index in the Vec
        let mut node_key_processed = Vec::with_capacity(real_nodes.len());
        let mut nodes = Vec::with_capacity(real_nodes.len());
        for &node_key in &self.node_order {
            let node = &real_nodes[node_key];
            let mut input_edges = Vec::new();
            if let Some(edges) = self.node_input_edges.get(node_key) {
                for (input_channel_index, edge) in edges
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| e.map(|e| (i, e)))
                {
                    input_edges.push(EdgeInspection {
                        source: match edge.source {
                            NodeKeyOrGraph::Node(key) => EdgeSource::Node(key),
                            NodeKeyOrGraph::Graph => EdgeSource::Graph,
                        },
                        from_index: edge.channel_in_source,
                        to_index: input_channel_index as u16,
                        is_feedback: false,
                    });
                }
            }

            nodes.push(NodeInspection {
                name: node.name.to_string(),
                key: node_key,
                inputs: node.data.inputs,
                outputs: node.data.outputs,
                input_edges,
                parameter_descriptions: node.parameter_descriptions().collect(),
                parameter_hints: node.parameter_hints().collect(),
                unconnected: self.disconnected_nodes.contains(&node_key),
                is_graph: None,
            });
            node_key_processed.push(node_key);
        }
        let mut graph_output_edges = Vec::new();
        for (channel_index, edge) in self.output_edges.iter().enumerate() {
            if let Some(edge) = edge {
                graph_output_edges.push(EdgeInspection {
                    source: match edge.source {
                        NodeKeyOrGraph::Node(key) => EdgeSource::Node(key),
                        NodeKeyOrGraph::Graph => EdgeSource::Graph,
                    },
                    from_index: edge.channel_in_source,
                    to_index: channel_index as u16,
                    is_feedback: edge.is_feedback,
                });
            }
        }

        GraphInspection {
            nodes,
            num_inputs: self.num_inputs,
            num_outputs: self.num_outputs,
            graph_id: self.graph_id,
            graph_output_edges,
            graph_name: self.name.clone(),
            param_sender: self
                .graph_gen_communicator
                .scheduling_event_producer
                .clone(),
            shared_frame_clock: self.graph_gen_communicator.shared_frame_clock.clone(),
        }
    }
    /// Returns the number of nodes that are pending removal. Used for testing.
    #[allow(unused)]
    pub(crate) fn num_nodes_pending_removal(&self) -> usize {
        self.node_keys_to_free_when_safe.len()
    }
    /// Check if there are any old nodes or other resources that have been
    /// removed from the graph and can now be freed since they are no longer
    /// used on the audio thread.
    fn free_old(&mut self) {
        // See if any nodes are marked for removal
        let mut free_queue = Vec::new();
        for (key, node) in self.get_nodes_mut() {
            if let Some(to_free) = &mut node.remove_me {
                if to_free.load(Ordering::SeqCst) {
                    free_queue.push(key);
                }
            }
        }
        for key in free_queue {
            self.free_node_from_key(key).ok();
        }
        // Remove orphaned internal math nodes.
        // Math nodes should be removed when one of its inputs was removed
        // This could be merged with the loop above, but then math nodes would be removed one step after its input(s)
        let node_keys: Vec<NodeKey> = unsafe {
            self.nodes
                .get()
                .as_mut()
                .expect("nodes are always available")
                .keys()
                .collect()
        };
        for key in node_keys {
            // TODO: Evaluate for affected nodes when changes occur instead of on all nodes.
            self.evaluate_if_node_should_be_removed(key);
        }

        // Remove old nodes
        let mut i = 0;
        while i < self.node_keys_to_free_when_safe.len() {
            let (key, flag) = &self.node_keys_to_free_when_safe[i];
            if flag.load(Ordering::SeqCst) {
                unsafe { self.nodes.get().as_mut().unwrap().remove(*key) };
                // nodes.remove(*key);
                // self.node_keys_pending_removal.remove(key);
                self.node_keys_to_free_when_safe.remove(i);
            } else {
                i += 1;
            }
        }
        // Remove old buffers
        if !self.buffers_to_free_when_safe.is_empty() {
            let mut i = self.buffers_to_free_when_safe.len() - 1;
            loop {
                if Arc::<OwnedRawBuffer<F>>::strong_count(&self.buffers_to_free_when_safe[i]) == 1 {
                    self.buffers_to_free_when_safe.remove(i);
                }
                if i == 0 {
                    break;
                }
                i -= 1;
            }
        }
    }

    /// Goes through all the nodes that are connected to nodes in `nodes_to_process` and adds them to the list in
    /// reverse depth first order.
    fn depth_first_search(
        &self,
        visited: &mut HashSet<NodeKey>,
        nodes_to_process: &mut Vec<NodeKey>,
    ) -> Vec<NodeKey> {
        let mut node_order = Vec::with_capacity(self.get_nodes().capacity());
        while !nodes_to_process.is_empty() {
            let node_key = *nodes_to_process.last().unwrap();

            let input_edges = &self.node_input_edges[node_key];
            let mut found_unvisited = false;
            // There is probably room for optimisation here by managing to
            // not iterate the edges multiple times.
            for edge in input_edges.iter().filter_map(|e| *e) {
                if !edge.is_feedback {
                    if let NodeKeyOrGraph::Node(source) = edge.source {
                        if !visited.contains(&source) {
                            nodes_to_process.push(source);
                            visited.insert(source);
                            found_unvisited = true;
                            break;
                        }
                    }
                }
            }
            if !found_unvisited {
                let param_input_edges = &self.node_parameter_edges[node_key];
                for edge in param_input_edges.iter() {
                    let source = edge.source;
                    if !visited.contains(&source) {
                        nodes_to_process.push(source);
                        visited.insert(source);
                        found_unvisited = true;
                        break;
                    }
                }
            }
            if !found_unvisited {
                node_order.push(nodes_to_process.pop().unwrap());
            }
        }
        node_order
    }
    /// Looks for the deepest (furthest away from the graph output) node that is also an output node, i.e.
    /// a node that is both an output node and an input to another node which is eventually connected to
    /// an output is deeper than a node which is only connected to an output.
    fn get_deepest_output_node(&self, start_node: NodeKey, visited: &HashSet<NodeKey>) -> NodeKey {
        let mut last_connected_node_index = start_node;
        let mut last_connected_output_node_index = start_node;
        loop {
            let mut found_later_node = false;
            for (key, input_edges) in &self.node_input_edges {
                for input_edge in input_edges.iter().filter_map(|e| *e) {
                    if let NodeKeyOrGraph::Node(source) = input_edge.source {
                        if source == last_connected_node_index && !visited.contains(&source) {
                            last_connected_node_index = key;
                            found_later_node = true;

                            // check if it's an output node
                            for edge in self.output_edges.iter().filter_map(|e| *e) {
                                if let NodeKeyOrGraph::Node(source) = edge.source {
                                    if last_connected_node_index == source {
                                        last_connected_output_node_index =
                                            last_connected_node_index;
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
                if found_later_node {
                    break;
                }
            }
            if !found_later_node {
                break;
            }
        }
        last_connected_output_node_index
    }
    /// Calculate the node order of the graph based on the outputs
    /// Post-ordered depth first search
    /// NB: Not real-time safe
    pub fn calculate_node_order(&mut self) {
        self.node_order.clear();
        // Set the visited status for all nodes to false
        let mut visited = HashSet::new();
        let mut nodes_to_process = Vec::with_capacity(self.get_nodes_mut().capacity());
        for edge in self.output_edges.iter().filter_map(|e| *e) {
            if let NodeKeyOrGraph::Node(source) = edge.source {
                // The same source node may be present in multiple output edges
                // e.g. for stereo so we need to check if visited. The input to
                // one graph output may also depend on the input to another
                // graph output. Therefore we need to make sure to start with
                // the deepest output nodes only.
                let deepest_node = self.get_deepest_output_node(source, &visited);
                if !visited.contains(&deepest_node) {
                    nodes_to_process.push(deepest_node);
                    visited.insert(deepest_node);
                }
            }
        }

        let stack = self.depth_first_search(&mut visited, &mut nodes_to_process);
        self.node_order.extend(stack);

        // Add all remaining nodes. These are not currently connected to anything.
        let mut remaining_nodes = vec![];
        for (node_key, _node) in self.get_nodes() {
            if !visited.contains(&node_key)
                && !self
                    .node_keys_to_free_when_safe
                    .iter()
                    .any(|(key, _)| key == &node_key)
            {
                remaining_nodes.push(node_key);
            }
        }
        self.node_order.extend(remaining_nodes.iter());
        self.disconnected_nodes = remaining_nodes;
        // debug
        // let nodes = self.get_nodes();
        // for (i, n) in self.node_order.iter().enumerate() {
        //     let name = nodes.get(*n).unwrap().name;
        //     println!("{i}: {name}, {n:?}");
        // }
        // dbg!(&self.node_order);
        // dbg!(&self.disconnected_nodes);
    }
    fn get_nodes(&self) -> &SlotMap<NodeKey, Node<F>> {
        unsafe { &*self.nodes.get() }
    }
    fn get_nodes_mut(&mut self) -> &mut SlotMap<NodeKey, Node<F>> {
        // # Safety:
        //
        // It is generally not safe to mutably access the content of an Arc
        // without a synchronisation method, but we know that the other holder
        // of a clone of this Arc will never access it. Additionally, the
        // content of the Arc is wrapped in an UnsafeCell to signal interior
        // mutability.
        unsafe { &mut *self.nodes.get() }
    }
    /// Set the mortality of a node, i.e. whether it can be removed or not. If `mortality` is false, the node cannot be removed.
    pub fn set_mortality(
        &mut self,
        node: impl Into<NodeId>,
        mortality: bool,
    ) -> Result<(), GraphError> {
        let node = node.into();
        if let Some(m) = self.node_mortality.get_mut(node.key()) {
            *m = mortality;
            Ok(())
        } else {
            Err(GraphError::NodeNotFound)
        }
    }
    /// Number of input channels going into this graph.
    pub fn inputs(&self) -> u16 {
        self.num_inputs
    }
    /// Number of output channels going out from this graph.
    pub fn outputs(&self) -> u16 {
        self.num_outputs
    }
    /// The sample rate of this graph.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    /// The block size of this graph.
    pub fn block_size(&self) -> usize {
        self.block_size
    }
    /// Returns the [`NodeId`] of the Graph node in the parent. The top level Graph has an invalid NodeId since it is not a node in any other Graph.
    pub fn id(&self) -> NodeId {
        self.self_node_id
    }
    // /// Connectable for connecting the Graph to other nodes within its parent graph.
    // #[deprecated]
    // pub fn as_node(&self) -> Connectable {
    //     Connectable::from_node(
    //         NodeSubset {
    //             node: NodeOrGraph::Node(self.self_node_id),
    //             channels: self.inputs(),
    //             start_channel: 0,
    //         },
    //         NodeSubset {
    //             node: NodeOrGraph::Node(self.self_node_id),
    //             channels: self.outputs(),
    //             start_channel: 0,
    //         },
    //     )
    // }
    // /// Connectable for connecting inside the graph. Note that inside the graph, the graph outputs
    // /// are sinks/inputs and the graph outputs are sources/outputs.
    // #[deprecated]
    // pub fn internal(&self) -> Connectable {
    //     Connectable::from_node(
    //         NodeSubset {
    //             node: NodeOrGraph::Graph,
    //             channels: self.outputs(),
    //             start_channel: 0,
    //         },
    //         NodeSubset {
    //             node: NodeOrGraph::Graph,
    //             channels: self.inputs(),
    //             start_channel: 0,
    //         },
    //     )
    // }
}

fn shorten_name(name: &str) -> EcoString {
    let mut short = EcoString::new();
    for path in name.split_inclusive(&['<', '>', ';', '(', ')', '[', ']'][..]) {
        // Push the last part of the extracted path
        if let Some(last) = path.rsplit_once(':') {
            short.push_str(last.1);
        } else {
            short.push_str(path);
        }
        if let Some(',') | Some(';') = short.chars().last() {
            short.push(' ');
        }
    }
    short
}

unsafe impl<F: Float> Send for Graph<F> {}
/// # Safety
///
/// The UnsafeCell within, making Graph !Sync, is actually only
/// mutateable using &mut Graph and not from any other threads. UnsafeCell
/// is used to give us free interior mutability inside the Arc. The Arc is there
/// to ensure the nodes aren't dropped if Graph is dropped, but GraphGen is still
/// alive.
unsafe impl<F: Float> Sync for Graph<F> {}

struct GraphGenCommunicator<F: Float> {
    /// The ring buffer for sending scheduled changes to the audio thread
    scheduling_event_producer: SchedulingChannelSender,
    /// The next change flag to be attached to a task update. When the changes
    /// in the update have been applied on the audio thread, this flag till be
    /// set to true. Its purpose is to make sure nodes can be safely dropped
    /// because they are guaranteed not to be accessed on the audio thread. This
    /// is done by each node to be deleted having a clone of this flag which
    /// corresponds to the update when that node was removed from the Tasks
    /// list.
    next_change_flag: Arc<AtomicBool>,
    shared_frame_clock: SharedFrameClock,

    task_data_to_be_dropped_consumer: rtrb::Consumer<TaskData<F>>,
    new_task_data_producer: rtrb::Producer<TaskData<F>>,
    // TODO: Removed from here, but may require other structures to be implemented
    // For sending clock updates to the audio thread
    // clock_update_producer: rtrb::Producer<ClockUpdate>,
    // free_node_queue_consumer: rtrb::Consumer<(NodeKey, GenState)>,
}
impl<F: Float> GraphGenCommunicator<F> {
    fn free_old_task_data(&mut self) {
        // If there are discarded tasks, check if they can be removed
        let num_to_remove = self.task_data_to_be_dropped_consumer.slots();
        let chunk = self
            .task_data_to_be_dropped_consumer
            .read_chunk(num_to_remove);
        if let Ok(chunk) = chunk {
            for td in chunk {
                drop(td);
            }
        }
    }

    /// Sends the updated tasks to the GraphGen. NB: Always check if any
    /// resoruces in the Graph can be freed before running this.
    /// GraphGenCommunicator will free its own resources.
    fn send_updated_tasks(&mut self, task_data: TaskData<F>) -> Result<(), GraphError> {
        match self.new_task_data_producer.push(task_data) {
            Err(e) => Err(GraphError::SendToGraphGen(format!("{e}"))),
            _ => Ok(()),
        }
    }
}

/// Errors that can occur when interacting with a [`Graph`].
#[derive(thiserror::Error, Debug)]
pub enum GraphError {
    #[allow(missing_docs)]
    #[error(
        "Connection would create a circular dependency. Cannot connect. Try using a feedback connection."
    )]
    CircularConnection,
    #[allow(missing_docs)]
    #[error("Error sending new data to GraphGen: `{0}`")]
    SendToGraphGen(String),
    #[allow(missing_docs)]
    #[error("Node cannot be found in current Graph.")]
    NodeNotFound,
    #[allow(missing_docs)]
    #[error("Source node is in a different Graph `{found_graph}`, expecting `{expected_graph}`")]
    WrongSourceNodeGraph {
        expected_graph: GraphId,
        found_graph: GraphId,
    },
    #[allow(missing_docs)]
    #[error("Sink node is in a different Graph `{found_graph}`, expecting `{expected_graph}`")]
    WrongSinkNodeGraph {
        expected_graph: GraphId,
        found_graph: GraphId,
    },
    #[allow(missing_docs)]
    #[error("Tried to connect a node input that doesn't exist: `{0}`")]
    InputOutOfBounds(u16),
    #[allow(missing_docs)]
    #[error("Tried to connect to a node output that doesn't exist: `{0}`")]
    OutputOutOfBounds(u16),
    #[error("Tried to connect a graph input that doesn't exist (`{0}`) to some destination")]
    #[allow(missing_docs)]
    GraphInputOutOfBounds(u16),
    #[error("Tried to connect a graph output that doesn't exist (`{0}`) to some destination")]
    #[allow(missing_docs)]
    GraphOutputOutOfBounds(u16),
    #[error("The parameter `{0}` is not a valid parameter description for the node")]
    #[allow(missing_docs)]
    ParameterDescriptionNotFound(String),
    #[error("The parameter `{0}` is not a valid parameter index for the node")]
    #[allow(missing_docs)]
    ParameterIndexOutOfBounds(usize),
    #[error(transparent)]
    #[allow(missing_docs)]
    ParameterError(#[from] ParameterError),
    #[error("There was an error sending the change: `{0}`")]
    #[allow(missing_docs)]
    PushChangeError(String),
    #[error(transparent)]
    #[allow(missing_docs)]
    FreeError(#[from] FreeError),
}

#[allow(missing_docs)]
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum FreeError {
    #[error("The NodeId does not exist. The Node may have been freed already.")]
    NodeNotFound,
    #[error(
        "The node you tried to free has been marked as immortal. Make it mortal before freeing."
    )]
    ImmortalNode,
    // #[error("The free action required making a new connection, but the connection failed.")]
    // ConnectionError(#[from] Box<connection::ConnectionError>),
}

impl<F: Float> From<&Graph<F>> for NodeId {
    fn from(value: &Graph<F>) -> Self {
        value.self_node_id
    }
}
impl<F: Float> From<&mut Graph<F>> for NodeId {
    fn from(value: &mut Graph<F>) -> Self {
        value.self_node_id
    }
}

// The impl of Default allows NumericArray::default() which is handy
#[derive(Debug, Default, Copy, Clone)]
/// A node or the graph in which the [`NodeOrGraph`] is used. Used for connections where the source
/// or sink can be either a node or the graph.
pub enum NodeOrGraph {
    #[allow(missing_docs)]
    #[default]
    Graph,
    #[allow(missing_docs)]
    Node(NodeId),
}
impl<T: Into<NodeId>> From<T> for NodeOrGraph {
    fn from(value: T) -> Self {
        Self::Node(value.into())
    }
}
/// A newtype for an array of channel indices.
///
/// The generic `Size` parameter lets us ensure that channel arrays as inputs for a connection
/// function match in arity at compile time.
pub struct Channels<N: Size> {
    channels: NumericArray<u16, N>,
}
impl<N: Size> IntoIterator for Channels<N> {
    type Item = u16;

    type IntoIter = <NumericArray<u16, N> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.channels.into_iter()
    }
}
impl<N: Size> From<NumericArray<u16, N>> for Channels<N> {
    fn from(value: NumericArray<u16, N>) -> Self {
        Self { channels: value }
    }
}
impl<N: Size, const N2: usize> From<[u16; N2]> for Channels<N>
where
    crate::typenum::Const<{ N2 }>: crate::typenum::ToUInt,
    crate::typenum::Const<{ N2 }>: knaster_core::numeric_array::generic_array::IntoArrayLength,
    knaster_core::numeric_array::generic_array::GenericArray<u16, N>: From<[u16; N2]>,
{
    fn from(value: [u16; N2]) -> Self {
        Self {
            channels: value.into(),
        }
    }
}
impl From<u16> for Channels<U1> {
    fn from(value: u16) -> Self {
        Self {
            channels: [value].into(),
        }
    }
}

/// The sink for a feedback connection, i.e. the UGen that stores data to be read by the other node
///
/// Highly unsafe. The allocation of buffers is done separately instead of using the init function.
/// Should only be used from inside the Graph. Assumes that a single Graph is always run
pub(crate) struct FeedbackSink<F: Float> {
    buffers: Arc<[OwnedRawBuffer<F>; 2]>,
    /// We promise that all nodes within a Graph will be processed sequentially and not
    /// concurrently. If that changes, this has to be changed into something like Arc<AtomicUsize>
    ///
    /// We currently guarantee that the two instances of this Arc are on the same thread, and therefore
    /// we can get away with using a `Cell` which is !Sync.
    bufnum: Arc<Cell<usize>>,
}
impl<F: Float> UGen for FeedbackSink<F> {
    type Sample = F;
    type Inputs = U1;
    type Outputs = U0;
    type Parameters = U0;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut knaster_core::UGenFlags,
        _input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        // Only for use inside Graph which won't use `process` directly
        unreachable!()
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut knaster_core::UGenFlags,
        input: &InBlock,
        _output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        let bufnum = self.bufnum.get();
        let input = input.channel_as_slice(0);
        unsafe { self.buffers[bufnum].as_slice_mut().copy_from_slice(input) }
    }

    fn param_hints()
    -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterHint, Self::Parameters>
    {
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
/// The source for a feedback connection
///
/// Highly unsafe. The allocation of buffers is done separately instead of using the init function.
/// Should only be used from inside the Graph.
pub(crate) struct FeedbackSource<F: Float> {
    buffers: Arc<[OwnedRawBuffer<F>; 2]>,
    /// We promise that all nodes within a Graph will be processed sequentially and not
    /// concurrently. If that changes, this has to be changed into something like Arc<AtomicUsize>
    bufnum: Arc<Cell<usize>>,
}
impl<F: Float> UGen for FeedbackSource<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;
    type Parameters = U0;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut knaster_core::UGenFlags,
        _input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        // Only for use inside Graph which won't use `process` directly
        unreachable!()
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut knaster_core::UGenFlags,
        _input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        let bufnum = self.bufnum.get();
        let prev_buffer = 1 - bufnum;
        self.bufnum.set(prev_buffer);
        let output = output.channel_as_slice_mut(0);
        unsafe { output.copy_from_slice(self.buffers[prev_buffer].as_slice_mut()) }
    }

    fn param_hints()
    -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterHint, Self::Parameters>
    {
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
#[cfg(test)]
mod tests {
    use knaster_core::{
        Done, PTrigger,
        envelopes::EnvAsr,
        typenum::{U0, U2},
    };

    use crate::{
        handle::HandleTrait,
        processor::{AudioProcessor, AudioProcessorOptions},
    };

    #[test]
    fn free_node_when_done() {
        let block_size = 16;
        let (mut graph, mut audio_processor, _log_receiver) =
            AudioProcessor::<f32>::new::<U0, U2>(AudioProcessorOptions {
                block_size,
                sample_rate: 48000,
                ring_buffer_size: 50,
                ..Default::default()
            });
        let asr = graph.push_with_done_action(EnvAsr::new(0.0, 0.0), Done::FreeSelf);
        asr.set(("attack_time", 0.0)).unwrap();
        asr.set(("release_time", 0.0)).unwrap();
        asr.set(("t_restart", PTrigger)).unwrap();
        asr.set(("t_release", PTrigger)).unwrap();
        graph.commit_changes().unwrap();
        assert_eq!(graph.inspection().nodes.len(), 1);
        for _ in 0..10 {
            unsafe {
                audio_processor.run(&[]);
            }
        }
        // Run the code to free old nodes
        graph.commit_changes().unwrap();
        assert_eq!(graph.inspection().nodes.len(), 0);
        assert_eq!(graph.num_nodes_pending_removal(), 1);
        // Apply the new TaskData on the audio thread so that the node can be removed
        unsafe {
            audio_processor.run(&[]);
        }
        // Now the node is removed
        graph.commit_changes().unwrap();
        assert_eq!(graph.num_nodes_pending_removal(), 0);
        assert_eq!(graph.inspection().nodes.len(), 0);
    }
}
