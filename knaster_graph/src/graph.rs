use crate::{
    buffer_allocator::BufferAllocator,
    connectable::{Channels, NodeOrGraph, NodeSubset},
    core::sync::atomic::AtomicU64,
    edge::{Edge, NodeKeyOrGraph, ParameterEdge},
    graph_gen::GraphGen,
    handle::{Handle, RawHandle},
    node::Node,
    task::{ArParameterChange, BlockOrGraphInput, OutputTask, Task, TaskData},
    SchedulingChannelProducer, SharedFrameClock,
};
use crate::{
    connectable::Connectable,
    core::{
        cell::UnsafeCell,
        dbg, format,
        sync::atomic::{AtomicBool, Ordering},
    },
};
use alloc::{borrow::ToOwned, boxed::Box, string::String, string::ToString, vec, vec::Vec};
use std::collections::VecDeque;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::inspection::{EdgeInspection, EdgeSource, GraphInspection, NodeInspection};
use crate::wrappers_graph::done::WrDone;
use knaster_core::{
    math::{Add, MathUGen},
    typenum::*,
    AudioCtx, Done, Float, Param, ParameterError, Size, UGen,
};
use rtrb::RingBuffer;
use slotmap::{new_key_type, SecondaryMap, SlotMap};

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
    pub(crate) fn top_level_graph_node_id() -> Self {
        Self {
            key: NodeKey::default(),
            // We should never reach the max of a u64, see comment for GraphId
            graph: GraphId::MAX,
        }
    }
    pub fn key(&self) -> NodeKey {
        self.key
    }
}

/// Some options for
#[derive(Clone, Debug)]
pub struct GraphOptions {
    /// The name of the Graph
    pub name: String,
    /// The number of messages that can be sent through any of the ring buffers.
    /// Ring buffers are used pass information back and forth between the audio
    /// thread (GraphGen) and the Graph.
    pub ring_buffer_size: usize,
}

impl Default for GraphOptions {
    fn default() -> Self {
        GraphOptions {
            name: String::new(),
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
}
impl<F: Float> Drop for OwnedRawBuffer<F> {
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.ptr)) }
    }
}

pub struct Graph<F: Float> {
    id: GraphId,
    name: String,
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
    node_keys_pending_removal: HashSet<NodeKey>,
    /// A list of input edges for every node. The input channel is the index into the boxed slice
    node_input_edges: SecondaryMap<NodeKey, Box<[Option<Edge>]>>,
    /// Edges which control a parameter of a node through the output of another
    /// node. These can be in addition to audio input edges.
    node_parameter_edges: SecondaryMap<NodeKey, Vec<ParameterEdge>>,
    /// List of feedback input edges for every node. The NodeKey in the tuple is the index of the FeedbackNode doing the buffering
    // node_feedback_edges: SecondaryMap<NodeKey, Vec<FeedbackEdge>>,
    node_feedback_node_key: SecondaryMap<NodeKey, NodeKey>,
    /// If a node can be freed or not. A node can be made immortal to avoid accidentally removing it.
    node_mortality: SecondaryMap<NodeKey, bool>,
    node_order: Vec<NodeKey>,
    disconnected_nodes: Vec<NodeKey>,
    feedback_node_indices: Vec<NodeKey>,
    /// The outputs of the Graph
    output_edges: Box<[Option<Edge>]>,
    /// If changes have been made that require recalculating the graph this will be set to true.
    recalculation_required: bool,
    num_inputs: usize,
    num_outputs: usize,
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
    ) -> (Self, Node<F>) {
        let GraphOptions {
            name,
            ring_buffer_size,
        } = options;
        const DEFAULT_NUM_NODES: usize = 4;
        let id = NEXT_GRAPH_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
            scheduling_event_producer: Arc::new(Mutex::new(scheduling_event_producer)),
            task_data_to_be_dropped_consumer,
            new_task_data_producer,
            next_change_flag: Arc::new(AtomicBool::new(false)),
            shared_frame_clock,
        };
        let remove_me = Arc::new(AtomicBool::new(false));
        let mut graph = Self {
            id,
            name,
            nodes,
            node_input_edges,
            node_parameter_edges,
            node_feedback_node_key: SecondaryMap::with_capacity(DEFAULT_NUM_NODES),
            // node_feedback_edges,
            node_mortality: SecondaryMap::with_capacity(DEFAULT_NUM_NODES),
            node_order: Vec::with_capacity(DEFAULT_NUM_NODES),
            disconnected_nodes: vec![],
            node_keys_to_free_when_safe: vec![],
            node_keys_pending_removal: HashSet::new(),
            feedback_node_indices: vec![],
            output_edges: vec![None; Outputs::USIZE].into(),
            num_inputs: Inputs::USIZE,
            num_outputs: Outputs::USIZE,
            block_size,
            sample_rate,
            graph_gen_communicator,
            recalculation_required: false,
            buffers_to_free_when_safe: vec![],
            buffer_allocator,
            self_node_id: node_id,
        };
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
            String::from("GraphGen"),
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

    pub fn shared_frame_clock(&self) -> SharedFrameClock {
        self.graph_gen_communicator.shared_frame_clock.clone()
    }

    /// Push something implementing [`UGen`] to the graph.
    pub fn push<T: UGen<Sample = F> + 'static>(&mut self, ugen: T) -> Handle<T> {
        let name = std::any::type_name::<T>();
        let name = shorten_name(name);
        let node = Node::new(name.to_owned(), ugen);
        let node_key = self.push_node(node);

        Handle::new(RawHandle::new(
            NodeId {
                key: node_key,
                graph: self.id,
            },
            self.graph_gen_communicator
                .scheduling_event_producer
                .clone(),
            self.graph_gen_communicator.shared_frame_clock.clone(),
        ))
    }
    /// Push something implementing [`UGen`] to the graph, adding the [`WrDone`] wrapper. This
    /// enables the node to free itself if it marks itself as done or for removal using [`GenFlags`].
    pub fn push_with_done_action<T: UGen<Sample = F> + 'static>(
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
        let name = std::any::type_name::<T>();
        let name = shorten_name(name);
        let mut node = Node::new(name.to_owned(), ugen);
        node.remove_me = Some(free_self_flag);
        let node_key = self.push_node(node);
        Handle::new(RawHandle::new(
            NodeId {
                key: node_key,
                graph: self.id,
            },
            self.graph_gen_communicator
                .scheduling_event_producer
                .clone(),
            self.graph_gen_communicator.shared_frame_clock.clone(),
        ))
    }

    /// Add a node to this Graph. The Node will be (re)initialised with the
    /// correct block size for this Graph.
    fn push_node(&mut self, mut node: Node<F>) -> NodeKey {
        self.recalculation_required = true;
        let ctx = AudioCtx::new(self.sample_rate, self.block_size);

        node.init(&ctx);
        let node_inputs = node.inputs;
        let key = self.get_nodes_mut().insert(node);
        self.node_input_edges
            .insert(key, vec![None; node_inputs].into_boxed_slice());
        // self.node_feedback_edges.insert(key, vec![]);
        self.node_mortality.insert(key, true);
        self.node_parameter_edges.insert(key, vec![]);

        key
    }

    pub fn connect_nodes(
        &mut self,
        source: impl Into<NodeId>,
        sink: impl Into<NodeId>,
        source_channel: usize,
        sink_channel: usize,
        additive: bool,
    ) -> Result<(), GraphError> {
        let source = source.into();
        let sink = sink.into();
        if !source.graph == self.id {
            return Err(GraphError::WrongGraph);
        }
        if !sink.graph == self.id {
            return Err(GraphError::WrongGraph);
        }

        let nodes = self.get_nodes();
        if !nodes.contains_key(source.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if source_channel >= nodes[source.key()].outputs {
            return Err(GraphError::OutputOutOfBounds(source_channel));
        }
        if !nodes.contains_key(sink.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if sink_channel >= nodes[sink.key()].inputs {
            return Err(GraphError::InputOutOfBounds(sink_channel));
        }
        self.connect_to_node_internal(
            NodeKeyOrGraph::Node(source.key()),
            sink.key(),
            source_channel,
            sink_channel,
            additive,
        );
        Ok(())
    }
    /// Connect a graph input directly to a graph output
    pub fn connect_input_to_output(
        &mut self,
        source_channel: usize,
        sink_channel: usize,
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
    pub fn connect_node_to_output(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: usize,
        sink_channel: usize,
        additive: bool,
    ) -> Result<(), GraphError> {
        let source = source.into();
        if !source.graph == self.id {
            return Err(GraphError::WrongGraph);
        }
        if sink_channel >= self.num_outputs {
            return Err(GraphError::GraphOutputOutOfBounds(sink_channel));
        }
        let nodes = self.get_nodes();
        if !nodes.contains_key(source.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if source_channel >= nodes[source.key()].outputs {
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
        source_channel: usize,
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
        source_channel: usize,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
    ) -> Result<(), GraphError> {
        self.connect_node_to_parameter(source, source_channel, parameter, sink, false)
    }
    fn connect_node_to_parameter(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: usize,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
        additive: bool,
    ) -> Result<(), GraphError> {
        let source = source.into();
        if !source.graph == self.id {
            return Err(GraphError::WrongGraph);
        }
        let sink = sink.into();
        if !sink.graph == self.id {
            return Err(GraphError::WrongGraph);
        }
        let nodes = self.get_nodes();
        let sink_node = &nodes[sink.key()];
        let param = parameter.into();
        let param_index = match param {
            Param::Index(param_index) => param_index,
            Param::Desc(desc) => {
                if let Some(index) = sink_node
                    .parameter_descriptions
                    .iter()
                    .position(|s| s == &desc)
                {
                    if index >= sink_node.parameter_descriptions.len() {
                        return Err(GraphError::ParameterIndexOutOfBounds(index));
                    }
                    index
                } else {
                    return Err(GraphError::ParameterDescriptionNotFound(desc.to_string()));
                }
            }
        };
        let nodes = self.get_nodes();
        if !nodes.contains_key(source.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if source_channel >= nodes[source.key()].outputs {
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
    pub fn connect_input_to_node(
        &mut self,
        sink: impl Into<NodeId>,
        source_channel: usize,
        sink_channel: usize,
        additive: bool,
    ) -> Result<(), GraphError> {
        let sink = sink.into();
        if !sink.graph == self.id {
            return Err(GraphError::WrongGraph);
        }
        dbg!(self.num_inputs, source_channel);
        if source_channel >= self.num_inputs {
            return Err(GraphError::GraphInputOutOfBounds(source_channel));
        }
        let nodes = self.get_nodes();
        if !nodes.contains_key(sink.key()) {
            return Err(GraphError::NodeNotFound);
        }
        if sink_channel >= nodes[sink.key()].inputs {
            return Err(GraphError::OutputOutOfBounds(sink_channel));
        }

        self.connect_to_node_internal(
            NodeKeyOrGraph::Graph,
            sink.key(),
            source_channel,
            sink_channel,
            additive,
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
        source: NodeKeyOrGraph,
        sink: NodeKey,
        so_channel: usize,
        si_channel: usize,
        additive: bool,
    ) {
        self.recalculation_required = true;
        // Fast and common path
        if !additive {
            self.node_input_edges[sink][si_channel] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            return;
        }
        // Connect additively
        // If no input exists for the channel, connect directly.
        // If an input does exist, create a new add node and connect it up, replacing the current edge.

        if let Some(existing_edge) = self.node_input_edges[sink][si_channel] {
            // Put an add node in between the input and the previous input,
            // adding the new source together with the old
            let add_node = self.new_additive_node();
            self.node_input_edges[add_node][0] = Some(existing_edge);
            self.node_input_edges[add_node][1] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            self.node_input_edges[sink][si_channel] = Some(Edge {
                source: NodeKeyOrGraph::Node(add_node),
                channel_in_source: 0,
                is_feedback: false,
            });
        } else {
            self.node_input_edges[sink][si_channel] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
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
        so_channel: usize,
        si_channel: usize,
        additive: bool,
    ) {
        // Only the pob functions do input checking on nodes and channels. This enables

        self.recalculation_required = true;
        // Fast and common path
        if !additive {
            self.output_edges[si_channel] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            return;
        }
        // Connect additively
        // If no input exists for the channel, connect directly.
        // If an input does exist, create a new add node and connect it up, replacing the current edge.

        if let Some(existing_edge) = self.output_edges[si_channel] {
            // Put an add node in between the input and the previous input,
            // adding the new source together with the old
            let add_node = self.new_additive_node();
            self.node_input_edges[add_node][0] = Some(existing_edge);
            self.node_input_edges[add_node][1] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
            self.output_edges[si_channel] = Some(Edge {
                source: NodeKeyOrGraph::Node(add_node),
                channel_in_source: 0,
                is_feedback: false,
            });
        } else {
            self.output_edges[si_channel] = Some(Edge {
                source,
                channel_in_source: so_channel,
                is_feedback: false,
            });
        }
    }

    fn new_additive_node(&mut self) -> NodeKey {
        let add_gen = MathUGen::<F, U1, Add>::new();
        // TODO: We don't need a full handle here
        let add_handle = self.push(add_gen);
        let add_node = add_handle.raw_handle.node.key;
        self.get_nodes_mut()[add_node].auto_added = true;
        add_node
    }

    /// Connect a source to a sink with the designated channels replacing any existing connections to the sink at those channels. If you want to add to any
    /// existing inputs to the sink, use [`Graph::connect`]
    ///
    /// # Example
    /// ```rust,ignore
    /// // Connect `sine` to `lpf`, channel 0 to 0
    /// graph.connect_replace(&sine, 0, 0, &lpf)?;
    /// // Connect `multi_oscillator` to the graph outputs, channels 1 to 0, 2, 1
    /// // and 0 to 3.
    /// graph.connect_replace(&multi_oscillator, [1, 2, 0], [0, 1, 2], Sink::Graph)?;
    /// ```
    pub fn connect_replace<N: Size>(
        &mut self,
        source: impl Into<Connectable>,
        source_channels: impl Into<Channels<N>>,
        sink_channels: impl Into<Channels<N>>,
        sink: impl Into<Connectable>,
    ) -> Result<(), GraphError> {
        let source = source.into();
        let sink = sink.into();
        for (so_chan, si_chan) in source_channels
            .into()
            .into_iter()
            .zip(sink_channels.into().into_iter())
        {
            if let Some((source, so_chan)) = source.for_output_channel(so_chan) {
                if let Some((sink, si_chan)) = sink.for_input_channel(si_chan) {
                    match (source, sink) {
                        (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
                            self.connect_input_to_node(sink, so_chan, si_chan, false)?;
                        }
                        (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
                            self.connect_nodes(source, sink, so_chan, si_chan, false)?;
                        }
                        (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
                            self.connect_node_to_output(source, so_chan, si_chan, false)?;
                        }
                        (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
                            self.connect_input_to_output(so_chan, si_chan, false)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    /// Connect a source to a sink with the designated channels, addin it to any existing connections to the sink at those channels. If you want to replace
    /// existing inputs to the sink, use [`Graph::connect_replace`]
    pub fn connect<N: Size>(
        &mut self,
        source: impl Into<Connectable>,
        source_channels: impl Into<Channels<N>>,
        sink_channels: impl Into<Channels<N>>,
        sink: impl Into<Connectable>,
    ) -> Result<(), GraphError> {
        let source = source.into();
        let sink = sink.into();
        for (so_chan, si_chan) in source_channels
            .into()
            .into_iter()
            .zip(sink_channels.into().into_iter())
        {
            if let Some((source, so_chan)) = source.for_output_channel(so_chan) {
                if let Some((sink, si_chan)) = sink.for_input_channel(si_chan) {
                    match (source, sink) {
                        (NodeOrGraph::Graph, NodeOrGraph::Node(sink)) => {
                            self.connect_input_to_node(sink, so_chan, si_chan, true)?;
                        }
                        (NodeOrGraph::Node(source), NodeOrGraph::Node(sink)) => {
                            self.connect_nodes(source, sink, so_chan, si_chan, true)?;
                        }
                        (NodeOrGraph::Node(source), NodeOrGraph::Graph) => {
                            self.connect_node_to_output(source, so_chan, si_chan, true)?;
                        }
                        (NodeOrGraph::Graph, NodeOrGraph::Graph) => {
                            self.connect_input_to_output(so_chan, si_chan, true)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn subgraph<Inputs: Size, Outputs: Size>(&mut self, options: GraphOptions) -> Self {
        let temporary_invalid_node_id = NodeId::top_level_graph_node_id();
        let (mut subgraph, graph_gen) = Self::new::<Inputs, Outputs>(
            options,
            temporary_invalid_node_id,
            self.graph_gen_communicator.shared_frame_clock.clone(),
            self.block_size,
            self.sample_rate,
        );
        // TODO: Store node key in graph
        let node_key = self.push_node(graph_gen);
        self.get_nodes_mut()[node_key].is_graph = Some(subgraph.id);
        // Set the real NodeId of the Graph
        subgraph.self_node_id = NodeId {
            key: node_key,
            graph: self.id,
        };

        subgraph
    }

    /// Create the list of node executions, with all the data they need to be
    /// run, in the correct order.
    fn generate_tasks(&mut self) -> Vec<Task<F>> {
        let mut tasks = vec![];
        // Safety: No other thread will access the SlotMap. All we're doing with the buffers is taking pointers; there's no manipulation.
        let nodes = unsafe { &mut *self.nodes.get() };
        for &node_key in &self.node_order {
            let node = &nodes[node_key];
            tasks.push(node.to_task());
        }
        tasks
    }
    fn generate_output_tasks(&mut self) -> OutputTask<F> {
        let mut output_task = OutputTask {
            channels: vec![None; self.num_outputs].into_boxed_slice(),
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
                    assert!(output_edge.channel_in_source < source.outputs);
                    output_task.channels[sink_channel] = Some(BlockOrGraphInput::Block(unsafe {
                        source_ptr.add(block_size * (output_edge.channel_in_source))
                    }));
                }
                NodeKeyOrGraph::Graph => {
                    output_task.channels[sink_channel] =
                        Some(BlockOrGraphInput::GraphInput(output_edge.channel_in_source));
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
                        assert!(channel_in_source < source_node.outputs);
                        // Safety: The buffer has at least `source_node.outputs`
                        // channels of data of size `self.block_size`.
                        let buffer = unsafe { buffer.add(channel_in_source * self.block_size) };
                        apc.push(ArParameterChange {
                            node: node_index,
                            parameter_index,
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
        let nodes = self.get_nodes();
        let gens: Vec<_> = self
            .node_order
            .iter()
            .map(|key| (*key, nodes[*key].ugen))
            .collect();
        let ar_parameter_changes = self.generate_ar_parameter_changes();
        TaskData {
            applied: applied_flag,
            tasks,
            output_task,
            current_buffer_allocation: Some(self.buffer_allocator.buffer()),
            ar_parameter_changes,
            gens,
            graph_input_channels_to_nodes,
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
            let outputs = self.get_nodes()[key].outputs;
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
                            .push((edge.channel_in_source, channel_index));
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
            let num_inputs = self.get_nodes()[node_key].inputs;
            let mut inputs = vec![crate::core::ptr::null(); num_inputs];

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
                    inputs[sink_channel] =
                        unsafe { source_output_ptr.add(edge.channel_in_source * self.block_size) };
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
    pub fn commit_changes(&mut self) -> Result<(), GraphError> {
        // We need to run free_old to know if there are nodes to free and hence a recalculation required.
        self.free_old();
        self.graph_gen_communicator.free_old();
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

    fn free_node_from_key(&mut self, node_key: NodeKey) -> Result<(), FreeError> {
        // Does the Node exist?
        if !self.get_nodes_mut().contains_key(node_key) {
            return Err(FreeError::NodeNotFound);
        }
        if !self.node_mortality[node_key] {
            return Err(FreeError::ImmortalNode);
        }

        self.recalculation_required = true;

        // Remove all edges leading to the node
        self.node_input_edges.remove(node_key);
        // feedback from the freed node requires removing the feedback node and all edges from the feedback node
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
        self.clear_feedback_for_node(node_key)?;
        let ggc = &mut self.graph_gen_communicator;
        self.node_keys_to_free_when_safe
            .push((node_key, ggc.next_change_flag.clone()));
        self.node_keys_pending_removal.insert(node_key);
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
                        to_index: input_channel_index,
                        is_feedback: false,
                    });
                }
            }

            nodes.push(NodeInspection {
                name: node.name.to_string(),
                key: node_key,
                inputs: node.inputs,
                outputs: node.outputs,
                input_edges,
                parameter_descriptions: node.parameter_descriptions.clone(),
                parameter_hints: node.parameter_hints.clone(),
                pending_removal: self.node_keys_pending_removal.contains(&node_key),
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
                    to_index: channel_index,
                    is_feedback: edge.is_feedback,
                });
            }
        }

        GraphInspection {
            nodes,
            num_inputs: self.num_inputs,
            num_outputs: self.num_outputs,
            graph_id: self.id,
            graph_output_edges,
            graph_name: self.name.clone(),
            param_sender: self
                .graph_gen_communicator
                .scheduling_event_producer
                .clone(),
            shared_frame_clock: self.graph_gen_communicator.shared_frame_clock.clone(),
        }
    }
    fn clear_feedback_for_node(&mut self, node_key: NodeKey) -> Result<(), FreeError> {
        // TODO: Update for new feedback node system
        // Remove all feedback edges leading from or to the node
        // let mut nodes_to_free = HashSet::new();
        // if let Some(&feedback_node) = self.node_feedback_node_key.get(node_key) {
        //     // The node that is being freed has a feedback node attached to it. Free that as well.
        //     nodes_to_free.insert(feedback_node);
        //     self.node_feedback_node_key.remove(node_key);
        // }
        // for (feedback_key, feedback_edges) in &mut self.node_feedback_edges {
        //     if !feedback_edges.is_empty() {
        //         let mut i = 0;
        //         while i < feedback_edges.len() {
        //             if feedback_edges[i].source == node_key
        //                 || feedback_edges[i].feedback_destination == node_key
        //             {
        //                 feedback_edges.remove(i);
        //             } else {
        //                 i += 1;
        //             }
        //         }
        //         if feedback_edges.is_empty() {
        //             // The feedback node has no more edges to it: free it
        //             nodes_to_free.insert(feedback_key);
        //             // TODO: Will this definitely remove all feedback node
        //             // key references? Can a feedback node be manually freed
        //             // in a different way?
        //             let mut node_feedback_node_belongs_to = None;
        //             for (source_node, &feedback_node) in &self.node_feedback_node_key {
        //                 if feedback_node == feedback_key {
        //                     node_feedback_node_belongs_to = Some(source_node);
        //                 }
        //             }
        //             if let Some(key) = node_feedback_node_belongs_to {
        //                 self.node_feedback_node_key.remove(key);
        //             }
        //         }
        //     }
        // }
        // for na in nodes_to_free {
        //     self.free_node_from_key(na)?;
        // }
        Ok(())
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
        let nodes = unsafe { &mut *self.nodes.get() };
        for (key, node) in nodes.iter_mut() {
            if node.auto_added {
                // We assume math nodes have two inputs and one output
                assert_eq!(node.inputs, 2);
                assert_eq!(node.outputs, 1);
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
        }

        // Remove old nodes
        let mut i = 0;
        while i < self.node_keys_to_free_when_safe.len() {
            let (key, flag) = &self.node_keys_to_free_when_safe[i];
            if flag.load(Ordering::SeqCst) {
                nodes.remove(*key);
                self.node_keys_pending_removal.remove(key);
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
                if let NodeKeyOrGraph::Node(source) = edge.source {
                    if !visited.contains(&source) {
                        nodes_to_process.push(source);
                        visited.insert(source);
                        found_unvisited = true;
                        break;
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
        // Add feedback nodes first, their order doesn't matter
        self.node_order.extend(self.feedback_node_indices.iter());
        // Set the visited status for all nodes to false
        let mut visited = HashSet::new();
        // add the feedback node indices as visited
        for &feedback_node_index in &self.feedback_node_indices {
            visited.insert(feedback_node_index);
        }
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

        // Check if feedback nodes need to be added to the node order
        // TODO: Update for new feedback edge system
        // let mut feedback_node_order_addition = vec![];
        // for (_key, feedback_edges) in self.node_feedback_edges.iter() {
        //     for feedback_edge in feedback_edges {
        //         if !visited.contains(&feedback_edge.source) {
        //             // The source of this feedback_edge needs to be added to the
        //             // node order at the end. Check if it's the input to any
        //             // other node and start a depth first search from the last
        //             // node.
        //             let mut last_connected_node_index = feedback_edge.source;
        //             let mut last_connected_not_visited_ni = feedback_edge.source;
        //             loop {
        //                 let mut found_later_node = false;
        //                 for (key, input_edges) in self.node_input_edges.iter() {
        //                     for input_edge in input_edges {
        //                         if input_edge.source == last_connected_node_index {
        //                             last_connected_node_index = key;

        //                             if !visited.contains(&key) {
        //                                 last_connected_not_visited_ni = key;
        //                             }
        //                             found_later_node = true;
        //                             break;
        //                         }
        //                     }
        //                     if found_later_node {
        //                         break;
        //                     }
        //                 }
        //                 if !found_later_node {
        //                     break;
        //                 }
        //             }
        //             // Do a depth first search from `last_connected_node_index`
        //             nodes_to_process.clear();
        //             visited.insert(last_connected_not_visited_ni);
        //             nodes_to_process.push(last_connected_not_visited_ni);
        //             let stack = self.depth_first_search(&mut visited, &mut nodes_to_process);
        //             feedback_node_order_addition.extend(stack);
        //         }
        //     }
        // }
        // self.node_order
        //     .extend(feedback_node_order_addition.into_iter());

        // Add all remaining nodes. These are not currently connected to anything.
        let mut remaining_nodes = vec![];
        for (node_key, _node) in self.get_nodes() {
            if !visited.contains(&node_key) && !self.node_keys_pending_removal.contains(&node_key) {
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
    pub fn ctx(&self) -> AudioCtx {
        AudioCtx::new(self.sample_rate, self.block_size)
    }
    /// Number of input channels going into this graph.
    pub fn inputs(&self) -> usize {
        self.num_inputs
    }
    /// Number of output channels going out from this graph.
    pub fn outputs(&self) -> usize {
        self.num_outputs
    }
    /// Connectable for connecting the Graph to other nodes within its parent graph.
    pub fn as_node(&self) -> Connectable {
        Connectable::from_node(
            NodeSubset {
                node: NodeOrGraph::Node(self.self_node_id),
                channels: self.inputs(),
                start_channel: 0,
            },
            NodeSubset {
                node: NodeOrGraph::Node(self.self_node_id),
                channels: self.outputs(),
                start_channel: 0,
            },
        )
    }
    /// Connectable for connecting inside the graph. Note that inside the graph, the graph outputs
    /// are sinks/inputs and the graph outputs are sources/outputs.
    pub fn as_graph(&self) -> Connectable {
        Connectable::from_node(
            NodeSubset {
                node: NodeOrGraph::Graph,
                channels: self.outputs(),
                start_channel: 0,
            },
            NodeSubset {
                node: NodeOrGraph::Graph,
                channels: self.inputs(),
                start_channel: 0,
            },
        )
    }
}

fn shorten_name(name: &str) -> String {
    let mut short = String::new();
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
    scheduling_event_producer: Arc<Mutex<SchedulingChannelProducer>>,
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
    fn free_old(&mut self) {
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

#[derive(thiserror::Error, Debug)]
pub enum GraphError {
    #[error("Error pushing to a Graph")]
    PushError(#[from] PushError),
    #[error("Error sending new data to GraphGen: `{0}`")]
    SendToGraphGen(String),
    #[error("Node cannot be found in current Graph.")]
    NodeNotFound,
    #[error("An id was given to a node in a different Graph")]
    WrongGraph,
    #[error("Tried to connect a node input that doesn't exist: `{0}`")]
    InputOutOfBounds(usize),
    #[error("Tried to connect to a node output that doesn't exist: `{0}`")]
    OutputOutOfBounds(usize),
    #[error("Tried to connect a graph input that doesn't exist (`{0}`) to some destination")]
    GraphInputOutOfBounds(usize),
    #[error("Tried to connect a graph output that doesn't exist (`{0}`) to some destination")]
    GraphOutputOutOfBounds(usize),
    #[error("The parameter `{0}` is not a valid parameter description for the node")]
    ParameterDescriptionNotFound(String),
    #[error("The parameter `{0}` is not a valid parameter index for the node")]
    ParameterIndexOutOfBounds(usize),
    #[error(transparent)]
    ParameterError(#[from] ParameterError),
    #[error("There was an error sending the change: `{0}`")]
    PushChangeError(String),
}
#[derive(thiserror::Error, Debug)]
pub enum PushError {}

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
impl<F: Float> From<&Graph<F>> for Connectable {
    fn from(h: &Graph<F>) -> Self {
        Connectable::from_node(
            NodeSubset {
                node: NodeOrGraph::Node(h.self_node_id),
                channels: h.inputs(),
                start_channel: 0,
            },
            NodeSubset {
                node: NodeOrGraph::Node(h.self_node_id),
                channels: h.outputs(),
                start_channel: 0,
            },
        )
    }
}
// impl<F: Float> From<&Graph<F>> for Source {
//     fn from(value: &Graph<F>) -> Self {
//         Source::Node(value.self_node_id)
//     }
// }
impl<F: Float> From<&mut Graph<F>> for NodeId {
    fn from(value: &mut Graph<F>) -> Self {
        value.self_node_id
    }
}
