//! Gantt chart renderer: time-scaled bars per section with a date axis.

use crate::theme::Theme;
use crate::{escape, FONT_STACK};
use layra_core::{GanttChart, TaskStatus};
use std::fmt::Write;

const ROW_H: f32 = 30.0;
const BAR_H: f32 = 19.0;
const SECTION_GAP: f32 = 10.0;
const AXIS_H: f32 = 28.0;
const LABEL_COL: f32 = 8.0;
const CHART_W: f32 = 640.0;

pub fn render_gantt(chart: &GanttChart, theme: &Theme) -> String {
    let Some((t0, t1)) = chart.time_range() else {
        return r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 220 60" width="220" height="60"><text x="110" y="30" text-anchor="middle" font-size="13">empty gantt chart</text></svg>"#.to_string();
    };
    let span = (t1 - t0).max(1) as f32;

    // Label column sized to the longest task label.
    let label_w = chart.tasks().map(|t| t.label.len()).max().unwrap_or(0) as f32 * 7.2 + 24.0;
    let x0 = LABEL_COL + label_w;
    let scale = CHART_W / span;

    let title_h = if chart.title.is_some() { 36.0 } else { 10.0 };
    let total_rows: usize = chart.sections.iter().map(|s| s.tasks.len()).sum();
    let named_sections = chart.sections.iter().filter(|s| !s.name.is_empty()).count();
    let height = title_h
        + AXIS_H
        + total_rows as f32 * ROW_H
        + named_sections as f32 * 18.0
        + chart.sections.len() as f32 * SECTION_GAP
        + 20.0;
    let width = x0 + CHART_W + 30.0;

    let mut svg = String::with_capacity(4096);
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0} {height:.0}" width="{width:.0}" height="{height:.0}" font-family="{FONT_STACK}">"#
    );
    let _ = write!(
        svg,
        r#"<rect width="{width:.0}" height="{height:.0}" fill="{}"/>"#,
        theme.background
    );

    if let Some(title) = &chart.title {
        let _ = write!(
            svg,
            r#"<text x="16" y="24" font-size="16" font-weight="700" fill="{}">{}</text>"#,
            theme.text,
            escape(title)
        );
    }

    // Date axis: weekly gridlines if the span is short, else ~8 ticks.
    let tick_days = pick_tick(span as i64);
    let chart_top = title_h + AXIS_H;
    let chart_bottom = height - 14.0;
    let mut day = t0 - t0.rem_euclid(tick_days);
    while day <= t1 {
        if day >= t0 {
            let x = x0 + (day - t0) as f32 * scale;
            let _ = write!(
                svg,
                r#"<line x1="{x:.1}" y1="{chart_top:.1}" x2="{x:.1}" y2="{chart_bottom:.1}" stroke="{}" stroke-width="0.7"/><text x="{x:.1}" y="{:.1}" font-size="10.5" fill="{}" text-anchor="middle">{}</text>"#,
                theme.cluster_stroke,
                chart_top - 8.0,
                theme.edge_label,
                format_date(day)
            );
        }
        day += tick_days;
    }

    // Bars.
    let mut y = chart_top + 6.0;
    for section in &chart.sections {
        if !section.name.is_empty() {
            let _ = write!(
                svg,
                r#"<text x="{LABEL_COL}" y="{:.1}" font-size="12" font-weight="700" fill="{}">{}</text>"#,
                y + 8.0,
                theme.cluster_title,
                escape(&section.name)
            );
            y += 18.0;
        }
        for task in &section.tasks {
            let bx = x0 + (task.start - t0) as f32 * scale;
            let bw = ((task.end - task.start) as f32 * scale).max(2.0);
            let by = y + (ROW_H - BAR_H) / 2.0;
            let color = status_color(task.status, theme);

            let _ = write!(
                svg,
                r#"<text x="{:.1}" y="{:.1}" font-size="12" fill="{}" dominant-baseline="central">{}</text>"#,
                LABEL_COL,
                y + ROW_H / 2.0,
                theme.text,
                escape(&task.label)
            );

            if task.milestone {
                // Diamond at the start instant.
                let cy = y + ROW_H / 2.0;
                let r = BAR_H / 2.0;
                let _ = write!(
                    svg,
                    r#"<path d="M{bx:.1} {:.1}L{:.1} {cy:.1}L{bx:.1} {:.1}L{:.1} {cy:.1}Z" fill="{}"/>"#,
                    cy - r,
                    bx + r,
                    cy + r,
                    bx - r,
                    theme.role_color(layra_core::ComponentRole::Highlight)
                );
            } else {
                let opacity = if task.status == TaskStatus::Done {
                    0.55
                } else {
                    1.0
                };
                let _ = write!(
                    svg,
                    r#"<rect x="{bx:.1}" y="{by:.1}" width="{bw:.1}" height="{BAR_H}" rx="4" fill="{color}" opacity="{opacity}"/>"#
                );
            }
            y += ROW_H;
        }
        y += SECTION_GAP;
    }

    svg.push_str("</svg>");
    svg
}

fn status_color(status: TaskStatus, theme: &Theme) -> &'static str {
    use layra_core::ComponentRole as R;
    match status {
        TaskStatus::Planned => theme.role_color(R::Service),
        TaskStatus::Active => theme.role_color(R::Client),
        TaskStatus::Done => theme.role_color(R::External),
        TaskStatus::Critical => theme.role_color(R::Cache),
    }
}

/// Pick a tick interval giving roughly 6-10 gridlines.
fn pick_tick(span_days: i64) -> i64 {
    for candidate in [1, 2, 7, 14, 30, 90, 180, 365] {
        if span_days / candidate <= 10 {
            return candidate;
        }
    }
    365
}

/// Civil days since epoch → `MM-DD` (year omitted to keep ticks compact).
fn format_date(days: i64) -> String {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use layra_core::{GanttSection, GanttTask};

    #[test]
    fn renders_bars_and_axis() {
        let chart = GanttChart {
            title: Some("Plan".into()),
            sections: vec![GanttSection {
                name: "Build".into(),
                tasks: vec![
                    GanttTask {
                        label: "Engine".into(),
                        id: None,
                        start: 0,
                        end: 30,
                        status: TaskStatus::Done,
                        milestone: false,
                    },
                    GanttTask {
                        label: "Ship".into(),
                        id: None,
                        start: 30,
                        end: 30,
                        status: TaskStatus::Planned,
                        milestone: true,
                    },
                ],
            }],
        };
        let svg = render_gantt(&chart, &Theme::light());
        assert!(svg.contains("Plan"));
        assert!(svg.contains("Engine"));
        assert!(svg.matches("<rect").count() >= 2); // background + bar
        assert!(svg.contains("<path"), "milestone diamond");
    }

    #[test]
    fn empty_chart_is_graceful() {
        let svg = render_gantt(&GanttChart::default(), &Theme::dark());
        assert!(svg.contains("empty gantt"));
    }

    #[test]
    fn date_roundtrip() {
        // 2026-01-01 = day 20454 (verified in the parser tests).
        assert_eq!(format_date(20454), "01-01");
        assert_eq!(format_date(20454 + 31), "02-01");
    }
}
