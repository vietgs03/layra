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

pub mod charts;
pub mod geometry;
pub mod graph;
pub mod sequence;
pub mod style;

pub use charts::PieChart;
pub use geometry::{Point, Rect, Size};
pub use graph::{Direction, Edge, EdgeId, EdgeKind, Graph, Node, NodeId, Subgraph, SubgraphId};
pub use sequence::{
    FrameKind, NotePosition, Participant, ParticipantId, SeqArrow, SeqItem, SeqMessage, SeqNote,
    SequenceDiagram,
};
pub use style::{ComponentRole, EdgeStyle, NodeShape};

/// A parsed document: every diagram type Layra understands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Document {
    /// Flowcharts, state diagrams, class diagrams, ER diagrams — anything
    /// that maps onto the graph pipeline.
    Graph(Graph),
    Sequence(SequenceDiagram),
    Pie(PieChart),
}
