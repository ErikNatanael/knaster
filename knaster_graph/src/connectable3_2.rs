// Lärdom: för att minska mängden operatoröverlagringar behövs en typ som äger alla dynamiska och
// en typ som äger alla statiska handles. Detta begränsar mängden till 4 implementationer per operator.

use core::{
    marker::PhantomData,
    ops::{Add, Div, Mul, Sub},
};

use std::sync::RwLock;

use knaster_core::{Float, Size, UGen, math::*, numeric_array::NumericArray, typenum::*};
use smallvec::SmallVec;

use crate::{
    connectable::NodeOrGraph,
    connectable3::{ChannelIterBuilder, ChannelsHandle, DynamicChannelsHandle},
    graph::{Graph, NodeId},
    handle::HandleTrait,
    node::NodeData,
};

/// Static Handle. Wrapper around static sources/sinks.
pub struct SH<'a, F: Float, T: Source3> {
    nodes: T,
    graph: &'a RwLock<Graph<F>>,
}
impl<'a, T: Source3, F: Float> SH<'a, F, T> {
    pub fn dynamic() -> DH<'a, F, DynamicChannelIter> {
        todo!()
    }
}
/// Dynamic Handle. Wrapper around dynamic source/sinks
pub struct DH<'a, F: Float, T: DynamicSource3> {
    nodes: T,
    graph: &'a RwLock<Graph<F>>,
}

pub trait Source3 {
    type Outputs: Size;
    fn iter(&self) -> ChannelIter<Self::Outputs>;
    fn dynamic(&self) -> DynamicChannelIter;
    // fn to<S: StSink3>(self, n: S) -> S
    // where
    //     S::Inputs: Same<Self::Outputs>;
}
pub trait DynamicSource3 {
    fn iter(&self) -> DynamicChannelIter;
    fn outputs(&self) -> u16;
    // fn to<S: DynamicSink3>(self, n: S) -> DynamicChannelsHandle<'a, Self::Sample>;
}
struct StaticHandle3<'a, U: UGen> {
    node_id: NodeId,
    ugen: PhantomData<U>,
    graph: &'a RwLock<Graph<U::Sample>>,
}
struct DynamicHandle3 {
    node_id: NodeId,
    data: NodeData,
}
impl DynamicSource3 for DynamicHandle3 {
    fn iter(&self) -> DynamicChannelIter {
        let mut channels = SmallVec::with_capacity(self.outputs() as usize);
        for i in 0..self.outputs() {
            channels.push((NodeOrGraph::Node(self.node_id), i));
        }
        DynamicChannelIter::new(channels)
    }
    fn outputs(&self) -> u16 {
        self.data.outputs
    }
}

pub struct ChannelIter<I: Size> {
    channels: NumericArray<(NodeOrGraph, u16), I>,
    current_index: usize,
}
impl<I: Size> Source3 for ChannelIter<I> {
    type Outputs = I;
    fn iter(&self) -> ChannelIter<Self::Outputs> {
        todo!()
    }

    fn dynamic(&self) -> DynamicChannelIter {
        todo!()
    }
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
impl DynamicSource3 for DynamicChannelIter {
    fn iter(&self) -> DynamicChannelIter {
        todo!()
    }
    fn outputs(&self) -> u16 {
        todo!()
    }
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

// Macros for implementing arithmetics on sources with statically known channel configurations
macro_rules! math_gen_fn {
    ($fn_name:ident, $op:ty) => {
        fn $fn_name<'a, F: Float, S0: Source3, S1: Source3>(
            s0: SH<'a, F, S0>,
            s1: SH<'a, F, S1>,
            graph: &RwLock<Graph<F>>,
        ) -> ChannelsHandle<S1::Outputs>
        where
            S0::Outputs: Same<S1::Outputs>,
        {
            let mut out_channels = ChannelIterBuilder::new();
            let mut g = graph.write().unwrap();
            for (i, (s0, s1)) in Source3::iter(&s0.nodes).zip(s1.nodes.iter()).enumerate() {
                let mul = g.push(MathUGen::<_, U1, $op>::new());
                if let Err(e) = g.connect2(s0.0, s0.1, 0, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                if let Err(e) = g.connect2(s1.0, s1.1, 1, NodeOrGraph::Node(mul.node_id())) {
                    log::error!("Failed to connect node to arithmetics node: {e}");
                }
                out_channels.push(NodeOrGraph::Node(mul.node_id()), i as u16);
            }
            out_channels
                .into_channel_iter()
                .expect("all the channels should be initialised")
                .into()
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
        fn $fn_name<'a, F: Float, S0: DynamicSource3, S1: DynamicSource3>(
            s0: DH<'a, F, S0>,
            s1: DH<'a, F, S1>,
            graph: &'a RwLock<Graph<F>>,
        ) -> DynamicChannelsHandle {
            if s0.nodes.outputs() != s1.nodes.outputs() {
                panic!("The number of outputs of the two sources must be the same");
            }
            let mut out_channels = SmallVec::with_capacity(s0.nodes.outputs() as usize);
            let mut g = graph.write().unwrap();
            for (i, (s0, s1)) in DynamicSource3::iter(&s0.nodes)
                .zip(s1.nodes.iter())
                .enumerate()
            {
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

macro_rules! math_impl_arithmetics {
    ($fn_name_static:ident, $fn_name:ident, $op:ident, $op_lowercase:ident) => {
        impl<'a, F: Float, S0: Source3, S1: Source3> $op<SH<'a, S1, F>> for SH<'a, S0, F> {
            type Output = SH<'a, ChannelIter<S0::Outputs>, F>;
            fn $op_lowercase(self, rhs: SH<'a, S1, F>) -> Self::Output {
                let graph = self.graph;
                $fn_name_static(self, rhs, graph)
            }
        }
        impl<'a, F: Float, S0: DynamicSource3, S1: Source3> $op<SH<'a, S1, F>> for DH<'a, S0, F> {
            type Output = DH<'a, DynamicChannelIter, F>;
            fn $op_lowercase(self, rhs: SH<'a, S1, F>) -> Self::Output {
                let graph = self.graph;
                $fn_name(self, rhs.dynamic(), graph)
            }
        }
        impl<'a, F: Float, S0: DynamicSource3, S1: DynamicSource3> $op<DH<'a, S1, F>>
            for DH<'a, S0, F>
        {
            type Output = DH<'a, DynamicChannelIter, F>;
            fn $op_lowercase(self, rhs: DH<'a, S1, F>) -> Self::Output {
                let graph = self.graph;
                $fn_name(self, rhs, graph)
            }
        }
        impl<'a, F: Float, S0: Source3, S1: DynamicSource3> $op<DH<'a, S1, F>> for SH<'a, S0, F> {
            type Output = DH<'a, DynamicChannelIter, F>;
            fn $op_lowercase(self, rhs: DH<'a, S1, F>) -> Self::Output {
                let graph = self.graph;
                $fn_name(self.dynamic(), rhs, graph)
            }
        }
    };
}

math_impl_arithmetics!(mul_sources, mul_sources_dynamic, Mul, mul);
math_impl_arithmetics!(add_sources, add_sources_dynamic, Add, add);
math_impl_arithmetics!(sub_sources, sub_sources_dynamic, Sub, sub);
math_impl_arithmetics!(div_sources, div_sources_dynamic, Div, div);
