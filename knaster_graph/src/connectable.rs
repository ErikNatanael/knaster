use crate::handle::HandleTrait;
use knaster_core::typenum::U1;
use knaster_core::{numeric_array::NumericArray, Size};
use smallvec::{smallvec, SmallVec};

use crate::graph::NodeId;

// TODO: Perhaps the size of the SmallVec should be changeable to eliminate unnecessary heap
// allocation on embedded.

#[derive(Clone, Debug)]
pub struct Connectable {
    sinks: SmallVec<[NodeSubset; 2]>,
    sources: SmallVec<[NodeSubset; 2]>,
}

// pub enum Connectable {
//     Graph,
//     SingleNode(NodeId),
//     NodeSeries(Vec<NodeSubset>),
// }
impl Connectable {
    /// Empty Connectable which will connect nothing
    pub fn empty() -> Self {
        Self {
            sinks: smallvec![],
            sources: smallvec![],
        }
    }
    pub fn from_node(inputs: NodeSubset, outputs: NodeSubset) -> Self {
        Self {
            sinks: smallvec![inputs],
            sources: smallvec![outputs],
        }
    }
    pub fn chain_input(&mut self, input: NodeSubset) {
        self.sinks.push(input);
    }
    pub fn chain_output(&mut self, output: NodeSubset) {
        self.sources.push(output);
    }
    /// Get the node/graph and node channel that represents the given channel in the Connectable.
    ///
    /// E.g. if the Connectable consists of `handle0[1..=3]` and `handle1[2..=4]`,
    /// `for_output_channel(5)` will return `(node1, 3)`
    pub fn for_output_channel(&self, chan: usize) -> Option<(NodeOrGraph, usize)> {
        let mut node_i = 0;
        let mut channels_in_previous_sources = 0;
        let sources = &self.sources;
        while sources[node_i].channels + channels_in_previous_sources <= chan {
            channels_in_previous_sources += sources[node_i].channels;
            node_i += 1;
            if node_i >= sources.len() {
                return None;
            }
        }
        Some((
            sources[node_i].node,
            chan - channels_in_previous_sources + sources[node_i].start_channel,
        ))
    }
    pub fn for_input_channel(&self, chan: usize) -> Option<(NodeOrGraph, usize)> {
        let mut node_i = 0;
        let mut channels_in_previous_sources = 0;
        let sinks = &self.sinks;
        while sinks[node_i].channels + channels_in_previous_sources <= chan {
            channels_in_previous_sources += sinks[node_i].channels;
            node_i += 1;
            if node_i >= sinks.len() {
                return None;
            }
        }
        Some((
            sinks[node_i].node,
            chan - channels_in_previous_sources + sinks[node_i].start_channel,
        ))
    }
    /// number of output channels
    pub fn outputs(&self) -> usize {
        self.sources.iter().map(|ns| ns.channels).sum()
    }
    /// number of input channels
    pub fn inputs(&self) -> usize {
        self.sinks.iter().map(|ns| ns.channels).sum()
    }
    pub fn input_subsets(&self) -> &[NodeSubset] {
        &self.sinks
    }
    pub fn output_subsets(&self) -> &[NodeSubset] {
        &self.sources
    }
}

impl From<&Connectable> for Connectable {
    fn from(value: &Connectable) -> Self {
        value.clone()
    }
}
impl<H: HandleTrait> From<&H> for Connectable {
    fn from(h: &H) -> Self {
        let input_channels = h.inputs();
        let output_channels = h.outputs();
        Connectable::from_node(h.subset(0, input_channels), h.subset(0, output_channels))
    }
}
#[derive(Debug, Copy, Clone)]
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
    NodeSeries(SmallVec<[NodeSubset; 2]>),
}
#[derive(Clone, Copy, Debug)]
/// A subset of a node's channels. Can be input or output channels depending on the context.
pub struct NodeSubset {
    pub(crate) node: NodeOrGraph,
    /// The number of channels to produce. `start_channel + ` is the
    /// last channel in the subset.
    pub(crate) channels: usize,
    /// The offset from the start of the channels of the node
    pub(crate) start_channel: usize,
}
impl<T: Into<NodeId>> From<T> for Source {
    fn from(value: T) -> Self {
        Self::Node(value.into())
    }
}
/// A newtype for an array of channel indices.
///
/// The generic `Size` parameter lets us ensure that channel arrays as inputs for a connection
/// function match in arity at compile time.
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
    use knaster_core::typenum::{U0, U1};

    use crate::{
        graph::{Graph, GraphOptions, NodeId},
        SharedFrameClock,
    };

    

    #[test]
    fn connect_connectables() {
        // Compile test, this should compile, there is no assertion
        let (g, node) = Graph::<f32>::new::<U0, U1>(
            GraphOptions::default(),
            NodeId::top_level_graph_node_id(),
            SharedFrameClock::new(),
            16,
            44100,
        );
        // let c0 = Connectable::SingleNode(NodeId::top_level_graph_node_id());
        // let c1 = Connectable::SingleNode(NodeId::top_level_graph_node_id());
        // g.connect(&c0, 0, 0, &c1).unwrap();
        // g.connect(c0, 0, 0, c1).unwrap();
    }
}
