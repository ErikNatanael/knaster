use crate::graph::NodeKey;

/// An edge in the Graph. Only stores the source in the Edge since they are stored per sink node.
#[derive(Clone, Debug, Copy)]
pub(crate) struct Edge {
    pub(crate) source: NodeKey,
    pub(crate) kind: EdgeKind,
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub(crate) enum NodeKeyOrGraph {
    Node(NodeKey),
    Graph,
}

#[derive(Clone, Debug, Copy)]
pub(crate) enum EdgeKind {
    /// Audio edge connection from the output of one node to the input of another
    Audio {
        /// number of channels to pipe from source
        channels: usize,
        /// what the first channel to input into is in the si
        channel_offset_in_sink: usize,
        /// what the first channel to pipe is in the source
        channel_offset_in_source: usize,
    },
    /// Parameter edge connection from one channel of output from a node to control a parameter of another node.
    ///
    /// Always only one channel of audio
    Parameter {
        /// what the first channel to pipe is in the source
        channel_offset_in_source: usize,
        parameter_index: usize,
    },
}

#[derive(Clone, Debug, Copy)]
pub(crate) struct InternalGraphEdge {
    /// the output index on the destination node
    pub(crate) from_output_index: usize,
    /// the input index on the origin node where the input from the node is placed
    pub(crate) to_input_index: usize,
}

/// Edge containing all metadata for a feedback connection since a feedback
/// connection includes several things that may need to be freed together:
/// - a node
/// - a feedback edge
/// - a normal edge
#[derive(Clone, Debug, Copy)]
pub(crate) struct FeedbackEdge {
    pub(crate) source: NodeKey,
    /// the output index on the destination node
    pub(crate) from_output_index: usize,
    /// the input index on the origin node where the input from the node is placed
    pub(crate) to_input_index: usize,
    /// If the source node is freed we want to remove the normal edge to the destination node.
    pub(crate) feedback_destination: NodeKey,
}
