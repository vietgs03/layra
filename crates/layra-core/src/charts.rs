//! Chart-style diagrams (no graph layout): pie today, gantt-adjacent later.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PieChart {
    pub title: Option<String>,
    /// `showData` flag: print values next to labels.
    pub show_data: bool,
    pub slices: Vec<(String, f64)>,
}

/// Task status; drives bar color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    #[default]
    Planned,
    Active,
    Done,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttTask {
    pub label: String,
    pub id: Option<String>,
    /// Civil days since 1970-01-01.
    pub start: i64,
    pub end: i64,
    pub status: TaskStatus,
    pub milestone: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GanttSection {
    pub name: String,
    pub tasks: Vec<GanttTask>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GanttChart {
    pub title: Option<String>,
    pub sections: Vec<GanttSection>,
}

impl GanttChart {
    pub fn tasks(&self) -> impl Iterator<Item = &GanttTask> {
        self.sections.iter().flat_map(|s| s.tasks.iter())
    }

    /// (min start, max end) across all tasks.
    pub fn time_range(&self) -> Option<(i64, i64)> {
        let mut range: Option<(i64, i64)> = None;
        for t in self.tasks() {
            range = Some(match range {
                None => (t.start, t.end),
                Some((lo, hi)) => (lo.min(t.start), hi.max(t.end)),
            });
        }
        range
    }
}

/* ---- timeline ---- */

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimelinePeriod {
    pub label: String,
    pub events: Vec<String>,
    /// Section this period belongs to, if any.
    pub section: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub title: Option<String>,
    pub periods: Vec<TimelinePeriod>,
}

/* ---- user journey ---- */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyTask {
    pub label: String,
    /// 1..=5 satisfaction score.
    pub score: u8,
    pub actors: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JourneySection {
    pub name: String,
    pub tasks: Vec<JourneyTask>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Journey {
    pub title: Option<String>,
    pub sections: Vec<JourneySection>,
}

/* ---- git graph ---- */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitOp {
    Commit {
        id: Option<String>,
        tag: Option<String>,
        branch: usize,
    },
    /// New branch forked from `from` at the current tip.
    Branch { name: String },
    Merge {
        from_branch: usize,
        into_branch: usize,
        tag: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitGraph {
    /// Branch names in creation order; index = lane.
    pub branches: Vec<String>,
    pub ops: Vec<GitOp>,
}
