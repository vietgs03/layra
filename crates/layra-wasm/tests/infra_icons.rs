//! L2 contract: a bundled AWS-style cloud/infra icon set ships with the
//! engine, so `{icon:aws:lambda}` (and the rest of the curated set) renders
//! inline SVG with no external pack to load. Each icon must emit one inline
//! `<svg ...>` glyph inside the node.

/// Count inline icon glyphs: every emitted icon is a nested `<svg x="...">`
/// (the document root is `<svg xmlns=...>`), so this is an exact count.
fn inline_icon_count(svg: &str) -> usize {
    svg.matches("<svg x=").count()
}

/// The curated infra set every diagram can use without loading a pack.
const INFRA_ICONS: &[&str] = &[
    "aws:database",
    "aws:queue",
    "aws:lambda",
    "aws:s3",
    "aws:vpc",
    "aws:gateway",
    "aws:cache",
    "aws:cdn",
    "aws:server",
    "aws:container",
    "aws:load-balancer",
];

#[test]
fn bundled_pack_is_nonempty_and_covers_infra_set() {
    let n = layra_icons::IconRegistry::with_builtins().len();
    assert!(n >= INFRA_ICONS.len(), "builtin pack too small: {n}");

    let reg = layra_icons::IconRegistry::with_builtins();
    for key in INFRA_ICONS {
        assert!(
            reg.get(key).is_some(),
            "builtin pack is missing curated icon {key}"
        );
    }
}

#[test]
fn flowchart_with_aws_icons_renders_inline_svg_for_each() {
    // One node per curated icon; every icon must inline its own glyph.
    let mut src = String::from("flowchart TB\n");
    for (i, key) in INFRA_ICONS.iter().enumerate() {
        // `{icon:aws:lambda}` form, stripped from the label by the parser.
        src.push_str(&format!("  n{i}[\"{{icon:{key}}} {key}\"]\n"));
        if i > 0 {
            src.push_str(&format!("  n{} --> n{i}\n", i - 1));
        }
    }

    let svg = layra_wasm::render_svg(&src, false).expect("render");
    assert_eq!(
        inline_icon_count(&svg),
        INFRA_ICONS.len(),
        "every {{icon:aws:*}} must render an inline glyph"
    );
}

#[test]
fn each_infra_icon_inlines_individually() {
    for key in INFRA_ICONS {
        let src = format!("flowchart LR\n  a[\"{{icon:{key}}} svc\"] --> b[\"sink\"]\n");
        let svg = layra_wasm::render_svg(&src, false).expect("render");
        assert_eq!(
            inline_icon_count(&svg),
            1,
            "icon {key} did not inline a glyph"
        );
        // Glyphs are themeable (currentColor substituted), never literal.
        assert!(
            !svg.contains("currentColor"),
            "icon {key} leaked currentColor into output"
        );
    }
}
