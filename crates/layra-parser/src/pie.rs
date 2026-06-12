//! Pie chart: Mermaid `pie` dialect — parsed and rendered in one module
//! since there is no layout problem to solve, only arc math.
//!
//! ```text
//! pie title Language share
//!     "Rust" : 62
//!     "TypeScript" : 28
//!     "Other" : 10
//! ```

use crate::ParseError;
use layra_core::PieChart;

pub(crate) fn parse_lenient(
    header_rest: &str,
    lines: &[(usize, &str)],
) -> (PieChart, Vec<ParseError>) {
    let mut chart = PieChart::default();
    let mut warnings = Vec::new();

    // Header may carry `showData` and/or `title ...`.
    let mut rest = header_rest.trim();
    if let Some(r) = rest.strip_prefix("showData") {
        chart.show_data = true;
        rest = r.trim();
    }
    if let Some(t) = rest.strip_prefix("title ") {
        chart.title = Some(t.trim().to_string());
    }

    for &(ln, line) in lines {
        if let Some(t) = line.strip_prefix("title ") {
            chart.title = Some(t.trim().to_string());
            continue;
        }
        if line == "showData" {
            chart.show_data = true;
            continue;
        }
        // `"Label" : value`
        let parsed = (|| {
            let rest = line.strip_prefix('"')?;
            let (label, after) = rest.split_once('"')?;
            let value: f64 = after.trim().strip_prefix(':')?.trim().parse().ok()?;
            Some((label.to_string(), value))
        })();
        match parsed {
            Some((label, value)) if value >= 0.0 => chart.slices.push((label, value)),
            _ => warnings.push(ParseError::Syntax {
                line: ln,
                message: format!("cannot parse pie slice '{line}'"),
            }),
        }
    }
    (chart, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title_and_slices() {
        let lines = vec![
            (2, "title Language share"),
            (3, r#""Rust" : 62"#),
            (4, r#""TypeScript" : 28.5"#),
        ];
        let (chart, warnings) = parse_lenient("", &lines);
        assert!(warnings.is_empty());
        assert_eq!(chart.title.as_deref(), Some("Language share"));
        assert_eq!(chart.slices.len(), 2);
        assert_eq!(chart.slices[1].1, 28.5);
    }

    #[test]
    fn header_show_data() {
        let (chart, _) = parse_lenient("showData title X", &[]);
        assert!(chart.show_data);
        assert_eq!(chart.title.as_deref(), Some("X"));
    }
}
