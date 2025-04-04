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

use core::mem::MaybeUninit;
use core::ops::{BitOr, Div, Shr, Sub};

use crate::core::sync::{Arc, Mutex};

use crate::Time;
use crate::graph::GraphError;
use crate::handle::SchedulingChannelSender;
use crate::node::NodeData;
use crate::{
    SchedulingChannelProducer,
    core::{
        clone::Clone,
        marker::PhantomData,
        ops::{Add, Mul},
        sync::RwLock,
    },
};

use ecow::EcoString;
use knaster_core::math::MathUGen;
use knaster_core::numeric_array::ArrayLength;
use knaster_core::{
    Float, Param, ParameterHint, Size, UGen, numeric_array::NumericArray, typenum::*,
};
use knaster_core::{PFloat, ParameterSmoothing, ParameterValue};
use smallvec::SmallVec;

use crate::{
    connectable::{Channels, NodeOrGraph},
    graph::{Graph, NodeId},
    handle::{AnyHandle, HandleTrait},
};

/// A wrapper around a [`Graph`] that provides access to an ergonomic and interface for adding and
/// connecting nodes in the graph. When the `GraphEdit` is dropped, the changes are committed to the
/// graph.
pub struct GraphEdit<'b, F: Float> {
    graph: RwLock<&'b mut Graph<F>>,
}
impl<'b, F: Float> GraphEdit<'b, F> {
    pub fn new(g: &'b mut Graph<F>) -> Self {
        Self {
            graph: RwLock::new(g),
        }
    }
    /// Create a new node in the graph and return a handle to it.
    pub fn push<'a, T: UGen<Sample = F> + 'static>(&'a self, ugen: T) -> SH<'a, 'b, F, Handle3<T>> {
        let handle = self.graph.write().unwrap().push(ugen);
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
        self.graph.read().unwrap().node_data(id).map(|data| DH {
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
            .unwrap()
            .node_data_from_name(name)
            .map(|(id, data)| DH {
                nodes: DynamicHandle3 { node_id: id, data },
                graph: &self.graph,
            })
    }

    pub fn set(
        &self,
        node: impl Into<NodeId>,
        param: impl Into<Param>,
        value: impl Into<ParameterValue>,
        t: Time,
    ) -> Result<(), GraphError> {
        self.graph.read().unwrap().set(node, param, value, t)?;
        Ok(())
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
impl<'a, F: Float> Drop for GraphEdit<'a, F> {
    fn drop(&mut self) {
        self.graph.write().unwrap().commit_changes().unwrap();
    }
}

/// Static Handle. Wrapper around static sources/sinks.
#[derive(Clone, Copy)]
pub struct SH<'a, 'b, F: Float, T> {
    nodes: T,
    graph: &'a RwLock<&'b mut Graph<F>>,
}
/// Dynamic Handle. Wrapper around dynamic source/sinks
#[derive(Clone, Copy)]
pub struct DH<'a, 'b, F: Float, T> {
    nodes: T,
    graph: &'a RwLock<&'b mut Graph<F>>,
}
impl<'a, 'b, F: Float, S0: Static> SH<'a, 'b, F, S0> {
    pub fn out<N: Size + Copy>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> SH<'a, 'b, F, ChannelsHandle<N>> {
        let mut channels = NumericArray::default();
        for c in source_channels.into() {
            channels[c as usize] = self.nodes.iter_outputs().nth(c as usize).unwrap();
        }
        SH {
            nodes: ChannelsHandle { channels },
            graph: &self.graph,
        }
    }
    /// Connect the output(s) of self to the input(s) of another node or nodes, summing the output
    /// of self with any existing connections.
    pub fn to<S1: Static>(self, n: SH<'a, 'b, F, S1>) -> SH<'a, 'b, F, S1>
    where
        S1::Inputs: Same<S0::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), (sink, sink_channel)) in
            Static::iter_outputs(&self.nodes).zip(Static::iter_inputs(&n.nodes))
        {
            g.connect2(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        n
    }

    /// Connect the output(s) of self to the input(s) of another node or nodes, replacing any
    /// existing connections.
    pub fn to_replace<S1: Static>(self, n: SH<'a, 'b, F, S1>) -> SH<'a, 'b, F, S1>
    where
        S1::Inputs: Same<S0::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), (sink, sink_channel)) in
            Static::iter_outputs(&self.nodes).zip(Static::iter_inputs(&n.nodes))
        {
            g.connect2_replace(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        n
    }

    pub fn to_graph_out(self) {
        let mut g = self.graph.write().unwrap();
        for (i, (source, source_channel)) in Static::iter_outputs(&self.nodes).enumerate() {
            g.connect2(source, source_channel, i as u16, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }

    pub fn to_graph_out_channels<N>(self, sink_channels: impl Into<Channels<N>>)
    where
        N: Size + Same<S0::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), sink_channel) in
            Static::iter_outputs(&self.nodes).zip(sink_channels.into())
        {
            g.connect2(source, source_channel, sink_channel, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
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
            graph: &self.graph,
        }
    }
    pub fn dynamic(self) -> DH<'a, 'b, F, S0::DynamicType> {
        DH {
            nodes: self.nodes.dynamic(self.graph),
            graph: &self.graph,
        }
    }
}
impl<'a, 'b, F: Float, D: Dynamic> DH<'a, 'b, F, D> {
    pub fn out<N: Size>(&self, source_channels: impl Into<Channels<N>>) -> ChannelsHandle<N> {
        todo!()
    }
    /// Connect self to another node or nodes.
    ///
    /// If there is an error connecting the nodes, that error is logged and subsequent connections
    /// are made. If you want to handle
    /// the error, use the `try_to` method instead.
    pub fn to<S: Dynamic>(self, n: DH<'a, 'b, F, S>) -> DH<'a, 'b, F, S> {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), (sink, sink_channel)) in
            self.nodes.iter_outputs().zip(n.nodes.iter_inputs())
        {
            if let Err(e) = g.connect2(source, source_channel, sink_channel, sink) {
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
        let mut g = self.graph.write().unwrap();
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
        let mut g = self.graph.write().unwrap();
        for (i, (source, source_channel)) in self.nodes.iter_outputs().enumerate() {
            if let Err(e) = g.connect2(source, source_channel, i as u16, NodeOrGraph::Graph) {
                log::error!("Failed to connect node to graph output: {e}");
            }
        }
    }

    /// Connect to the graph output(s), selecting graph output channels from the channels provided.
    ///
    /// Any errors are logged and subsequent connections are made.
    pub fn to_graph_out_channels<N: Size>(self, sink_channels: impl Into<Channels<N>>) {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), sink_channel) in
            self.nodes.iter_outputs().zip(sink_channels.into())
        {
            if let Err(e) = g.connect2(source, source_channel, sink_channel, NodeOrGraph::Graph) {
                log::error!("Failed to connect node to graph output: {e}");
            }
        }
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
    pub fn dynamic(self) -> Self {
        self
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
impl<'a, 'b, F: Float, U: UGen<Sample = F>> SH<'a, 'b, F, Handle3<U>> {
    /// Change the name of the node in the [`Graph`].
    pub fn name(self, n: impl Into<EcoString>) -> Self {
        self.graph
            .write()
            .unwrap()
            .set_name(self.nodes.node_id, n.into());
        self
    }
    /// Link the parameter to a node output
    pub fn link<S: Static<Outputs = U1>>(
        self,
        p: impl Into<Param>,
        source: SH<'a, 'b, F, S>,
    ) -> Self {
        let input = source.nodes.iter_outputs().next().unwrap();
        let mut g = self.graph.write().unwrap();
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
    pub fn node_id(self) -> NodeId {
        self.nodes.node_id
    }
    /// Get a parameter from the node this handle points to if it exists.
    pub fn param(self, p: impl Into<Param>) -> Option<Parameter> {
        let p = p.into();
        match p {
            Param::Index(i) => {
                if i < U::Parameters::USIZE {
                    Some(Parameter {
                        node: self.nodes.node_id,
                        param_index: i as u16,
                        sender: self.graph.read().unwrap().scheduling_channel_sender(),
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
                            sender: self.graph.read().unwrap().scheduling_channel_sender(),
                        });
                    }
                }
                None
            }
        }
    }
}
impl<'a, 'b, F: Float> DH<'a, 'b, F, DynamicHandle3> {
    /// Change the name of the node in the [`Graph`].
    pub fn name(self, n: impl Into<EcoString>) -> Self {
        self.graph
            .write()
            .unwrap()
            .set_name(self.nodes.node_id, n.into());
        self
    }
    /// Link the parameter to a node output
    pub fn link<S: Static<Outputs = U1>>(
        self,
        p: impl Into<Param>,
        source: SH<'a, 'b, F, S>,
    ) -> Self {
        let input = source.nodes.iter_outputs().next().unwrap();
        let mut g = self.graph.write().unwrap();
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
    pub fn node_id(self) -> NodeId {
        self.nodes.node_id
    }
    /// Get a parameter from the node this handle points to if it exists.
    pub fn param(self, p: impl Into<Param>) -> Option<Parameter> {
        let p = p.into();
        match p {
            Param::Index(i) => {
                if (i as u16) < self.nodes.data.parameters {
                    Some(Parameter {
                        node: self.nodes.node_id,
                        param_index: i as u16,
                        sender: self.graph.read().unwrap().scheduling_channel_sender(),
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
                            sender: self.graph.read().unwrap().scheduling_channel_sender(),
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

    fn dynamic<F: Float>(&self, graph: &RwLock<&mut Graph<F>>) -> Self::DynamicType {
        let data = graph.read().unwrap().node_data(self.node_id).unwrap();
        DynamicHandle3 {
            node_id: self.node_id,
            data,
        }
    }
}
// Macros for implementing arithmetics on sources with statically known channel configurations
macro_rules! math_gen_fn {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<F: Float, S0: Static, S1: Static>(
            s0: S0,
            s1: S1,
            graph: &RwLock<&mut Graph<F>>,
        ) -> ChannelsHandle<S1::Outputs>
        where
            S0::Outputs: Same<S1::Outputs>,
        {
            let mut out_channels = ChannelIterBuilder::new();
            let mut g = graph.write().unwrap();
            for (s0, s1) in Static::iter_outputs(&s0).zip(s1.iter_outputs()) {
                let mul = g.push(MathUGen::<_, U1, $op>::new());
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
math_gen_fn!(add_sources, knaster_core::math::Add);
math_gen_fn!(sub_sources, knaster_core::math::Sub);
math_gen_fn!(mul_sources, knaster_core::math::Mul);
math_gen_fn!(div_sources, knaster_core::math::Div);

// Macros for implementing arithmetics on sources without statically known channel configurations
macro_rules! math_gen_dynamic_fn {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<F: Float, S0: Dynamic, S1: Dynamic>(
            s0: S0,
            s1: S1,
            graph: &RwLock<&mut Graph<F>>,
        ) -> DynamicChannelsHandle {
            if s0.outputs() != s1.outputs() {
                panic!("The number of outputs of the two sources must be the same");
            }
            let mut out_channels = SmallVec::with_capacity(s0.outputs() as usize);
            let mut g = graph.write().unwrap();
            for (s0, s1) in Dynamic::iter_outputs(&s0).zip(s1.iter_outputs()) {
                let mul = g.push(MathUGen::<_, U1, $op>::new());
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
math_gen_dynamic_fn!(add_sources_dynamic, knaster_core::math::Add);
math_gen_dynamic_fn!(sub_sources_dynamic, knaster_core::math::Sub);
math_gen_dynamic_fn!(mul_sources_dynamic, knaster_core::math::Mul);
math_gen_dynamic_fn!(div_sources_dynamic, knaster_core::math::Div);

// Arithmetics with Handle3 and static types
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
            type Output = DynamicChannelsHandle;

            fn $op_lowercase(self, rhs: DH<'a, 'b, F, S1>) -> Self::Output {
                let graph = self.graph;
                $fn_name(self.nodes, rhs.nodes, graph)
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

#[derive(Copy, Clone)]
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

    fn dynamic<F: Float>(&self, _graph: &RwLock<&mut Graph<F>>) -> Self::DynamicType {
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

    fn dynamic<F: Float>(&self, _graph: &RwLock<&mut Graph<F>>) -> Self::DynamicType {
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

pub trait Static {
    type Outputs: Size;
    type Inputs: Size;
    type DynamicType: Dynamic;
    fn iter_outputs(&self) -> ChannelIter<Self::Outputs>;
    fn iter_inputs(&self) -> ChannelIter<Self::Inputs>;
    fn dynamic<F: Float>(&self, graph: &RwLock<&mut Graph<F>>) -> Self::DynamicType;
}
pub struct ChannelIterBuilder<I: Size> {
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
pub struct ChannelIter<I: Size> {
    channels: NumericArray<(NodeOrGraph, u16), I>,
    current_index: usize,
}
impl ChannelIter<U0> {
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
pub struct DynamicChannelIter {
    channels: SmallVec<[(NodeOrGraph, u16); 1]>,
    current_index: usize,
}
impl DynamicChannelIter {
    pub fn new(channels: SmallVec<[(NodeOrGraph, u16); 1]>) -> Self {
        Self {
            channels,
            current_index: 0,
        }
    }
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

pub trait Dynamic {
    fn iter_outputs(&self) -> DynamicChannelIter;
    fn outputs(&self) -> u16;
    fn iter_inputs(&self) -> DynamicChannelIter;
    fn inputs(&self) -> u16;
    fn dynamic<F: Float>(&self, _graph: &RwLock<&mut Graph<F>>) -> &Self {
        self
    }
}

#[derive(Clone)]
pub struct Parameter {
    pub(crate) node: NodeId,
    pub(crate) param_index: u16,
    /// Allows us to send parameter changes straight to the audio thread
    sender: SchedulingChannelSender,
}
impl Parameter {
    pub fn set(&mut self, value: impl Into<ParameterValue>, t: Time) -> Result<(), GraphError> {
        let value = value.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(value),
            smoothing: None,
            token: None,
            time: Some(t),
        })?;
        Ok(())
    }
    pub fn smooth(&mut self, s: impl Into<ParameterSmoothing>, t: Time) -> Result<(), GraphError> {
        let s = s.into();
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: None,
            smoothing: Some(s),
            token: None,
            time: Some(t),
        })?;
        Ok(())
    }
    pub fn trig(&mut self, t: Time) -> Result<(), GraphError> {
        self.sender.send(crate::SchedulingEvent {
            node_key: self.node.key(),
            parameter: self.param_index as usize,
            value: Some(ParameterValue::Trigger),
            smoothing: None,
            token: None,
            time: Some(t),
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
    use core::ops::Mul;

    use crate::{
        Time,
        graph::{GraphOptions, NodeId},
        runner::{Runner, RunnerOptions},
    };
    use knaster_core::{
        Seconds, noise::WhiteNoise, onepole::OnePoleLpf, osc::SinWt, pan::Pan2, typenum::*,
        util::Constant, wrappers_core::UGenWrapperCoreExt,
    };

    use super::{Dynamic, GraphEdit, Static};
    #[test]
    fn scope() {
        let block_size = 16;
        let (mut graph, mut runner) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
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
                let d = (a / c - c) + c * c;
                let e = (d | d) - (c | c);
                ((a >> lpf >> pan >> (lpf_l | lpf_r)) * (amp | amp)).to_graph_out();
                lpf.to_graph_out_channels(1);

                let exciter_amp = graph.push(Constant::new(0.5));
                let exciter = graph.push(SinWt::new(2000.).wr_mul(0.1));
                let noise_mix = graph.push(Constant::new(0.25));
                let noise = graph.push(WhiteNoise::new());
                let exciter_lpf = graph.push(OnePoleLpf::new(2600.));
                let ex = exciter * exciter_amp;
                ((noise * noise_mix * ex + ex) >> exciter_lpf).to_graph_out_channels([1]);

                let t = Time::absolute(Seconds::from_secs_f64(2.5));
                // p.set("freq", 400., t).unwrap();
                let mut freq = sine.param("freq").unwrap();
                freq.set(400., t).unwrap();

                let freq = sine.param("freq").unwrap();
                // s = Some(sine);
                let c = graph.push(Constant::new(0.2));
                let c = sine * sine2 * c;
                (sine.node_id(), lpf.node_id())
            }
        });
        graph.edit(|graph| {
            // do a bunch of stuff
            {
                // Retreive a handle to the node
                let handle = graph.handle(kept_sine).unwrap();
                let handle2 = graph.handle_from_name("MySine").unwrap();
                assert_eq!(handle.node_id(), handle2.node_id());
                let lpf = graph.handle(kept_lpf).unwrap();
                // Use the handle to connect it to another node
                let new_sine = graph.push(SinWt::new(200.));
                let new_pan = graph.push(Pan2::new(0.));
                let a = (handle * new_sine) >> new_pan >> (lpf | lpf | new_pan);
                let a = (new_sine * handle) >> new_pan;
                (new_pan + (handle2 | handle2)).to(lpf | lpf | new_pan);
                a.to_graph_out();
                (new_sine + handle) >> new_pan;
            }
        });
    }

    #[test]
    fn connectable3() {
        let block_size = 16;
        let (mut graph, mut runner) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
        });
        let sine = graph.push(SinWt::new(200.));
        // Remake the following into something easier to read:
        /*
        let exciter_amp = g.push(Constant::new(0.5));
        let exciter = g.push(HalfSineWt::new(2000.).wr_mul(0.1));
        let noise_mix = g.push(Constant::new(0.25));
        let noise = g.push(WhiteNoise::new());
        let exciter_lpf = g.push(OnePoleLpf::new(2600.));
        let en = ugen_mul(&exciter, &exciter_amp, g)?;
        let en2 = ugen_mul(&noise, &noise_mix, g)?;
        let en3 = ugen_mul(&en, &en2, g)?;
        let add = ugen_add(&en, &en3, g)?;
        g.connect(&add, 0, 0, &exciter_lpf)?;
        */
        // Could nodes be named inline, e.g. with a `name("exciter_amp")`. We need named nodes for
        // two reasons:
        // 1 connecting between them, audio and parameters
        // 2 manually changing parameter values later
        //
        // For 1, the connections can almost always be made in a chain with some parallel tracks.
        // Sometimes we need to add nodes later, especially whole sub-graphs going into effects.
        //
        // For 2, this would be neater in a larger Synth like interface since there is no type
        // safety for parameters anyway.
        //
        // graph.edit().push(Constant::new(0.5)).name("exciter_amp_node").store_param(0, "exciter_amp");
        // graph.edit().push(Constant::new(0.5)).name("exciter_amp_node").store_param(0, "exciter_amp");
        // let exciter_amp = graph.node("exciter_amp_node")?;
        // graph.edit().connect(exciter_amp).to(SinWt::new(2000.)).name("sine").store_param("freq",
        // "freq");
        // graph.set("exciter_amp")?.value(0.2).smoothing(Linear(0.5));
        // graph.set("freq")?.value().smoothing(Linear(0.5));

        // let exciter_amp = graph.push(Constant::new(0.5));
        // let noise_mix = graph.push(Constant::new(0.25));
        // let exc = (graph.connect_from_new(SinWt::new(2000.).wr_mul(0.1)) * exciter_amp).inner();
        //
        // let noise = graph.connect_from_new(WhiteNoise::new()) * noise_mix;
        // let c = ((noise * exc) + exc).to_new(OnePoleLpf::new(2600.));
        // let lpf = c.handle();
        // let c = c.to_new(Pan2::new(0.0));
        // let pan = c.handle();
        // c.to_graph_out([0, 1]);

        graph.commit_changes().unwrap();
        assert_eq!(graph.inspection().nodes.len(), 1);
        for _ in 0..10 {
            unsafe {
                runner.run(&[]);
            }
        }
    }
}
