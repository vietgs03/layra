//! Pie chart renderer: arc math + side legend, no layout engine needed.

use crate::theme::Theme;
use crate::{escape, FONT_STACK};
use layra_core::PieChart;
use std::fmt::Write;

const RADIUS: f32 = 110.0;
const CX: f32 = 150.0;
const LEGEND_X: f32 = 300.0;
const LEGEND_ROW: f32 = 24.0;

/// Categorical palette tuned to match the role-color family.
const PALETTE: [&str; 10] = [
    "#3b82f6", "#8b5cf6", "#f59e0b", "#10b981", "#ef4444", "#06b6d4", "#ec4899", "#84cc16",
    "#f97316", "#64748b",
];

pub fn render_pie(chart: &PieChart, theme: &Theme) -> String {
    let total: f64 = chart.slices.iter().map(|(_, v)| v).sum();
    let title_h = if chart.title.is_some() { 38.0 } else { 12.0 };
    let legend_h = chart.slices.len() as f32 * LEGEND_ROW;
    let height = (RADIUS * 2.0 + 40.0).max(legend_h + 40.0) + title_h;
    let max_label = chart.slices.iter().map(|(l, _)| l.len()).max().unwrap_or(0) as f32;
    let width = LEGEND_X + 30.0 + max_label * 8.0 + 80.0;

    let mut svg = String::with_capacity(2048);
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
            r#"<text x="{:.0}" y="24" font-size="16" font-weight="700" fill="{}">{}</text>"#,
            16.0,
            theme.text,
            escape(title)
        );
    }

    if total <= 0.0 || chart.slices.is_empty() {
        let _ = write!(
            svg,
            r#"<text x="{CX}" y="{:.0}" font-size="13" fill="{}" text-anchor="middle">no data</text>"#,
            title_h + RADIUS,
            theme.edge_label
        );
        svg.push_str("</svg>");
        return svg;
    }

    let cy = title_h + RADIUS + 8.0;
    let mut angle = -std::f64::consts::FRAC_PI_2; // start at 12 o'clock

    for (i, (label, value)) in chart.slices.iter().enumerate() {
        let frac = value / total;
        let sweep = frac * std::f64::consts::TAU;
        let color = PALETTE[i % PALETTE.len()];

        if frac >= 0.999_99 {
            // Full circle: a single arc path degenerates; use <circle>.
            let _ = write!(
                svg,
                r#"<circle cx="{CX}" cy="{cy:.1}" r="{RADIUS}" fill="{color}" stroke="{}" stroke-width="1.5"/>"#,
                theme.background
            );
        } else {
            let (x0, y0) = point_on(angle, cy);
            let (x1, y1) = point_on(angle + sweep, cy);
            let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
            let _ = write!(
                svg,
                r#"<path d="M{CX} {cy:.1}L{x0:.1} {y0:.1}A{RADIUS} {RADIUS} 0 {large} 1 {x1:.1} {y1:.1}Z" fill="{color}" stroke="{}" stroke-width="1.5"/>"#,
                theme.background
            );
        }

        // Percentage label inside the slice (skip slivers < 4%).
        if frac >= 0.04 {
            let mid = angle + sweep / 2.0;
            let lx = CX as f64 + (RADIUS as f64 * 0.62) * mid.cos();
            let ly = cy as f64 + (RADIUS as f64 * 0.62) * mid.sin();
            let _ = write!(
                svg,
                r##"<text x="{lx:.1}" y="{ly:.1}" font-size="12.5" font-weight="600" fill="#fff" text-anchor="middle" dominant-baseline="central">{:.0}%</text>"##,
                frac * 100.0
            );
        }

        // Legend row.
        let ly = title_h + 16.0 + i as f32 * LEGEND_ROW;
        let value_str = if chart.show_data {
            format!(" — {}", trim_float(*value))
        } else {
            String::new()
        };
        let _ = write!(
            svg,
            r#"<rect x="{LEGEND_X}" y="{:.1}" width="13" height="13" rx="3" fill="{color}"/><text x="{:.1}" y="{:.1}" font-size="13" fill="{}" dominant-baseline="central">{}{}</text>"#,
            ly,
            LEGEND_X + 21.0,
            ly + 7.0,
            theme.text,
            escape(label),
            escape(&value_str)
        );

        angle += sweep;
    }

    svg.push_str("</svg>");
    svg
}

fn point_on(angle: f64, cy: f32) -> (f64, f64) {
    (
        CX as f64 + RADIUS as f64 * angle.cos(),
        cy as f64 + RADIUS as f64 * angle.sin(),
    )
}

/// `62.0` → `62`, `28.5` stays `28.5`.
fn trim_float(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_slices_and_legend() {
        let chart = PieChart {
            title: Some("Share".into()),
            show_data: true,
            slices: vec![("Rust".into(), 62.0), ("TS".into(), 38.0)],
        };
        let svg = render_pie(&chart, &Theme::light());
        assert!(svg.contains("Share"));
        assert!(svg.matches("<path").count() == 2);
        assert!(svg.contains("62%"));
        assert!(svg.contains("— 62"));
    }

    #[test]
    fn single_slice_renders_full_circle() {
        let chart = PieChart {
            title: None,
            show_data: false,
            slices: vec![("All".into(), 10.0)],
        };
        let svg = render_pie(&chart, &Theme::light());
        assert!(svg.contains("<circle"), "full pie must use <circle>");
    }

    #[test]
    fn empty_pie_is_graceful() {
        let svg = render_pie(&PieChart::default(), &Theme::dark());
        assert!(svg.contains("no data"));
    }
}
