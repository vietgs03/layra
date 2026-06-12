//! gitGraph parser: Mermaid `gitGraph` dialect.
//!
//! ```text
//! gitGraph
//!    commit
//!    commit id: "v0.1" tag: "release"
//!    branch develop
//!    checkout develop
//!    commit
//!    checkout main
//!    merge develop tag: "v1.0"
//! ```

use crate::ParseError;
use layra_core::{GitGraph, GitOp};

pub(crate) fn parse_lenient(lines: &[(usize, &str)]) -> (GitGraph, Vec<ParseError>) {
    let mut g = GitGraph {
        branches: vec!["main".to_string()],
        ops: Vec::new(),
    };
    let mut warnings = Vec::new();
    let mut current = 0usize;

    for &(ln, line) in lines {
        if let Some(rest) = line.strip_prefix("commit") {
            let (id, tag) = parse_kv(rest);
            g.ops.push(GitOp::Commit {
                id,
                tag,
                branch: current,
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("branch ") {
            let name = rest
                .trim()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                warnings.push(err(ln, line));
                continue;
            }
            g.branches.push(name.clone());
            g.ops.push(GitOp::Branch { name });
            current = g.branches.len() - 1;
            continue;
        }
        if let Some(rest) = line
            .strip_prefix("checkout ")
            .or_else(|| line.strip_prefix("switch "))
        {
            let name = rest.trim();
            match g.branches.iter().position(|b| b == name) {
                Some(i) => current = i,
                None => warnings.push(err(ln, line)),
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("merge ") {
            let mut parts = rest.trim().splitn(2, char::is_whitespace);
            let name = parts.next().unwrap_or("");
            let (_, tag) = parse_kv(parts.next().unwrap_or(""));
            match g.branches.iter().position(|b| b == name) {
                Some(from) => g.ops.push(GitOp::Merge {
                    from_branch: from,
                    into_branch: current,
                    tag,
                }),
                None => warnings.push(err(ln, line)),
            }
            continue;
        }
        if line.starts_with("cherry-pick") {
            // Render-irrelevant for v1; accept silently.
            continue;
        }
        warnings.push(err(ln, line));
    }
    (g, warnings)
}

fn err(line: usize, text: &str) -> ParseError {
    ParseError::Syntax {
        line,
        message: format!("cannot parse gitGraph statement '{text}'"),
    }
}

/// `id: "v0.1" tag: "release"` → (Some("v0.1"), Some("release")).
fn parse_kv(s: &str) -> (Option<String>, Option<String>) {
    let grab = |key: &str| -> Option<String> {
        let at = s.find(key)?;
        let rest = s[at + key.len()..].trim_start();
        let rest = rest.strip_prefix('"')?;
        Some(rest.split('"').next()?.to_string())
    };
    (grab("id:"), grab("tag:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_src(src: &str) -> GitGraph {
        let owned: Vec<(usize, String)> = src
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim().to_string()))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        let borrowed: Vec<(usize, &str)> = owned.iter().map(|(n, l)| (*n, l.as_str())).collect();
        let (g, warnings) = parse_lenient(&borrowed);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        g
    }

    #[test]
    fn branch_checkout_merge_flow() {
        let g = parse_src(
            "commit\n\
             commit id: \"v0.1\" tag: \"release\"\n\
             branch develop\n\
             commit\n\
             checkout main\n\
             merge develop tag: \"v1.0\"",
        );
        assert_eq!(g.branches, vec!["main", "develop"]);
        assert_eq!(g.ops.len(), 5);

        let GitOp::Commit { id, tag, .. } = &g.ops[1] else {
            panic!()
        };
        assert_eq!(id.as_deref(), Some("v0.1"));
        assert_eq!(tag.as_deref(), Some("release"));

        // `branch develop` auto-checks-out: next commit is on lane 1.
        let GitOp::Commit { branch, .. } = &g.ops[3] else {
            panic!()
        };
        assert_eq!(*branch, 1);

        let GitOp::Merge {
            from_branch,
            into_branch,
            tag,
        } = &g.ops[4]
        else {
            panic!()
        };
        assert_eq!((*from_branch, *into_branch), (1, 0));
        assert_eq!(tag.as_deref(), Some("v1.0"));
    }
}
