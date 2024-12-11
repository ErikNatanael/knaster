use core::marker::PhantomData;

use knaster_core::{
    numeric_array::NumericArray,
    typenum::{U1, U2, U3},
    Gen, PFloat, Param, Parameterable, Size,
};

use crate::{graph::NodeKey, handle::Handle};

/// Trait for something that can be connected in the Graph.
///
/// This trait provides a consistent interface for anything that can connect
/// through the graph, e.g. handles, [`ConnectionChain`]s.
///
/// TODO: Implementors of this interface should also check input/output channel
/// arity at runtime and report an error if there is an arity mismatch.
pub trait Connectable {
    fn to<T: Into<ChainSink>>(&self, other: T) -> ConnectionChain;
    fn mul<T: Into<ChainSourceOrConstant>>(&self, other: T) -> ConnectionChain;
}

pub enum ChainSourceOrConstant {
    ChainSource(ChainSink),
    Constant(PFloat),
}
impl From<ChainSink> for ChainSourceOrConstant {
    fn from(value: ChainSink) -> Self {
        ChainSourceOrConstant::ChainSource(value)
    }
}
impl From<PFloat> for ChainSourceOrConstant {
    fn from(value: PFloat) -> Self {
        ChainSourceOrConstant::Constant(value)
    }
}

impl Connectable for ConnectionChain {
    fn to<T: Into<ChainSink>>(&self, other: T) -> ConnectionChain {
        ConnectionChain {
            source: Some(Box::new(self.clone())),
            sink: other.into(),
        }
    }

    fn mul<T: Into<ChainSourceOrConstant>>(&self, other: T) -> ConnectionChain {
        ConnectionChain {
            source: Some(Box::new(self.clone())),
            sink: ChainSink {
                kind: ChainSinkKind::NewInlineNode(InlineNodeKind::Mul),
                inputs: self.sink.outputs,
                outputs: self.sink.outputs,
            },
        }
    }
}
impl<G: Gen + Parameterable<G::Sample>> Connectable for Handle<G> {
    fn to<T: Into<ChainSink>>(&self, other: T) -> ConnectionChain {
        ConnectionChain {
            source: Some(Box::new(ConnectionChain {
                source: None,
                sink: ChainSink {
                    kind: ChainSinkKind::Node {
                        key: self.untyped_handle.node,
                        from_chan: 0,
                        channels: self.outputs(),
                    },
                    inputs: 0,
                    outputs: self.outputs(),
                },
            })),
            sink: other.into(),
        }
    }

    fn mul<T: Into<ChainSourceOrConstant>>(&self, other: T) -> ConnectionChain {
        todo!()
    }
}

// #[derive(Clone, Debug)]
// pub struct ChainSource {
//     pub(crate) kind: ChainSourceKind,
//     pub(crate) inputs: usize,
//     pub(crate) outputs: usize,
// }
// #[derive(Clone, Debug)]
// pub enum ChainSourceKind {
//     Node {
//         key: NodeKey,
//         from_chan: usize,
//         to_chan: usize,
//     },
//     Chain(Box<ConnectionChain>),
//     GraphConnection,
//     FeedbackNode,
//     MergeAdd(Vec<ChainSource>),
// }
// impl Connectable for ChainSource {
//     fn to<T: Into<ChainSink>>(&self, other: T) -> ConnectionChain {
//         ConnectionChain {
//             source: Some(Box::new(self.clone())),
//             sink: other.into(),
//         }
//     }

//     fn mul<T: Into<ChainSourceOrConstant>>(&self, other: T) -> ConnectionChain {
//         ConnectionChain {
//             source: self.clone(),
//             sink: ChainSink {
//                 kind: ChainSinkKind::NewInlineNode(InlineNodeKind::Mul),
//                 inputs: self.outputs,
//                 outputs: self.outputs,
//             },
//         }
//     }
// }
// impl ChainSource {
//     // pub fn to_connection_node(&self) -> Option<ConnectionNode> {
//     //     match self.kind {
//     //         ChainSourceKind::Node(n) => Some(ConnectionNode::Node(n)),
//     //         ChainSourceKind::Chain(_) => None,
//     //         ChainSourceKind::GraphConnection => Some(ConnectionNode::Graph),
//     //         ChainSourceKind::FeedbackNode => todo!(),
//     //         ChainSourceKind::MergeAdd(_) => todo!(),
//     //     }
//     // }
// }
#[derive(Clone, Debug)]
pub struct ChainSink {
    pub(crate) kind: ChainSinkKind,
    pub(crate) inputs: usize,
    pub(crate) outputs: usize,
}
#[derive(Clone, Debug)]
pub enum ChainSinkKind {
    Node {
        key: NodeKey,
        from_chan: usize,
        channels: usize,
    },
    /// Inline Mul, Add, Sub, Div etc. that should be added to the graph in between.
    NewInlineNode(InlineNodeKind),
    GraphConnection {
        from_chan: usize,
        channels: usize,
    },
    FeedbackNode {
        key: NodeKey,
        from_chan: usize,
        to_chan: usize,
    },
    Chain(Box<ConnectionChain>),
    Parameter(Param),
}
impl ChainSink {
    // pub fn to_connection_node(&self) -> Option<ConnectionNode> {
    //     match self.kind {
    //         ChainSinkKind::NewInlineNode(_) => todo!(),
    //         ChainSinkKind::GraphConnection => Some(ConnectionNode::Graph),
    //         ChainSinkKind::FeedbackNode => todo!(),
    //         ChainSinkKind::Chain(_) => None,
    //     }
    // }
}

#[derive(Clone, Debug)]
pub enum InlineNodeKind {
    Mul,
    Add,
    Sub,
    Div,
}

/// A simple model of a chain of connections within a graph.
///
/// The real source is simply the sink of the source chain
#[derive(Clone, Debug)]
pub struct ConnectionChain {
    source: Option<Box<ConnectionChain>>,
    sink: ChainSink,
}
impl ConnectionChain {
    pub fn deconstruct(self) -> (Option<Box<ConnectionChain>>, ChainSink) {
        (self.source, self.sink)
    }
    pub fn sink(&self) -> &ChainSink {
        &self.sink
    }
    pub fn source(&self) -> &Option<Box<ConnectionChain>> {
        &self.source
    }
}

impl From<ConnectionChain> for ChainSink {
    fn from(value: ConnectionChain) -> Self {
        ChainSink {
            inputs: value
                .source
                .as_ref()
                .map_or(value.sink.inputs, |so| so.sink.inputs),
            outputs: value.sink.outputs,
            kind: ChainSinkKind::Chain(Box::new(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn connect_chains() {}
}
