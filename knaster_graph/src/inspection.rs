//! # Inspection
//!
//! Metadata from the structs in this module can be used to visualise and/or
//! manipulate a graph based on the whole graph structure.

use crate::core::sync::{Arc, Mutex};

use crate::graph::{GraphId, NodeId, NodeKey};
use crate::handle::{AnyHandle, RawHandle, SchedulingChannelSender};
use crate::{SchedulingChannelProducer, SharedFrameClock};
use alloc::{format, string::String, string::ToString, vec::Vec};
use ecow::EcoString;
use knaster_core::ParameterHint;

/// The metadata of a Graph
// TODO: Feedback edges
// TODO: Parameter edges
#[derive(Debug, Clone)]
pub struct GraphInspection {
    /// All the nodes currently in the Graph (including those pending removal)
    pub nodes: Vec<NodeInspection>,
    /// The indices of nodes connected to the graph output(s)
    pub graph_output_edges: Vec<EdgeInspection>,
    /// Number of inputs to the graph
    pub num_inputs: u16,
    /// Number of outputs from the graph
    pub num_outputs: u16,
    /// The ID of the graph
    pub graph_id: crate::graph::GraphId,
    pub graph_name: EcoString,
    /// The same kind of send that is used in a Handle
    pub param_sender: SchedulingChannelSender,
    /// The frame clock of the graph
    pub shared_frame_clock: SharedFrameClock,
}

impl GraphInspection {
    /// Create an empty GraphInspection
    // pub fn empty() -> Self {
    //     Self {
    //         nodes: vec![],
    //         num_inputs: 0,
    //         num_outputs: 0,
    //         graph_id: 0,
    //         graph_output_edges: vec![],
    //         graph_name: String::new(),
    //     }
    // }
    pub fn node_handles(&self) -> Vec<AnyHandle> {
        let mut handles = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            handles.push(AnyHandle {
                raw_handle: RawHandle::new(
                    NodeId {
                        key: node.key,
                        graph: self.graph_id,
                    },
                    self.param_sender.clone(),
                    self.shared_frame_clock.clone(),
                ),
                parameters: node.parameter_descriptions.clone(),
                parameter_hints: node.parameter_hints.clone(),
                inputs: node.inputs,
                outputs: node.outputs,
            });
        }
        handles
    }
    /// Generates the input to display the graph inspection using the Graphviz dot tool.
    pub fn to_dot_string(&self) -> String {
        let mut s = String::new();
        s.push_str("digraph D {\n");
        if self.num_outputs > 0 {
            // Create the nodes for graph inputs and outputs
            s.push_str(
                r"graph_out [
   shape=plaintext
   label=<
     <table border='1' cellborder='1'>
       <tr>",
            );
            for i in 0..self.num_outputs {
                s.push_str(&format!("<td port='i{i}'>Out {i}</td>"));
            }
            s.push_str(&format!(
                "</tr><tr><td colspan=\"{}\">Graph outputs</td></tr></table>>];\n\n",
                self.num_outputs
            ));
        }
        if self.num_inputs > 0 {
            s.push_str(&format!(
                "graph_in [
   shape=plaintext
   label=<
     <table border='1' cellborder='1'>
     <tr><td colspan=\"{}\">Graph inputs</td></tr>
       <tr>",
                self.num_inputs
            ));
            for i in 0..self.num_inputs {
                s.push_str(&format!("<td port='i{i}'>In {i}</td>"));
            }
            s.push_str("</tr></table>>];\n\n");
        }

        // Generate every node
        for (i, node) in self.nodes.iter().enumerate() {
            let color = "black";
            s.push_str(&format!(
                "\"{i}_{}\" [
                style = \"filled\" penwidth = 5 fillcolor = \"white\" shape = \"plain\"
            label=<\n<table border='0' cellborder='1' cellpadding='3'>\n",
                node.name
            ));
            if node.inputs > 0 {
                s.push_str("<tr>\n");
                for j in 0..node.inputs {
                    s.push_str(&format!("<td port='i{j}'>{j}</td>\n"));
                }
                s.push_str("</tr>\n");
            }
            s.push_str(&format!(
                "<tr><td bgcolor='{color}' colspan='{}'><font color='white'>\n",
                node.inputs.max(node.outputs)
            ));
            // Can't use < or > inside label names so we replace them with their HTML codes
            let name = node.name.clone();
            let name = name.replace("<", "&#60;");
            let name = name.replace(">", "&#62;");
            s.push_str(&format!("{i}: {}\n", name));
            s.push_str("</font></td></tr>\n");
            if node.outputs > 0 {
                s.push_str("<tr>");
                for j in 0..node.outputs {
                    s.push_str(&format!("<td port='o{j}'>{j}</td>"));
                }
                s.push_str("</tr>");
            }
            s.push_str(
                "</table>
                >];\n\n
            ",
            );
        }

        // Add all edges
        for (j, node) in self.nodes.iter().enumerate() {
            for edge in &node.input_edges {
                let EdgeInspection {
                    source,
                    from_index,
                    to_index,
                    is_feedback: _,
                } = edge;

                let source_name = match source {
                    EdgeSource::Node(node_key) => {
                        if let Some(i) = self.nodes.iter().position(|n| n.key == *node_key) {
                            format!("\"{i}_{}\"", self.nodes[i].name)
                        } else {
                            log::error!("Node in edge not found: {:?}", node_key);
                            continue;
                        }
                    }
                    EdgeSource::Graph => "graph_in".to_string(),
                };
                let node_name = &node.name;

                s.push_str(&format!(
                    "{source_name}:o{from_index} -> \"{j}_{node_name}\":i{to_index}\n"
                ));
            }
        }
        for edge in self.graph_output_edges.iter() {
            let EdgeInspection {
                source,
                from_index,
                to_index,
                is_feedback: _,
            } = edge;
            let source_name = match source {
                EdgeSource::Node(node_key) => {
                    if let Some(j) = self.nodes.iter().position(|n| n.key == *node_key) {
                        format!("{j}_{}", self.nodes[j].name)
                    } else {
                        log::error!("Node in edge not found: {:?}", node_key);
                        continue;
                    }
                }
                EdgeSource::Graph => "graph_in".to_string(),
            };
            let node_name = "graph_out";
            s.push_str(&format!(
                "\"{source_name}\":o{from_index} -> {node_name}:i{to_index} [penwidth=3] \n"
            ));
        }

        s.push_str("\n}");
        s
    }
    #[cfg(feature = "std")]
    pub fn show_dot_svg(&self) {
        let dot_string = self.to_dot_string();
        let mut dot_command = std::process::Command::new("dot")
            .arg("-Tsvg")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = dot_command.stdin.take().expect("Failed to open stdin");
        std::thread::spawn(move || {
            std::io::Write::write_all(&mut stdin, dot_string.as_bytes()).unwrap();
        });
        let output = dot_command.wait_with_output().unwrap();
        std::fs::write("graph.svg", output.stdout).unwrap();
        open::that("graph.svg").unwrap();
    }
}

#[derive(Debug, Clone)]
/// Metadata about a node in a graph
pub struct NodeInspection {
    /// The name of the node (usually the name of the UGen inside it)
    pub name: String,
    /// The address of the n    ode, usable to schedule changes to the node or free it
    pub key: NodeKey,
    pub inputs: u16,
    pub outputs: u16,
    /// Edges going into this node
    pub input_edges: Vec<EdgeInspection>,
    pub parameter_descriptions: Vec<&'static str>,
    pub parameter_hints: Vec<ParameterHint>,
    pub unconnected: bool,
    pub is_graph: Option<GraphId>,
}

#[derive(Debug, Clone, Copy)]
/// Metadata for an edge.
#[allow(missing_docs)]
pub struct EdgeInspection {
    pub source: EdgeSource,
    pub from_index: u16,
    pub to_index: u16,
    pub is_feedback: bool,
}

#[derive(Debug, Clone, Copy)]
/// Edge source type used for inspection. The index of a node is only valid for that specific GraphInspection.
#[allow(missing_docs)]
pub enum EdgeSource {
    Node(NodeKey),
    Graph,
}
