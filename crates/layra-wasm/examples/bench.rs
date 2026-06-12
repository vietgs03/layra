//! Pipeline benchmark across graph sizes + per-stage breakdown.
use std::time::Instant;

fn tree(n: usize) -> String {
    let mut s = String::from("flowchart TB\n");
    for i in 1..n {
        let p = (i - 1) / 3;
        s.push_str(&format!(
            "  n{p}[\"Service {p}\"] --> n{i}[\"Service {i}\"]\n"
        ));
    }
    s
}

fn layered(n: usize) -> String {
    // Denser: each node connects to 2 in next layer — more crossing work.
    let mut s = String::from("flowchart TB\n");
    let width = 20;
    for i in width..n {
        let a = i - width;
        let b = i - width + (i % 3);
        s.push_str(&format!("  n{a} --> n{i}\n"));
        if b < i {
            s.push_str(&format!("  n{b} --> n{i}\n"));
        }
    }
    s
}

fn time<F: FnMut()>(label: &str, runs: usize, mut f: F) {
    // warmup
    for _ in 0..3 {
        f();
    }
    let t = Instant::now();
    for _ in 0..runs {
        f();
    }
    let per = t.elapsed().as_secs_f64() / runs as f64;
    println!("{label}: {:.3} ms", per * 1e3);
}

fn main() {
    for (name, src) in [
        ("tree-500", tree(500)),
        ("tree-2000", tree(2000)),
        ("dense-1000", layered(1000)),
        ("dense-5000", layered(5000)),
    ] {
        let runs = if src.len() > 100_000 { 5 } else { 20 };
        // full pipeline
        time(&format!("{name} full"), runs, || {
            let _ = layra_wasm::render_svg(&src, false).unwrap();
        });
        // stage breakdown
        let (doc, _) = layra_parser::parse_document_lenient(&src);
        let layra_core::Document::Graph(graph0) = doc else {
            unreachable!()
        };
        time(&format!("{name}   parse"), runs, || {
            let _ = layra_parser::parse_document_lenient(&src);
        });
        time(&format!("{name}   measure"), runs, || {
            let mut g = graph0.clone();
            layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
        });
        let mut measured = graph0.clone();
        layra_text::measure_graph(&mut measured, &layra_text::TextOptions::default());
        time(&format!("{name}   layout"), runs, || {
            let mut g = measured.clone();
            layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
        });
        let mut laid = measured.clone();
        layra_layout::layout(&mut laid, &layra_layout::LayoutOptions::default());
        time(&format!("{name}   route"), runs, || {
            let mut g = laid.clone();
            layra_router::route(&mut g);
        });
        let mut routed = laid.clone();
        layra_router::route(&mut routed);
        time(&format!("{name}   svg"), runs, || {
            let _ = layra_render_svg::render(&routed, &layra_render_svg::Theme::light());
        });
        println!();
    }
}
