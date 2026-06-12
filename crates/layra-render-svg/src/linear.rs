//! Renderers for the linear chart types: timeline, journey, gitGraph.

use crate::theme::Theme;
use crate::{escape, FONT_STACK};
use layra_core::{ComponentRole, GitGraph, GitOp, Journey, Timeline};
use std::fmt::Write;

const PALETTE: [&str; 8] = [
    "#3b82f6", "#8b5cf6", "#f59e0b", "#10b981", "#ef4444", "#06b6d4", "#ec4899", "#84cc16",
];

fn svg_open(svg: &mut String, w: f32, h: f32, theme: &Theme) {
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.0} {h:.0}" width="{w:.0}" height="{h:.0}" font-family="{FONT_STACK}"><rect width="{w:.0}" height="{h:.0}" fill="{}"/>"#,
        theme.background
    );
}

fn title_block(svg: &mut String, title: &Option<String>, theme: &Theme) -> f32 {
    if let Some(t) = title {
        let _ = write!(
            svg,
            r#"<text x="16" y="24" font-size="16" font-weight="700" fill="{}">{}</text>"#,
            theme.text,
            escape(t)
        );
        40.0
    } else {
        14.0
    }
}

/* ---------------- timeline ---------------- */

pub fn render_timeline(tl: &Timeline, theme: &Theme) -> String {
    if tl.periods.is_empty() {
        return empty("empty timeline");
    }
    const COL_W: f32 = 150.0;
    const EVENT_H: f32 = 26.0;

    let max_events = tl.periods.iter().map(|p| p.events.len()).max().unwrap_or(0);
    let width = 32.0 + tl.periods.len() as f32 * COL_W;
    let mut svg = String::with_capacity(2048);
    let title_h = if tl.title.is_some() { 40.0 } else { 14.0 };
    let spine_y = title_h + 46.0;
    let height = spine_y + 30.0 + max_events as f32 * EVENT_H + 16.0;

    svg_open(&mut svg, width, height, theme);
    title_block(&mut svg, &tl.title, theme);

    // Spine.
    let _ = write!(
        svg,
        r#"<line x1="16" y1="{spine_y:.1}" x2="{:.1}" y2="{spine_y:.1}" stroke="{}" stroke-width="2"/>"#,
        width - 16.0,
        theme.cluster_stroke
    );

    // Section color assignment in order of appearance.
    let mut sections: Vec<&str> = Vec::new();
    for p in &tl.periods {
        if let Some(s) = &p.section {
            if !sections.contains(&s.as_str()) {
                sections.push(s);
            }
        }
    }

    for (i, period) in tl.periods.iter().enumerate() {
        let cx = 16.0 + COL_W / 2.0 + i as f32 * COL_W;
        let color = period
            .section
            .as_deref()
            .and_then(|s| sections.iter().position(|x| *x == s))
            .map(|i| PALETTE[i % PALETTE.len()])
            .unwrap_or(PALETTE[0]);

        // Period pill on the spine.
        let pw = (period.label.len() as f32 * 7.5 + 20.0).min(COL_W - 12.0);
        let _ = write!(
            svg,
            r##"<circle cx="{cx:.1}" cy="{spine_y:.1}" r="5" fill="{color}"/><rect x="{:.1}" y="{:.1}" width="{pw:.1}" height="24" rx="12" fill="{color}"/><text x="{cx:.1}" y="{:.1}" font-size="12.5" font-weight="600" fill="#fff" text-anchor="middle" dominant-baseline="central">{}</text>"##,
            cx - pw / 2.0,
            spine_y - 38.0,
            spine_y - 26.0,
            escape(&period.label)
        );

        // Events stacked below.
        for (k, event) in period.events.iter().enumerate() {
            let ey = spine_y + 28.0 + k as f32 * EVENT_H;
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{ey:.1}" width="{:.1}" height="20" rx="4" fill="{}" stroke="{}" stroke-width="1"/><text x="{cx:.1}" y="{:.1}" font-size="11.5" fill="{}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
                cx - (COL_W - 16.0) / 2.0,
                COL_W - 16.0,
                theme.cluster_fill,
                theme.cluster_stroke,
                ey + 10.0,
                theme.text,
                escape(event)
            );
        }
        let _ = write!(
            svg,
            r#"<line x1="{cx:.1}" y1="{spine_y:.1}" x2="{cx:.1}" y2="{:.1}" stroke="{color}" stroke-width="1.4"/>"#,
            spine_y + 24.0
        );
    }

    svg.push_str("</svg>");
    svg
}

/* ---------------- journey ---------------- */

pub fn render_journey(j: &Journey, theme: &Theme) -> String {
    let total: usize = j.sections.iter().map(|s| s.tasks.len()).sum();
    if total == 0 {
        return empty("empty journey");
    }
    const COL_W: f32 = 130.0;
    const SCORE_H: f32 = 26.0; // vertical px per score point

    let width = 32.0 + total as f32 * COL_W;
    let mut svg = String::with_capacity(2048);
    let title_h = if j.title.is_some() { 40.0 } else { 14.0 };
    let band_top = title_h + 26.0;
    let band_h = 5.0 * SCORE_H;
    let label_h = 64.0;
    let height = band_top + band_h + label_h;

    svg_open(&mut svg, width, height, theme);
    title_block(&mut svg, &j.title, theme);

    // Score bands (5 at top → 1 at bottom).
    for s in 1..=5u8 {
        let y = band_top + (5 - s) as f32 * SCORE_H;
        let _ = write!(
            svg,
            r#"<line x1="16" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="{}" stroke-width="0.6"/><text x="8" y="{:.1}" font-size="10" fill="{}">{s}</text>"#,
            width - 16.0,
            theme.cluster_stroke,
            y + 4.0,
            theme.edge_label
        );
    }

    let mut x = 16.0;
    let mut prev: Option<(f32, f32)> = None;
    for (si, section) in j.sections.iter().enumerate() {
        let color = PALETTE[si % PALETTE.len()];
        if !section.name.is_empty() && !section.tasks.is_empty() {
            let sw = section.tasks.len() as f32 * COL_W;
            let _ = write!(
                svg,
                r##"<rect x="{x:.1}" y="{:.1}" width="{sw:.1}" height="18" rx="4" fill="{color}" opacity="0.85"/><text x="{:.1}" y="{:.1}" font-size="11.5" font-weight="600" fill="#fff" text-anchor="middle" dominant-baseline="central">{}</text>"##,
                height - 24.0,
                x + sw / 2.0,
                height - 15.0,
                escape(&section.name)
            );
        }
        for task in &section.tasks {
            let cx = x + COL_W / 2.0;
            let cy = band_top + (5 - task.score) as f32 * SCORE_H + SCORE_H / 2.0;
            if let Some((px, py)) = prev {
                let _ = write!(
                    svg,
                    r#"<line x1="{px:.1}" y1="{py:.1}" x2="{cx:.1}" y2="{cy:.1}" stroke="{}" stroke-width="1.6"/>"#,
                    theme.edge
                );
            }
            let face = match task.score {
                4 | 5 => "#10b981",
                3 => "#f59e0b",
                _ => "#ef4444",
            };
            let _ = write!(
                svg,
                r##"<circle cx="{cx:.1}" cy="{cy:.1}" r="11" fill="{face}"/><text x="{cx:.1}" y="{cy:.1}" font-size="11" font-weight="700" fill="#fff" text-anchor="middle" dominant-baseline="central">{}</text>"##,
                task.score
            );
            let _ = write!(
                svg,
                r#"<text x="{cx:.1}" y="{:.1}" font-size="11.5" fill="{}" text-anchor="middle">{}</text>"#,
                height - label_h + 18.0,
                theme.text,
                escape(&task.label)
            );
            if !task.actors.is_empty() {
                let _ = write!(
                    svg,
                    r#"<text x="{cx:.1}" y="{:.1}" font-size="10" fill="{}" text-anchor="middle">{}</text>"#,
                    height - label_h + 32.0,
                    theme.edge_label,
                    escape(&task.actors.join(", "))
                );
            }
            prev = Some((cx, cy));
            x += COL_W;
        }
    }

    svg.push_str("</svg>");
    svg
}

/* ---------------- gitGraph ---------------- */

pub fn render_git(g: &GitGraph, theme: &Theme) -> String {
    let commit_count = g
        .ops
        .iter()
        .filter(|op| matches!(op, GitOp::Commit { .. } | GitOp::Merge { .. }))
        .count();
    if commit_count == 0 {
        return empty("empty git graph");
    }

    const STEP_X: f32 = 56.0;
    const LANE_H: f32 = 44.0;
    const R: f32 = 7.0;

    let label_w = g.branches.iter().map(|b| b.len()).max().unwrap_or(4) as f32 * 7.5 + 24.0;
    let width = label_w + commit_count as f32 * STEP_X + 40.0;
    let height = 24.0 + g.branches.len() as f32 * LANE_H + 16.0;

    let mut svg = String::with_capacity(2048);
    svg_open(&mut svg, width, height, theme);

    let lane_y = |lane: usize| 24.0 + lane as f32 * LANE_H + LANE_H / 2.0;

    // Branch labels + lane lines.
    for (i, name) in g.branches.iter().enumerate() {
        let y = lane_y(i);
        let color = PALETTE[i % PALETTE.len()];
        let _ = write!(
            svg,
            r#"<text x="10" y="{y:.1}" font-size="12" font-weight="600" fill="{color}" dominant-baseline="central">{}</text>"#,
            escape(name)
        );
    }

    // Walk ops, advancing one x step per commit/merge.
    let mut x = label_w;
    let mut last_dot_on: Vec<Option<f32>> = vec![None; g.branches.len()];
    let mut fork_from: Vec<Option<(usize, f32)>> = vec![None; g.branches.len()];
    let mut current = 0usize;

    for op in &g.ops {
        match op {
            GitOp::Branch { name } => {
                let idx = g.branches.iter().position(|b| b == name).unwrap_or(0);
                fork_from[idx] = Some((current, x));
                current = idx;
            }
            GitOp::Commit { id, tag, branch } => {
                current = *branch;
                x += STEP_X;
                let y = lane_y(*branch);
                let color = PALETTE[*branch % PALETTE.len()];

                // Lane segment from previous dot (or fork point).
                if let Some(px) = last_dot_on[*branch] {
                    let _ = write!(
                        svg,
                        r#"<line x1="{px:.1}" y1="{y:.1}" x2="{x:.1}" y2="{y:.1}" stroke="{color}" stroke-width="2"/>"#
                    );
                } else if let Some((from, fx)) = fork_from[*branch] {
                    let fy = lane_y(from);
                    let _ = write!(
                        svg,
                        r#"<path d="M{fx:.1} {fy:.1}C{:.1} {fy:.1} {:.1} {y:.1} {x:.1} {y:.1}" fill="none" stroke="{color}" stroke-width="2"/>"#,
                        fx + STEP_X * 0.6,
                        x - STEP_X * 0.6
                    );
                }
                let _ = write!(
                    svg,
                    r#"<circle cx="{x:.1}" cy="{y:.1}" r="{R}" fill="{color}" stroke="{}" stroke-width="2"/>"#,
                    theme.background
                );
                if let Some(text) = id.as_deref().or(tag.as_deref()) {
                    let _ = write!(
                        svg,
                        r#"<text x="{x:.1}" y="{:.1}" font-size="10" fill="{}" text-anchor="middle">{}</text>"#,
                        y - 13.0,
                        theme.edge_label,
                        escape(text)
                    );
                }
                last_dot_on[*branch] = Some(x);
            }
            GitOp::Merge {
                from_branch,
                into_branch,
                tag,
            } => {
                current = *into_branch;
                x += STEP_X;
                let y = lane_y(*into_branch);
                let fy = lane_y(*from_branch);
                let color = PALETTE[*into_branch % PALETTE.len()];
                let from_color = PALETTE[*from_branch % PALETTE.len()];

                if let Some(px) = last_dot_on[*into_branch] {
                    let _ = write!(
                        svg,
                        r#"<line x1="{px:.1}" y1="{y:.1}" x2="{x:.1}" y2="{y:.1}" stroke="{color}" stroke-width="2"/>"#
                    );
                }
                let fx = last_dot_on[*from_branch].unwrap_or(x - STEP_X);
                let _ = write!(
                    svg,
                    r#"<path d="M{fx:.1} {fy:.1}C{:.1} {fy:.1} {:.1} {y:.1} {x:.1} {y:.1}" fill="none" stroke="{from_color}" stroke-width="2"/>"#,
                    fx + STEP_X * 0.6,
                    x - STEP_X * 0.6
                );
                // Merge commit: double ring.
                let _ = write!(
                    svg,
                    r#"<circle cx="{x:.1}" cy="{y:.1}" r="{R}" fill="{}" stroke="{color}" stroke-width="2.5"/>"#,
                    theme.background
                );
                if let Some(t) = tag {
                    let _ = write!(
                        svg,
                        r#"<text x="{x:.1}" y="{:.1}" font-size="10" fill="{}" text-anchor="middle">{}</text>"#,
                        y - 13.0,
                        theme.edge_label,
                        escape(t)
                    );
                }
                last_dot_on[*into_branch] = Some(x);
            }
        }
    }

    svg.push_str("</svg>");
    svg
}

fn empty(message: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 220 60" width="220" height="60"><text x="110" y="30" text-anchor="middle" font-size="13">{message}</text></svg>"#
    )
}

// Keep role colors imported for future use in journey faces.
#[allow(dead_code)]
fn _unused(theme: &Theme) -> &'static str {
    theme.role_color(ComponentRole::Generic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use layra_core::{JourneySection, JourneyTask, TimelinePeriod};

    #[test]
    fn timeline_renders_periods() {
        let tl = Timeline {
            title: Some("History".into()),
            periods: vec![
                TimelinePeriod {
                    label: "2024".into(),
                    events: vec!["idea".into(), "proto".into()],
                    section: None,
                },
                TimelinePeriod {
                    label: "2025".into(),
                    events: vec!["launch".into()],
                    section: None,
                },
            ],
        };
        let svg = render_timeline(&tl, &Theme::light());
        assert!(svg.contains("2024") && svg.contains("launch"));
    }

    #[test]
    fn journey_renders_scores() {
        let j = Journey {
            title: None,
            sections: vec![JourneySection {
                name: "Morning".into(),
                tasks: vec![JourneyTask {
                    label: "Coffee".into(),
                    score: 5,
                    actors: vec!["Me".into()],
                }],
            }],
        };
        let svg = render_journey(&j, &Theme::dark());
        assert!(svg.contains("Coffee"));
        assert!(svg.contains("#10b981"), "score 5 = green face");
    }

    #[test]
    fn git_renders_merge_curve() {
        let g = GitGraph {
            branches: vec!["main".into(), "dev".into()],
            ops: vec![
                GitOp::Commit {
                    id: None,
                    tag: None,
                    branch: 0,
                },
                GitOp::Branch { name: "dev".into() },
                GitOp::Commit {
                    id: None,
                    tag: None,
                    branch: 1,
                },
                GitOp::Merge {
                    from_branch: 1,
                    into_branch: 0,
                    tag: Some("v1".into()),
                },
            ],
        };
        let svg = render_git(&g, &Theme::light());
        assert!(svg.contains("main") && svg.contains("dev"));
        assert!(svg.contains("v1"));
        assert!(svg.matches("<circle").count() >= 3);
        assert!(svg.contains("<path"), "fork/merge curves");
    }
}
