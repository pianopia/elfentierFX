//! Procedural node graph stub.

use serde::{Deserialize, Serialize};

/// Unique identifier for a node in the procedural graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// Unique identifier for an edge connecting two node ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub u64);

/// A procedural computation graph (stub).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub name: String,
    pub nodes: Vec<NodeId>,
    pub edges: Vec<EdgeId>,
}

impl Graph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}
