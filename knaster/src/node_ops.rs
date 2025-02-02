//! Shortcuts for mathematical operations on nodes.
//!

use crate::core::vec;

use crate::{
    core::vec::Vec,
    math_ugens::{add, mul},
};
use knaster_graph::{
    connectable::{Connectable, NodeSubset},
    graph::{Graph, GraphError},
    handle::HandleTrait,
    Float,
};

pub fn ugen_mul<H0: HandleTrait, H1: HandleTrait, F: Float>(
    h0: &H0,
    h1: &H1,
    graph: &mut Graph<F>,
) -> Result<Connectable, GraphError> {
    let min_output_channels = h0.outputs().min(h1.outputs());
    let mut sources: Vec<NodeSubset> = vec![];
    for chan in 0..min_output_channels {
        let mul = graph.push(mul());
        graph.connect(h0, chan, 0, &mul)?;
        graph.connect(h1, chan, 1, &mul)?;
        sources.push(mul.subset(0, 1));
    }
    Ok(Connectable::NodeSeries(sources))
}
