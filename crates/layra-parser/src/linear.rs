//! Timeline + user-journey parsers (Mermaid `timeline` / `journey`).
//! Both are linear chart structures with dedicated renderers.

use crate::ParseError;
use layra_core::{Journey, JourneySection, JourneyTask, Timeline, TimelinePeriod};

/* ---------------- timeline ---------------- */

/// ```text
/// timeline
///     title History
///     section 2024
///         Q1 : idea : prototype
///         Q2 : launch
/// ```
/// Periods may continue events on following lines starting with `:`.
pub(crate) fn parse_timeline(lines: &[(usize, &str)]) -> (Timeline, Vec<ParseError>) {
    let mut tl = Timeline::default();
    let mut warnings = Vec::new();
    let mut section: Option<String> = None;

    for &(ln, line) in lines {
        if let Some(t) = line.strip_prefix("title ") {
            tl.title = Some(t.trim().to_string());
            continue;
        }
        if let Some(s) = line.strip_prefix("section ") {
            section = Some(s.trim().to_string());
            continue;
        }
        // Continuation: `: more event`
        if let Some(rest) = line.strip_prefix(':') {
            match tl.periods.last_mut() {
                Some(p) => p.events.push(rest.trim().to_string()),
                None => warnings.push(ParseError::Syntax {
                    line: ln,
                    message: "event continuation before any period".into(),
                }),
            }
            continue;
        }
        // `Period : event : event`
        let mut parts = line.split(" : ").map(str::trim);
        let Some(label) = parts.next().filter(|l| !l.is_empty()) else {
            warnings.push(ParseError::Syntax {
                line: ln,
                message: format!("cannot parse timeline line '{line}'"),
            });
            continue;
        };
        tl.periods.push(TimelinePeriod {
            label: label.to_string(),
            events: parts.map(String::from).collect(),
            section: section.clone(),
        });
    }
    (tl, warnings)
}

/* ---------------- journey ---------------- */

/// ```text
/// journey
///     title My day
///     section Morning
///       Wake up: 3: Me
///       Coffee: 5: Me, Cat
/// ```
pub(crate) fn parse_journey(lines: &[(usize, &str)]) -> (Journey, Vec<ParseError>) {
    let mut j = Journey::default();
    let mut warnings = Vec::new();

    for &(ln, line) in lines {
        if let Some(t) = line.strip_prefix("title ") {
            j.title = Some(t.trim().to_string());
            continue;
        }
        if let Some(s) = line.strip_prefix("section ") {
            j.sections.push(JourneySection {
                name: s.trim().to_string(),
                tasks: Vec::new(),
            });
            continue;
        }
        // `Task: score: actor, actor`
        let parsed = (|| {
            let mut parts = line.splitn(3, ':');
            let label = parts.next()?.trim();
            let score: u8 = parts.next()?.trim().parse().ok()?;
            let actors = parts
                .next()
                .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            (!label.is_empty() && (1..=5).contains(&score)).then(|| JourneyTask {
                label: label.to_string(),
                score,
                actors,
            })
        })();

        match parsed {
            Some(task) => {
                if j.sections.is_empty() {
                    j.sections.push(JourneySection::default());
                }
                j.sections.last_mut().unwrap().tasks.push(task);
            }
            None => warnings.push(ParseError::Syntax {
                line: ln,
                message: format!("cannot parse journey task '{line}'"),
            }),
        }
    }
    (j, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(src: &str) -> Vec<(usize, String)> {
        src.lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim().to_string()))
            .filter(|(_, l)| !l.is_empty())
            .collect()
    }

    fn borrowed(owned: &[(usize, String)]) -> Vec<(usize, &str)> {
        owned.iter().map(|(n, l)| (*n, l.as_str())).collect()
    }

    #[test]
    fn timeline_periods_and_continuations() {
        let owned =
            lines("title History\nsection 2024\nQ1 : idea : prototype\n: extra\nQ2 : launch");
        let (tl, w) = parse_timeline(&borrowed(&owned));
        assert!(w.is_empty());
        assert_eq!(tl.periods.len(), 2);
        assert_eq!(tl.periods[0].events, vec!["idea", "prototype", "extra"]);
        assert_eq!(tl.periods[0].section.as_deref(), Some("2024"));
    }

    #[test]
    fn journey_scores_and_actors() {
        let owned = lines("title Day\nsection Morning\nWake up: 3: Me\nCoffee: 5: Me, Cat");
        let (j, w) = parse_journey(&borrowed(&owned));
        assert!(w.is_empty());
        assert_eq!(j.sections[0].tasks.len(), 2);
        assert_eq!(j.sections[0].tasks[1].score, 5);
        assert_eq!(j.sections[0].tasks[1].actors, vec!["Me", "Cat"]);
    }

    #[test]
    fn journey_rejects_bad_score() {
        let owned = lines("Task: 9: Me");
        let (_, w) = parse_journey(&borrowed(&owned));
        assert_eq!(w.len(), 1);
    }
}
