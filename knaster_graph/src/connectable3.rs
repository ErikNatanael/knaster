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
pub struct GraphEdit<F: Float> {
    graph: RwLock<Graph<F>>,
}
impl<F: Float> GraphEdit<F> {
    pub fn new(g: Graph<F>) -> Self {
        Self {
            graph: RwLock::new(g),
        }
    }
    /// Create a new node in the graph and return a handle to it.
    pub fn push<T: UGen<Sample = F> + 'static>(&self, ugen: T) -> Handle3<T> {
        let handle = self.graph.write().unwrap().push(ugen);
        let node_id = handle.node_id();
        Handle3 {
            node_id,
            ugen: PhantomData,
            graph: &self.graph,
        }
    }
    /// Get a non typesafe handle to node with the given [`NodeId`] if it exists.
    pub fn handle(&self, id: impl Into<NodeId>) -> Option<DynamicHandle3<'_, F>> {
        let id = id.into();
        self.graph
            .read()
            .unwrap()
            .node_data(id)
            .map(|data| DynamicHandle3 {
                node_id: id,
                graph: &self.graph,
                data,
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
impl<F: Float> Drop for GraphEdit<F> {
    fn drop(&mut self) {
        self.graph.write().unwrap().commit_changes().unwrap();
    }
}
// pub trait Connect: Sized {
//     type Outputs: Size;
//
//     fn to<S: Sink3>(self, n: S) -> S
//     where
//         S::Inputs: Same<Self::Outputs>;
//     /// Connect to the graph output(s) in the order the channels are produced.
//     fn to_graph_out(self);
//
//     /// Connect to the graph output(s), selecting graph output channels from the channels provided
//     fn to_graph_out_channels<N>(&self, sink_channels: impl Into<Channels<N>>)
//     where
//         N: Size + Same<Self::Outputs>;
// }
/// Handle to a node with a lifetime connected to Graph3
#[derive(Clone, Copy)]
pub struct DynamicHandle3<'a, F: Float> {
    node_id: NodeId,
    graph: &'a RwLock<Graph<F>>,
    data: NodeData,
}
impl<'a, F: Float> DynamicHandle3<'a, F> {
    /// Connect this handle to another handle, returning a [`Stack`] which can be used to connect
    /// to other handles.
    ///
    /// This is useful for connecting multiple outputs of a single node to multiple nodes or vice
    /// versa.
    fn stack<S: DynamicSource3<'a> + DynamicSink3>(self, s: S) -> DynamicChannelsHandle<'a, F> {
        let mut in_channels = SmallVec::with_capacity((self.inputs() + s.inputs()) as usize);
        let mut out_channels = SmallVec::with_capacity((self.outputs() + s.outputs()) as usize);
        for chan in DynamicSink3::iter(&self) {
            in_channels.push(chan);
        }
        for chan in DynamicSink3::iter(&s) {
            in_channels.push(chan);
        }
        for chan in DynamicSource3::iter(&self) {
            out_channels.push(chan);
        }
        for chan in DynamicSource3::iter(&s) {
            out_channels.push(chan);
        }
        DynamicChannelsHandle {
            graph: self.graph,
            in_channels,
            out_channels,
            _float: PhantomData,
        }
    }
}
impl<'a, F: Float> DynamicSource3<'a> for DynamicHandle3<'a, F> {
    type Sample = F;

    fn iter(&self) -> DynamicChannelIter {
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

    fn out<N: Size>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> ChannelsHandle<'a, Self::Sample, N> {
        todo!()
    }

    fn to_graph_out(self) {
        todo!()
    }

    fn to_graph_out_channels<N: Size>(self, sink_channels: impl Into<Channels<N>>) {
        todo!()
    }

    fn to<S: DynamicSink3>(self, n: S) -> DynamicChannelsHandle<'a, Self::Sample> {
        todo!()
    }
}
impl<'a, F: Float> DynamicSink3 for DynamicHandle3<'a, F> {
    fn iter(&self) -> DynamicChannelIter {
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
pub struct Handle3<'a, U: UGen> {
    node_id: NodeId,
    ugen: PhantomData<U>,
    graph: &'a RwLock<Graph<U::Sample>>,
}
impl<'a, U: UGen> Handle3<'a, U> {
    /// Change the name of the node in the [`Graph`].
    pub fn name(self, n: impl Into<EcoString>) -> Self {
        self.graph.write().unwrap().set_name(self.node_id, n.into());
        self
    }
    /// Link the parameter to a node output
    pub fn link<S: Source3<'a, Sample = U::Sample, Outputs = U1>>(
        self,
        p: impl Into<Param>,
        source: S,
    ) -> Self {
        let input = source.iter().next().unwrap();
        let mut g = self.graph.write().unwrap();
        if let NodeOrGraph::Node(source_node) = input.0 {
            if let Err(e) = g.connect_replace_to_parameter(source_node, input.1, p, self.node_id) {
                log::error!("Failed to connect signal to parameter: {e}");
            }
        } else {
            log::error!(
                "Graph input provided as input to a parameter. This is not currently supported. Connection ignored."
            );
        }
        self
    }

    /// Converts the handle into less typed DynamicHandle3. Returns None if the node no longer exists in the
    /// Graph and therefore cannot be converted.
    pub fn dynamic(self) -> Option<DynamicHandle3<'a, U::Sample>> {
        todo!()
    }
    /// Returns the [`NodeId`] of the node this handle points to.
    pub fn node_id(self) -> NodeId {
        self.node_id
    }
    /// Get a parameter from the node this handle points to if it exists.
    pub fn param(self, p: impl Into<Param>) -> Option<Parameter> {
        let p = p.into();
        match p {
            Param::Index(i) => {
                if i < U::Parameters::USIZE {
                    Some(Parameter {
                        node: self.node_id,
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
                            node: self.node_id,
                            param_index: i as u16,
                            sender: self.graph.read().unwrap().scheduling_channel_sender(),
                        });
                    }
                }
                None
            }
        }
    }

    /// Connect this handle to another handle, returning a [`Stack`] which can be used to connect
    /// to other handles.
    ///
    /// This is useful for connecting multiple outputs of a single node to multiple nodes or vice
    /// versa.
    fn stack<S: Source3<'a> + Sink3>(self, s: S) -> Stack<'a, U::Sample, Handle3<'a, U>, S> {
        Stack {
            s0: self,
            s1: s,
            graph: self.graph,
            _float: PhantomData,
        }
    }
}
// Manual Clone and Copy impls necessary because of PhantomData
impl<U: UGen> Clone for Handle3<'_, U> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<U: UGen> Copy for Handle3<'_, U> {}
impl<U: UGen> From<Handle3<'_, U>> for NodeId {
    fn from(value: Handle3<'_, U>) -> Self {
        value.node_id
    }
}
impl<'a, U: UGen> Source3<'a> for Handle3<'a, U> {
    type Sample = U::Sample;
    type Outputs = U::Outputs;

    fn iter(&self) -> ChannelIter<Self::Outputs> {
        let mut channels = ChannelIterBuilder::new();
        for i in 0..U::Outputs::U16 {
            channels.push(NodeOrGraph::Node(self.node_id()), i);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }
    fn out<N: Size>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> ChannelsHandle<'a, Self::Sample, N> {
        let mut channels = NumericArray::default();
        for c in source_channels.into() {
            channels[c as usize] = <Self as Source3<'a>>::iter(self).nth(c as usize).unwrap();
        }
        ChannelsHandle {
            graph: self.graph,
            channels,
            _float: PhantomData,
        }
    }

    fn to<S: Sink3>(self, sink: S) -> S
    where
        S::Inputs: Same<Self::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), (sink, sink_channel)) in
            Source3::iter(&self).zip(Sink3::iter(&sink))
        {
            g.connect2(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        sink
    }

    fn to_graph_out(self) {
        let mut g = self.graph.write().unwrap();
        for (i, (source, source_channel)) in Source3::iter(&self).enumerate() {
            g.connect2(source, source_channel, i as u16, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }

    fn to_graph_out_channels<N>(self, sink_channels: impl Into<Channels<N>>)
    where
        N: Size + Same<Self::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), sink_channel) in
            Source3::iter(&self).zip(sink_channels.into())
        {
            g.connect2(source, source_channel, sink_channel, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }
}
impl<U: UGen> Sink3 for Handle3<'_, U> {
    type Inputs = U::Inputs;

    fn iter(&self) -> ChannelIter<Self::Inputs> {
        let mut channels = ChannelIterBuilder::new();
        for i in 0..U::Inputs::U16 {
            channels.push(NodeOrGraph::Node(self.node_id()), i);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }
}
// impl<U: UGen> DynamicSink3 for Handle3<'_, U> {
//     fn inputs(&self) -> u16 {
//         U::Inputs::U16
//     }
//
//     fn iter(&self) -> DynamicChannelIter {
//         let mut channels = SmallVec::with_capacity(self.inputs() as usize);
//         for i in 0..self.inputs() {
//             channels.push((NodeOrGraph::Node(self.node_id()), i));
//         }
//         DynamicChannelIter {
//             channels,
//             current_index: 0,
//         }
//     }
// }
// impl<'a, U: UGen> DynamicSource3<'a> for Handle3<'a, U> {
//     type Sample = U::Sample;
//
//     fn iter(&self) -> DynamicChannelIter {
//         let mut channels = SmallVec::with_capacity(self.inputs() as usize);
//         for i in 0..self.outputs() {
//             channels.push((NodeOrGraph::Node(self.node_id()), i));
//         }
//         DynamicChannelIter {
//             channels,
//             current_index: 0,
//         }
//     }
//
//     fn outputs(&self) -> u16 {
//         U::Outputs::U16
//     }
//
//     fn out<N: Size>(
//         &self,
//         source_channels: impl Into<Channels<N>>,
//     ) -> ChannelsHandle<'a, Self::Sample, N> {
//         let mut channels = NumericArray::default();
//         for c in source_channels.into() {
//             channels[c as usize] = <Self as DynamicSource3<'a>>::iter(self)
//                 .nth(c as usize)
//                 .unwrap();
//         }
//         ChannelsHandle {
//             graph: self.graph,
//             channels,
//             _float: PhantomData,
//         }
//     }
//
//     fn to_graph_out(self) {
//         todo!()
//     }
//
//     fn to_graph_out_channels<N: Size>(self, sink_channels: impl Into<Channels<N>>) {
//         todo!()
//     }
//
//     fn to<S: DynamicSink3>(self, n: S) -> DynamicChannelsHandle<'a, Self::Sample> {
//         todo!()
//     }
// }

// Macros for implementing arithmetics on sources with statically known channel configurations
macro_rules! math_gen_fn {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<'a, S0: Source3<'a>, S1: Source3<'a>>(
            s0: S0,
            s1: S1,
            graph: &RwLock<Graph<S0::Sample>>,
        ) -> ChannelsHandle<'_, S0::Sample, S1::Outputs>
        where
            S0::Outputs: Same<S1::Outputs>,
        {
            let mut out_channels = ChannelIterBuilder::new();
            let mut g = graph.write().unwrap();
            for (i, (s0, s1)) in Source3::iter(&s0).zip(s1.iter()).enumerate() {
                let mul = g.push(MathUGen::<_, U1, $op>::new());
                if let Err(e) = g.connect2(s0.0, s0.1, 0, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                if let Err(e) = g.connect2(s1.0, s1.1, 1, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                out_channels.push(NodeOrGraph::Node(mul.node_id()), i as u16);
            }
            let channels = out_channels
                .into_channel_iter()
                .expect("all the channels should be initialised");
            ChannelsHandle {
                graph,
                channels: channels.channels,
                _float: PhantomData,
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
        fn $fn_name<'a, S0: DynamicSource3<'a>, S1: DynamicSource3<'a>>(
            s0: S0,
            s1: S1,
            graph: &'a RwLock<Graph<S0::Sample>>,
        ) -> DynamicChannelsHandle<'a, S0::Sample> {
            if s0.outputs() != s1.outputs() {
                panic!("The number of outputs of the two sources must be the same");
            }
            let mut out_channels = SmallVec::with_capacity(s0.outputs() as usize);
            let mut g = graph.write().unwrap();
            for (i, (s0, s1)) in DynamicSource3::iter(&s0).zip(s1.iter()).enumerate() {
                let mul = g.push(MathUGen::<_, U1, $op>::new());
                if let Err(e) = g.connect2(s0.0, s0.1, 0, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                if let Err(e) = g.connect2(s1.0, s1.1, 1, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                out_channels.push((NodeOrGraph::Node(mul.node_id()), i as u16));
            }
            DynamicChannelsHandle {
                graph,
                in_channels: SmallVec::new(),
                out_channels,
                _float: PhantomData,
            }
        }
    };
}
math_gen_dynamic_fn!(add_sources_dynamic, knaster_core::math::Add);
math_gen_dynamic_fn!(sub_sources_dynamic, knaster_core::math::Sub);
math_gen_dynamic_fn!(mul_sources_dynamic, knaster_core::math::Mul);
math_gen_dynamic_fn!(div_sources_dynamic, knaster_core::math::Div);

// Arithmetics with Handle3 and static types
macro_rules! math_impl_handle3 {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, F: Float, U0: UGen<Sample = F>, S: Source3<'a>> $op<S> for Handle3<'a, U0>
        where
            U0::Outputs: Same<S::Outputs>,
        {
            type Output = ChannelsHandle<'a, F, S::Outputs>;

            fn $op_lowercase(self, rhs: S) -> Self::Output {
                let graph = self.graph;
                $fn_name(self, rhs, graph)
            }
        }
    };
}
math_impl_handle3!(mul_sources, Mul, mul);
math_impl_handle3!(add_sources, Add, add);
math_impl_handle3!(sub_sources, Sub, sub);
math_impl_handle3!(div_sources, Div, div);

impl<'a, F: Float, U0: UGen<Sample = F>, S: Sink3> Shr<S> for Handle3<'a, U0>
where
    S::Inputs: Same<U0::Outputs>,
{
    type Output = S;

    fn shr(self, rhs: S) -> Self::Output {
        <Self as Source3<'a>>::to(self, rhs)
    }
}
impl<'a, F: Float, U0: UGen<Sample = F>, S: Sink3 + Source3<'a>> BitOr<S> for Handle3<'a, U0> {
    type Output = Stack<'a, F, Self, S>;

    fn bitor(self, rhs: S) -> Self::Output {
        self.stack(rhs)
    }
}
// Static Handle3 and DynamicSource3 impls
//
macro_rules! math_impl_handle3_dynamic {
    ($fn_name:ident, $op:ident, $op_lowercase:ident, $ty:ty) => {
        impl<'a, F: Float, U0: UGen<Sample = F>> $op<$ty> for Handle3<'a, U0> {
            type Output = DynamicChannelsHandle<'a, F>;

            fn $op_lowercase(self, rhs: $ty) -> Self::Output {
                let dynamic_handle = self
                    .dynamic()
                    .expect("Node handle should be valid for as long as the current edit cycle is in effect");
                let graph = self.graph;
                $fn_name(dynamic_handle, rhs, graph)
            }
        }
    };
}
math_impl_handle3_dynamic!(mul_sources_dynamic, Mul, mul, DynamicHandle3<'a, F>);
math_impl_handle3_dynamic!(add_sources_dynamic, Add, add, DynamicHandle3<'a, F>);
math_impl_handle3_dynamic!(sub_sources_dynamic, Sub, sub, DynamicHandle3<'a, F>);
math_impl_handle3_dynamic!(div_sources_dynamic, Div, div, DynamicHandle3<'a, F>);

math_impl_handle3_dynamic!(mul_sources_dynamic, Mul, mul, DynamicChannelsHandle<'a, F>);
math_impl_handle3_dynamic!(add_sources_dynamic, Add, add, DynamicChannelsHandle<'a, F>);
math_impl_handle3_dynamic!(sub_sources_dynamic, Sub, sub, DynamicChannelsHandle<'a, F>);
math_impl_handle3_dynamic!(div_sources_dynamic, Div, div, DynamicChannelsHandle<'a, F>);

// DynamicHandle3 and DynamicSource3 impls
//
macro_rules! math_impl_dynamic_handle3_dynamic {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, F: Float, S: DynamicSource3<'a>> $op<S> for DynamicHandle3<'a, F> {
            type Output = DynamicChannelsHandle<'a, F>;

            fn $op_lowercase(self, rhs: S) -> Self::Output {
                let graph = self.graph;
                $fn_name(self, rhs, graph)
            }
        }
    };
}
math_impl_dynamic_handle3_dynamic!(mul_sources_dynamic, Mul, mul);
math_impl_dynamic_handle3_dynamic!(add_sources_dynamic, Add, add);
math_impl_dynamic_handle3_dynamic!(sub_sources_dynamic, Sub, sub);
math_impl_dynamic_handle3_dynamic!(div_sources_dynamic, Div, div);

impl<'a, F: Float, S: DynamicSink3> Shr<S> for DynamicHandle3<'a, F> {
    type Output = DynamicChannelsHandle<'a, F>;

    fn shr(self, rhs: S) -> Self::Output {
        self.to(rhs)
    }
}
impl<'a, F: Float, S: DynamicSink3 + DynamicSource3<'a>> BitOr<S> for DynamicHandle3<'a, F> {
    type Output = DynamicChannelsHandle<'a, F>;

    fn bitor(self, rhs: S) -> Self::Output {
        self.stack(rhs)
    }
}
// DynamicHandle3 and Handle3 arithmetics impls
macro_rules! math_impl_dynamic_handle3_type {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, F: Float, U0: UGen> $op<Handle3<'a, U0>> for DynamicHandle3<'a, F> {
            type Output = DynamicChannelsHandle<'a, F>;

            fn $op_lowercase(self, rhs: Handle3<'a, U0>) -> Self::Output {
                let graph = self.graph;
                let dynamic_handle = rhs.dynamic().unwrap();
                $fn_name(self, dynamic_handle, graph)
            }
        }
    };
}
math_impl_dynamic_handle3_type!(mul_sources_dynamic, Mul, mul);
math_impl_dynamic_handle3_type!(add_sources_dynamic, Add, add);
math_impl_dynamic_handle3_type!(sub_sources_dynamic, Sub, sub);
math_impl_dynamic_handle3_type!(div_sources_dynamic, Div, div);

#[derive(Copy, Clone)]
pub struct Stack<'a, F: Float, S0, S1> {
    s0: S0,
    s1: S1,
    graph: &'a RwLock<Graph<F>>,
    _float: PhantomData<F>,
}
// impl<F: Float, S0: Source3 + Sink3, S1: Source3 + Sink3> Connect for Stack<'_, F, S0, S1>
// where
//     <S0::Outputs as Add<S1::Outputs>>::Output: Size,
//     <S0 as Source3>::Outputs: core::ops::Add<<S1 as Source3>::Outputs>,
// {
//     type Outputs = <Self as Source3>::Outputs;
//
// }
//
impl<'a, F: Float, S0: Sink3, S1: Sink3> Stack<'a, F, S0, S1> {
    pub fn stack<S: Source3<'a> + Sink3>(self, s: S) -> Stack<'a, F, Self, S> {
        let graph = self.graph;
        Stack {
            s0: self,
            s1: s,
            graph,
            _float: PhantomData,
        }
    }
}

impl<'a, F: Float, S0: Sink3, S1: Sink3> Sink3 for Stack<'a, F, S0, S1>
where
    <S0::Inputs as Add<S1::Inputs>>::Output: Size,
    <S0 as Sink3>::Inputs: core::ops::Add<<S1 as Sink3>::Inputs>,
{
    type Inputs = <S0::Inputs as Add<S1::Inputs>>::Output;

    fn iter(&self) -> ChannelIter<Self::Inputs> {
        let mut channels = ChannelIterBuilder::new();
        for (node, index) in Sink3::iter(&self.s0).chain(Sink3::iter(&self.s1)) {
            channels.push(node, index);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }
}
impl<'a, F: Float, S0: Source3<'a>, S1: Source3<'a>> Source3<'a> for Stack<'a, F, S0, S1>
where
    <S0::Outputs as Add<S1::Outputs>>::Output: Size,
    <S0 as Source3<'a>>::Outputs: core::ops::Add<<S1 as Source3<'a>>::Outputs>,
{
    type Sample = F;

    type Outputs = <S0::Outputs as Add<S1::Outputs>>::Output;

    fn iter(&self) -> ChannelIter<Self::Outputs> {
        let mut channels = ChannelIterBuilder::new();
        for (node, index) in Source3::iter(&self.s0).chain(Source3::iter(&self.s1)) {
            channels.push(node, index);
        }
        channels
            .into_channel_iter()
            .expect("all the channels should be initialised")
    }

    fn out<N: Size>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> ChannelsHandle<'a, Self::Sample, N> {
        let mut channels = ChannelIterBuilder::new();
        for index in source_channels.into() {
            let (node, i) = Source3::iter(self).nth(index as usize).expect("");
            channels.push(node, i);
        }
        let channels = channels
            .into_channel_iter()
            .expect("all the channels should be initialised");
        ChannelsHandle {
            graph: self.graph,
            channels: channels.channels,
            _float: PhantomData,
        }
    }

    fn to<S: Sink3>(self, sink: S) -> S
    where
        S::Inputs: Same<Self::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), (sink, sink_channel)) in
            Source3::iter(&self).zip(Sink3::iter(&sink))
        {
            g.connect2(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        sink
    }

    fn to_graph_out(self) {
        let mut g = self.graph.write().unwrap();
        for (i, (source, source_channel)) in Source3::iter(&self).enumerate() {
            g.connect2(source, source_channel, i as u16, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }

    fn to_graph_out_channels<N>(self, sink_channels: impl Into<Channels<N>>)
    where
        N: Size + Same<Self::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), sink_channel) in
            Source3::iter(&self).zip(sink_channels.into())
        {
            g.connect2(source, source_channel, sink_channel, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }
}

// Stack arithmetics impls
macro_rules! math_impl_stack {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, F: Float, S0: Source3<'a> + Sink3, S1: Source3<'a> + Sink3, S: Source3<'a>> $op<S>
            for Stack<'a, F, S0, S1>
        where
            <Self as Source3<'a>>::Outputs: Same<S::Outputs>,
            <S0 as Source3<'a>>::Outputs: core::ops::Add<<S1 as Source3<'a>>::Outputs>,
            <<S0 as Source3<'a>>::Outputs as core::ops::Add<<S1 as Source3<'a>>::Outputs>>::Output:
                Size,
        {
            type Output = ChannelsHandle<'a, F, S::Outputs>;

            fn $op_lowercase(self, rhs: S) -> Self::Output {
                let graph = self.graph;
                $fn_name(self, rhs, graph)
            }
        }
    };
}
math_impl_stack!(mul_sources, Mul, mul);
math_impl_stack!(add_sources, Add, add);
math_impl_stack!(sub_sources, Sub, sub);
math_impl_stack!(div_sources, Div, div);
impl<'a, F: Float, S0: Source3<'a> + Sink3, S1: Source3<'a> + Sink3, S: Sink3> Shr<S>
    for Stack<'a, F, S0, S1>
where
    <Self as Source3<'a>>::Outputs: Same<S::Inputs>,
    <S0 as Source3<'a>>::Outputs: core::ops::Add<<S1 as Source3<'a>>::Outputs>,
    <<S0 as Source3<'a>>::Outputs as core::ops::Add<<S1 as Source3<'a>>::Outputs>>::Output: Size,
    <S as Sink3>::Inputs:
        Same<<<S0 as Source3<'a>>::Outputs as Add<<S1 as Source3<'a>>::Outputs>>::Output>,
{
    type Output = S;

    fn shr(self, rhs: S) -> Self::Output {
        self.to(rhs)
    }
}
impl<'a, F: Float, S0: Source3<'a> + Sink3, S1: Source3<'a> + Sink3, S: Sink3 + Source3<'a>>
    BitOr<S> for Stack<'a, F, S0, S1>
where
    <Self as Source3<'a>>::Outputs: Same<S::Inputs>,
    <S0 as Source3<'a>>::Outputs: core::ops::Add<<S1 as Source3<'a>>::Outputs>,
    <<S0 as Source3<'a>>::Outputs as core::ops::Add<<S1 as Source3<'a>>::Outputs>>::Output: Size,
    <S as Sink3>::Inputs:
        Same<<<S0 as Source3<'a>>::Outputs as Add<<S1 as Source3<'a>>::Outputs>>::Output>,
{
    type Output = Stack<'a, F, Self, S>;

    fn bitor(self, rhs: S) -> Self::Output {
        self.stack(rhs)
    }
}

#[derive(Clone)]
pub struct ChannelsHandle<'a, F: Float, O: Size> {
    graph: &'a RwLock<Graph<F>>,
    channels: NumericArray<(NodeOrGraph, u16), O>,
    _float: PhantomData<F>,
}
// // Copy workaround, see the `ArrayLength` docs for more info.
impl<F: Float, O: Size> Copy for ChannelsHandle<'_, F, O> where
    <O as knaster_core::numeric_array::ArrayLength>::ArrayType<(NodeOrGraph, u16)>:
        core::marker::Copy
{
}
// impl<F: Float, O: Size> Connect for ChannelsHandle<'_, F, O> {
//     type Outputs = O;
//
// }
// Implementing for ChannelsHandle to enable them to stack, even though they have no inputs
impl<'a, F: Float, O: Size> ChannelsHandle<'a, F, O> {
    fn stack<S: Source3<'a> + Sink3>(self, s: S) -> Stack<'a, F, Self, S> {
        let graph = self.graph;
        Stack {
            s0: self,
            s1: s,
            graph,
            _float: PhantomData,
        }
    }
}
impl<F: Float, O: Size> Sink3 for ChannelsHandle<'_, F, O> {
    type Inputs = U0;

    fn iter(&self) -> ChannelIter<Self::Inputs> {
        ChannelIter::empty()
    }
}
impl<'a, F: Float, O: Size> Source3<'a> for ChannelsHandle<'a, F, O> {
    type Sample = F;
    type Outputs = O;

    fn iter(&self) -> ChannelIter<Self::Outputs> {
        ChannelIter {
            channels: self.channels.clone(),
            current_index: 0,
        }
    }
    fn out<N: Size>(&self, source_channels: impl Into<Channels<N>>) -> ChannelsHandle<'a, F, N> {
        let mut channels = NumericArray::default();
        for c in source_channels.into() {
            channels[c as usize] = <Self as Source3<'a>>::iter(self).nth(c as usize).unwrap();
        }
        ChannelsHandle {
            graph: self.graph,
            channels,
            _float: PhantomData,
        }
    }
    fn to<S: Sink3>(self, sink: S) -> S
    where
        S::Inputs: Same<Self::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), (sink, sink_channel)) in
            Source3::iter(&self).zip(Sink3::iter(&sink))
        {
            g.connect2(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        sink
    }

    fn to_graph_out(self) {
        let mut g = self.graph.write().unwrap();
        for (i, (source, source_channel)) in Source3::iter(&self).enumerate() {
            g.connect2(source, source_channel, i as u16, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }

    fn to_graph_out_channels<N>(self, sink_channels: impl Into<Channels<N>>)
    where
        N: Size + Same<Self::Outputs>,
    {
        let mut g = self.graph.write().unwrap();
        for ((source, source_channel), sink_channel) in
            Source3::iter(&self).zip(sink_channels.into())
        {
            g.connect2(source, source_channel, sink_channel, NodeOrGraph::Graph)
                .expect("Error connecting to graph output channel.");
        }
    }
}
// ChannelsHandle arithmetics impls
macro_rules! math_impl_channels_handle {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, F: Float, O: Size, S: Source3<'a, Outputs = O>> $op<S> for ChannelsHandle<'a, F, O>
        where
            S::Outputs: Same<O>,
        {
            type Output = ChannelsHandle<'a, F, O>;

            fn $op_lowercase(self, rhs: S) -> Self::Output {
                let graph = self.graph;
                $fn_name(self, rhs, graph)
            }
        }
    };
}
math_impl_channels_handle!(mul_sources, Mul, mul);
math_impl_channels_handle!(add_sources, Add, add);
math_impl_channels_handle!(sub_sources, Sub, sub);
math_impl_channels_handle!(div_sources, Div, div);

impl<'a, F: Float, O: Size, S: Sink3> Shr<S> for ChannelsHandle<'a, F, O>
where
    S::Inputs: Same<O>,
{
    type Output = S;

    fn shr(self, rhs: S) -> Self::Output {
        self.to(rhs)
    }
}

impl<'a, F: Float, O: Size, S: Source3<'a> + Sink3> BitOr<S> for ChannelsHandle<'a, F, O>
where
    S::Outputs: Same<O>,
{
    type Output = Stack<'a, F, Self, S>;

    fn bitor(self, rhs: S) -> Self::Output {
        self.stack(rhs)
    }
}

// DynamicChannelsHandle arithmetics impls
macro_rules! math_impl_dynamic_channels_handle {
    ($fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, F: Float, S: DynamicSource3<'a>> $op<S> for DynamicChannelsHandle<'a, F> {
            type Output = DynamicChannelsHandle<'a, F>;

            fn $op_lowercase(self, rhs: S) -> Self::Output {
                let graph = self.graph;
                $fn_name(self, rhs, graph)
            }
        }
    };
}
math_impl_dynamic_channels_handle!(mul_sources_dynamic, Mul, mul);
math_impl_dynamic_channels_handle!(add_sources_dynamic, Add, add);
math_impl_dynamic_channels_handle!(sub_sources_dynamic, Sub, sub);
math_impl_dynamic_channels_handle!(div_sources_dynamic, Div, div);

impl<'a, F: Float, S: DynamicSink3> Shr<S> for DynamicChannelsHandle<'a, F> {
    type Output = DynamicChannelsHandle<'a, F>;

    fn shr(self, rhs: S) -> Self::Output {
        self.to(rhs)
    }
}

impl<'a, F: Float, S: DynamicSource3<'a> + DynamicSink3> BitOr<S> for DynamicChannelsHandle<'a, F> {
    type Output = DynamicChannelsHandle<'a, F>;

    fn bitor(self, rhs: S) -> Self::Output {
        self.stack(rhs)
    }
}

/// Handle to any combination of input and output channels within a graph, without
/// type information. Unlike [`ChannelsHandle`], this type represents all kinds of
/// collections of channels that aren't direct references to specific nodes.
#[derive(Clone)]
pub struct DynamicChannelsHandle<'a, F: Float> {
    graph: &'a RwLock<Graph<F>>,
    in_channels: SmallVec<[(NodeOrGraph, u16); 1]>,
    out_channels: SmallVec<[(NodeOrGraph, u16); 1]>,
    _float: PhantomData<F>,
}
impl<'a, F: Float> DynamicChannelsHandle<'a, F> {
    fn stack<S: DynamicSource3<'a> + DynamicSink3>(mut self, s: S) -> DynamicChannelsHandle<'a, F> {
        for chan in DynamicSink3::iter(&s) {
            self.in_channels.push(chan);
        }
        for chan in DynamicSource3::iter(&s) {
            self.out_channels.push(chan);
        }
        self
    }
}
impl<'a, F: Float> DynamicSource3<'a> for DynamicChannelsHandle<'a, F> {
    type Sample = F;

    fn iter(&self) -> DynamicChannelIter {
        DynamicChannelIter {
            channels: self.out_channels.clone(),
            current_index: 0,
        }
    }

    fn outputs(&self) -> u16 {
        self.out_channels.len() as u16
    }

    fn out<N: Size>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> ChannelsHandle<'a, Self::Sample, N> {
        todo!()
    }

    fn to<S: DynamicSink3>(self, n: S) -> DynamicChannelsHandle<'a, F> {
        todo!()
    }

    fn to_graph_out(self) {
        todo!()
    }

    fn to_graph_out_channels<N: Size>(self, sink_channels: impl Into<Channels<N>>) {
        todo!()
    }
}
impl<'a, F: Float> DynamicSink3 for DynamicChannelsHandle<'a, F> {
    fn inputs(&self) -> u16 {
        self.in_channels.len() as u16
    }

    fn iter(&self) -> DynamicChannelIter {
        DynamicChannelIter {
            channels: self.in_channels.clone(),
            current_index: 0,
        }
    }
}

// We need Sink and Source because some things such as binary op connections can't reasonably be
// have things connected to their inputs

pub trait Sink3 {
    type Inputs: Size;
    fn iter(&self) -> ChannelIter<Self::Inputs>;
}
pub trait Source3<'a>: Sized {
    type Sample: Float;
    type Outputs: Size;
    fn iter(&self) -> ChannelIter<Self::Outputs>;
    fn out<N: Size>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> ChannelsHandle<'a, Self::Sample, N>;

    fn to<S: Sink3>(self, n: S) -> S
    where
        S::Inputs: Same<Self::Outputs>;
    /// Connect to the graph output(s) in the order the channels are produced.
    fn to_graph_out(self);

    /// Connect to the graph output(s), selecting graph output channels from the channels provided
    fn to_graph_out_channels<N>(self, sink_channels: impl Into<Channels<N>>)
    where
        N: Size + Same<Self::Outputs>;
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

pub trait DynamicSink3 {
    fn iter(&self) -> DynamicChannelIter;
    fn inputs(&self) -> u16;
}
pub trait DynamicSource3<'a> {
    type Sample: Float;
    fn iter(&self) -> DynamicChannelIter;
    fn outputs(&self) -> u16;
    fn out<N: Size>(
        &self,
        source_channels: impl Into<Channels<N>>,
    ) -> ChannelsHandle<'a, Self::Sample, N>;
    // This gives a DynamicChannelsHandle because the API is more ergonomic if the output of
    // an operation with a dynamic type always yields a dynamic type.
    fn to<S: DynamicSink3>(self, n: S) -> DynamicChannelsHandle<'a, Self::Sample>;
    /// Connect to the graph output(s) in the order the channels are produced.
    fn to_graph_out(self);

    /// Connect to the graph output(s), selecting graph output channels from the channels provided
    fn to_graph_out_channels<N: Size>(self, sink_channels: impl Into<Channels<N>>);
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
        graph::GraphOptions,
        runner::{Runner, RunnerOptions},
    };
    use knaster_core::{
        Seconds, noise::WhiteNoise, onepole::OnePoleLpf, osc::SinWt, pan::Pan2, typenum::*,
        util::Constant, wrappers_core::UGenWrapperCoreExt,
    };

    use super::{DynamicSource3, GraphEdit, Source3};
    #[test]
    fn scope() {
        let block_size = 16;
        let (mut graph, mut runner) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
        });
        // let mut s = None;
        {
            let graph = GraphEdit::new(graph);
            std::thread::spawn(move || {
                let kept_sine;
                let kept_lpf;
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

                    // let p = Parameters::new();
                    // let mut p = p
                    //     .push("freq", sine.param("freq").unwrap())
                    //     .push("freq", sine2.param("freq").unwrap())
                    //     .push("pan", pan.param("pan").unwrap());
                    let t = Time::absolute(Seconds::from_secs_f64(2.5));
                    // p.set("freq", 400., t).unwrap();
                    let mut freq = sine.param("freq").unwrap();
                    freq.set(400., t).unwrap();

                    let freq = sine.param("freq").unwrap();
                    // s = Some(sine);
                    let c = graph.push(Constant::new(0.2));
                    let c = sine * sine2 * c;
                    kept_sine = sine.node_id();
                    kept_lpf = lpf.node_id();
                }
                // do a bunch of stuff
                {
                    // Retreive a handle to the node
                    let handle = graph.handle(kept_sine).unwrap();
                    let lpf = graph.handle(kept_lpf).unwrap();
                    // Use the handle to connect it to another node
                    let new_sine = graph.push(SinWt::new(200.));
                    let new_pan = graph.push(Pan2::new(0.));
                    let a = (handle * new_sine) >> new_pan >> (lpf | lpf | new_pan);
                    a.to_graph_out();
                    (new_sine + handle) >> new_pan;
                }
            });
        }
        // let s = s.unwrap();
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
