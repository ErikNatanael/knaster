//! Shortcuts for mathematical operations on nodes.
//!

use crate::core::vec;

use crate::{
    core::vec::Vec,
    math_ugens::{add, div, mul, sub},
};
use knaster_graph::{
    connectable::{Connectable, NodeSubset},
    graph::{Graph, GraphError},
    handle::HandleTrait,
    Float,
};

macro_rules! ugen_math_helper_impl {
    ($h0:ident, $h1:ident, $graph:ident, $func:ident) => {{
        let h0 = $h0.into();
        let h1 = $h1.into();
        let min_output_channels = h0.outputs().min(h1.outputs());
        let mut sources: Vec<NodeSubset> = vec![];
        for chan in 0..min_output_channels {
            let mul = $graph.push($func());
            $graph.connect(&h0, chan, 0, &mul)?;
            $graph.connect(&h1, chan, 1, &mul)?;
            sources.push(mul.subset(0, 1));
        }
        let mut c = Connectable::empty();
        for input_subset in h0.input_subsets() {
            c.chain_input(*input_subset);
        }
        for input_subset in h1.input_subsets() {
            c.chain_input(*input_subset);
        }
        for output_subset in sources {
            c.chain_output(output_subset);
        }
        Ok(c)
    }};
}

pub fn ugen_mul<F: Float>(
    h0: impl Into<Connectable>,
    h1: impl Into<Connectable>,
    graph: &mut Graph<F>,
) -> Result<Connectable, GraphError> {
    ugen_math_helper_impl!(h0, h1, graph, mul)
}
pub fn ugen_add<F: Float>(
    h0: impl Into<Connectable>,
    h1: impl Into<Connectable>,
    graph: &mut Graph<F>,
) -> Result<Connectable, GraphError> {
    ugen_math_helper_impl!(h0, h1, graph, add)
}
pub fn ugen_sub<F: Float>(
    h0: impl Into<Connectable>,
    h1: impl Into<Connectable>,
    graph: &mut Graph<F>,
) -> Result<Connectable, GraphError> {
    ugen_math_helper_impl!(h0, h1, graph, sub)
}
pub fn ugen_div<F: Float>(
    h0: impl Into<Connectable>,
    h1: impl Into<Connectable>,
    graph: &mut Graph<F>,
) -> Result<Connectable, GraphError> {
    ugen_math_helper_impl!(h0, h1, graph, div)
}
