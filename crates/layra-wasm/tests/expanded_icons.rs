//! L5 contract: the bundled infra pack grew from the original 14 hand-drawn
//! AWS line glyphs to ~40 cloud/dev-infra icons (real Iconify bodies). Every
//! curated name must render an inline `<svg>` glyph, and the `aws:` / `infra:`
//! namespaces resolve interchangeably so authors can write whichever reads
//! best (`{icon:aws:postgres}` or `{icon:infra:postgres}`).

/// Count inline icon glyphs: every emitted icon is a nested `<svg x="...">`
/// (the document root is `<svg xmlns=...>`), so this is an exact count.
fn inline_icon_count(svg: &str) -> usize {
    svg.matches("<svg x=").count()
}

/// The expanded curated set, by logical name (sans namespace). Authors may
/// prefix any of these with `aws:` or `infra:`.
const EXPANDED_NAMES: &[&str] = &[
    // generic cloud/dev infra
    "api",
    "auth",
    "user",
    "mobile",
    "browser",
    "dns",
    "firewall",
    "monitoring",
    "logging",
    "secret",
    "key",
    "kubernetes",
    "docker",
    "redis",
    "postgres",
    "mysql",
    "mongodb",
    "kafka",
    "rabbitmq",
    "nginx",
    "email",
    "storage",
    "bucket-versions",
    // AWS service-specific
    "step-function",
    "eventbridge",
    "sns",
    "sqs",
    "ec2",
    "fargate",
    "route53",
    "cloudfront",
    "waf",
    "iam",
    // original hand-drawn set still present
    "database",
    "queue",
    "lambda",
    "s3",
    "vpc",
    "gateway",
    "cache",
    "cdn",
    "server",
    "container",
    "load-balancer",
];

#[test]
fn bundled_pack_has_at_least_forty_icons() {
    let n = layra_icons::IconRegistry::with_builtins().len();
    assert!(n >= 40, "expanded infra pack too small: {n} (< 40)");
}

#[test]
fn every_expanded_name_resolves_under_both_namespaces() {
    let reg = layra_icons::IconRegistry::with_builtins();
    for name in EXPANDED_NAMES {
        let aws = format!("aws:{name}");
        let infra = format!("infra:{name}");
        assert!(
            reg.get(&aws).is_some(),
            "icon {aws} not resolvable in builtin pack"
        );
        assert!(
            reg.get(&infra).is_some(),
            "icon {infra} not resolvable in builtin pack"
        );
    }
}

#[test]
fn each_expanded_icon_inlines_via_aws_and_infra_prefix() {
    for name in EXPANDED_NAMES {
        for prefix in ["aws", "infra"] {
            let key = format!("{prefix}:{name}");
            let src = format!("flowchart LR\n  a[\"{{icon:{key}}} svc\"] --> b[\"sink\"]\n");
            let svg = layra_wasm::render_svg(&src, false).expect("render");
            assert_eq!(
                inline_icon_count(&svg),
                1,
                "icon {key} did not inline exactly one glyph"
            );
        }
    }
}
