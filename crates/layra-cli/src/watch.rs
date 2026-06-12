//! `layra watch <dir>` — render `.mmd` files to SVG whenever they change.
//!
//! The agent-workflow glue for editors without MCP: the agent (or human)
//! writes/edits `diagram.mmd`, the sibling `diagram.svg` regenerates
//! within the poll interval. Polling (500ms mtime scan) keeps us
//! dependency-free and works on every filesystem including network
//! mounts where inotify is unreliable; a directory of even hundreds of
//! diagrams costs microseconds to scan.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const POLL: Duration = Duration::from_millis(500);

pub(crate) fn watch(dir: &str, dark: bool) -> ! {
    let root = PathBuf::from(dir);
    if !root.is_dir() {
        eprintln!("error: {dir} is not a directory");
        std::process::exit(1);
    }

    eprintln!("layra: watching {dir} for .mmd changes (Ctrl-C to stop)");
    let mut seen: HashMap<PathBuf, SystemTime> = HashMap::new();

    loop {
        let mut found: Vec<(PathBuf, SystemTime)> = Vec::new();
        collect_mmd(&root, &mut found);

        for (path, mtime) in found {
            // First sighting counts as changed, so startup renders all.
            let changed = seen.get(&path).is_none_or(|&prev| mtime > prev);
            if changed {
                seen.insert(path.clone(), mtime);
                render_one(&path, dark);
            }
        }
        std::thread::sleep(POLL);
    }
}

fn collect_mmd(dir: &Path, out: &mut Vec<(PathBuf, SystemTime)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip dot-directories and node_modules; recurse otherwise.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with('.') && name != "node_modules" && name != "target" {
                collect_mmd(&path, out);
            }
        } else if path
            .extension()
            .is_some_and(|e| e == "mmd" || e == "mermaid")
        {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    out.push((path, mtime));
                }
            }
        }
    }
}

fn render_one(path: &Path, dark: bool) {
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let out = path.with_extension("svg");
    match layra_wasm::render_svg_lenient(&source, dark) {
        Ok((svg, warnings)) => {
            if std::fs::write(&out, &svg).is_ok() {
                eprintln!(
                    "layra: {} → {} ({} bytes{})",
                    path.display(),
                    out.display(),
                    svg.len(),
                    if warnings.is_empty() {
                        String::new()
                    } else {
                        format!(", {} skipped lines", warnings.len())
                    }
                );
            }
        }
        Err(e) => eprintln!("layra: {}: {e}", path.display()),
    }
}
