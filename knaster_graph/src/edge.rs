use crate::graph::NodeKey;

/// An edge in the Graph. Only stores the source in the Edge since they are stored per sink node.
#[derive(Clone, Debug, Copy)]
pub(crate) struct Edge {
    pub(crate) source: NodeKeyOrGraph,
    pub(crate) channel_in_source: usize,
    pub(crate) is_feedback: bool,
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub(crate) enum NodeKeyOrGraph {
    Node(NodeKey),
    Graph,
}
impl From<NodeKey> for NodeKeyOrGraph {
    fn from(value: NodeKey) -> Self {
       Self::Node(value)
    }
}

// #[derive(Clone, Debug, Copy)]
// pub(crate) enum EdgeKind {
//     /// Audio edge connection from the output of one node to the input of another. Always one channel per edge.
//     Audio {
//         /// what the channel to connect is in the source
//         channel_in_source: usize,
//     },
//     Feedback {
//         channel_in_source: usize,
//     },
//     // /// Parameter edge connection from one channel of output from a node to control a parameter of another node.
//     // ///
//     // /// Always only one channel of audio
//     // Parameter {
//     //     /// what the first channel to pipe is in the source
//     //     channel_in_source: usize,
//     //     parameter_index: usize,
//     // },
// }

pub(crate) struct ParameterEdge {
    pub(crate) source: NodeKey,
    /// what the first channel to pipe is in the source
    pub(crate) channel_in_source: usize,
    pub(crate) parameter_index: usize,
}

// /// Edge containing all metadata for a feedback connection since a feedback
// /// connection includes several things that may need to be freed together:
// /// - a node
// /// - a feedback edge
// /// - a normal edge
// #[derive(Clone, Debug, Copy)]
// pub(crate) struct FeedbackEdge {
//     pub(crate) source: NodeKey,
//     /// the output index on the destination node
//     pub(crate) from_output_index: usize,
//     /// the input index on the origin node where the input from the node is placed
//     pub(crate) to_input_index: usize,
//     /// If the source node is freed we want to remove the normal edge to the destination node.
//     pub(crate) feedback_destination: NodeKey,
// }
