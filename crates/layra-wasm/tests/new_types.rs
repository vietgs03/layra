//! End-to-end coverage for classDiagram, erDiagram, and pie.

#[test]
fn class_diagram_end_to_end() {
    let svg = layra_wasm::render_svg(
        "classDiagram\n\
         direction LR\n\
         class Animal {\n\
           +String name\n\
           +eat()\n\
         }\n\
         class Dog {\n\
           +bark()\n\
         }\n\
         class Cat\n\
         Animal <|-- Dog\n\
         Animal <|-- Cat\n\
         Dog \"1\" --> \"*\" Bone : buries",
        false,
    )
    .unwrap();
    assert!(svg.contains("Animal"));
    assert!(svg.contains("+eat()"));
    assert!(svg.contains("url(#triangle)"), "inheritance marker");
    assert!(svg.contains(">1<") && svg.contains(">*<"), "cardinalities");
}

#[test]
fn er_diagram_end_to_end() {
    let svg = layra_wasm::render_svg(
        "erDiagram\n\
         CUSTOMER ||--o{ ORDER : places\n\
         ORDER ||--|{ LINE_ITEM : contains\n\
         ORDER {\n\
           int id PK\n\
           string status\n\
         }",
        false,
    )
    .unwrap();
    assert!(svg.contains("CUSTOMER"));
    assert!(svg.contains("id: int  [PK]"));
    assert!(svg.contains("0..*"), "crow's-foot cardinality label");
    assert!(svg.contains("1..*"));
}

#[test]
fn pie_end_to_end() {
    let svg = layra_wasm::render_svg(
        "pie showData title Language share\n\
         \"Rust\" : 62\n\
         \"TypeScript\" : 28\n\
         \"Other\" : 10",
        false,
    )
    .unwrap();
    assert!(svg.contains("Language share"));
    assert_eq!(svg.matches("<path").count(), 3);
    assert!(svg.contains("62%"));
    assert!(svg.contains("— 62"), "showData appends values");
}

#[test]
fn dark_theme_works_for_all_new_types() {
    for src in [
        "classDiagram\n A <|-- B",
        "erDiagram\n A ||--o{ B : has",
        "pie\n \"x\" : 1",
    ] {
        let svg = layra_wasm::render_svg(src, true).unwrap();
        assert!(svg.contains("#0f1115"), "dark background for {src:?}");
    }
}

#[test]
fn gantt_end_to_end() {
    let svg = layra_wasm::render_svg(
        "gantt\n\
         title Release plan\n\
         dateFormat YYYY-MM-DD\n\
         section Build\n\
         Engine :done, eng, 2026-01-01, 30d\n\
         Playground :active, play, after eng, 14d\n\
         section Ship\n\
         Launch :milestone, 2026-03-01, 0d",
        false,
    )
    .unwrap();
    assert!(svg.contains("Release plan"));
    assert!(svg.contains("Engine"));
    assert!(svg.contains("Playground"));
    // Bars + milestone diamond present.
    assert!(svg.matches("<rect").count() >= 3);
    assert!(svg.contains("<path"), "milestone diamond");
    // Axis ticks exist (MM-DD labels).
    assert!(svg.contains("01-"), "date axis ticks");
}

#[test]
fn mindmap_timeline_journey_git_end_to_end() {
    let cases = [
        ("mindmap\n  root((Layra))\n    Engine\n      Layout\n    Playground", "Layra"),
        ("timeline\n  title History\n  section 2024\n  Q1 : idea : prototype\n  Q2 : launch", "prototype"),
        ("journey\n  title My day\n  section Morning\n  Wake up: 3: Me\n  Coffee: 5: Me, Cat", "Coffee"),
        ("gitGraph\n  commit\n  branch develop\n  commit\n  checkout main\n  merge develop tag: \"v1.0\"", "v1.0"),
    ];
    for (src, needle) in cases {
        for dark in [false, true] {
            let svg = layra_wasm::render_svg(src, dark).unwrap();
            assert!(svg.contains(needle), "missing {needle:?} for {src:?}");
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        }
    }
}
