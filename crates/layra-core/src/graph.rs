//! The diagram graph: arena-style storage with index-based IDs.
//!
//! IDs are plain `u32` newtypes into `Vec` arenas — cache-friendly, trivially
//! serializable, and free of borrow-checker friction in graph algorithms.

use crate::geometry::{Point, Rect, Size};
use crate::style::{ComponentRole, EdgeStyle, NodeShape};
use serde::{Deserialize, Serialize};

macro_rules! arena_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

arena_id!(NodeId);
arena_id!(EdgeId);
arena_id!(SubgraphId);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Stable user-facing identifier from the source text (e.g. `db` in
    /// `db["Postgres"]`).
    pub name: String,
    pub label: String,
    pub shape: NodeShape,
    pub role: ComponentRole,
    /// Optional icon reference, e.g. `logos:postgresql`.
    pub icon: Option<String>,
    /// Measured label size; filled by the text-measure stage.
    pub size: Size,
    /// Final position; filled by the layout stage.
    pub rect: Rect,
    /// Owning subgraph, if any.
    pub parent: Option<SubgraphId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    #[default]
    Arrow,
    Open,
    Bidirectional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub label: Option<String>,
    pub style: EdgeStyle,
    pub kind: EdgeKind,
    /// Routed polyline; filled by the routing stage.
    pub points: Vec<Point>,
    /// Position for the edge label; filled by the routing stage.
    pub label_pos: Option<Point>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subgraph {
    pub name: String,
    pub label: String,
    pub nodes: Vec<NodeId>,
    pub parent: Option<SubgraphId>,
    /// Computed bounding box; filled by the layout stage.
    pub rect: Rect,
}

/// Layout direction, matching Mermaid's `TB`/`LR`/... vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    TopBottom,
    LeftRight,
    BottomTop,
    RightLeft,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub subgraphs: Vec<Subgraph>,
}

impl Graph {
    pub fn new(direction: Direction) -> Self {
        Self {
            direction,
            ..Default::default()
        }
    }

    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(node);
        id
    }

    pub fn add_edge(&mut self, edge: Edge) -> EdgeId {
        let id = EdgeId(self.edges.len() as u32);
        self.edges.push(edge);
        id
    }

    pub fn add_subgraph(&mut self, subgraph: Subgraph) -> SubgraphId {
        let id = SubgraphId(self.subgraphs.len() as u32);
        self.subgraphs.push(subgraph);
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.index()]
    }

    /// Find a node by its source-text name.
    pub fn node_by_name(&self, name: &str) -> Option<NodeId> {
        self.nodes
            .iter()
            .position(|n| n.name == name)
            .map(|i| NodeId(i as u32))
    }

    /// Bounding box of the entire laid-out diagram.
    pub fn bounds(&self) -> Rect {
        let mut iter = self.nodes.iter().map(|n| n.rect);
        let Some(first) = iter.next() else {
            return Rect::default();
        };
        let node_bounds = iter.fold(first, |acc, r| acc.union(&r));
        self.subgraphs
            .iter()
            .fold(node_bounds, |acc, sg| acc.union(&sg.rect))
    }
}

impl Node {
    /// Convenience constructor with sensible defaults; sizes and positions
    /// are filled by later pipeline stages.
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            shape: NodeShape::default(),
            role: ComponentRole::default(),
            icon: None,
            size: Size::default(),
            rect: Rect::default(),
            parent: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_basics() {
        let mut g = Graph::new(Direction::LeftRight);
        let a = g.add_node(Node::new("a", "Service A"));
        let b = g.add_node(Node::new("b", "Service B"));
        g.add_edge(Edge {
            source: a,
            target: b,
            label: Some("calls".into()),
            style: EdgeStyle::Solid,
            kind: EdgeKind::Arrow,
            points: vec![],
            label_pos: None,
        });
        assert_eq!(g.node_by_name("b"), Some(b));
        assert_eq!(g.edges.len(), 1);
    }
}
