//! Connection primitives to express any supported connection, see [`Graph::connect`]

use knaster_core::Param;

#[expect(unused)]
use crate::graph::Graph;
use crate::graph::{NodeId, NodeOrGraph};

/// Source, i.e. where the signal is coming from, for a connection.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// The output from a node channel
    #[allow(missing_docs)]
    Node { id: NodeId, channel: u16 },
    /// The output from a graph input channel
    #[allow(missing_docs)]
    GraphInput { channel: u16 },
}

impl Source {
    /// The output from a node channel
    pub fn node(node: impl Into<NodeId>, output_channel: u16) -> Self {
        Self::Node {
            id: node.into(),
            channel: output_channel,
        }
    }
    /// The output from a graph input channel
    pub fn graph(input_channel: u16) -> Self {
        Self::GraphInput {
            channel: input_channel,
        }
    }
    /// Extract the key/graph and channel number
    pub fn into_parts(self) -> (NodeOrGraph, u16) {
        match self {
            Source::Node { id, channel } => (NodeOrGraph::Node(id), channel),
            Source::GraphInput { channel } => (NodeOrGraph::Graph, channel),
        }
    }
    /// Create a source from a [`NodeOrGraph`] and a channel number
    pub fn from_node_or_graph(source: impl Into<NodeOrGraph>, source_channel: u16) -> Self {
        match source.into() {
            NodeOrGraph::Node(id) => Self::node(id, source_channel),
            NodeOrGraph::Graph => Self::graph(source_channel),
        }
    }
}

/// Sink, i.e. where the signal is going to, for a connection.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sink {
    /// The input to a node channel
    #[allow(missing_docs)]
    Node { id: NodeId, channel: u16 },
    /// The input to a graph output channel
    #[allow(missing_docs)]
    GraphOutput { channel: u16 },
    /// Connecting a graph source to a node parameter
    #[allow(missing_docs)]
    Parameter { id: NodeId, param: Param },
}

impl Sink {
    /// The input to a node channel
    pub fn node(node: impl Into<NodeId>, input_channel: u16) -> Self {
        Self::Node {
            id: node.into(),
            channel: input_channel,
        }
    }
    /// The input to a graph output channel
    pub fn graph(output_channel: u16) -> Self {
        Self::GraphOutput {
            channel: output_channel,
        }
    }
    /// Connecting a graph source to a node parameter
    pub fn param(node: impl Into<NodeId>, param: impl Into<Param>) -> Self {
        Self::Parameter {
            id: node.into(),
            param: param.into(),
        }
    }
    /// Create a sink from a [`NodeOrGraph`] and a channel number
    pub fn from_node_or_graph(sink: impl Into<NodeOrGraph>, sink_channel: u16) -> Self {
        match sink.into() {
            NodeOrGraph::Node(id) => Self::node(id, sink_channel),
            NodeOrGraph::Graph => Self::graph(sink_channel),
        }
    }
}

/// Options for a connection
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ConnectionOptions {
    #[allow(missing_docs)]
    pub replace: bool,
    #[allow(missing_docs)]
    pub feedback: bool,
}

impl ConnectionOptions {
    /// Set replace to true. Any existing connections to the sink will be replaced.
    pub fn replace(mut self) -> Self {
        self.replace = true;
        self
    }
    /// Set feedback to true. The signal data will be delayed by one block, breaking potential
    /// cycles in the graph.
    pub fn feedback(mut self) -> Self {
        self.feedback = true;
        self
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            replace: false,
            feedback: false,
        }
    }
}
