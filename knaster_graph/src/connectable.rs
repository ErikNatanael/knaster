use crate::core::boxed::Box;
use crate::core::vec::Vec;
use crate::graph::Graph;
use crate::handle::HandleTrait;
use knaster_core::typenum::U1;
use knaster_core::Float;
use knaster_core::{numeric_array::NumericArray, PFloat, Param, Size, UGen};

use crate::{
    graph::{NodeId, NodeKey},
    handle::Handle,
};

pub enum Connectable {
    Graph,
    SingleNode(NodeId),
    NodeSeries(Vec<NodeSubset>),
}
impl Connectable {
    pub fn for_channel(&self, chan: usize) -> (NodeOrGraph, usize) {
        match self {
            Connectable::Graph => (NodeOrGraph::Graph, chan),
            Connectable::SingleNode(node_id) => (NodeOrGraph::Node(*node_id), chan),
            Connectable::NodeSeries(sources) => {
                let mut source_i = 0;
                let mut channels_in_previous_sources = 0;
                while sources[source_i].channels + channels_in_previous_sources <= chan {
                    channels_in_previous_sources += sources[source_i].channels;
                    source_i += 1;
                }
                (
                    NodeOrGraph::Node(sources[source_i].id),
                    chan - channels_in_previous_sources + sources[source_i].start_channel,
                )
            }
        }
    }
}
impl From<NodeOrGraph> for Connectable {
    fn from(value: NodeOrGraph) -> Self {
        match value {
            NodeOrGraph::Graph => Connectable::Graph,
            NodeOrGraph::Node(node_id) => Connectable::SingleNode(node_id),
        }
    }
}
impl<H: HandleTrait> From<&H> for Connectable {
    fn from(value: &H) -> Self {
        Connectable::SingleNode(value.node_id())
    }
}
pub enum NodeOrGraph {
    Graph,
    Node(NodeId),
}
impl<T: Into<NodeId>> From<T> for NodeOrGraph {
    fn from(value: T) -> Self {
        Self::Node(value.into())
    }
}
pub enum Source {
    Graph,
    Node(NodeId),
    NodeSeries(Vec<NodeSubset>),
}
pub struct NodeSubset {
    pub(crate) id: NodeId,
    pub(crate) channels: usize,
    /// The offset from the start of the channels of the node
    pub(crate) start_channel: usize,
}
impl<T: Into<NodeId>> From<T> for Source {
    fn from(value: T) -> Self {
        Self::Node(value.into())
    }
}
pub struct Channels<N: Size> {
    channels: NumericArray<usize, N>,
}
impl<N: Size> IntoIterator for Channels<N> {
    type Item = usize;

    type IntoIter = <NumericArray<usize, N> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.channels.into_iter()
    }
}
impl<N: Size> From<NumericArray<usize, N>> for Channels<N> {
    fn from(value: NumericArray<usize, N>) -> Self {
        Self { channels: value }
    }
}
impl<N: Size, const N2: usize> From<[usize; N2]> for Channels<N>
where
    crate::typenum::Const<N2>: crate::typenum::ToUInt,
    crate::typenum::Const<N2>: knaster_core::numeric_array::generic_array::IntoArrayLength,
    knaster_core::numeric_array::generic_array::GenericArray<usize, N>: From<[usize; N2]>,
{
    fn from(value: [usize; N2]) -> Self {
        Self {
            channels: value.into(),
        }
    }
}
impl From<usize> for Channels<U1> {
    fn from(value: usize) -> Self {
        Self {
            channels: [value].into(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum InlineNodeKind {
    Mul,
    Add,
    Sub,
    Div,
}

#[cfg(test)]
mod tests {
    #[test]
    fn connect_chains() {}
}
