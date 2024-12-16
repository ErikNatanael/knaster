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
    // fn to_element(&self) -> ChainElement;
    fn to<T: Into<ChainElement>>(&self, sink: T) -> ConnectionChain;
    /// Connect to `other` by adding any additional inputs with this one.
    ///
    /// This is different from first adding inputs together in that the
    /// [`Graph`] is responsible for keeping track of what other inputs the sink
    /// node has.
    fn add_to<T: Into<ChainElement>>(&self, sink: T) -> ConnectionChain {
        let mut chain = self.to(sink);
        chain.additive = true;
        chain
    }
    // Implement by connecting self `to` a new inline node with `other` as the other input
    fn mul<T: Into<ChainSourceOrConstant>>(&self, other: T) -> ConnectionChain {
        todo!()
        // let other = other.into();
        // match other {
        //     ChainSourceOrConstant::ChainSource(source) => {
        //         // TODO: hmmm there have to be an equal number of inputs and outputs in the two sources.
        //         let source = ChainElement::pair(self.to_element(), source);
        //         let sink = ChainElement {
        //             kind: ChainSinkKind::NewInlineNode {
        //                 op: InlineNodeKind::Mul,
        //             },
        //             inputs: source.outputs,
        //             outputs: source.outputs / 2,
        //         };
        //         ConnectionChain {
        //             source: Some(Box::new(ConnectionChain::chain_start(source))),
        //             sink,
        //             additive: false,
        //         }
        //     }
        //     ChainSourceOrConstant::Constant(constant) => {
        //         todo!()
        //     }
        // }
    }
}

#[derive(Clone, Debug)]
pub enum ChainSourceOrConstant {
    ChainSource(ChainElement),
    Constant(PFloat),
}
impl From<ChainElement> for ChainSourceOrConstant {
    fn from(value: ChainElement) -> Self {
        ChainSourceOrConstant::ChainSource(value)
    }
}
impl From<ConnectionChain> for ChainSourceOrConstant {
    fn from(value: ConnectionChain) -> Self {
        ChainSourceOrConstant::ChainSource(value.into())
    }
}
impl From<PFloat> for ChainSourceOrConstant {
    fn from(value: PFloat) -> Self {
        ChainSourceOrConstant::Constant(value)
    }
}

impl Connectable for ConnectionChain {
    fn to<T: Into<ChainElement>>(&self, sink: T) -> ConnectionChain {
        ConnectionChain {
            source: Some(Box::new(self.clone())),
            sink: sink.into(),
            additive: false,
        }
    }
}
impl<G: Gen + Parameterable<G::Sample>> Connectable for Handle<G> {
    fn to<T: Into<ChainElement>>(&self, other: T) -> ConnectionChain {
        ConnectionChain {
            source: Some(Box::new(ConnectionChain::chain_start(ChainElement {
                kind: ChainSinkKind::Node {
                    key: self.untyped_handle.node.key(),
                    from_chan: 0,
                    channels: self.outputs(),
                },
                inputs: 0,
                outputs: self.outputs(),
            }))),
            sink: other.into(),
            additive: false,
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
pub struct ChainElement {
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
    NewInlineNode {
        op: InlineNodeKind,
    },
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
    Pair(Box<(ChainElement, ChainElement)>),
}
impl ChainElement {
    pub fn pair(e0: ChainElement, e1: ChainElement) -> Self {
        let inputs = e0.inputs + e1.inputs;
        let outputs = e0.outputs + e1.outputs;
        Self {
            kind: ChainSinkKind::Pair(Box::new((e0, e1))),
            inputs,
            outputs,
        }
    }
    // pub fn to_connection_node(&self) -> Option<ConnectionNode> {
    //     match self.kind {
    //         ChainSinkKind::NewInlineNode(_) => todo!(),
    //         ChainSinkKind::GraphConnection => Some(ConnectionNode::Graph),
    //         ChainSinkKind::FeedbackNode => todo!(),
    //         ChainSinkKind::Chain(_) => None,
    //     }
    // }
    // pub fn offset(self, offset: usize) -> Self {
    //     match &mut self.kind {
    //         ChainSinkKind::Node {
    //             key,
    //             from_chan,
    //             channels,
    //         } => *from_chan += offset,
    //         ChainSinkKind::NewInlineNode(_) => todo!(),
    //         ChainSinkKind::GraphConnection {
    //             from_chan,
    //             channels,
    //         } => todo!(),
    //         ChainSinkKind::FeedbackNode {
    //             key,
    //             from_chan,
    //             to_chan,
    //         } => todo!(),
    //         ChainSinkKind::Chain(_) => todo!(),
    //         ChainSinkKind::Parameter(_) => todo!(),
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
    sink: ChainElement,
    additive: bool,
}
impl ConnectionChain {
    fn chain_start(source: ChainElement) -> Self {
        ConnectionChain {
            source: None,
            sink: source,
            additive: false,
        }
    }
    pub fn deconstruct(self) -> (Option<Box<ConnectionChain>>, ChainElement) {
        (self.source, self.sink)
    }
    pub fn sink(&self) -> &ChainElement {
        &self.sink
    }
    pub fn source(&self) -> &Option<Box<ConnectionChain>> {
        &self.source
    }
    /// Returns true if this connection should add to the input of the sink or
    /// false if it should replace it.
    pub fn additive_connection(&self) -> bool {
        self.additive
    }
}

impl From<ConnectionChain> for ChainElement {
    fn from(value: ConnectionChain) -> Self {
        ChainElement {
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
