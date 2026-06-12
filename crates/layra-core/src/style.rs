//! Style vocabulary: shapes, component roles (the taxonomy from
//! diagram-toolkit), and edge styles. Roles drive theming; shapes drive
//! geometry. They are deliberately separate axes.

use serde::{Deserialize, Serialize};

/// Geometric shape of a node. Determines outline path and anchor points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeShape {
    #[default]
    Rect,
    RoundedRect,
    /// Database / storage cylinder.
    Cylinder,
    Diamond,
    Hexagon,
    Circle,
    /// Horizontal queue / pipe shape.
    Queue,
    /// Stadium (pill) shape.
    Stadium,
    /// Person / actor figure.
    Actor,
    Cloud,
}

/// Semantic role of a component. Drives color taxonomy (BBG-style role
/// borders) independent of shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentRole {
    #[default]
    Generic,
    Service,
    Database,
    Cache,
    Queue,
    Gateway,
    Client,
    External,
    Storage,
    Compute,
    Highlight,
}

/// Visual style of an edge. In Layra, edge style is semantic: solid =
/// request flow, dashed = async/event, thick = hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeStyle {
    #[default]
    Solid,
    Dashed,
    Thick,
    Dotted,
}
