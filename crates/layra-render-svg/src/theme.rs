//! Color theme: the component-role taxonomy ported from diagram-toolkit's
//! BBG look — flat fills, colored role borders, neutral edges.

use layra_core::ComponentRole;

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: &'static str,
    pub node_fill: &'static str,
    pub text: &'static str,
    pub edge: &'static str,
    pub edge_label: &'static str,
    pub cluster_fill: &'static str,
    pub cluster_stroke: &'static str,
    pub cluster_title: &'static str,
    pub note_fill: &'static str,
    pub note_stroke: &'static str,
    pub note_text: &'static str,
    pub is_dark: bool,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            background: "#ffffff",
            node_fill: "#ffffff",
            text: "#1a1d23",
            edge: "#8a919e",
            edge_label: "#4b5563",
            cluster_fill: "#fafbfc",
            cluster_stroke: "#c3c9d4",
            cluster_title: "#475569",
            note_fill: "#fef9c3",
            note_stroke: "#d9c84a",
            note_text: "#5b5314",
            is_dark: false,
        }
    }

    pub fn dark() -> Self {
        Self {
            background: "#0f1115",
            node_fill: "#171a21",
            text: "#e5e9f0",
            edge: "#6b7280",
            edge_label: "#9ca3af",
            cluster_fill: "#13161c",
            cluster_stroke: "#2d3340",
            cluster_title: "#64748b",
            note_fill: "#2a2616",
            note_stroke: "#6b5f1d",
            note_text: "#d8c95a",
            is_dark: true,
        }
    }

    /// Role border colors — one hue per semantic role, consistent across
    /// light and dark.
    pub fn role_color(&self, role: ComponentRole) -> &'static str {
        match role {
            ComponentRole::Generic => "#94a3b8",
            ComponentRole::Service => "#3b82f6",
            ComponentRole::Database => "#8b5cf6",
            ComponentRole::Cache => "#ef4444",
            ComponentRole::Queue => "#f59e0b",
            ComponentRole::Gateway => "#06b6d4",
            ComponentRole::Client => "#10b981",
            ComponentRole::External => "#64748b",
            ComponentRole::Storage => "#a855f7",
            ComponentRole::Compute => "#0ea5e9",
            ComponentRole::Highlight => "#ec4899",
        }
    }
}
