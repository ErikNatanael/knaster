//! # Inspection
//!
//! Metadata from the structs in this module can be used to visualise and/or
//! manipulate a graph based on the whole graph structure.

use crate::core::eprintln;
use crate::graph::{GraphId, NodeKey};
use alloc::{format, string::String, string::ToString, vec, vec::Vec};

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
    pub num_inputs: usize,
    /// Number of outputs from the graph
    pub num_outputs: usize,
    /// The ID of the graph
    pub graph_id: crate::graph::GraphId,
    pub graph_name: String,
}

impl GraphInspection {
    /// Create an empty GraphInspection
    pub fn empty() -> Self {
        Self {
            nodes: vec![],
            num_inputs: 0,
            num_outputs: 0,
            graph_id: 0,
            graph_output_edges: vec![],
            graph_name: String::new(),
        }
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
            let color = if node.pending_removal { "red" } else { "black" };
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
                        if let Some(i) = self.nodes.iter().position(|n| n.address == *node_key) {
                            format!("\"{i}_{}\"", self.nodes[i].name)
                        } else {
                            eprintln!("Node in edge not found: {:?}", node_key);
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
                    if let Some(j) = self.nodes.iter().position(|n| n.address == *node_key) {
                        format!("{j}_{}", self.nodes[j].name)
                    } else {
                        eprintln!("Node in edge not found: {:?}", node_key);
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
}

#[derive(Debug, Clone)]
/// Metadata about a node in a graph
pub struct NodeInspection {
    /// The name of the node (usually the name of the UGen inside it)
    pub name: String,
    /// The address of the n    ode, usable to schedule changes to the node or free it
    pub address: NodeKey,
    pub inputs: usize,
    pub outputs: usize,
    /// Edges going into this node
    pub input_edges: Vec<EdgeInspection>,
    pub parameter_descriptions: Vec<String>,
    pub pending_removal: bool,
    pub unconnected: bool,
    pub is_graph: Option<GraphId>,
}

#[derive(Debug, Clone, Copy)]
/// Metadata for an edge.
#[allow(missing_docs)]
pub struct EdgeInspection {
    pub source: EdgeSource,
    pub from_index: usize,
    pub to_index: usize,
    pub is_feedback: bool,
}

#[derive(Debug, Clone, Copy)]
/// Edge source type used for inspection. The index of a node is only valid for that specific GraphInspection.
#[allow(missing_docs)]
pub enum EdgeSource {
    Node(NodeKey),
    Graph,
}
