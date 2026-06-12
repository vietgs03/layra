//! Every diagram extracted from blog.viethx.com/posts/networking-essentials
//! must parse and render. This is the real-world compatibility contract:
//! if a change breaks any of the 25 production diagrams, this fails.

#[test]
fn entire_blog_corpus_renders() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    let mut count = 0;
    let mut failures = Vec::new();

    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("corpus dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "mmd"))
        .collect();
    paths.sort();
    assert!(paths.len() >= 25, "corpus shrank? found {}", paths.len());

    for path in paths {
        let src = std::fs::read_to_string(&path).unwrap();
        count += 1;
        for dark in [false, true] {
            match layra_wasm::render_svg(&src, dark) {
                Ok(svg) => {
                    assert!(svg.starts_with("<svg"), "{path:?} bad svg");
                    assert!(svg.ends_with("</svg>"), "{path:?} truncated");
                }
                Err(e) => failures.push(format!("{path:?} (dark={dark}): {e}")),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} corpus failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    println!("{count} corpus diagrams render in both themes");
}
