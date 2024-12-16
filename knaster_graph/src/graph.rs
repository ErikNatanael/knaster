use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::{
    buffer_allocator::BufferAllocator,
    connectable::{ChainElement, ChainSinkKind, ConnectionChain},
    core::sync::atomic::AtomicU64,
    dyngen::DynGen,
    edge::{Edge, EdgeKind, FeedbackEdge, InternalGraphEdge, NodeKeyOrGraph, ParameterEdge},
    graph_gen::{self, GraphGen},
    handle::{Handle, UntypedHandle},
    node::Node,
    task::{ArParameterChange, InputToOutputTask, OutputTask, Task, TaskData},
    SchedulingChannelProducer,
};

use knaster_core::{
    math::{Add, MathGen},
    typenum::*,
    AudioCtx, Float, Gen, Parameterable, Size,
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
    /// Node identifier in a specific Graph. For referring to a Node outside of the context of a Graph, use NodeId instead.
    pub struct NodeKey;
}

/// Unique identifier for a specific Node
#[derive(Copy, Clone, Debug)]
pub struct NodeId {
    /// The key is only unique within the specific Graph
    key: NodeKey,
    pub(crate) graph: GraphId,
}
impl NodeId {
    pub fn key(&self) -> NodeKey {
        self.key
    }
}

/// Pass to `Graph::new` to set the options the Graph is created with in an ergonomic and clear way.
#[derive(Clone, Debug)]
pub struct GraphSettings {
    /// The name of the Graph
    pub name: String,
    /// The block size this Graph uses for processing.
    pub block_size: usize,
    /// The sample rate this Graph uses for processing.
    pub sample_rate: u32,
    /// The number of messages that can be sent through any of the ring buffers.
    /// Ring buffers are used pass information back and forth between the audio
    /// thread (GraphGen) and the Graph.
    pub ring_buffer_size: usize,
}

impl GraphSettings {
    /// Set the oversampling to a new value
    pub fn block_size(mut self, block_size: usize) -> Self {
        self.block_size = block_size;
        self
    }
}

impl Default for GraphSettings {
    fn default() -> Self {
        GraphSettings {
            name: String::new(),
            block_size: 64,
            sample_rate: 48000,
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
    new_inputs_buffers_ptr: bool,
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
    /// If a node is a graph, that graph will be added with the same key here.
    graphs_per_node: SecondaryMap<NodeKey, Graph<F>>,
    /// The outputs of the Graph
    output_edges: Box<[Option<Edge>]>,
    /// The edges from the graph inputs to nodes, one Vec per node. `source` in the edge is really the sink here.
    graph_input_edges: SecondaryMap<NodeKey, Vec<Edge>>,
    /// Edges going straight from a graph input to a graph output
    graph_input_to_output_edges: Vec<InternalGraphEdge>,
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
}

impl<F: Float> Graph<F> {
    /// Create a new empty [`Graph`] with a unique atomically generated [`GraphId`]
    pub fn new<Inputs: Size, Outputs: Size>(options: GraphSettings) -> (Self, Node<F>) {
        let GraphSettings {
            name,
            block_size,
            sample_rate,
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
        let graph_input_edges = SecondaryMap::with_capacity(DEFAULT_NUM_NODES);
        let graph_input_to_output_edges = Vec::new();
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
        };
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
            graphs_per_node: SecondaryMap::with_capacity(DEFAULT_NUM_NODES),
            output_edges: vec![None; Outputs::USIZE].into(),
            graph_input_edges,
            num_inputs: Inputs::USIZE,
            num_outputs: Outputs::USIZE,
            block_size,
            sample_rate,
            graph_gen_communicator,
            recalculation_required: false,
            buffers_to_free_when_safe: vec![],
            new_inputs_buffers_ptr: false,
            graph_input_to_output_edges,
            buffer_allocator,
        };
        // graph_gen
        let task_data = graph.generate_task_data(Arc::new(AtomicBool::new(false)));
        let remove_me = Arc::new(AtomicBool::new(false));

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
                waiting_parameter_changes: Vec::with_capacity(ring_buffer_size),
                _arc_nodes: graph.nodes.clone(),
                _arc_buffer_allocation_ptr: graph.buffer_allocator.buffer(),
                _channels: core::marker::PhantomData,
                remove_me_flag: remove_me.clone(),
            },
        );

        graph_gen.remove_me = Some(remove_me);

        (graph, graph_gen)
    }

    /// Push something implementing [`Gen`] or a [`Graph`] to the graph with the
    /// id provided, storing its address in the NodeAddress provided. The node
    /// will start processing at the `start_time`.
    pub fn push<T: Gen<Sample = F> + Parameterable<F> + 'static>(
        &mut self,
        gen: T,
    ) -> Result<Handle<T>, GraphError> {
        let name = std::any::type_name::<T>();
        let node = Node::new(name.to_owned(), gen);
        let node_key = self.push_node(node);
        let handle = Handle::new(UntypedHandle::new(
            NodeId {
                key: node_key,
                graph: self.id,
            },
            self.graph_gen_communicator
                .scheduling_event_producer
                .clone(),
        ));
        Ok(handle)
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
        self.graph_input_edges.insert(key, vec![]);
        self.node_mortality.insert(key, true);

        key
    }

    pub fn connect_nodes(
        &mut self,
        source: impl Into<NodeId>,
        sink: impl Into<NodeId>,
        source_from_channel: usize,
        sink_from_channel: usize,
        channels: usize,
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
        self.connect_to_node_internal(
            NodeKeyOrGraph::Node(source.key()),
            sink.key(),
            source_from_channel,
            sink_from_channel,
            channels,
            additive,
        )
    }
    pub fn connect_node_to_output(
        &mut self,
        source: impl Into<NodeId>,
        source_from_channel: usize,
        sink_from_channel: usize,
        channels: usize,
        additive: bool,
    ) -> Result<(), GraphError> {
        let source = source.into();
        if !source.graph == self.id {
            return Err(GraphError::WrongGraph);
        }
        self.connect_node_to_output_internal(
            source.key(),
            source_from_channel,
            sink_from_channel,
            channels,
            additive,
        )
    }
    pub fn connect_input_to_node(
        &mut self,
        sink: impl Into<NodeId>,
        source_from_channel: usize,
        sink_from_channel: usize,
        channels: usize,
        additive: bool,
    ) -> Result<(), GraphError> {
        let sink = sink.into();
        if !sink.graph == self.id {
            return Err(GraphError::WrongGraph);
        }
        self.connect_to_node_internal(
            NodeKeyOrGraph::Graph,
            sink.key(),
            source_from_channel,
            sink_from_channel,
            channels,
            additive,
        )
    }

    /// Make a connection between two nodes in the Graph when it is certain that
    /// the NodeKeys are from this graph
    fn connect_to_node_internal(
        &mut self,
        source: NodeKeyOrGraph,
        sink: NodeKey,
        so_from: usize,
        si_from: usize,
        channels: usize,
        additive: bool,
    ) -> Result<(), GraphError> {
        let nodes = self.get_nodes();
        if let NodeKeyOrGraph::Node(source) = source {
            if !nodes.contains_key(source) {
                return Err(GraphError::NodeNotFound);
            }
        }
        if !nodes.contains_key(sink) {
            return Err(GraphError::NodeNotFound);
        }

        // Fast and common path
        if !additive {
            for i in 0..channels {
                self.node_input_edges[sink][si_from + i] = Some(Edge {
                    source,
                    kind: EdgeKind::Audio {
                        channel_in_source: so_from + i,
                    },
                });
            }
            return Ok(());
        }
        // Connect additively
        // If no input exists for the channel, connect directly.
        // If an input does exist, create a new add node and connect it up, replacing the current edge.

        for i in 0..channels {
            if let Some(existing_edge) = self.node_input_edges[sink][si_from + i] {
                // Put an add node in between the input and the previous input,
                // adding the new source together with the old
                let add_gen = MathGen::<F, U1, Add>::new();
                // TODO: We don't need a full handle here
                let add_handle = self.push(add_gen)?;
                let add_node = add_handle.untyped_handle.node.key;
                self.node_input_edges[add_node][0] = Some(existing_edge);
                self.node_input_edges[add_node][1] = Some(Edge {
                    source,
                    kind: EdgeKind::Audio {
                        channel_in_source: so_from + i,
                    },
                });
                self.node_input_edges[sink][si_from + i] = Some(Edge {
                    source: NodeKeyOrGraph::Node(add_node),
                    kind: EdgeKind::Audio {
                        channel_in_source: 0,
                    },
                });
            } else {
                self.node_input_edges[sink][si_from + i] = Some(Edge {
                    source,
                    kind: EdgeKind::Audio {
                        channel_in_source: so_from + i,
                    },
                });
            }
        }
        Ok(())
    }
    // TODO: This would be much cleaner if the output was represented by a node
    // in the graph, created in `new`.
    /// The internal function for connecting a node to the output
    fn connect_node_to_output_internal(
        &mut self,
        source: NodeKey,
        so_from: usize,
        si_from: usize,
        channels: usize,
        additive: bool,
    ) -> Result<(), GraphError> {
        let nodes = self.get_nodes();
        if !nodes.contains_key(source) {
            return Err(GraphError::NodeNotFound);
        }

        // Fast and common path
        if !additive {
            for i in 0..channels {
                self.output_edges[si_from + i] = Some(Edge {
                    source,
                    kind: EdgeKind::Audio {
                        channel_in_source: so_from + i,
                    },
                });
            }
            return Ok(());
        }
        // Connect additively
        // If no input exists for the channel, connect directly.
        // If an input does exist, create a new add node and connect it up, replacing the current edge.

        for i in 0..channels {
            if let Some(existing_edge) = self.output_edges[si_from + i] {
                // Put an add node in between the input and the previous input,
                // adding the new source together with the old
                let add_gen = MathGen::<F, U1, Add>::new();
                // TODO: We don't need a full handle here
                let add_handle = self.push(add_gen)?;
                let add_node = add_handle.untyped_handle.node.key;
                self.node_input_edges[add_node][0] = Some(existing_edge);
                self.node_input_edges[add_node][1] = Some(Edge {
                    source,
                    kind: EdgeKind::Audio {
                        channel_in_source: so_from + i,
                    },
                });
                self.output_edges[si_from + i] = Some(Edge {
                    source: add_node,
                    kind: EdgeKind::Audio {
                        channel_in_source: 0,
                    },
                });
                dbg!(self.node_input_edges[add_node][0]);
                dbg!(self.node_input_edges[add_node][1]);
                dbg!(self.output_edges[si_from + i]);
            } else {
                self.output_edges[si_from + i] = Some(Edge {
                    source,
                    kind: EdgeKind::Audio {
                        channel_in_source: so_from + i,
                    },
                });
            }
        }
        Ok(())
    }

    pub fn connect(&mut self, chain: impl Into<ConnectionChain>) -> Result<(), GraphError> {
        let chain = chain.into();
        let mut chains_to_connect = vec![chain];
        while let Some(chain) = chains_to_connect.pop() {
            let additive = chain.additive_connection();
            let (source, sink) = chain.deconstruct();
            if let Some(source_chain) = source {
                let source = source_chain.sink();
                match (&source.kind, sink.kind) {
                    (
                        ChainSinkKind::Node {
                            key: source_node,
                            from_chan: so_from,
                            channels: so_channels,
                        },
                        ChainSinkKind::Node {
                            key: sink_node,
                            from_chan: si_from,
                            channels: si_channels,
                        },
                    ) => {
                        let channels = (*so_channels).min(si_channels);
                        self.connect_to_node_internal(
                            *source_node,
                            sink_node,
                            *so_from,
                            si_from,
                            channels,
                            additive,
                        )?;
                    }
                    (
                        ChainSinkKind::Node {
                            key: source_node,
                            from_chan: so_from,
                            channels: so_channels,
                        },
                        ChainSinkKind::GraphConnection {
                            from_chan: si_from,
                            channels: si_channels,
                        },
                    ) => {
                        let channels = (*so_channels).min(si_channels);
                        self.connect_node_to_output_internal(
                            *source_node,
                            *so_from,
                            si_from,
                            channels,
                            additive,
                        )?;
                    }
                    (
                        ChainSinkKind::GraphConnection {
                            from_chan: so_from,
                            channels: so_channels,
                        },
                        ChainSinkKind::Node {
                            key: sink_node,
                            from_chan: si_from,
                            channels: si_channels,
                        },
                    ) => {
                        let channels = (*so_channels).min(si_channels);

                        // TODO: Incorporate graph input edges into normal input edges
                        todo!();
                        // self.graph_input_edges[sink_node].push(Edge {
                        //     source: sink_node,
                        //     kind: EdgeKind::Audio {
                        //         channels,
                        //         channel_offset_in_sink: si_from,
                        //         channel_offset_in_source: *so_from,
                        //     },
                        // })
                    }
                    _ => eprintln!("Unhandled connection"),
                }
                chains_to_connect.push(*source_chain);
            }
        }
        Ok(())
    }

    pub fn subgraph<Inputs: Size, Outputs: Size>(&mut self, options: GraphSettings) -> Self {
        let (subgraph, graph_gen) = Self::new::<Inputs, Outputs>(options);
        // TODO: Store node key in graph
        let node_key = self.push_node(graph_gen);

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
            if let EdgeKind::Audio { channel_in_source } = output_edge.kind {
                let source = &self.get_nodes()[output_edge.source];
                let source_ptr = source
                    .node_output_ptr()
                    .expect("Node output should be ptr at this point");
                assert!(channel_in_source < source.outputs);
                output_task.channels[sink_channel] =
                    Some(unsafe { source_ptr.add(block_size * (channel_in_source)) });
            }
        }
        output_task
    }
    fn generate_input_to_output_tasks(&mut self) -> Vec<InputToOutputTask> {
        let mut output_tasks = vec![];
        for output_edge in &self.graph_input_to_output_edges {
            output_tasks.push(InputToOutputTask {
                graph_input_index: output_edge.from_output_index,
                graph_output_index: output_edge.to_input_index,
            });
        }
        output_tasks
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
                        let buffer = unsafe {
                            buffer.offset((channel_in_source * self.block_size) as isize)
                        };
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
    fn generate_task_data(&mut self, applied_flag: Arc<AtomicBool>) -> TaskData<F> {
        let tasks = self.generate_tasks().into_boxed_slice();
        let output_task = self.generate_output_tasks();
        let nodes = self.get_nodes();
        let gens: Vec<_> = self
            .node_order
            .iter()
            .map(|key| (*key, nodes[*key].gen))
            .collect();
        let ar_parameter_changes = self.generate_ar_parameter_changes();
        TaskData {
            applied: applied_flag,
            tasks,
            output_task,
            current_buffer_allocation: Some(self.buffer_allocator.buffer()),
            input_to_output_tasks: self.generate_input_to_output_tasks().into_boxed_slice(),
            ar_parameter_changes,
            gens,
        }
    }
    /// Assign buffers to nodes maximizing buffer reuse and cache locality
    /// (ideally, there are surely optimisations left)
    fn allocate_node_buffers(&mut self) {
        // Recalculate the number of dependent channels of a node
        // TODO: This makes a lot of node lookups. Optimise?
        for (_key, node) in self.get_nodes_mut() {
            node.num_output_dependents = 0;
        }
        for (_key, edges) in &self.node_input_edges {
            for edge in edges.iter().filter_map(|e| *e) {
                // Safety:
                //
                // Accessing self.nodes is always safe because the
                // Arc owned by the GraphGen will never touch it, it just
                // guarantees that the allocation stays valid.
                (unsafe { &mut *self.nodes.get() })[edge.source].num_output_dependents += 1;
            }
        }

        // Assign buffers
        self.buffer_allocator.reset(self.block_size);
        // TODO: Iterate by index instead
        let node_order = self.node_order.clone();
        for &key in &node_order {
            let outputs = self.get_nodes()[key].outputs;
            let num_borrows = self.get_nodes()[key].num_output_dependents;
            let offset = self
                .buffer_allocator
                .get_block(outputs, self.block_size, num_borrows);
            self.get_nodes_mut()[key].assign_output_offset(offset);
            // Return every block that is used as an input
            for edge in self.node_input_edges[key].iter().filter_map(|e| *e) {
                let block = self.get_nodes()[edge.source].node_output;
                if let crate::node::NodeOutput::Offset(block) = block {
                    self.buffer_allocator.return_block(block);
                }
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
                if let EdgeKind::Audio { channel_in_source } = edge.kind {
                    let source_output_ptr = self.get_nodes()[edge.source]
                        .node_output_ptr()
                        .expect("real buffer was just assigned");
                    inputs[sink_channel] =
                        unsafe { source_output_ptr.add(channel_in_source * self.block_size) };
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
    }

    /// Applies the latest changes to connections and added nodes in the graph on the audio thread and updates the scheduler.
    pub fn commit_changes(&mut self) -> Result<(), GraphError> {
        // We need to run free_old to know if there are nodes to free and hence a recalculation required.
        self.free_old();
        if self.recalculation_required {
            self.calculate_node_order();
            self.allocate_node_buffers();

            let ggc = &mut self.graph_gen_communicator;
            let current_change_flag = crate::core::mem::replace(
                &mut ggc.next_change_flag,
                Arc::new(AtomicBool::new(false)),
            );
            let task_data = self.generate_task_data(current_change_flag);
            for t in &task_data.tasks {
                dbg!(&t.in_buffers);
                dbg!(t.out_buffer);
            }
            self.graph_gen_communicator.send_updated_tasks(task_data)?;
            self.recalculation_required = false;
        }
        dbg!(&self.node_order);
        for node in &self.node_order {
            let n = self.get_nodes().get(*node).unwrap();
            dbg!(n.node_output_ptr());
        }
        dbg!(&self.output_edges);
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
        self.graph_input_edges.remove(node_key);
        // feedback from the freed node requires removing the feedback node and all edges from the feedback node
        self.node_parameter_edges.remove(node_key);
        // Remove all edges leading from the node to other nodes
        for (_k, input_edges) in &mut self.node_input_edges {
            let mut i = 0;
            while i < input_edges.len() {
                if let Some(edge) = input_edges[i] {
                    if edge.source == node_key {
                        input_edges[i] = None;
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
                    if edge.source == node_key {
                        self.output_edges[i] = None;
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
        // The GraphGen has been created so we have to be more careful
        self.node_keys_to_free_when_safe
            .push((node_key, ggc.next_change_flag.clone()));
        self.node_keys_pending_removal.insert(node_key);
        Ok(())
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
        // See if the GraphGen has reported any nodes that should be freed
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
        // Remove old nodes
        let nodes = unsafe { &mut *self.nodes.get() };
        let mut i = 0;
        while i < self.node_keys_to_free_when_safe.len() {
            let (key, flag) = &self.node_keys_to_free_when_safe[i];
            if flag.load(Ordering::SeqCst) {
                nodes.remove(*key);
                // If the node was a graph, free the graph as well (it will be returned and  dropped here)
                // The Graph should be dropped after the GraphGen Node.
                self.graphs_per_node.remove(*key);
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

    /// Goes through all of the nodes that are connected to nodes in `nodes_to_process` and adds them to the list in
    /// reverse depth first order.
    ///
    fn depth_first_search(
        &self,
        visited: &mut HashSet<NodeKey>,
        nodes_to_process: &mut Vec<NodeKey>,
    ) -> Vec<NodeKey> {
        let mut node_order = Vec::with_capacity(self.get_nodes().capacity());
        while !nodes_to_process.is_empty() {
            let node_index = *nodes_to_process.last().unwrap();

            let input_edges = &self.node_input_edges[node_index];
            let mut found_unvisited = false;
            // There is probably room for optimisation here by managing to
            // not iterate the edges multiple times.
            for edge in input_edges.iter() {
                if let Some(edge) = edge {
                    if !visited.contains(&edge.source) {
                        nodes_to_process.push(edge.source);
                        visited.insert(edge.source);
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
                    if input_edge.source == last_connected_node_index
                        && !visited.contains(&input_edge.source)
                    {
                        last_connected_node_index = key;
                        found_later_node = true;

                        // check if it's an output node
                        for edge in self.output_edges.iter().filter_map(|e| *e) {
                            if last_connected_node_index == edge.source {
                                last_connected_output_node_index = last_connected_node_index;
                            }
                        }
                        break;
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
            // The same source node may be present in multiple output edges e.g.
            // for stereo so we need to check if visited. One output may also
            // depend on another. Therefore we need to make sure to start with
            // the deepest output nodes only.
            let deepest_node = self.get_deepest_output_node(edge.source, &visited);
            if !visited.contains(&deepest_node) {
                nodes_to_process.push(deepest_node);
                visited.insert(deepest_node);
            }
        }

        let stack = self.depth_first_search(&mut visited, &mut nodes_to_process);
        self.node_order.extend(stack.into_iter());

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
    pub fn output(&self) -> ChainElement {
        ChainElement {
            kind: ChainSinkKind::GraphConnection {
                from_chan: 0,
                channels: self.num_outputs,
            },
            inputs: self.num_inputs,
            outputs: self.num_outputs,
        }
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
}

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
        //
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
        if let Err(e) = self.new_task_data_producer.push(task_data) {
            Err(GraphError::SendToGraphGen(format!("{e}")))
        } else {
            Ok(())
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
    #[error("Tried to connect a graph input that doesn't exist: `{0}`")]
    InputOutOfBounds(usize),
    #[error("Tried to connect to a graph output that doesn't exist: `{0}`")]
    OutputOutOfBounds(usize),
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
