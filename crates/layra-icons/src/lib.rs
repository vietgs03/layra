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
        self.icons.get(key)
    }

    /// Emit an inline `<svg>` fragment for `key` at the given position and
    /// size. Returns `None` for unknown icons (caller decides the fallback).
    ///
    /// `color` replaces `currentColor` so themed glyphs follow light/dark.
    pub fn emit_svg(&self, key: &str, x: f32, y: f32, size: f32, color: &str) -> Option<String> {
        let icon = self.icons.get(key)?;
        let instance = self.counter.get();
        self.counter.set(instance + 1);

        let body = namespace_ids(&icon.body, instance).replace("currentColor", color);
        Some(format!(
            r#"<svg x="{x:.1}" y="{y:.1}" width="{size:.1}" height="{size:.1}" viewBox="0 0 {} {}">{body}</svg>"#,
            icon.width, icon.height
        ))
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
}
