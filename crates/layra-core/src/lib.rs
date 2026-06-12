//! # layra-core
//!
//! Core intermediate representation (IR) for the Layra diagram engine.
//!
//! The IR is the single contract between every stage of the pipeline:
//!
//! ```text
//! parser → IR → text measure → layout → routing → render
//! ```
//!
//! Everything downstream of the parser operates on [`Graph`]; nothing
//! depends on any source syntax (Mermaid, native DSL, JSON, ...).

pub mod geometry;
pub mod graph;
pub mod style;

pub use geometry::{Point, Rect, Size};
pub use graph::{Direction, Edge, EdgeId, EdgeKind, Graph, Node, NodeId, Subgraph, SubgraphId};
pub use style::{ComponentRole, EdgeStyle, NodeShape};
