//! # layra-icons
//!
//! Icon registry for diagram nodes. Loads Iconify-format packs
//! (`{"icons": {"mdi:laptop": {"body": "<path .../>", "width": 24, ...}}}`)
//! and emits **inline SVG** — unlike Mermaid's `<img>` approach, exported
//! SVG/PNG files carry their icons with no external fetches and no
//! `securityLevel: loose`.
//!
//! Two correctness details handled here that naive inlining gets wrong:
//!
//! 1. **ID collisions.** Icon bodies (notably the `logos` pack) contain
//!    `<linearGradient id="...">` defs. Two icons on one canvas — or the
//!    same icon twice — would collide and corrupt each other's fills.
//!    Every emission rewrites `id="x"` / `url(#x)` / `href="#x"` with a
//!    unique per-instance prefix.
//! 2. **`currentColor`.** MDI glyphs use `fill="currentColor"`; we
//!    substitute the theme's text color so icons follow light/dark.

use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;

/// Bundled AWS-architecture-style infra icon pack (24x24, themeable). Shipped
/// in the binary via `include_str!` so the curated set needs no external
/// fetch. See `assets/infra.json`.
pub const BUILTIN_INFRA_PACK: &str = include_str!("../assets/infra.json");

#[derive(Debug, Error)]
pub enum IconError {
    #[error("invalid icon pack JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Icon {
    pub body: String,
    #[serde(default = "default_dim")]
    pub width: f32,
    #[serde(default = "default_dim")]
    pub height: f32,
    /// AWS-style service category. Drives the accent color of monochrome
    /// glyphs (rendered as a colored tile with a white mark) and node
    /// theming inferred from the icon (see the renderer's L10 path).
    #[serde(default)]
    pub category: Option<IconCategory>,
}

/// Service category for an icon, mapped to the real AWS architecture palette.
/// A monochrome glyph in a category renders as a colored rounded tile with a
/// white mark (the AWS "service tile" look) instead of a flat grey line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconCategory {
    /// Compute (EC2, Lambda, ECS, Fargate, ...). AWS orange.
    Compute,
    /// Storage (S3, EBS, EFS, ...). AWS green.
    Storage,
    /// Database (RDS, DynamoDB, ElastiCache, ...). AWS blue.
    Database,
    /// Networking & content delivery (VPC, Route 53, CloudFront, ELB, ...).
    /// AWS purple.
    Network,
    /// Security, identity & compliance (IAM, KMS, WAF, Cognito, ...). AWS red.
    Security,
    /// Application integration & messaging (SNS, SQS, EventBridge, Step
    /// Functions, ...). AWS pink.
    Integration,
    /// Analytics (Kinesis, Athena, Glue, Redshift, ...). AWS teal.
    Analytics,
    /// Management & governance (CloudWatch, logging, ...). AWS brown.
    Management,
    /// Uncategorized / client-side (browsers, users, generic). Neutral slate.
    General,
}

impl IconCategory {
    /// Accent color for this category (the real AWS architecture palette).
    pub fn color(self) -> &'static str {
        match self {
            IconCategory::Compute => "#ED7100",
            IconCategory::Storage => "#7AA116",
            IconCategory::Database => "#2E73B8",
            IconCategory::Network => "#8C4FFF",
            IconCategory::Security => "#D13212",
            IconCategory::Integration => "#E7157B",
            IconCategory::Analytics => "#01A88D",
            IconCategory::Management => "#8C4D38",
            IconCategory::General => "#5A6B86",
        }
    }
}

impl Icon {
    /// Whether this icon is a single-color line/fill mark (uses
    /// `currentColor`), as opposed to a full-color brand glyph (`logos:*`).
    fn is_monochrome(&self) -> bool {
        self.body.contains("currentColor")
    }
}

fn default_dim() -> f32 {
    16.0
}

#[derive(Debug, Deserialize)]
struct Pack {
    icons: HashMap<String, Icon>,
}

/// Registry of icons keyed by `prefix:name` (e.g. `mdi:laptop`).
#[derive(Debug, Default)]
pub struct IconRegistry {
    icons: HashMap<String, Icon>,
    /// Monotonic counter for ID namespacing across emissions.
    counter: std::cell::Cell<u64>,
}

impl IconRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry pre-loaded with the bundled AWS-architecture-style infra
    /// icon set (`aws:database`, `aws:lambda`, `aws:s3`, ...). These ship in
    /// the binary so diagrams can use `{icon:aws:lambda}` with no external
    /// pack to fetch. Additional packs still merge on top.
    ///
    /// The set spans ~40 cloud/dev-infra glyphs: hand-drawn `aws:*` line marks
    /// plus real Iconify bodies (`mdi:*` monochrome, `logos:*` brand marks) for
    /// common services (`postgres`, `redis`, `kafka`, `docker`, `kubernetes`,
    /// `nginx`, ...). The `aws:` and `infra:` prefixes alias the same namespace
    /// (see [`get`](Self::get)).
    pub fn with_builtins() -> Self {
        let mut reg = Self::default();
        // The bundled pack is authored in-repo and validated by tests, so a
        // parse failure here is a build-time bug, not a runtime condition.
        reg.load_pack(BUILTIN_INFRA_PACK)
            .expect("bundled infra icon pack must parse");
        reg
    }

    /// Merge the bundled infra icon set into this registry (idempotent).
    pub fn load_builtins(&mut self) -> usize {
        self.load_pack(BUILTIN_INFRA_PACK)
            .expect("bundled infra icon pack must parse")
    }

    /// Merge a pack (Iconify-format JSON) into the registry.
    pub fn load_pack(&mut self, json: &str) -> Result<usize, IconError> {
        let pack: Pack = serde_json::from_str(json)?;
        let n = pack.icons.len();
        self.icons.extend(pack.icons);
        Ok(n)
    }

    pub fn len(&self) -> usize {
        self.icons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&Icon> {
        if let Some(icon) = self.icons.get(key) {
            return Some(icon);
        }
        // `aws:` and `infra:` name the same bundled namespace, so an author can
        // write whichever reads best (`aws:postgres` or `infra:postgres`)
        // without having to know which prefix a given glyph was authored under.
        self.alias_key(key).and_then(|alias| self.icons.get(&alias))
    }

    /// If `key` is in the `aws:` / `infra:` namespace, return the same name
    /// under the sibling prefix (`aws:x` -> `infra:x` and vice versa). Other
    /// prefixes (`mdi:`, `logos:`, user packs) have no alias.
    fn alias_key(&self, key: &str) -> Option<String> {
        if let Some(name) = key.strip_prefix("aws:") {
            Some(format!("infra:{name}"))
        } else {
            key.strip_prefix("infra:").map(|name| format!("aws:{name}"))
        }
    }

    /// Emit an inline `<svg>` fragment for `key` at the given position and
    /// size. Returns `None` for unknown icons (caller decides the fallback).
    ///
    /// `color` replaces `currentColor` so themed glyphs follow light/dark.
    ///
    /// A **monochrome** glyph that declares an [`IconCategory`] is rendered
    /// the real-AWS way: a rounded tile in the category color with the mark
    /// drawn in white on top. This guarantees more than one color per icon
    /// (the v0.4 "beautiful by default" goal) and groups services by hue.
    /// Full-color brand glyphs (`logos:*`) keep their own palette untouched.
    pub fn emit_svg(&self, key: &str, x: f32, y: f32, size: f32, color: &str) -> Option<String> {
        let icon = self.get(key)?;
        let instance = self.counter.get();
        self.counter.set(instance + 1);

        // AWS service-tile look: colored rounded square + white glyph. Only
        // for monochrome marks (full-color brand glyphs already carry hues).
        if let Some(category) = icon.category.filter(|_| icon.is_monochrome()) {
            let tile = category.color();
            // Inset the glyph a touch so it breathes inside the tile.
            let pad = size * 0.16;
            let gx = x + pad;
            let gy = y + pad;
            let gsize = size - pad * 2.0;
            let r = (size * 0.18).clamp(2.0, 6.0);
            let body = namespace_ids(&icon.body, instance).replace("currentColor", "#ffffff");
            return Some(format!(
                r#"<g><rect x="{x:.1}" y="{y:.1}" width="{size:.1}" height="{size:.1}" rx="{r:.1}" fill="{tile}"/><svg x="{gx:.1}" y="{gy:.1}" width="{gsize:.1}" height="{gsize:.1}" viewBox="0 0 {} {}">{body}</svg></g>"#,
                icon.width, icon.height
            ));
        }

        let body = namespace_ids(&icon.body, instance).replace("currentColor", color);
        Some(format!(
            r#"<svg x="{x:.1}" y="{y:.1}" width="{size:.1}" height="{size:.1}" viewBox="0 0 {} {}">{body}</svg>"#,
            icon.width, icon.height
        ))
    }

    /// The declared [`IconCategory`] for `key`, if any. Used by node theming
    /// (L10) to derive an accent border/fill from the icon a node carries.
    pub fn category(&self, key: &str) -> Option<IconCategory> {
        self.get(key).and_then(|icon| icon.category)
    }
}

/// Rewrite `id="foo"`, `url(#foo)`, and `href="#foo"` with a unique prefix
/// so multiple icon instances never collide. Single pass, no regex dep.
fn namespace_ids(body: &str, instance: u64) -> String {
    let prefix = format!("ly{instance}-");
    let mut out = String::with_capacity(body.len() + 64);
    let mut rest = body;

    while !rest.is_empty() {
        // Find the earliest of our three markers.
        let candidates = [
            (rest.find("id=\""), "id=\"", "\""),
            (rest.find("url(#"), "url(#", ")"),
            (rest.find("href=\"#"), "href=\"#", "\""),
        ];
        let next = candidates
            .iter()
            .filter_map(|(pos, open, close)| pos.map(|p| (p, *open, *close)))
            .min_by_key(|&(p, _, _)| p);

        let Some((pos, open, close)) = next else {
            out.push_str(rest);
            break;
        };

        let id_start = pos + open.len();
        let Some(id_len) = rest[id_start..].find(close) else {
            out.push_str(rest);
            break;
        };

        out.push_str(&rest[..id_start]);
        out.push_str(&prefix);
        out.push_str(&rest[id_start..id_start + id_len]);
        rest = &rest[id_start + id_len..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACK: &str = r##"{"icons":{
        "mdi:laptop":{"body":"<path fill=\"currentColor\" d=\"M4 6h16v10H4z\"/>","width":24,"height":24},
        "logos:pg":{"body":"<defs><linearGradient id=\"grad\"><stop stop-color=\"#1b660f\"/></linearGradient></defs><path fill=\"url(#grad)\" d=\"M0 0h2v2H0z\"/>","width":256,"height":264}
    }}"##;

    #[test]
    fn loads_and_emits() {
        let mut reg = IconRegistry::new();
        assert_eq!(reg.load_pack(PACK).unwrap(), 2);

        let svg = reg
            .emit_svg("mdi:laptop", 10.0, 20.0, 24.0, "#1a1d23")
            .unwrap();
        assert!(svg.contains(r#"x="10.0""#));
        assert!(svg.contains(r#"viewBox="0 0 24 24""#));
        assert!(svg.contains("#1a1d23"));
        assert!(!svg.contains("currentColor"));
    }

    #[test]
    fn namespaces_ids_per_instance() {
        let mut reg = IconRegistry::new();
        reg.load_pack(PACK).unwrap();

        let a = reg.emit_svg("logos:pg", 0.0, 0.0, 24.0, "#000").unwrap();
        let b = reg.emit_svg("logos:pg", 30.0, 0.0, 24.0, "#000").unwrap();

        // Both instances reference their own gradient.
        let id_a = a.split("id=\"").nth(1).unwrap().split('"').next().unwrap();
        let id_b = b.split("id=\"").nth(1).unwrap().split('"').next().unwrap();
        assert_ne!(id_a, id_b, "instances must not share IDs");
        assert!(
            a.contains(&format!("url(#{id_a})")),
            "url ref must match def"
        );
        assert!(b.contains(&format!("url(#{id_b})")));
    }

    #[test]
    fn unknown_icon_is_none() {
        let reg = IconRegistry::new();
        assert!(reg.emit_svg("mdi:nope", 0.0, 0.0, 24.0, "#000").is_none());
    }

    #[test]
    fn aws_and_infra_prefixes_alias_each_other() {
        // `aws:x` and `infra:x` resolve to the same glyph regardless of which
        // prefix the bundled pack authored it under.
        let reg = IconRegistry::with_builtins();

        // `s3` ships under `aws:`; `postgres` ships under `infra:`.
        assert!(reg.get("aws:s3").is_some());
        assert!(reg.get("infra:s3").is_some(), "infra: must alias to aws:");
        assert!(reg.get("infra:postgres").is_some());
        assert!(
            reg.get("aws:postgres").is_some(),
            "aws: must alias to infra:"
        );

        // Non aws/infra prefixes are not aliased.
        assert!(reg.get("mdi:postgres").is_none());
    }

    // ---- L9: colored icon system ----

    const CAT_PACK: &str = r##"{"icons":{
        "aws:lambda":{"body":"<path fill=\"none\" stroke=\"currentColor\" d=\"M7 20L12 7z\"/>","width":24,"height":24,"category":"compute"},
        "aws:s3":{"body":"<path fill=\"currentColor\" d=\"M4 6h16z\"/>","width":24,"height":24,"category":"storage"},
        "plain:line":{"body":"<path fill=\"currentColor\" d=\"M0 0h2z\"/>","width":24,"height":24},
        "logos:brand":{"body":"<path fill=\"#c8511b\" d=\"M0 0h2z\"/>","width":256,"height":256,"category":"compute"}
    }}"##;

    #[test]
    fn category_palette_maps_to_aws_hues() {
        assert_eq!(IconCategory::Compute.color(), "#ED7100");
        assert_eq!(IconCategory::Storage.color(), "#7AA116");
        assert_eq!(IconCategory::Database.color(), "#2E73B8");
        assert_eq!(IconCategory::Network.color(), "#8C4FFF");
        assert_eq!(IconCategory::Security.color(), "#D13212");
        assert_eq!(IconCategory::Integration.color(), "#E7157B");
    }

    #[test]
    fn monochrome_categorized_icon_renders_more_than_one_color() {
        // The whole point of L9: a previously all-grey glyph now shows its
        // category tile color AND the white mark — strictly >1 color.
        let mut reg = IconRegistry::new();
        reg.load_pack(CAT_PACK).unwrap();

        let svg = reg
            .emit_svg("aws:lambda", 0.0, 0.0, 24.0, "#1a1d23")
            .unwrap();
        assert!(svg.contains("#ED7100"), "compute tile must be AWS orange");
        assert!(svg.contains("#ffffff"), "glyph mark must be white on tile");
        assert!(
            !svg.contains("currentColor"),
            "currentColor must be substituted"
        );
        // Distinct colors present => not monochrome anymore.
        let colors: std::collections::HashSet<_> = ["#ED7100", "#ffffff"]
            .iter()
            .filter(|c| svg.contains(**c))
            .collect();
        assert!(
            colors.len() > 1,
            "icon must render with more than one color"
        );
    }

    #[test]
    fn storage_icon_is_green_not_grey() {
        let mut reg = IconRegistry::new();
        reg.load_pack(CAT_PACK).unwrap();
        let svg = reg.emit_svg("aws:s3", 0.0, 0.0, 24.0, "#1a1d23").unwrap();
        assert!(svg.contains("#7AA116"), "storage tile must be AWS green");
        assert!(!svg.contains("#1a1d23"), "must not fall back to theme grey");
    }

    #[test]
    fn uncategorized_monochrome_keeps_themed_color() {
        // Icons with no category keep the original currentColor->theme path.
        let mut reg = IconRegistry::new();
        reg.load_pack(CAT_PACK).unwrap();
        let svg = reg
            .emit_svg("plain:line", 0.0, 0.0, 24.0, "#1a1d23")
            .unwrap();
        assert!(svg.contains("#1a1d23"), "uncategorized follows theme color");
        assert!(!svg.contains("<rect"), "no tile for uncategorized icons");
    }

    #[test]
    fn full_color_brand_glyph_keeps_its_palette() {
        // logos:* are already multi-color; a category must not overpaint them
        // with a white-on-tile treatment.
        let mut reg = IconRegistry::new();
        reg.load_pack(CAT_PACK).unwrap();
        let svg = reg
            .emit_svg("logos:brand", 0.0, 0.0, 24.0, "#1a1d23")
            .unwrap();
        assert!(svg.contains("#c8511b"), "brand color preserved");
        assert!(!svg.contains("#ffffff"), "brand glyph not repainted white");
    }

    #[test]
    fn category_lookup_resolves_through_alias() {
        let reg = IconRegistry::with_builtins();
        assert_eq!(reg.category("aws:lambda"), Some(IconCategory::Compute));
        assert_eq!(reg.category("aws:s3"), Some(IconCategory::Storage));
        // alias path: query under the sibling prefix
        assert_eq!(reg.category("infra:s3"), Some(IconCategory::Storage));
    }

    #[test]
    fn every_bundled_monochrome_icon_has_a_category() {
        // Guarantees no infra glyph renders as a flat grey line: each
        // currentColor mark must carry a category so it gets a colored tile.
        let reg = IconRegistry::with_builtins();
        let uncategorized: Vec<_> = reg
            .icons
            .iter()
            .filter(|(_, i)| i.is_monochrome() && i.category.is_none())
            .map(|(k, _)| k.clone())
            .collect();
        assert!(
            uncategorized.is_empty(),
            "monochrome icons missing a category: {uncategorized:?}"
        );
    }
}
