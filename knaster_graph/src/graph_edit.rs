//! Graph connection API using compile time type safety, providing a more ergonomic and less error
//! prone interface for hand written code.
//!
//! Since this interface requires types to be known as compile time, it is not as good for use
//! cases where the Graph is liberally changed at runtime.
//!
//! This is a second iteration
//!
//! This API will bypass the use of individual types handles for scheduling parameter changes. Instead, a special Synth interface
//! will be available which allows access to parameter changes in all nodes in a Graph.
//!
//!
//! - [ ] Synth interface
//! - [X] Finish implementing the concrete methods
//! - [X] Implement remaining arithmetics
//! - [ ] API for scheduling parameter changes

use crate::core::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use core::mem::MaybeUninit;
use core::ops::{BitOr, Div, Shr, Sub};

use crate::Time;
use crate::core::{
    clone::Clone,
    marker::PhantomData,
    ops::{Add, Mul},
};
use crate::graph::{Channels, GraphError, GraphOptions, NodeOrGraph};
use crate::graph_gen::GraphGen;
use crate::handle::SchedulingChannelSender;
use crate::node::NodeData;
use crate::wrappers_graph::done::WrDone;

use ecow::EcoString;
use knaster_core::{Done, ParameterSmoothing, ParameterValue, Seconds};
use knaster_core::{Float, Param, Size, UGen, numeric_array::NumericArray, typenum::*};
use knaster_core_dsp::math::MathUGen;
use knaster_core_dsp::util::Constant;
use smallvec::SmallVec;

use crate::{
    graph::{Graph, NodeId},
    handle::HandleTrait,
};

/// A reference to a graph, used to access the graph from a [`GraphEdit`] or an [`SH`] or [`DH`] handle.
pub struct GraphRef<'a, F: Float>(RwLock<&'a mut Graph<F>>);
impl<'a, F: Float> GraphRef<'a, F> {
    fn new(g: &'a mut Graph<F>) -> Self {
        Self(RwLock::new(g))
    }
    fn read(&self) -> RwLockReadGuard<'_, &'a mut Graph<F>> {
        #[cfg(feature = "std")]
        {
            self.0.read().unwrap()
        }
        #[cfg(not(feature = "std"))]
        {
            self.0.read()
        }
    }
    fn write(&self) -> RwLockWriteGuard<'_, &'a mut Graph<F>> {
        #[cfg(feature = "std")]
        {
            self.0.write().unwrap()
        }
        #[cfg(not(feature = "std"))]
        {
            self.0.write()
        }
    }
}

/// A wrapper around a [`Graph`] that provides access to an ergonomic and interface for adding and
/// connecting nodes in the graph. When the `GraphEdit` is dropped, the changes are committed to the
/// graph.
pub struct GraphEdit<'b, F: Float> {
    graph: GraphRef<'b, F>,
}
impl<'b, F: Float> GraphEdit<'b, F> {
    /// Use [`Graph::edit`] instead. Creates a new `GraphEdit` wrapper around a `&mut Graph`.
    pub fn new(g: &'b mut Graph<F>) -> Self {
        Self {
            graph: GraphRef::new(g),
        }
    }
    /// Create a new node in the graph and return a handle to it.
    pub fn push<'a, T: UGen<Sample = F> + 'static>(&'a self, ugen: T) -> SH<'a, 'b, F, Handle3<T>> {
        let handle = self.graph.write().push_internal(ugen);
        let node_id = handle.node_id();
        SH {
            nodes: Handle3 {
                node_id,
                ugen: PhantomData,
            },
            graph: &self.graph,
        }
    }

    /// Push something implementing [`UGen`] to the graph, adding the [`WrDone`] wrapper. This
    /// enables the node to free itself if it marks itself as done or for removal using [`GenFlags`].
    pub fn push_with_done_action<'a, T: UGen<Sample = F> + 'static>(
        &'a self,
        ugen: T,
        default_done_action: Done,
    ) -> SH<'a, 'b, F, Handle3<WrDone<T>>>
    where
        // Make sure we can add a parameter
        <T as UGen>::Parameters: crate::core::ops::Add<B1>,
        <<T as UGen>::Parameters as crate::core::ops::Add<B1>>::Output: Size,
    {
        let handle = self
            .graph
            .write()
            .push_with_done_action(ugen, default_done_action);
        let node_id = handle.node_id();
        SH {
            nodes: Handle3 {
                node_id,
                ugen: PhantomData,
            },
            graph: &self.graph,
        }
    }
    /// Get a non typesafe handle to node with the given [`NodeId`] if it exists.
    pub fn handle<'a>(&'a self, id: impl Into<NodeId>) -> Option<DH<'a, 'b, F, DynamicHandle3>> {
        let id = id.into();
        self.graph.read().node_data(id).map(|data| DH {
            nodes: DynamicHandle3 { node_id: id, data },
            graph: &self.graph,
        })
    }
    /// Get a non typesafe handle to node with the given name if it exists.
    pub fn handle_from_name<'a>(
        &'a self,
        name: impl Into<EcoString>,
    ) -> Option<DH<'a, 'b, F, DynamicHandle3>> {
        self.graph
            .read()
            .node_data_from_name(name)
            .map(|(id, data)| DH {
                nodes: DynamicHandle3 { node_id: id, data },
                graph: &self.graph,
            })
    }

    /// Set a parameter value on a node. Note that this operation is sent to the audio thread
    /// instantly, unlike changes to the graph structure which take effect when the [`GraphEdit`] is dropped.
    pub fn set(
        &self,
        node: impl Into<NodeId>,
        param: impl Into<Param>,
        value: impl Into<ParameterValue>,
        t: Time,
    ) -> Result<(), GraphError> {
        self.graph.read().set(node, param, value, t)?;
        Ok(())
    }
    /// Free a node from the graph. This will remove the node and any of its dependent nodes from the graph.
    pub fn free_node(&self, node: impl Into<NodeId>) -> Result<(), GraphError> {
        let node = node.into();
        let graph_id = self.graph.read().graph_id();
        if !node.graph == graph_id {
            return Err(GraphError::WrongSinkNodeGraph {
                expected_graph: graph_id,
                found_graph: node.graph,
            });
        }
        self.graph.write().free_node_from_key(node.key())?;
        Ok(())
    }
    /// Create a new handle to the graph input(s).
    ///
    /// # Example
    /// ```rust,
    /// # use knaster_graph::{processor::AudioProcessor, osc::SinWt, processor::AudioProcessorOptions, typenum::*};
    /// # let (mut graph, mut audio_processor, _log_receiver) = AudioProcessor::new::<U1, U1>(AudioProcessorOptions{
    /// #     block_size: 16,
    /// #     sample_rate: 48000,
    /// #     ring_buffer_size: 50,
    /// #     ..Default::default()
    /// # });
    /// graph.edit(|graph| {
    ///     let sine = graph.push(SinWt::new(200.));
    ///     let input = graph.from_inputs(0).unwrap();
    ///     (sine * input).to_graph_out();
    /// });
    /// ```
    pub fn from_inputs<'a, N: Size + Copy>(
        &'a self,
        source_channels: impl Into<Channels<N>>,
    ) -> Result<SH<'a, 'b, F, ChannelsHandle<N>>, GraphError> {
        let mut channels = NumericArray::default();
        let num_inputs = self.graph.read().inputs();
        for (i, c) in source_channels.into().into_iter().enumerate() {
            if c >= num_inputs {
                return Err(GraphError::GraphInputOutOfBounds(c));
            }
            channels[i] = (NodeOrGraph::Graph, c);
        }
        Ok(SH {
            nodes: ChannelsHandle { channels },
            graph: &self.graph,
        })
    }

    /// Create a subgraph as a new node in this graph. `init_callback` is called with a [`GraphEdit`] to edit the subgraph before it is initialized and sent to the audio thread.
    #[allow(clippy::type_complexity)]
    pub fn subgraph<'a, Inputs: Size, Outputs: Size>(
        &'a self,
        options: GraphOptions,
        init_callback: impl FnOnce(GraphEdit<F>),
    ) -> (
        SH<'a, 'b, F, Handle3<GraphGen<F, Inputs, Outputs>>>,
        Graph<F>,
    ) {
        let mut g = self.graph.write();
        let subgraph = g.subgraph_init::<Inputs, Outputs>(options, init_callback);
        (
            SH {
                nodes: Handle3 {
                    node_id: subgraph.id(),
                    ugen: PhantomData,
                },
                graph: &self.graph,
            },
            subgraph,
        )
    }
    // pub fn smooth(
    //     &mut self,
    //     s: impl Into<ParameterSmoothing>,
    //     t: SchedulingTime,
    // ) -> Result<(), GraphError> {
    //     let s = s.into();
    //     self.sender.send(crate::SchedulingEvent {
    //         node_key: self.node.key(),
    //         parameter: self.param_index as usize,
    //         value: None,
    //         smoothing: Some(s),
    //         token: None,
    //         time: Some(t),
    //     })?;
    //     Ok(())
    // }
    // pub fn trig(&mut self, t: SchedulingTime) -> Result<(), GraphError> {
    //     self.sender.send(crate::SchedulingEvent {
    //         node_key: self.node.key(),
    //         parameter: self.param_index as usize,
    //         value: Some(ParameterValue::Trigger),
    //         smoothing: None,
    //         token: None,
    //         time: Some(t),
    //     })?;
    //     Ok(())
    // }
}
impl<F: Float> Drop for GraphEdit<'_, F> {
    fn drop(&mut self) {
        self.graph.write().commit_changes().unwrap();
    }
}

/// Static Handle. Wrapper around static sources/sinks.
#[derive(Clone, Copy)]
pub struct SH<'a, 'b, F: Float, T> {
    nodes: T,
    // graph: &'a RwLock<&'b mut Graph<F>>,
    graph: &'a GraphRef<'b, F>,
}
/// Dynamic Handle. Wrapper around dynamic source/sinks
#[derive(Clone, Copy)]
pub struct DH<'a, 'b, F: Float, T> {
    nodes: T,
    // graph: &'a RwLock<&'b mut Graph<F>>,
    graph: &'a GraphRef<'b, F>,
}
impl<'a, 'b, F: Float, S0: Static> SH<'a, 'b, F, S0> {
    /// Create a new handle to certain outputs from this handle.
    pub fn out<N: Size + Copy>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> SH<'a, 'b, F, ChannelsHandle<N>> {
        let mut channels = NumericArray::default();
        for (i, c) in source_channels.into().into_iter().enumerate() {
            channels[i] = self.nodes.iter_outputs().nth(c as usize).unwrap();
        }
        SH {
            nodes: ChannelsHandle { channels },
            graph: self.graph,
        }
    }
    /// Connect the output(s) of self to the input(s) of another node or nodes, summing the output
    /// of self with any existing connections.
    pub fn to<S1: Static>(self, n: SH<'a, 'b, F, S1>) -> SH<'a, 'b, F, S1>
    where
        S1::Inputs: Same<S0::Outputs>,
    {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            Static::iter_outputs(&self.nodes).zip(Static::iter_inputs(&n.nodes))
        {
            if let Err(e) = g.connect2(source, source_channel, sink_channel, sink) {
                log::error!(
                    "Failed to connect {source:?}:{source_channel} to {sink:?}:{sink_channel}: {e}"
                );
            }
        }
        n
    }
    /// Connect the output(s) of self to the input(s) of another node or nodes via a feedback edge, summing the output
    /// of self with any existing connections.
    pub fn to_feedback<S1: Static>(self, n: SH<'a, 'b, F, S1>) -> SH<'a, 'b, F, S1>
    where
        S1::Inputs: Same<S0::Outputs>,
    {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            Static::iter_outputs(&self.nodes).zip(Static::iter_inputs(&n.nodes))
        {
            if let Err(e) = g.connect2_feedback(source, source_channel, sink_channel, sink) {
                log::error!(
                    "Failed to connect {source:?}:{source_channel} to {sink:?}:{sink_channel}: {e}"
                );
            }
        }
        n
    }

    /// Connect the output(s) of self to the input(s) of another node or nodes, replacing any
    /// existing connections.
    pub fn to_replace<S1: Static>(self, n: SH<'a, 'b, F, S1>) -> SH<'a, 'b, F, S1>
    where
        S1::Inputs: Same<S0::Outputs>,
    {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            Static::iter_outputs(&self.nodes).zip(Static::iter_inputs(&n.nodes))
        {
            g.connect2_replace(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        n
    }

    /// Connect the output(s) of self to the input(s) of another node or nodes, replacing any
    /// existing connections.
    pub fn to_feedback_replace<S1: Static>(self, n: SH<'a, 'b, F, S1>) -> SH<'a, 'b, F, S1>
    where
        S1::Inputs: Same<S0::Outputs>,
    {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            Static::iter_outputs(&self.nodes).zip(Static::iter_inputs(&n.nodes))
        {
            g.connect2_feedback_replace(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        n
    }

    /// Connect the output(s) of self to the graph output(s).
    pub fn to_graph_out(self) {
        let mut g = self.graph.write();
        for (i, (source, source_channel)) in Static::iter_outputs(&self.nodes).enumerate() {
            g.connect2(source, source_channel, i as u16, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel");
        }
    }
    /// Connect the output(s) of self to the graph output(s), replacing any existing connections at
    /// the sink.
    pub fn to_graph_out_replace(self) {
        let mut g = self.graph.write();
        for (i, (source, source_channel)) in Static::iter_outputs(&self.nodes).enumerate() {
            g.connect2_replace(source, source_channel, i as u16, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel");
        }
    }

    /// Connect the output(s) of self to the graph output(s), selecting graph output channels from the channels provided.
    pub fn to_graph_out_channels<N>(self, sink_channels: impl Into<Channels<N>>)
    where
        N: Size + Same<S0::Outputs>,
    {
        let mut g = self.graph.write();
        for ((source, source_channel), sink_channel) in
            Static::iter_outputs(&self.nodes).zip(sink_channels.into())
        {
            g.connect2(source, source_channel, sink_channel, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }
    /// Connect the output(s) of self to the graph output(s), selecting graph output channels from the channels provided, and replacing any existing connections at those graph output channels.
    pub fn to_graph_out_channels_replace<N>(self, sink_channels: impl Into<Channels<N>>)
    where
        N: Size + Same<S0::Outputs>,
    {
        let mut g = self.graph.write();
        for ((source, source_channel), sink_channel) in
            Static::iter_outputs(&self.nodes).zip(sink_channels.into())
        {
            g.connect2_replace(source, source_channel, sink_channel, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }
    /// Disconnect all outputs from the specified channel.
    pub fn disconnect_output(&self, source_channel: u16) {
        let mut g = self.graph.write();
        let source = self
            .nodes
            .iter_outputs()
            .nth(source_channel as usize)
            .expect("Output channel to disconnect from does not exist.");
        g.disconnect_output_from_source(source.0, source.1)
            .expect("Error disconnecting from output channel.");
    }
    /// Disconnect any input from the specified channel.
    pub fn disconnect_input(&self, sink_channel: u16) {
        let mut g = self.graph.write();
        let sink = self
            .nodes
            .iter_inputs()
            .nth(sink_channel as usize)
            .expect("Input channel to disconnect does not exist.");
        g.disconnect_input_to_sink(sink.1, sink.0)
            .expect("Error disconnecting input channel.");
    }
    /// Connect this handle to another handle, returning a [`Stack`] which can be used to connect
    /// to other handles.
    ///
    /// This is useful for connecting multiple outputs of a single node to multiple nodes or vice
    /// versa.
    pub fn stack<S1: Static>(self, s: SH<'a, 'b, F, S1>) -> SH<'a, 'b, F, Stack<S0, S1>> {
        SH {
            nodes: Stack {
                s0: self.nodes,
                s1: s.nodes,
            },
            graph: self.graph,
        }
    }
    /// Turn this static handle [`SH`] into a dynamic handle.
    pub fn dynamic(self) -> DH<'a, 'b, F, S0::DynamicType> {
        DH {
            nodes: self.nodes.dynamic(self.graph),
            graph: self.graph,
        }
    }
    /// Perform a power operation on each channel pair, i.e. `self[0].pow(rhs[0])` etc.
    // Arithmetic operations that lack an operator overload.
    pub fn pow<S1: Static>(
        self,
        rhs: SH<'a, 'b, F, S1>,
    ) -> SH<'a, 'b, F, ChannelsHandle<S1::Outputs>>
    where
        S0::Outputs: Same<S1::Outputs>,
    {
        let nodes = pow_sources(self.nodes, rhs.nodes, self.graph);
        SH {
            nodes,
            graph: self.graph,
        }
    }
}
impl<'a, 'b, F: Float, D: Dynamic> DH<'a, 'b, F, D> {
    /// Create a new handle to specific output channels from this handle.
    pub fn out<N: Size>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> DH<'a, 'b, F, DynamicChannelsHandle> {
        let mut channels = SmallVec::with_capacity(N::USIZE);
        for c in source_channels.into() {
            channels.push(self.nodes.iter_outputs().nth(c as usize).unwrap());
        }
        DH {
            nodes: DynamicChannelsHandle {
                in_channels: SmallVec::new(),
                out_channels: channels,
            },
            graph: self.graph,
        }
    }
    /// Connect self to another node or nodes.
    ///
    /// If there is an error connecting the nodes, that error is logged and subsequent connections
    /// are made. If you want to handle
    /// the error, use the `try_to` method instead.
    pub fn to<S: Dynamic>(self, n: DH<'a, 'b, F, S>) -> DH<'a, 'b, F, S> {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            self.nodes.iter_outputs().zip(n.nodes.iter_inputs())
        {
            if let Err(e) = g.connect2(source, source_channel, sink_channel, sink) {
                log::error!("Failed to connect nodes: {e}");
            }
        }
        n
    }
    /// Connect self to another node or nodes via a feedback edge.
    ///
    /// If there is an error connecting the nodes, that error is logged and subsequent connections
    /// are made. If you want to handle
    /// the error, use the `try_to` method instead.
    pub fn to_feedback<S: Dynamic>(self, n: DH<'a, 'b, F, S>) -> DH<'a, 'b, F, S> {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            self.nodes.iter_outputs().zip(n.nodes.iter_inputs())
        {
            if let Err(e) = g.connect2_feedback(source, source_channel, sink_channel, sink) {
                log::error!("Failed to connect nodes: {e}");
            }
        }
        n
    }
    /// Connect self to another node or nodes, replacing any existing connections.
    ///
    /// If there is an error connecting the nodes, that error is logged and subsequent connections
    /// are made. If you want to handle
    /// the error, use the `try_to` method instead.
    pub fn to_replace<S: Dynamic>(self, n: DH<'a, 'b, F, S>) -> DH<'a, 'b, F, S> {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            self.nodes.iter_outputs().zip(n.nodes.iter_inputs())
        {
            if let Err(e) = g.connect2_replace(source, source_channel, sink_channel, sink) {
                log::error!("Failed to connect nodes: {e}");
            }
        }
        n
    }
    /// Connect self to another node or nodes via a feedback edge, replacing any existing connections.
    ///
    /// If there is an error connecting the nodes, that error is logged and subsequent connections
    /// are made. If you want to handle
    /// the error, use the `try_to` method instead.
    pub fn to_feedback_replace<S: Dynamic>(self, n: DH<'a, 'b, F, S>) -> DH<'a, 'b, F, S> {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            self.nodes.iter_outputs().zip(n.nodes.iter_inputs())
        {
            if let Err(e) = g.connect2_feedback_replace(source, source_channel, sink_channel, sink)
            {
                log::error!("Failed to connect nodes: {e}");
            }
        }
        n
    }
    /// Connect self to another node or nodes.
    ///
    /// If there is an error connecting the nodes, that error is returned and the connecting is
    /// interrupted. Any subsequent channels are not connected.
    pub fn try_to<S: Dynamic>(self, n: DH<'a, 'b, F, S>) -> Result<DH<'a, 'b, F, S>, GraphError> {
        let mut g = self.graph.write();
        for ((source, source_channel), (sink, sink_channel)) in
            self.nodes.iter_outputs().zip(n.nodes.iter_inputs())
        {
            g.connect2(source, source_channel, sink_channel, sink)?;
        }
        Ok(n)
    }
    /// Connect to the graph output(s) in the order the channels are produced.
    ///
    /// Any errors are logged and subsequent connections are made.
    pub fn to_graph_out(self) {
        let mut g = self.graph.write();
        for (i, (source, source_channel)) in self.nodes.iter_outputs().enumerate() {
            if let Err(e) = g.connect2(source, source_channel, i as u16, NodeOrGraph::Graph) {
                log::error!("Failed to connect node to graph output: {e}");
            }
        }
    }
    /// Connect to the graph output(s) in the order the channels are produced, replacing any existing connections.
    ///
    /// Any errors are logged and subsequent connections are made.
    pub fn to_graph_out_replace(self) {
        let mut g = self.graph.write();
        for (i, (source, source_channel)) in self.nodes.iter_outputs().enumerate() {
            if let Err(e) = g.connect2_replace(source, source_channel, i as u16, NodeOrGraph::Graph)
            {
                log::error!("Failed to connect node to graph output: {e}");
            }
        }
    }

    /// Connect to the graph output(s), selecting graph output channels from the channels provided.
    ///
    /// Any errors are logged and subsequent connections are made.
    pub fn to_graph_out_channels<N: Size>(self, sink_channels: impl Into<Channels<N>>) {
        let mut g = self.graph.write();
        for ((source, source_channel), sink_channel) in
            self.nodes.iter_outputs().zip(sink_channels.into())
        {
            if let Err(e) = g.connect2(source, source_channel, sink_channel, NodeOrGraph::Graph) {
                log::error!("Failed to connect node to graph output: {e}");
            }
        }
    }
    /// Connect to the graph output(s), selecting graph output channels from the channels provided,
    /// replacing any existing connections.
    ///
    /// Any errors are logged and subsequent connections are made.
    pub fn to_graph_out_channels_replace<N: Size>(self, sink_channels: impl Into<Channels<N>>) {
        let mut g = self.graph.write();
        for ((source, source_channel), sink_channel) in
            self.nodes.iter_outputs().zip(sink_channels.into())
        {
            if let Err(e) =
                g.connect2_replace(source, source_channel, sink_channel, NodeOrGraph::Graph)
            {
                log::error!("Failed to connect node to graph output: {e}");
            }
        }
    }
    /// Disconnect all outputs from the specified channel.
    pub fn disconnect_output(&self, source_channel: u16) {
        let mut g = self.graph.write();
        let source = self
            .nodes
            .iter_outputs()
            .nth(source_channel as usize)
            .expect("Output channel to disconnect from does not exist.");
        g.disconnect_output_from_source(source.0, source.1)
            .expect("Error disconnecting from output channel.");
    }
    /// Disconnect any input from the specified channel.
    pub fn disconnect_input(&self, sink_channel: u16) {
        let mut g = self.graph.write();
        let sink = self
            .nodes
            .iter_inputs()
            .nth(sink_channel as usize)
            .expect("Input channel to disconnect does not exist.");
        g.disconnect_input_to_sink(sink.1, sink.0)
            .expect("Error disconnecting input channel.");
    }
    /// Connect this handle to another handle, returning a [`Stack`] which can be used to connect
    /// to other handles.
    ///
    /// This is useful for connecting multiple outputs of a single node to multiple nodes or vice
    /// versa.
    pub fn stack<S: Dynamic>(self, s: DH<'a, 'b, F, S>) -> DH<'a, 'b, F, DynamicChannelsHandle> {
        let mut in_channels =
            SmallVec::with_capacity((self.nodes.inputs() + s.nodes.inputs()) as usize);
        let mut out_channels =
            SmallVec::with_capacity((self.nodes.outputs() + s.nodes.outputs()) as usize);
        for chan in Dynamic::iter_inputs(&self.nodes) {
            in_channels.push(chan);
        }
        for chan in Dynamic::iter_inputs(&s.nodes) {
            in_channels.push(chan);
        }
        for chan in Dynamic::iter_outputs(&self.nodes) {
            out_channels.push(chan);
        }
        for chan in Dynamic::iter_outputs(&s.nodes) {
            out_channels.push(chan);
        }
        DH {
            nodes: DynamicChannelsHandle {
                in_channels,
                out_channels,
            },
            graph: self.graph,
        }
    }
    /// Get a dynamic handle to this node. Provided for symmetry with [`SH`].
    pub fn dynamic(self) -> Self {
        self
    }
    /// Perform a power operation on each channel pair, i.e. `self[0].pow(rhs[0])` etc.
    pub fn pow<S1: Dynamic>(self, rhs: DH<'a, 'b, F, S1>) -> DH<'a, 'b, F, DynamicChannelsHandle> {
        assert_eq!(self.nodes.outputs(), rhs.nodes.outputs());
        let nodes = pow_sources_dynamic(self.nodes, rhs.nodes, self.graph);
        DH {
            nodes,
            graph: self.graph,
        }
    }
}

/// Handle to a node with a lifetime connected to Graph3
#[derive(Clone, Copy)]
pub struct DynamicHandle3 {
    node_id: NodeId,
    data: NodeData,
}
impl Dynamic for DynamicHandle3 {
    fn iter_outputs(&self) -> DynamicChannelIter {
        let mut channels = SmallVec::with_capacity(self.outputs() as usize);
        for i in 0..self.outputs() {
            channels.push((NodeOrGraph::Node(self.node_id), i));
        }
        DynamicChannelIter {
            channels,
            current_index: 0,
        }
    }

    fn outputs(&self) -> u16 {
        self.data.outputs
    }
    fn iter_inputs(&self) -> DynamicChannelIter {
        let mut channels = SmallVec::with_capacity(self.inputs() as usize);
        for i in 0..self.inputs() {
            channels.push((NodeOrGraph::Node(self.node_id), i));
        }
        DynamicChannelIter {
            channels,
            current_index: 0,
        }
    }

    fn inputs(&self) -> u16 {
        self.data.inputs
    }
}

/// Handle to a node with a lifetime connected to Graph3
pub struct Handle3<U: UGen> {
    node_id: NodeId,
    ugen: PhantomData<U>,
}
impl<'a, 'b, F: Float, U: UGen<Sample = F>> From<SH<'a, 'b, F, Handle3<U>>> for NodeId {
    fn from(value: SH<'a, 'b, F, Handle3<U>>) -> Self {
        value.nodes.node_id
    }
}
impl<'a, 'b, F: Float, U: UGen<Sample = F>> SH<'a, 'b, F, Handle3<U>> {
    /// Change the name of the node in the [`Graph`].
    pub fn name(self, n: impl Into<EcoString>) -> Self {
        self.graph.write().set_name(self.nodes.node_id, n.into());
        self
    }
    /// Link the parameter to a node output
    pub fn link<S: Static<Outputs = U1>>(
        self,
        p: impl Into<Param>,
        source: SH<'a, 'b, F, S>,
    ) -> Self {
        let input = source.nodes.iter_outputs().next().unwrap();
        let mut g = self.graph.write();
        if let NodeOrGraph::Node(source_node) = input.0 {
            if let Err(e) =
                g.connect_replace_to_parameter(source_node, input.1, p, self.nodes.node_id)
            {
                log::error!("Failed to connect signal to parameter: {e}");
            }
        } else {
            log::error!(
                "Graph input provided as input to a parameter. This is not currently supported. Connection ignored."
            );
        }
        self
    }

    /// Returns the [`NodeId`] of the node this handle points to.
    pub fn id(self) -> NodeId {
        self.nodes.node_id
    }
    /// Get a parameter from the node this handle points to if it exists. Panics if the parameter doesn't exist.
    pub fn param(self, p: impl Into<Param>) -> Parameter {
        let p = p.into();
        match self.try_param(p) {
            Some(param) => param,
            None => panic!("Parameter {:?} doesn't exist on node {:?}", p, self.id()),
        }
    }
    /// Get a parameter from the node this handle points to if it exists.
    pub fn try_param(self, p: impl Into<Param>) -> Option<Parameter> {
        let p = p.into();
        match p {
            Param::Index(i) => {
                if i < U::Parameters::USIZE {
                    Some(Parameter {
                        node: self.nodes.node_id,
                        param_index: i as u16,
                        sender: self.graph.read().scheduling_channel_sender(),
                    })
                } else {
                    None
                }
            }
            Param::Desc(s) => {
                for (i, desc) in U::param_descriptions().into_iter().enumerate() {
                    if s == desc {
                        return Some(Parameter {
                            node: self.nodes.node_id,
                            param_index: i as u16,
                            sender: self.graph.read().scheduling_channel_sender(),
                        });
                    }
                }
                None
            }
        }
    }
}
impl<'a, 'b, F: Float> From<DH<'a, 'b, F, DynamicHandle3>> for NodeId {
    fn from(value: DH<'a, 'b, F, DynamicHandle3>) -> Self {
        value.nodes.node_id
    }
}
impl<'a, 'b, F: Float> DH<'a, 'b, F, DynamicHandle3> {
    /// Turn this dynamic handle to a specific node into a [`DynamicChannelsHandle`] which can hold
    /// and combination of channels.
    pub fn to_channels_handle(self) -> DH<'a, 'b, F, DynamicChannelsHandle> {
        let mut in_channels = SmallVec::with_capacity(self.nodes.inputs() as usize);
        let mut out_channels = SmallVec::with_capacity(self.nodes.outputs() as usize);
        for chan in Dynamic::iter_inputs(&self.nodes) {
            in_channels.push(chan);
        }
        for chan in Dynamic::iter_outputs(&self.nodes) {
            out_channels.push(chan);
        }
        DH {
            nodes: DynamicChannelsHandle {
                in_channels,
                out_channels,
            },
            graph: self.graph,
        }
    }
    /// Change the name of the node in the [`Graph`].
    pub fn name(self, n: impl Into<EcoString>) -> Self {
        self.graph.write().set_name(self.nodes.node_id, n.into());
        self
    }
    /// Link the parameter to a node output
    pub fn link<S: Static<Outputs = U1>>(
        self,
        p: impl Into<Param>,
        source: SH<'a, 'b, F, S>,
    ) -> Self {
        let input = source.nodes.iter_outputs().next().unwrap();
        let mut g = self.graph.write();
        if let NodeOrGraph::Node(source_node) = input.0 {
            if let Err(e) =
                g.connect_replace_to_parameter(source_node, input.1, p, self.nodes.node_id)
            {
                log::error!("Failed to connect signal to parameter: {e}");
            }
        } else {
            log::error!(
                "Graph input provided as input to a parameter. This is not currently supported. Connection ignored."
            );
        }
        self
    }

    /// Returns the [`NodeId`] of the node this handle points to.
    pub fn id(self) -> NodeId {
        self.nodes.node_id
    }
    /// Get a parameter from the node this handle points to if it exists. Panics if the parameter doesn't exist.
    pub fn param(self, p: impl Into<Param>) -> Parameter {
        self.try_param(p).unwrap()
    }
    /// Get a parameter from the node this handle points to if it exists.
    pub fn try_param(self, p: impl Into<Param>) -> Option<Parameter> {
        let p = p.into();
        match p {
            Param::Index(i) => {
                if (i as u16) < self.nodes.data.parameters {
                    Some(Parameter {
                        node: self.nodes.node_id,
                        param_index: i as u16,
                        sender: self.graph.read().scheduling_channel_sender(),
                    })
                } else {
                    None
                }
            }
            Param::Desc(s) => {
                for (i, desc) in self.nodes.data.parameter_descriptions().enumerate() {
                    if s == desc {
                        return Some(Parameter {
                            node: self.nodes.node_id,
                            param_index: i as u16,
                            sender: self.graph.read().scheduling_channel_sender(),
                        });
                    }
                }
                None
            }
        }
    }
}
// Manual Clone and Copy impls necessary because of PhantomData
impl<U: UGen> Clone for Handle3<U> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<U: UGen> Copy for Handle3<U> {}
impl<U: UGen> From<Handle3<U>> for NodeId {
    fn from(value: Handle3<U>) -> Self {
        value.node_id
    }
}
impl<U: UGen> Static for Handle3<U> {
    type Outputs = U::Outputs;

    type Inputs = U::Inputs;

    type DynamicType = DynamicHandle3;

    fn iter_outputs(&self) -> ChannelIter<Self::Outputs> {
        let mut channels = ChannelIterBuilder::new();
        for i in 0..U::Outputs::U16 {
            channels.push(NodeOrGraph::Node(self.node_id), i);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }

    fn iter_inputs(&self) -> ChannelIter<Self::Inputs> {
        let mut channels = ChannelIterBuilder::new();
        for i in 0..U::Inputs::U16 {
            channels.push(NodeOrGraph::Node(self.node_id), i);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }

    fn dynamic<F: Float>(&self, graph: &GraphRef<F>) -> Self::DynamicType {
        let data = graph.read().node_data(self.node_id).unwrap();
        DynamicHandle3 {
            node_id: self.node_id,
            data,
        }
    }
}
// Macros for implementing arithmetics on sources with statically known channel configurations
macro_rules! math_gen_fn_static {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<F: Float, S0: Static, S1: Static>(
            s0: S0,
            s1: S1,
            graph: &GraphRef<F>,
        ) -> ChannelsHandle<S1::Outputs>
        where
            S0::Outputs: Same<S1::Outputs>,
        {
            let mut out_channels = ChannelIterBuilder::new();
            let mut g = graph.write();
            for (s0, s1) in Static::iter_outputs(&s0).zip(s1.iter_outputs()) {
                let mul = g.push_internal(MathUGen::<_, U1, $op>::new());
                if let Err(e) = g.connect2(s0.0, s0.1, 0, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                if let Err(e) = g.connect2(s1.0, s1.1, 1, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                out_channels.push(NodeOrGraph::Node(mul.node_id()), 0);
            }
            let channels = out_channels
                .into_channel_iter()
                .expect("all the channels should be initialised");
            ChannelsHandle {
                channels: channels.channels,
            }
        }
    };
}
math_gen_fn_static!(add_sources, knaster_core_dsp::math::Add);
math_gen_fn_static!(sub_sources, knaster_core_dsp::math::Sub);
math_gen_fn_static!(mul_sources, knaster_core_dsp::math::Mul);
math_gen_fn_static!(div_sources, knaster_core_dsp::math::Div);
math_gen_fn_static!(pow_sources, knaster_core_dsp::math::Pow);

/// A number that can be used as a constant in a graph.
pub enum ConstantNumber {
    #[allow(missing_docs)]
    F32(f32),
    #[allow(missing_docs)]
    F64(f64),
    #[allow(missing_docs)]
    Usize(usize),
    #[allow(missing_docs)]
    I32(i32),
}
impl ConstantNumber {
    /// Convert the constant number into a float of type F.
    pub fn into_f<F: Float>(self) -> F {
        match self {
            ConstantNumber::F32(f) => F::new(f),
            ConstantNumber::F64(f) => F::new(f),
            ConstantNumber::Usize(u) => F::from_usize(u),
            ConstantNumber::I32(i) => F::new(i as f32),
        }
    }
}
impl From<f32> for ConstantNumber {
    fn from(val: f32) -> Self {
        ConstantNumber::F32(val)
    }
}
impl From<f64> for ConstantNumber {
    fn from(val: f64) -> Self {
        ConstantNumber::F64(val)
    }
}
impl From<usize> for ConstantNumber {
    fn from(val: usize) -> Self {
        ConstantNumber::Usize(val)
    }
}
impl From<i32> for ConstantNumber {
    fn from(val: i32) -> Self {
        ConstantNumber::I32(val)
    }
}
impl From<ConstantNumber> for f32 {
    fn from(value: ConstantNumber) -> Self {
        match value {
            ConstantNumber::F32(f) => f,
            ConstantNumber::F64(f) => f as f32,
            ConstantNumber::Usize(u) => u as f32,
            ConstantNumber::I32(i) => i as f32,
        }
    }
}
impl From<ConstantNumber> for f64 {
    fn from(value: ConstantNumber) -> Self {
        match value {
            ConstantNumber::F32(f) => f as f64,
            ConstantNumber::F64(f) => f,
            ConstantNumber::Usize(u) => u as f64,
            ConstantNumber::I32(i) => i as f64,
        }
    }
}

macro_rules! math_gen_fn_static_constant {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<F: Float, S0: Static, S1: Into<ConstantNumber>>(
            s0: S0,
            s1: S1,
            graph: &GraphRef<F>,
        ) -> ChannelsHandle<S0::Outputs> {
            let mut out_channels = ChannelIterBuilder::new();
            let mut g = graph.write();
            // TODO: Make a separate UGen for constant number maths to avoid this extra node
            let c: F = s1.into().into_f();
            let c = g.push_internal(Constant::new(c));
            for s0 in Static::iter_outputs(&s0) {
                let mul = g.push_internal(MathUGen::<_, U1, $op>::new());
                if let Err(e) = g.connect2(s0.0, s0.1, 0, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                if let Err(e) = g.connect2(c.node_id(), 0, 1, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                out_channels.push(NodeOrGraph::Node(mul.node_id()), 0);
            }
            let channels = out_channels
                .into_channel_iter()
                .expect("all the channels should be initialised");
            ChannelsHandle {
                channels: channels.channels,
            }
        }
    };
}
math_gen_fn_static_constant!(add_sources_static_constant, knaster_core_dsp::math::Add);
math_gen_fn_static_constant!(sub_sources_static_constant, knaster_core_dsp::math::Sub);
math_gen_fn_static_constant!(mul_sources_static_constant, knaster_core_dsp::math::Mul);
math_gen_fn_static_constant!(div_sources_static_constant, knaster_core_dsp::math::Div);
// math_gen_fn_static_constant!(pow_sources_static_constant, knaster_core::math::Pow);

// Macros for implementing arithmetics on sources without statically known channel configurations
macro_rules! math_gen_fn_dynamic {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<F: Float, S0: Dynamic, S1: Dynamic>(
            s0: S0,
            s1: S1,
            graph: &GraphRef<F>,
        ) -> DynamicChannelsHandle {
            if s0.outputs() != s1.outputs() {
                panic!("The number of outputs of the two sources must be the same");
            }
            let mut out_channels = SmallVec::with_capacity(s0.outputs() as usize);
            let mut g = graph.write();
            for (s0, s1) in Dynamic::iter_outputs(&s0).zip(s1.iter_outputs()) {
                let mul = g.push_internal(MathUGen::<_, U1, $op>::new());
                if let Err(e) = g.connect2(s0.0, s0.1, 0, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                if let Err(e) = g.connect2(s1.0, s1.1, 1, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                out_channels.push((NodeOrGraph::Node(mul.node_id()), 0));
            }
            DynamicChannelsHandle {
                in_channels: SmallVec::new(),
                out_channels,
            }
        }
    };
}
math_gen_fn_dynamic!(add_sources_dynamic, knaster_core_dsp::math::Add);
math_gen_fn_dynamic!(sub_sources_dynamic, knaster_core_dsp::math::Sub);
math_gen_fn_dynamic!(mul_sources_dynamic, knaster_core_dsp::math::Mul);
math_gen_fn_dynamic!(div_sources_dynamic, knaster_core_dsp::math::Div);
math_gen_fn_dynamic!(pow_sources_dynamic, knaster_core_dsp::math::Pow);

// Macros for implementing arithmetics on sources without statically known channel configurations
macro_rules! math_gen_fn_dynamic_constant {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<F: Float, S0: Dynamic, S1: Into<ConstantNumber>>(
            s0: S0,
            s1: S1,
            graph: &GraphRef<F>,
        ) -> DynamicChannelsHandle {
            let mut out_channels = SmallVec::with_capacity(s0.outputs() as usize);
            let mut g = graph.write();
            let c: F = s1.into().into_f();
            let c = g.push_internal(Constant::new(c));
            for s0 in Dynamic::iter_outputs(&s0) {
                let mul = g.push_internal(MathUGen::<_, U1, $op>::new());
                if let Err(e) = g.connect2(s0.0, s0.1, 0, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                if let Err(e) = g.connect2(c.node_id(), 0, 1, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                out_channels.push((NodeOrGraph::Node(mul.node_id()), 0));
            }
            DynamicChannelsHandle {
                in_channels: SmallVec::new(),
                out_channels,
            }
        }
    };
}
math_gen_fn_dynamic_constant!(add_sources_dynamic_constant, knaster_core_dsp::math::Add);
math_gen_fn_dynamic_constant!(sub_sources_dynamic_constant, knaster_core_dsp::math::Sub);
math_gen_fn_dynamic_constant!(mul_sources_dynamic_constant, knaster_core_dsp::math::Mul);
math_gen_fn_dynamic_constant!(div_sources_dynamic_constant, knaster_core_dsp::math::Div);
// math_gen_fn_dynamic_constant!(pow_sources_dynamic_constant, knaster_core::math::Pow);

// Arithmetics with static types
macro_rules! math_impl_static_static {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, 'b, F: Float, S0: Static, S1: Static> $op<SH<'a, 'b, F, S1>> for SH<'a, 'b, F, S0>
        where
            S0::Outputs: Same<S1::Outputs>,
        {
            type Output = SH<'a, 'b, F, ChannelsHandle<S1::Outputs>>;

            fn $op_lowercase(self, rhs: SH<'a, 'b, F, S1>) -> Self::Output {
                let graph = self.graph;
                SH {
                    nodes: $fn_name(self.nodes, rhs.nodes, graph),
                    graph: &self.graph,
                }
            }
        }
    };
}
math_impl_static_static!(mul_sources, Mul, mul);
math_impl_static_static!(add_sources, Add, add);
math_impl_static_static!(sub_sources, Sub, sub);
math_impl_static_static!(div_sources, Div, div);

// Arithmetics with and static types and constants
// Create implementations for arithmetics with constants f32 f64 or usize
macro_rules! math_impl_static_constant_type {
    ($fn_name:ident, $op:ident, $op_lowercase:ident, $ty:ty) => {
        impl<'a, 'b, F: Float, S0: Static> $op<$ty> for SH<'a, 'b, F, S0> {
            type Output = SH<'a, 'b, F, ChannelsHandle<S0::Outputs>>;

            fn $op_lowercase(self, rhs: $ty) -> Self::Output {
                let graph = self.graph;
                SH {
                    nodes: $fn_name(self.nodes, rhs, graph),
                    graph: &self.graph,
                }
            }
        }
        impl<'a, 'b, F: Float, S0: Static> $op<SH<'a, 'b, F, S0>> for $ty {
            type Output = SH<'a, 'b, F, ChannelsHandle<S0::Outputs>>;

            fn $op_lowercase(self, rhs: SH<'a, 'b, F, S0>) -> Self::Output {
                let graph = rhs.graph;
                SH {
                    nodes: $fn_name(rhs.nodes, self, graph),
                    graph: &rhs.graph,
                }
            }
        }
    };
}
macro_rules! math_impl_static_constant {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        math_impl_static_constant_type!($fn_name, $op, $op_lowercase, f32);
        math_impl_static_constant_type!($fn_name, $op, $op_lowercase, f64);
        math_impl_static_constant_type!($fn_name, $op, $op_lowercase, usize);
        math_impl_static_constant_type!($fn_name, $op, $op_lowercase, i32);
    };
}
math_impl_static_constant!(mul_sources_static_constant, Mul, mul);
math_impl_static_constant!(add_sources_static_constant, Add, add);
math_impl_static_constant!(sub_sources_static_constant, Sub, sub);
math_impl_static_constant!(div_sources_static_constant, Div, div);

impl<'a, 'b, F: Float, S0: Static, S1: Static> Shr<SH<'a, 'b, F, S1>> for SH<'a, 'b, F, S0>
where
    S1::Inputs: Same<S0::Outputs>,
{
    type Output = SH<'a, 'b, F, S1>;

    fn shr(self, rhs: SH<'a, 'b, F, S1>) -> Self::Output {
        self.to(rhs)
    }
}
impl<'a, 'b, F: Float, S0: Static, S1: Static> BitOr<SH<'a, 'b, F, S1>> for SH<'a, 'b, F, S0> {
    type Output = SH<'a, 'b, F, Stack<S0, S1>>;

    fn bitor(self, rhs: SH<'a, 'b, F, S1>) -> Self::Output {
        self.stack(rhs)
    }
}

impl<'a, 'b, F: Float, S0: Static, S1: Dynamic> Shr<DH<'a, 'b, F, S1>> for SH<'a, 'b, F, S0> {
    type Output = DH<'a, 'b, F, S1>;

    fn shr(self, rhs: DH<'a, 'b, F, S1>) -> Self::Output {
        let s = self.dynamic();
        s.to(rhs)
    }
}
impl<'a, 'b, F: Float, S0: Static, S1: Dynamic> BitOr<DH<'a, 'b, F, S1>> for SH<'a, 'b, F, S0> {
    type Output = DH<'a, 'b, F, DynamicChannelsHandle>;

    fn bitor(self, rhs: DH<'a, 'b, F, S1>) -> Self::Output {
        let s = self.dynamic();
        s.stack(rhs)
    }
}
impl<'a, 'b, F: Float, S0: Static + Clone, S1: Dynamic> Shr<SH<'a, 'b, F, S0>>
    for DH<'a, 'b, F, S1>
{
    type Output = SH<'a, 'b, F, S0>;

    fn shr(self, rhs: SH<'a, 'b, F, S0>) -> Self::Output {
        self.to(rhs.clone().dynamic());
        rhs
    }
}
impl<'a, 'b, F: Float, S0: Static, S1: Dynamic> BitOr<SH<'a, 'b, F, S0>> for DH<'a, 'b, F, S1> {
    type Output = DH<'a, 'b, F, DynamicChannelsHandle>;

    fn bitor(self, rhs: SH<'a, 'b, F, S0>) -> Self::Output {
        self.stack(rhs.dynamic())
    }
}
// Static Handle3 and DynamicSource3 impls
//
macro_rules! math_impl_static_dynamic {
    ($fn_name:ident, $op:ident, $op_lowercase:ident, $ty0:ty, $ty1:ty) => {
        impl<'a, 'b, F: Float, S0: Static, S1: Dynamic> $op<$ty1> for $ty0 {
            type Output = DH<'a, 'b, F, DynamicChannelsHandle>;

            fn $op_lowercase(self, rhs: $ty1) -> Self::Output {
                let graph = self.graph;
                let dh0 = self.dynamic();
                let dh1 = rhs.dynamic();
                DH {
                    nodes: $fn_name(dh0.nodes, dh1.nodes, graph),
                    graph,
                }
            }
        }
    };
}
math_impl_static_dynamic!(
    mul_sources_dynamic,
    Mul,
    mul,
    SH<'a, 'b, F, S0>,
    DH<'a, 'b, F, S1>
);
math_impl_static_dynamic!(
    mul_sources_dynamic,
    Mul,
    mul,
    DH<'a, 'b, F, S1>,
    SH<'a, 'b, F, S0>
);
math_impl_static_dynamic!(
    add_sources_dynamic,
    Add,
    add,
    SH<'a, 'b, F, S0>,
    DH<'a, 'b, F, S1>
);
math_impl_static_dynamic!(
    add_sources_dynamic,
    Add,
    add,
    DH<'a, 'b, F, S1>,
    SH<'a, 'b, F, S0>
);
math_impl_static_dynamic!(
    sub_sources_dynamic,
    Sub,
    sub,
    SH<'a, 'b, F, S0>,
    DH<'a, 'b, F, S1>
);
math_impl_static_dynamic!(
    sub_sources_dynamic,
    Sub,
    sub,
    DH<'a, 'b, F, S1>,
    SH<'a, 'b, F, S0>
);
math_impl_static_dynamic!(
    div_sources_dynamic,
    Div,
    div,
    SH<'a, 'b, F, S0>,
    DH<'a, 'b, F, S1>
);
math_impl_static_dynamic!(
    div_sources_dynamic,
    Div,
    div,
    DH<'a, 'b, F, S1>,
    SH<'a, 'b, F, S0>
);

// DynamicHandle3 and DynamicSource3 impls
//
macro_rules! math_impl_dynamic_handle3_dynamic {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, 'b, F: Float, S: Dynamic, S1: Dynamic> $op<DH<'a, 'b, F, S1>>
            for DH<'a, 'b, F, S>
        {
            type Output = DH<'a, 'b, F, DynamicChannelsHandle>;

            fn $op_lowercase(self, rhs: DH<'a, 'b, F, S1>) -> Self::Output {
                let graph = self.graph;
                let handle = $fn_name(self.nodes, rhs.nodes, graph);
                DH {
                    nodes: handle,
                    graph,
                }
            }
        }
    };
}
math_impl_dynamic_handle3_dynamic!(mul_sources_dynamic, Mul, mul);
math_impl_dynamic_handle3_dynamic!(add_sources_dynamic, Add, add);
math_impl_dynamic_handle3_dynamic!(sub_sources_dynamic, Sub, sub);
math_impl_dynamic_handle3_dynamic!(div_sources_dynamic, Div, div);

impl<'a, 'b, F: Float, D0: Dynamic, D1: Dynamic> Shr<DH<'a, 'b, F, D1>> for DH<'a, 'b, F, D0> {
    type Output = DH<'a, 'b, F, D1>;

    fn shr(self, rhs: DH<'a, 'b, F, D1>) -> Self::Output {
        self.to(rhs)
    }
}
impl<'a, 'b, F: Float, D0: Dynamic, D1: Dynamic> BitOr<DH<'a, 'b, F, D1>> for DH<'a, 'b, F, D0> {
    type Output = DH<'a, 'b, F, DynamicChannelsHandle>;

    fn bitor(self, rhs: DH<'a, 'b, F, D1>) -> Self::Output {
        self.stack(rhs)
    }
}

// Arithmetics with and dynamic types and constants
// Create implementations for arithmetics with constants f32 f64 or usize
macro_rules! math_impl_dynamic_constant_type {
    ($fn_name:ident, $op:ident, $op_lowercase:ident, $ty:ty) => {
        impl<'a, 'b, F: Float, S: Dynamic> $op<$ty> for DH<'a, 'b, F, S> {
            type Output = DH<'a, 'b, F, DynamicChannelsHandle>;

            fn $op_lowercase(self, rhs: $ty) -> Self::Output {
                let graph = self.graph;
                let handle = $fn_name(self.nodes, rhs, graph);
                DH {
                    nodes: handle,
                    graph,
                }
            }
        }
        impl<'a, 'b, F: Float, S: Dynamic> $op<DH<'a, 'b, F, S>> for $ty {
            type Output = DH<'a, 'b, F, DynamicChannelsHandle>;

            fn $op_lowercase(self, rhs: DH<'a, 'b, F, S>) -> Self::Output {
                let graph = rhs.graph;
                let handle = $fn_name(rhs.nodes, self, graph);
                DH {
                    nodes: handle,
                    graph,
                }
            }
        }
    };
}
macro_rules! math_impl_dynamic_constant {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        math_impl_dynamic_constant_type!($fn_name, $op, $op_lowercase, f32);
        math_impl_dynamic_constant_type!($fn_name, $op, $op_lowercase, f64);
        math_impl_dynamic_constant_type!($fn_name, $op, $op_lowercase, usize);
        math_impl_dynamic_constant_type!($fn_name, $op, $op_lowercase, i32);
    };
}
math_impl_dynamic_constant!(mul_sources_dynamic_constant, Mul, mul);
math_impl_dynamic_constant!(add_sources_dynamic_constant, Add, add);
math_impl_dynamic_constant!(sub_sources_dynamic_constant, Sub, sub);
math_impl_dynamic_constant!(div_sources_dynamic_constant, Div, div);

#[derive(Copy, Clone)]
/// A stack of two nodes, where the channels are mapped sequentially first to the first node, then to the second node, when creating connections.
///
/// This is useful e.g. when you want to pass two different single channel nodes to a node that takes two channels as input.
pub struct Stack<S0, S1> {
    s0: S0,
    s1: S1,
}

impl<S0: Static, S1: Static> Static for Stack<S0, S1>
where
    <S0::Inputs as Add<S1::Inputs>>::Output: Size,
    <S0 as Static>::Inputs: core::ops::Add<<S1 as Static>::Inputs>,
    <S0::Outputs as Add<S1::Outputs>>::Output: Size,
    <S0 as Static>::Outputs: core::ops::Add<<S1 as Static>::Outputs>,
{
    type Outputs = <S0::Outputs as Add<S1::Outputs>>::Output;
    type Inputs = <S0::Inputs as Add<S1::Inputs>>::Output;

    fn iter_inputs(&self) -> ChannelIter<Self::Inputs> {
        let mut channels = ChannelIterBuilder::new();
        for (node, index) in Static::iter_inputs(&self.s0).chain(Static::iter_inputs(&self.s1)) {
            channels.push(node, index);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }
    fn iter_outputs(&self) -> ChannelIter<Self::Outputs> {
        let mut channels = ChannelIterBuilder::new();
        for (node, index) in Static::iter_outputs(&self.s0).chain(Static::iter_outputs(&self.s1)) {
            channels.push(node, index);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }

    type DynamicType = DynamicChannelsHandle;

    fn dynamic<F: Float>(&self, _graph: &GraphRef<F>) -> Self::DynamicType {
        let mut in_channels = SmallVec::with_capacity(S0::Inputs::USIZE + S1::Inputs::USIZE);
        let mut out_channels = SmallVec::with_capacity(S0::Outputs::USIZE + S1::Outputs::USIZE);
        for chan in Static::iter_inputs(&self.s0) {
            in_channels.push(chan);
        }
        for chan in Static::iter_inputs(&self.s1) {
            in_channels.push(chan);
        }
        for chan in Static::iter_outputs(&self.s0) {
            out_channels.push(chan);
        }
        for chan in Static::iter_outputs(&self.s1) {
            out_channels.push(chan);
        }
        DynamicChannelsHandle {
            in_channels,
            out_channels,
        }
    }
}

#[derive(Clone)]
/// A statically sized array of output channels.
pub struct ChannelsHandle<O: Size> {
    channels: NumericArray<(NodeOrGraph, u16), O>,
}
// // Copy workaround, see the `ArrayLength` docs for more info.
impl<O: Size> Copy for ChannelsHandle<O> where
    <O as knaster_core::numeric_array::ArrayLength>::ArrayType<(NodeOrGraph, u16)>:
        core::marker::Copy
{
}
impl<O: Size> From<ChannelIter<O>> for ChannelsHandle<O> {
    fn from(value: ChannelIter<O>) -> Self {
        ChannelsHandle {
            channels: value.channels,
        }
    }
}

impl<O: Size> Static for ChannelsHandle<O> {
    type Outputs = O;
    type Inputs = U0;

    fn iter_outputs(&self) -> ChannelIter<Self::Outputs> {
        ChannelIter {
            channels: self.channels.clone(),
            current_index: 0,
        }
    }
    fn iter_inputs(&self) -> ChannelIter<Self::Inputs> {
        ChannelIter::empty()
    }

    type DynamicType = DynamicChannelsHandle;

    fn dynamic<F: Float>(&self, _graph: &GraphRef<F>) -> Self::DynamicType {
        let mut out_channels = SmallVec::with_capacity(self.channels.len());
        for &chan in self.channels.iter() {
            out_channels.push(chan);
        }
        DynamicChannelsHandle {
            in_channels: SmallVec::new(),
            out_channels,
        }
    }
}

/// Handle to any combination of input and output channels within a graph, without
/// type information. Unlike [`ChannelsHandle`], this type represents all kinds of
/// collections of channels that aren't direct references to specific nodes.
#[derive(Clone)]
pub struct DynamicChannelsHandle {
    in_channels: SmallVec<[(NodeOrGraph, u16); 1]>,
    out_channels: SmallVec<[(NodeOrGraph, u16); 1]>,
}
impl Dynamic for DynamicChannelsHandle {
    fn iter_outputs(&self) -> DynamicChannelIter {
        DynamicChannelIter {
            channels: self.out_channels.clone(),
            current_index: 0,
        }
    }

    fn outputs(&self) -> u16 {
        self.out_channels.len() as u16
    }
    fn inputs(&self) -> u16 {
        self.in_channels.len() as u16
    }

    fn iter_inputs(&self) -> DynamicChannelIter {
        DynamicChannelIter {
            channels: self.in_channels.clone(),
            current_index: 0,
        }
    }
}

// We need Sink and Source because some things such as binary op connections can't reasonably be
// have things connected to their inputs

/// Trait for handles with statically known channel configurations.
pub trait Static {
    /// Number of output channels
    type Outputs: Size;
    /// Number of input channels
    type Inputs: Size;
    /// What type to use when converting self to a dynamic handle
    type DynamicType: Dynamic;
    /// Returns an iterator over the output channels
    fn iter_outputs(&self) -> ChannelIter<Self::Outputs>;
    /// Returns an iterator over the input channels
    fn iter_inputs(&self) -> ChannelIter<Self::Inputs>;
    /// Convert self to a dynamic handle.
    ///
    /// Dynamic in this context means that the channel configuration is only known at runtime.
    fn dynamic<F: Float>(&self, graph: &GraphRef<F>) -> Self::DynamicType;
}
/// Convenience struct for building a [`ChannelIter`].
struct ChannelIterBuilder<I: Size> {
    channels: knaster_core::numeric_array::generic_array::GenericArray<
        MaybeUninit<(NodeOrGraph, u16)>,
        I,
    >,
    current_index: usize,
}
impl<I: Size> ChannelIterBuilder<I> {
    pub fn new() -> Self {
        Self {
            channels: knaster_core::numeric_array::generic_array::GenericArray::uninit(),
            current_index: 0,
        }
    }
    pub fn push(&mut self, node: NodeOrGraph, index: u16) {
        if self.current_index < I::USIZE {
            self.channels[self.current_index].write((node, index));
            self.current_index += 1;
        }
    }
    pub fn into_channel_iter(self) -> Option<ChannelIter<I>> {
        if self.current_index == I::USIZE {
            // Safety:
            // Only if all the elements are initialised do we run this
            let channels = unsafe {
                NumericArray::from(
                    knaster_core::numeric_array::generic_array::GenericArray::assume_init(
                        self.channels,
                    ),
                )
            };
            Some(ChannelIter {
                channels,
                current_index: 0,
            })
        } else {
            None
        }
    }
}
/// Iterator over a statically known number of channels.
pub struct ChannelIter<I: Size> {
    channels: NumericArray<(NodeOrGraph, u16), I>,
    current_index: usize,
}
impl ChannelIter<U0> {
    /// Create an empty iterator
    pub fn empty() -> Self {
        Self {
            channels: knaster_core::numeric_array::narr!(),
            current_index: 0,
        }
    }
}
impl<I: Size> Iterator for ChannelIter<I> {
    type Item = (NodeOrGraph, u16);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < I::USIZE {
            let o = self.channels[self.current_index];
            self.current_index += 1;
            Some(o)
        } else {
            None
        }
    }
}
/// Iterator over a dynamically sized number of channels.
pub struct DynamicChannelIter {
    channels: SmallVec<[(NodeOrGraph, u16); 1]>,
    current_index: usize,
}
impl DynamicChannelIter {
    /// Create a new iterator from the given channels
    pub fn new(channels: SmallVec<[(NodeOrGraph, u16); 1]>) -> Self {
        Self {
            channels,
            current_index: 0,
        }
    }
    /// Create an empty iterator
    pub fn empty() -> Self {
        Self {
            channels: SmallVec::with_capacity(0),
            current_index: 0,
        }
    }
}
impl Iterator for DynamicChannelIter {
    type Item = (NodeOrGraph, u16);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < self.channels.len() {
            let o = self.channels[self.current_index];
            self.current_index += 1;
            Some(o)
        } else {
            None
        }
    }
}

/// Trait for handles with dynamically known channel configurations, i.e. the number of inputs and outputs is not known at compile time.
pub trait Dynamic {
    /// Get an iterator over the output channels
    fn iter_outputs(&self) -> DynamicChannelIter;
    /// Get the number of output channels
    fn outputs(&self) -> u16;
    /// Get an iterator over the input channels
    fn iter_inputs(&self) -> DynamicChannelIter;
    /// Get the number of input channels
    fn inputs(&self) -> u16;
    /// Get a dynamic handle to this node. Provided for symmetry with [`Static`].
    fn dynamic<F: Float>(&self, _graph: &GraphRef<F>) -> &Self {
        self
    }
}

/// A handle to a specific parameter of a specific node.
#[derive(Clone)]
pub struct Parameter {
    pub(crate) node: NodeId,
    pub(crate) param_index: u16,
    /// Allows us to send parameter changes straight to the audio thread
    sender: SchedulingChannelSender,
}
impl Parameter {
    /// Set the value of the parameter.
    pub fn set(&mut self, value: impl Into<ParameterValue>) -> Result<(), GraphError> {
        let value = value.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(value),
            smoothing: None,
            token: None,
            time: None,
        })?;
        Ok(())
    }
    /// Set the value of the parameter with the given `Time` setting.
    pub fn set_time(
        &mut self,
        value: impl Into<ParameterValue>,
        t: impl Into<Time>,
    ) -> Result<(), GraphError> {
        let value = value.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(value),
            smoothing: None,
            token: None,
            time: Some(t.into()),
        })?;
        Ok(())
    }
    /// Set the value of the parameter _at_ the given time in [`Seconds`], in absolute time.
    pub fn set_at(
        &mut self,
        value: impl Into<ParameterValue>,
        t: impl Into<Seconds>,
    ) -> Result<(), GraphError> {
        let value = value.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(value),
            smoothing: None,
            token: None,
            time: Some(Time::at(t.into())),
        })?;
        Ok(())
    }
    /// Set the value of the parameter _after_ the given time in [`Seconds`], in relative time to
    /// when it it scheduled on the audio thread.
    pub fn set_after(
        &mut self,
        value: impl Into<ParameterValue>,
        t: impl Into<Seconds>,
    ) -> Result<(), GraphError> {
        let value = value.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(value),
            smoothing: None,
            token: None,
            time: Some(Time::after(t.into())),
        })?;
        Ok(())
    }
    /// Set the smoothing setting for the parameter.
    pub fn smooth(&mut self, s: impl Into<ParameterSmoothing>) -> Result<(), GraphError> {
        let s = s.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: None,
            smoothing: Some(s),
            token: None,
            time: None,
        })?;
        Ok(())
    }
    /// Set the smoothing setting for the parameter with the given `Time` setting.
    pub fn smooth_time(
        &mut self,
        s: impl Into<ParameterSmoothing>,
        t: impl Into<Time>,
    ) -> Result<(), GraphError> {
        let s = s.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: None,
            smoothing: Some(s),
            token: None,
            time: Some(t.into()),
        })?;
        Ok(())
    }
    /// Set the smoothing setting for the parameter _at_ the given time in [`Seconds`], in absolute time.
    pub fn smooth_at(
        &mut self,
        s: impl Into<ParameterSmoothing>,
        t: impl Into<Seconds>,
    ) -> Result<(), GraphError> {
        let s = s.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: None,
            smoothing: Some(s),
            token: None,
            time: Some(Time::at(t.into())),
        })?;
        Ok(())
    }
    /// Set the smoothing setting for the parameter _after_ the given time in [`Seconds`], in relative time to
    /// when it it scheduled on the audio thread.
    pub fn smooth_after(
        &mut self,
        s: impl Into<ParameterSmoothing>,
        t: impl Into<Seconds>,
    ) -> Result<(), GraphError> {
        let s = s.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: None,
            smoothing: Some(s),
            token: None,
            time: Some(Time::after(t.into())),
        })?;
        Ok(())
    }
    /// Trigger the parameter.
    pub fn trig(&mut self) -> Result<(), GraphError> {
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(ParameterValue::Trigger),
            smoothing: None,
            token: None,
            time: None,
        })?;
        Ok(())
    }
    /// Trigger the parameter with the given `Time` setting.
    pub fn trig_time(&mut self, t: impl Into<Time>) -> Result<(), GraphError> {
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(ParameterValue::Trigger),
            smoothing: None,
            token: None,
            time: Some(t.into()),
        })?;
        Ok(())
    }
    /// Trigger the parameter _at_ the given time in [`Seconds`], in absolute time.
    pub fn trig_at(&mut self, t: impl Into<Seconds>) -> Result<(), GraphError> {
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(ParameterValue::Trigger),
            smoothing: None,
            token: None,
            time: Some(Time::at(t.into())),
        })?;
        Ok(())
    }
    /// Trigger the parameter _after_ the given time in [`Seconds`], in relative time to
    /// when it it scheduled on the audio thread.
    pub fn trig_after(&mut self, t: impl Into<Seconds>) -> Result<(), GraphError> {
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(ParameterValue::Trigger),
            smoothing: None,
            token: None,
            time: Some(Time::after(t.into())),
        })?;
        Ok(())
    }
}

// /// Statically allocated collection of parameters to which changes can be scheduled.
// ///
// /// [`Parameters`] is a way of collecting many parameters without unnecessary heap
// /// allocations.
// pub struct Parameters<S: Size> {
//     params: NumericArray<Parameter, S>,
// }
//
// impl Parameters<U0> {
//     pub fn new() -> Self {
//         Self {
//             params: knaster_core::numeric_array::narr!(),
//         }
//     }
// }
// impl<S: Size + Sync + Send> Parameters<S> {
//     pub fn set(
//         &mut self,
//         n: impl AsRef<str>,
//         value: impl Into<ParameterValue>,
//         t: SchedulingTime,
//     ) -> Result<(), GraphError> {
//         let n = n.as_ref();
//         let value = value.into();
//         for (name, p) in &mut self.params {
//             if name == n {
//                 p.sender.send(crate::SchedulingEvent {
//                     node_key: p.node.key(),
//                     parameter: p.param_index as usize,
//                     value: Some(value),
//                     smoothing: None,
//                     token: None,
//                     time: Some(t),
//                 })?;
//             }
//         }
//         Ok(())
//     }
//     pub fn smooth(
//         &mut self,
//         n: impl AsRef<str>,
//         s: impl Into<ParameterSmoothing>,
//         t: SchedulingTime,
//     ) -> Result<(), GraphError> {
//         let n = n.as_ref();
//         let s = s.into();
//         for (name, p) in &mut self.params {
//             if name == n {
//                 p.sender.send(crate::SchedulingEvent {
//                     node_key: p.node.key(),
//                     parameter: p.param_index as usize,
//                     value: None,
//                     smoothing: Some(s),
//                     token: None,
//                     time: Some(t),
//                 })?;
//             }
//         }
//         Ok(())
//     }
//     pub fn trig(&mut self, n: impl AsRef<str>, t: SchedulingTime) -> Result<(), GraphError> {
//         let n = n.as_ref();
//         for (name, p) in &mut self.params {
//             if name == n {
//                 p.sender.send(crate::SchedulingEvent {
//                     node_key: p.node.key(),
//                     parameter: p.param_index as usize,
//                     value: Some(ParameterValue::Trigger),
//                     smoothing: None,
//                     token: None,
//                     time: Some(t),
//                 })?;
//             }
//         }
//         Ok(())
//     }
// }
// impl<S: Size + Sync + Send> Parameters<S>
// where
//     S: Add<B1>,
//     <S as Add<B1>>::Output: Size,
// {
//     pub fn push(self, name: impl Into<EcoString>, p: Parameter) -> Parameters<Add1<S>> {
//         let mut array: knaster_core::numeric_array::generic_array::GenericArray<
//             MaybeUninit<_>,
//             Add1<S>,
//         > = knaster_core::numeric_array::generic_array::GenericArray::uninit();
//
//         // Copy existing elements
//         for (i, p) in self.params.into_iter().enumerate() {
//             array[i].write(p);
//         }
//
//         // Write new element
//         array[S::USIZE].write((name.into(), p));
//
//         // SAFETY: All items are initialized
//         let params = unsafe {
//             NumericArray::from(
//                 knaster_core::numeric_array::generic_array::GenericArray::assume_init(array),
//             )
//         };
//         Parameters { params }
//     }
// }

#[cfg(test)]
mod tests {

    use crate::{
        Time,
        processor::{AudioProcessor, AudioProcessorOptions},
    };
    use knaster_core::{Block, Seconds, typenum::*};
    use knaster_core_dsp::{
        noise::WhiteNoise, onepole::OnePoleLpf, osc::SinWt, pan::Pan2, util::Constant,
        wrappers_core::UGenWrapperCoreExt,
    };

    #[test]
    fn scope() {
        let block_size = 16;
        let (mut graph, _audio_processor, _log_receiver) =
            AudioProcessor::<f32>::new::<U0, U2>(AudioProcessorOptions {
                block_size,
                sample_rate: 48000,
                ring_buffer_size: 50,
                ..Default::default()
            });
        let (kept_sine, kept_lpf) = graph.edit(|graph| {
            {
                let sine = graph.push(SinWt::new(200.));
                let sine2 = graph.push(SinWt::new(200.));
                let lpf = graph.push(OnePoleLpf::new(2000.));
                let c = graph.push(Constant::new(0.2));
                let amp = graph.push(Constant::new(0.2));
                let pan = graph.push(Pan2::new(0.2));
                sine.name("MySine").link("freq", sine2 * c);
                let a = sine * sine2 * c;
                let lpf_l = graph.push(OnePoleLpf::new(2600.));
                let lpf_r = graph.push(OnePoleLpf::new(2600.));
                a.to(lpf).to(pan).to_graph_out();
                let d = (a / c - c) + c * c * 0.2 / 4 - c;
                let _e = (d | d) - (c | c);
                // (0.2 * e).to_graph_out();
                ((a >> lpf >> pan >> (lpf_l | lpf_r)) * (amp | amp)).to_graph_out();
                lpf.to_graph_out_channels(1);

                let exciter_amp = graph.push(Constant::new(0.5));
                let exciter = graph.push(SinWt::new(2000.).wr_mul(0.1));
                let noise_mix = graph.push(Constant::new(0.25));
                let noise = graph.push(WhiteNoise::new());
                let exciter_lpf = graph.push(OnePoleLpf::new(2600.));
                let ex = exciter * exciter_amp;
                ((noise * noise_mix * ex + ex) >> exciter_lpf).to_graph_out_channels([1]);

                let t = Time::at(Seconds::from_secs_f64(2.5));
                // p.set("freq", 400., t).unwrap();
                let mut freq = sine.param("freq");
                freq.set_time(400., t).unwrap();

                let _freq = sine.try_param("freq").unwrap();
                // s = Some(sine);
                let c = graph.push(Constant::new(0.2));
                let _c = sine * sine2 * c;
                (sine.id(), lpf.id())
            }
        });
        graph.edit(|graph| {
            // do a bunch of stuff
            {
                // Retreive a handle to the node
                let handle = graph.handle(kept_sine).unwrap();
                let handle2 = graph.handle_from_name("MySine").unwrap();
                assert_eq!(handle.id(), handle2.id());
                let lpf = graph.handle(kept_lpf).unwrap();
                // Use the handle to connect it to another node
                let new_sine = graph.push(SinWt::new(200.));
                let new_pan = graph.push(Pan2::new(0.));
                let _a = (handle * new_sine) >> new_pan >> (lpf | lpf);
                let a = (new_sine * handle) >> new_pan;
                let _result = ((handle * 2.0) - 3) / 5.0 + handle;
                (new_pan + (handle2 | handle2)).to(lpf | lpf);
                a.to_graph_out();
                let _ = (new_sine + handle) >> new_pan;
            }
        });
    }

    use crate::tests::utils::TestInPlusParamUGen;
    #[test]
    fn disconnect() {
        let block_size = 16;
        let (mut g, mut audio_processor, _log_receiver) =
            AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
                block_size,
                sample_rate: 48000,
                ring_buffer_size: 50,
                ..Default::default()
            });

        g.edit(|g| {
            let n1 = g.push(TestInPlusParamUGen::new()).name("n1");
            g.set(n1, 0, 0.5, Time::asap()).unwrap();
            let n2 = g.push(TestInPlusParamUGen::new()).name("n2");
            g.set(n2, 0, 1.25, Time::asap()).unwrap();
            let n3 = g.push(TestInPlusParamUGen::new()).name("n3");
            g.set(n3, 0, 0.125, Time::asap()).unwrap();
            (n1 >> n2 >> n3).to_graph_out();
        });

        // Block 1
        audio_processor.run_without_inputs();
        let output = audio_processor.output_block();
        assert_eq!(output.read(0, 0), 0.5 + 1.25 + 0.125);

        g.edit(|g| {
            let n1 = g.handle_from_name("n1").unwrap();
            n1.disconnect_output(0);
        });

        // Block 2
        audio_processor.run_without_inputs();
        let output = audio_processor.output_block();
        assert_eq!(output.read(0, 0), 1.25 + 0.125);

        g.edit(|g| {
            let n3 = g.handle_from_name("n3").unwrap();
            n3.disconnect_input(0);
        });
        // Block 3
        audio_processor.run_without_inputs();
        let output = audio_processor.output_block();
        assert_eq!(output.read(0, 0), 0.125);
    }
}
