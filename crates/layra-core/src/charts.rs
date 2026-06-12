//! Chart-style diagrams (no graph layout): pie today, gantt-adjacent later.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PieChart {
    pub title: Option<String>,
    /// `showData` flag: print values next to labels.
    pub show_data: bool,
    pub slices: Vec<(String, f64)>,
}
