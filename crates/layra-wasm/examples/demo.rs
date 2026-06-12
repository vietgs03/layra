//! Dump a demo SVG + crude perf numbers. `cargo run --release --example demo`
use std::time::Instant;

const DEMO: &str = r#"flowchart LR
  client(["Browser"]):::client --> cdn["CDN"]:::external
  cdn --> api["API Gateway"]:::gateway
  api --> auth["Auth Service"]:::service
  api --> orders["Order Service"]:::service
  api --> users["User Service"]:::service
  orders -->|persist| db[("Postgres")]:::database
  orders -.->|events| mq{{Kafka}}
  mq -.-> notif["Notification Svc"]:::service
  users --> cache(("Redis")):::cache
  users --> db
  auth --> cache
  subgraph data["Data Plane"]
    db
    mq
    cache
  end
"#;

fn main() {
    let svg = layra_wasm::render_svg(DEMO, false).unwrap();
    std::fs::write("demo.svg", &svg).unwrap();
    let dark = layra_wasm::render_svg(DEMO, true).unwrap();
    std::fs::write("demo-dark.svg", &dark).unwrap();
    println!("wrote demo.svg ({} bytes)", svg.len());

    let n = 10_000;
    let t = Instant::now();
    for _ in 0..n {
        let _ = layra_wasm::render_svg(DEMO, false).unwrap();
    }
    let per = t.elapsed().as_secs_f64() / n as f64 * 1e6;
    println!("full pipeline (12 nodes): {per:.1} us/render");

    // Synthetic 500-node tree.
    let mut src = String::from("flowchart TB\n");
    for i in 1..500usize {
        let parent = (i - 1) / 3;
        src.push_str(&format!(
            "  n{parent}[\"Service {parent}\"] --> n{i}[\"Service {i}\"]\n"
        ));
    }
    let t = Instant::now();
    let runs = 50;
    for _ in 0..runs {
        let _ = layra_wasm::render_svg(&src, false).unwrap();
    }
    let per = t.elapsed().as_secs_f64() / runs as f64 * 1e3;
    println!("full pipeline (500 nodes): {per:.2} ms/render");
}
