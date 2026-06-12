//! State diagram parser: Mermaid `stateDiagram-v2` dialect, mapped onto the
//! flowchart [`Graph`] IR — states are nodes, transitions are edges, so the
//! whole Sugiyama pipeline and SVG renderer come for free.
//!
//! Mappings:
//! - `[*]` initial/final pseudo-state → small filled circle (`Circle` shape,
//!   `Highlight` role, label `●`)
//! - `A --> B: label` → edge with label
//! - `state "Long name" as A` / bare `A` declarations
//! - `note right of A ... end note` → ignored for layout v1 (rendered as a
//!   future enhancement), `direction X` honored
//! - composite states (`state A { ... }`) → subgraphs

use crate::ParseError;
use layra_core::{
    ComponentRole, Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, NodeId, NodeShape, Subgraph,
    SubgraphId,
};
use std::collections::HashMap;

pub(crate) fn parse(lines: &[(usize, &str)]) -> Result<Graph, ParseError> {
    let mut p = StateParser {
        graph: Graph::new(Direction::TopBottom),
        by_name: HashMap::new(),
        star_count: 0,
        subgraph_stack: Vec::new(),
        in_note: false,
    };

    for &(ln, line) in lines {
        p.line(ln, line)?;
    }
    Ok(p.graph)
}

struct StateParser {
    graph: Graph,
    by_name: HashMap<String, NodeId>,
    /// Each `[*]` occurrence is a distinct pseudo-state.
    star_count: usize,
    subgraph_stack: Vec<SubgraphId>,
    in_note: bool,
}

impl StateParser {
    fn line(&mut self, ln: usize, line: &str) -> Result<(), ParseError> {
        // Multi-line notes: swallow until `end note`.
        if self.in_note {
            if line == "end note" {
                self.in_note = false;
            }
            return Ok(());
        }
        if line.starts_with("note ") {
            if !line.contains(':') {
                self.in_note = true; // block form
            }
            return Ok(());
        }

        if let Some(rest) = line.strip_prefix("direction ") {
            self.graph.direction = match rest.trim() {
                "LR" => Direction::LeftRight,
                "RL" => Direction::RightLeft,
                "BT" => Direction::BottomTop,
                _ => Direction::TopBottom,
            };
            return Ok(());
        }

        if line.starts_with("classDef") || line.starts_with("class ") || line.starts_with("style") {
            return Ok(());
        }

        // Composite state open/close.
        if let Some(rest) = line.strip_prefix("state ") {
            let rest = rest.trim();
            if let Some(body) = rest.strip_suffix('{') {
                let name = body.trim().trim_matches('"');
                let parent = self.subgraph_stack.last().copied();
                let id = self.graph.add_subgraph(Subgraph {
                    name: name.to_string(),
                    label: name.to_string(),
                    nodes: Vec::new(),
                    parent,
                    rect: Default::default(),
                });
                self.subgraph_stack.push(id);
                return Ok(());
            }
            // `state "Label" as A`
            if let Some((label, name)) = rest.split_once(" as ") {
                let id = self.intern(name.trim());
                self.graph.node_mut(id).label = label.trim().trim_matches('"').to_string();
                return Ok(());
            }
            // bare `state A`
            self.intern(rest.trim_matches('"'));
            return Ok(());
        }
        if line == "}" {
            self.subgraph_stack.pop();
            return Ok(());
        }

        // Transition: `A --> B` / `A --> B: label`
        if let Some(arrow_at) = line.find("-->") {
            let from = line[..arrow_at].trim();
            let rest = line[arrow_at + 3..].trim();
            let (to, label) = match rest.split_once(':') {
                Some((t, l)) => (t.trim(), Some(crate::clean_edge_label(l))),
                None => (rest, None),
            };
            let src = self.intern(from);
            let dst = self.intern(to);
            self.graph.add_edge(Edge {
                source: src,
                target: dst,
                label,
                style: EdgeStyle::Solid,
                kind: EdgeKind::Arrow,
                points: vec![],
                label_pos: None,
            });
            return Ok(());
        }

        // Standalone state reference.
        if !line.is_empty() && line.chars().all(|c| c.is_alphanumeric() || c == '_') {
            self.intern(line);
            return Ok(());
        }

        Err(ParseError::Syntax {
            line: ln,
            message: format!("cannot parse state statement '{line}'"),
        })
    }

    fn intern(&mut self, name: &str) -> NodeId {
        if name == "[*]" {
            // Each [*] is positionally distinct (initial vs final).
            self.star_count += 1;
            let key = format!("[*]{}", self.star_count);
            let mut node = Node::new(key.clone(), "●");
            node.shape = NodeShape::Circle;
            node.role = ComponentRole::Highlight;
            node.parent = self.subgraph_stack.last().copied();
            let id = self.graph.add_node(node);
            self.claim(id);
            return id;
        }

        if let Some(&id) = self.by_name.get(name) {
            return id;
        }
        let mut node = Node::new(name, name);
        node.shape = NodeShape::RoundedRect;
        node.role = ComponentRole::Generic;
        node.parent = self.subgraph_stack.last().copied();
        let id = self.graph.add_node(node);
        self.by_name.insert(name.to_string(), id);
        self.claim(id);
        id
    }

    fn claim(&mut self, id: NodeId) {
        if let Some(&sg) = self.subgraph_stack.last() {
            let list = &mut self.graph.subgraphs[sg.index()].nodes;
            if !list.contains(&id) {
                list.push(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_src(src: &str) -> Graph {
        let owned: Vec<(usize, String)> = src
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim().to_string()))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        let borrowed: Vec<(usize, &str)> = owned.iter().map(|(n, l)| (*n, l.as_str())).collect();
        parse(&borrowed).unwrap()
    }

    #[test]
    fn parses_tcp_state_machine_shape() {
        let g = parse_src(
            "direction TB\n\
             [*] --> CLOSED\n\
             CLOSED --> SYN_SENT: active open / send SYN\n\
             SYN_SENT --> ESTABLISHED: recv SYN-ACK / send ACK\n\
             ESTABLISHED --> [*]",
        );
        // 2 pseudo-states + 3 named states
        assert_eq!(g.nodes.len(), 5);
        assert_eq!(g.edges.len(), 4);
        assert_eq!(g.edges[1].label.as_deref(), Some("active open / send SYN"));
        // pseudo-states are circles
        assert_eq!(g.nodes[0].shape, NodeShape::Circle);
    }

    #[test]
    fn block_notes_are_skipped() {
        let g = parse_src(
            "direction LR\n\
             Closed --> Open : failures > threshold\n\
             note right of Closed\n\
             normal — calls go through\n\
             end note\n\
             Open --> Closed : probe succeeds",
        );
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 2);
    }
}
