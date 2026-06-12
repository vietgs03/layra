//! Gantt chart parser: Mermaid `gantt` dialect.
//!
//! ```text
//! gantt
//!     title Release plan
//!     dateFormat YYYY-MM-DD
//!     section Build
//!     Engine        :done,    eng,  2026-01-01, 30d
//!     Playground    :active,  play, after eng, 14d
//!     section Ship
//!     Launch        :milestone, 2026-03-01, 0d
//! ```
//!
//! Dates are parsed as days since epoch (civil-days math, no chrono dep).
//! Supported task forms: `name : [tags,] [id,] start, duration|end` where
//! start = date | `after <id>`, duration = `Nd`/`Nw`, tags include
//! done/active/crit/milestone.

use crate::ParseError;
use layra_core::charts::{GanttChart, GanttSection, GanttTask, TaskStatus};
use std::collections::HashMap;

pub(crate) fn parse_lenient(lines: &[(usize, &str)]) -> (GanttChart, Vec<ParseError>) {
    let mut chart = GanttChart::default();
    let mut warnings = Vec::new();
    let mut by_id: HashMap<String, (i64, i64)> = HashMap::new(); // id -> (start, end)
    let mut cursor: Option<i64> = None; // sequential default start

    for &(ln, line) in lines {
        if let Some(t) = line.strip_prefix("title ") {
            chart.title = Some(t.trim().to_string());
            continue;
        }
        if line.starts_with("dateFormat") {
            continue; // we accept ISO dates; the directive is a no-op
        }
        if line.starts_with("excludes") || line.starts_with("axisFormat") {
            continue;
        }
        if let Some(name) = line.strip_prefix("section ") {
            chart.sections.push(GanttSection {
                name: name.trim().to_string(),
                tasks: Vec::new(),
            });
            continue;
        }

        // Task: `Label : meta`
        let Some((label, meta)) = line.split_once(':') else {
            warnings.push(ParseError::Syntax {
                line: ln,
                message: format!("cannot parse gantt statement '{line}'"),
            });
            continue;
        };

        match parse_task(label.trim(), meta.trim(), &by_id, cursor) {
            Some(task) => {
                by_id.insert(
                    task.id.clone().unwrap_or_else(|| task.label.clone()),
                    (task.start, task.end),
                );
                cursor = Some(task.end);
                if chart.sections.is_empty() {
                    chart.sections.push(GanttSection {
                        name: String::new(),
                        tasks: Vec::new(),
                    });
                }
                chart.sections.last_mut().unwrap().tasks.push(task);
            }
            None => warnings.push(ParseError::Syntax {
                line: ln,
                message: format!("cannot parse gantt task '{line}'"),
            }),
        }
    }
    (chart, warnings)
}

fn parse_task(
    label: &str,
    meta: &str,
    by_id: &HashMap<String, (i64, i64)>,
    cursor: Option<i64>,
) -> Option<GanttTask> {
    let mut status = TaskStatus::Planned;
    let mut milestone = false;
    let mut id: Option<String> = None;
    let mut start: Option<i64> = None;
    let mut end: Option<i64> = None;
    let mut duration: Option<i64> = None;

    for part in meta.split(',').map(str::trim) {
        match part {
            "done" => status = TaskStatus::Done,
            "active" => status = TaskStatus::Active,
            "crit" => status = TaskStatus::Critical,
            "milestone" => milestone = true,
            _ if part.is_empty() => {}
            _ => {
                if let Some(rest) = part.strip_prefix("after ") {
                    // Start when the referenced task(s) end (max of ends).
                    let mut latest = None;
                    for r in rest.split_whitespace() {
                        if let Some(&(_, e)) = by_id.get(r) {
                            latest = Some(latest.map_or(e, |l: i64| l.max(e)));
                        }
                    }
                    start = latest.or(cursor);
                } else if let Some(days) = parse_duration(part) {
                    duration = Some(days);
                } else if let Some(d) = parse_date(part) {
                    if start.is_none() {
                        start = Some(d);
                    } else {
                        end = Some(d);
                    }
                } else if start.is_none() && id.is_none() && is_task_id(part) {
                    id = Some(part.to_string());
                }
            }
        }
    }

    let start = start.or(cursor).unwrap_or(0);
    let end = end
        .or(duration.map(|d| start + d))
        .unwrap_or(start + if milestone { 0 } else { 1 });

    Some(GanttTask {
        label: label.to_string(),
        id,
        start,
        end: end.max(start),
        status,
        milestone,
    })
}

fn is_task_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// `30d` → 30, `2w` → 14, `1m` ~ 30.
fn parse_duration(s: &str) -> Option<i64> {
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let n: i64 = num.parse().ok()?;
    match unit {
        "d" => Some(n),
        "w" => Some(n * 7),
        "m" => Some(n * 30),
        _ => None,
    }
}

/// `YYYY-MM-DD` → civil days since 1970-01-01 (Howard Hinnant's algorithm).
pub(crate) fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_src(src: &str) -> GanttChart {
        let owned: Vec<(usize, String)> = src
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim().to_string()))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        let borrowed: Vec<(usize, &str)> = owned.iter().map(|(n, l)| (*n, l.as_str())).collect();
        let (chart, warnings) = parse_lenient(&borrowed);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        chart
    }

    #[test]
    fn date_math_is_correct() {
        assert_eq!(parse_date("1970-01-01"), Some(0));
        assert_eq!(parse_date("1970-01-31"), Some(30));
        assert_eq!(parse_date("2026-01-01"), Some(20454));
    }

    #[test]
    fn parses_sections_and_after_dependency() {
        let chart = parse_src(
            "title Plan\n\
             dateFormat YYYY-MM-DD\n\
             section Build\n\
             Engine :done, eng, 2026-01-01, 30d\n\
             Playground :active, play, after eng, 14d\n\
             section Ship\n\
             Launch :milestone, 2026-03-01, 0d",
        );
        assert_eq!(chart.title.as_deref(), Some("Plan"));
        assert_eq!(chart.sections.len(), 2);

        let eng = &chart.sections[0].tasks[0];
        assert_eq!(eng.status, TaskStatus::Done);
        assert_eq!(eng.end - eng.start, 30);

        let play = &chart.sections[0].tasks[1];
        assert_eq!(play.start, eng.end, "after eng");
        assert_eq!(play.status, TaskStatus::Active);

        let launch = &chart.sections[1].tasks[0];
        assert!(launch.milestone);
    }
}
