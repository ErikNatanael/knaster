// Lärdom: för att minska mängden operatoröverlagringar behövs en typ som äger alla dynamiska och
// en typ som äger alla statiska handles. Detta begränsar mängden till 4 implementationer per operator.

use core::{marker::PhantomData, ops::Add};

use std::sync::RwLock;

use knaster_core::{Float, Size, UGen, numeric_array::NumericArray, typenum::U0};
use smallvec::SmallVec;

use crate::{
    connectable::NodeOrGraph,
    graph::{Graph, NodeId},
    node::NodeData,
};

pub struct N<'a, T: Source3, F: Float> {
    node: T,
    graph: &'a RwLock<Graph<F>>,
}
pub struct DN<'a, T: DynamicSource3, F: Float> {
    node: T,
    graph: &'a RwLock<Graph<F>>,
}

pub trait Source3 {
    type Outputs: Size;
    fn iter(&self) -> ChannelIter<Self::Outputs>;
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
struct ChannelsHandle<F: Float, O: Size> {
    channels: NumericArray<(NodeOrGraph, u16), O>,
    _float: PhantomData<F>,
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

impl<'a, F: Float, S0: Source3, S1: Source3> Add<N<'a, S1, F>> for N<'a, S0, F> {
    type Output = N<'a, ChannelIter<S0::Outputs>, F>;
    fn add(self, rhs: N<'a, S1, F>) -> Self::Output {
        todo!()
    }
}
impl<'a, F: Float, S0: DynamicSource3, S1: Source3> Add<N<'a, S1, F>> for DN<'a, S0, F> {
    type Output = DN<'a, DynamicChannelIter, F>;
    fn add(self, rhs: N<'a, S1, F>) -> Self::Output {
        todo!()
    }
}
impl<'a, F: Float, S0: DynamicSource3, S1: DynamicSource3> Add<DN<'a, S1, F>> for DN<'a, S0, F> {
    type Output = DN<'a, DynamicChannelIter, F>;
    fn add(self, rhs: DN<'a, S1, F>) -> Self::Output {
        todo!()
    }
}
impl<'a, F: Float, S0: Source3, S1: DynamicSource3> Add<DN<'a, S1, F>> for N<'a, S0, F> {
    type Output = DN<'a, DynamicChannelIter, F>;
    fn add(self, rhs: DN<'a, S1, F>) -> Self::Output {
        todo!()
    }
}
