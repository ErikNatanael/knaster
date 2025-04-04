use crate::core::collections::VecDeque;

use crate::core::{string::String, vec::Vec};

use knaster_core::Seconds;

use crate::graph::GraphError;

pub fn graph_log_error(e: GraphError) {
    match e {
        GraphError::SendToGraphGen(_) => todo!(),
        GraphError::NodeNotFound => todo!(),
        GraphError::InputOutOfBounds(_) => todo!(),
        GraphError::OutputOutOfBounds(_) => todo!(),
        GraphError::GraphInputOutOfBounds(_) => todo!(),
        GraphError::GraphOutputOutOfBounds(_) => todo!(),
        GraphError::ParameterDescriptionNotFound(_) => todo!(),
        GraphError::ParameterIndexOutOfBounds(_) => todo!(),
        GraphError::ParameterError(parameter_error) => todo!(),
        GraphError::PushChangeError(_) => todo!(),
        GraphError::WrongSourceNodeGraph {
            expected_graph,
            found_graph,
        } => todo!(),
        GraphError::WrongSinkNodeGraph {
            expected_graph,
            found_graph,
        } => todo!(),
        GraphError::CircularConnection => todo!(),
    }
}
pub fn graph_log_warn(e: GraphError) {}
