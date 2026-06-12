//! Compatibility tests against real diagrams from blog.viethx.com
//! (networking-essentials post) — HTML-in-label Mermaid style.

const NAT_DIAGRAM: &str = r#"flowchart LR
    laptop["<img src="/icons/mdi-laptop.svg" alt="mdi:laptop" class="diagram-node-icon" width="24" height="24" style="display:block;margin:0 auto 4px;" /> Your laptop<br/><span class='sub'>192.168.1.42:51000</span>"]:::client
    router["<img src="/icons/mdi-router-wireless.svg" alt="mdi:router-wireless" class="diagram-node-icon" width="24" height="24" style="display:block;margin:0 auto 4px;" /> Router (NAT)<br/><span class='sub'>translation table</span>"]:::highlight
    target["<img src="/icons/mdi-web.svg" alt="mdi:web" class="diagram-node-icon" width="24" height="24" style="display:block;margin:0 auto 4px;" /> example.com<br/><span class='sub'>93.184.216.34:443</span>"]:::external

    laptop -->|"outbound<br/>src 192.168.1.42:51000"| router
    router ==>|"rewritten<br/>src 203.0.113.17:60321"| target
    target -.->|"reply<br/>dst 203.0.113.17:60321"| router
    router -.->|"rewritten<br/>dst 192.168.1.42:51000"| laptop

    classDef highlight fill:#f3ebff,stroke:#7c3aed,stroke-width:1.5px,color:#4c1d95;

classDef client stroke:#64748b,stroke-width:1.75px;
classDef external stroke:#64748b,stroke-width:1.75px;"#;

#[test]
fn parses_blog_nat_diagram() {
    let g = layra_parser::parse(NAT_DIAGRAM).unwrap();
    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 4);

    let laptop = g.node(g.node_by_name("laptop").unwrap());
    assert_eq!(laptop.icon.as_deref(), Some("mdi:laptop"));
    assert_eq!(laptop.label, "Your laptop\n192.168.1.42:51000");

    let router = g.node(g.node_by_name("router").unwrap());
    assert_eq!(router.icon.as_deref(), Some("mdi:router-wireless"));

    // Edge labels: <br/> became a newline, quotes stripped.
    assert_eq!(
        g.edges[0].label.as_deref(),
        Some("outbound\nsrc 192.168.1.42:51000")
    );
    // ==> is thick, -.-> dashed.
    assert_eq!(g.edges[1].style, layra_core::EdgeStyle::Thick);
    assert_eq!(g.edges[2].style, layra_core::EdgeStyle::Dashed);
}

#[test]
fn renders_blog_nat_with_icons() {
    let pack = r##"{"icons":{
        "mdi:laptop":{"body":"<path fill=\"currentColor\" d=\"M4 6h16v10H4z\"/>","width":24,"height":24},
        "mdi:router-wireless":{"body":"<path fill=\"currentColor\" d=\"M2 2h2v2H2z\"/>","width":24,"height":24},
        "mdi:web":{"body":"<path fill=\"currentColor\" d=\"M0 0h4v4H0z\"/>","width":24,"height":24}
    }}"##;
    layra_wasm::load_icon_pack(pack).unwrap();

    let svg = layra_wasm::render_svg(NAT_DIAGRAM, false).unwrap();
    // Icons are inline <svg> fragments, not <img>.
    assert!(
        svg.matches("viewBox=\"0 0 24 24\"").count() >= 3,
        "3 inline icons expected"
    );
    assert!(!svg.contains("<img"), "no foreign <img> elements in output");
    assert!(svg.contains("Your laptop"));
    assert!(svg.contains("192.168.1.42:51000"));
}

#[test]
fn lenient_mode_survives_corrupted_paste() {
    // Reproduces a real user paste where line breaks were lost and the
    // classDef tail got mangled into garbage tokens.
    let src = r#"flowchart LR
    subgraph DED ["Dedicated LB"]
        C2["Client"]:::client
        VIP["Load balancer"]:::highlight
        C2 --> VIP
    end
    classDef highlight fill:#f3ebff,strokBackend C"]oup phckend C-4" he:1.5px;
    highlighcl kend C"]64748bckend C-4" he:1.75px;
    highlighc kend C"]oup phckend C-4" he:1.75px;"#;

    // Style debris is recognized and skipped silently: clean render, zero
    // warnings, even in strict mode.
    let svg = layra_wasm::render_svg(src, false).unwrap();
    assert!(svg.contains("Load balancer"));
    assert!(svg.contains("Client"));

    let (_, warnings) = layra_wasm::render_svg_lenient(src, false).unwrap();
    assert!(
        warnings.is_empty(),
        "style debris should not warn: {warnings:?}"
    );
}

#[test]
fn lenient_mode_warns_on_non_style_garbage() {
    // Garbage that is NOT style debris must still be surfaced.
    let src = "flowchart LR\n  a --> b\n  totally ((broken ]] garbage";
    let (svg, warnings) = layra_wasm::render_svg_lenient(src, false).unwrap();
    assert!(svg.contains("<svg"));
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("line 3"));
}

#[test]
fn lenient_mode_errors_when_nothing_parses() {
    let res = layra_wasm::render_svg_lenient("flowchart LR\n###garbage###", false);
    assert!(res.is_err());
}
