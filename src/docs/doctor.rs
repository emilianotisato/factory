//! Never writes: a v2 repo has no `docs/.gitignore` to hide a `docs.lock`.
use std::fs;
use std::path::Path;

use super::{check, frontmatter, project};
use crate::{Result, skills};

/// What `docs.mjs check` returned in a v2 or v1 repo.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framework {
    V3,
    V2,
    V1,
    None,
}

impl Framework {
    pub fn label(self) -> &'static str {
        match self {
            Framework::V3 => "v3",
            Framework::V2 => "v2 (run factory docs migrate --dry)",
            Framework::V1 => "v1",
            Framework::None => "not on the framework",
        }
    }
}

pub fn detect(root: &Path) -> Framework {
    if project::is_v3(root) {
        return Framework::V3;
    }
    let Ok(text) = fs::read_to_string(root.join("docs/index.md")) else {
        return Framework::None;
    };
    let fm = frontmatter::parse(&text);
    match fm.get("framework").and_then(|v| v.as_i64()) {
        Some(n) if n >= 2 => Framework::V2,
        _ => Framework::V1,
    }
}

fn line(key: &str, value: impl std::fmt::Display) -> String {
    format!("{key:<15}{value}")
}

pub fn run(root: &Path, legacy_check: impl FnOnce() -> Result<ToolOutput>) -> String {
    let fw = detect(root);
    let mut out = vec![line("Framework", fw.label())];
    match fw {
        Framework::None => {}
        Framework::V3 => {
            let gate = project::load(root)
                .ok()
                .flatten()
                .and_then(|p| p.gate)
                .unwrap_or_else(|| "none declared".into());
            out.push(line("gate", gate));
            for l in skills::verdict(root).lines() {
                out.push(line("skills", l));
            }
            let r = check::run(root);
            if r.is_clean() {
                out.push(line("drift", "clean"));
            } else {
                out.push(line("drift", r.counts()));
                out.extend(r.lines());
            }
        }
        Framework::V2 | Framework::V1 => match legacy_check() {
            Err(e) => out.push(line("drift", format!("unknown: {e}"))),
            Ok(o) => {
                let text = format!("{}{}", o.stdout, o.stderr);
                if text.contains("docs check: clean") {
                    out.push(line("drift", "clean"));
                } else {
                    let counts = text
                        .lines()
                        .find(|l| l.contains(" errors, ") || l.contains(" error, "))
                        .map(|l| l.trim().to_string())
                        .unwrap_or_else(|| format!("docs.mjs check exited {}", o.code));
                    out.push(line("drift", &counts));
                    out.extend(
                        text.lines()
                            .filter(|l| !l.trim().is_empty() && l.trim() != counts)
                            .map(str::to_string),
                    );
                }
            }
        },
    }
    out.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_reads_project_then_index() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        assert_eq!(detect(root), Framework::None);
        fs::create_dir(root.join("docs")).unwrap();
        fs::write(root.join("docs/index.md"), "# Board\n").unwrap();
        assert_eq!(detect(root), Framework::V1);
        fs::write(
            root.join("docs/index.md"),
            "---\nframework: 2\n---\n# Board\n",
        )
        .unwrap();
        assert_eq!(detect(root), Framework::V2);
        fs::write(root.join("docs/project.toml"), "[docs]\nframework = 3\n").unwrap();
        assert_eq!(detect(root), Framework::V3);
    }

    #[test]
    fn a_legacy_repo_reports_the_tool_verdict() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        fs::create_dir(root.join("docs")).unwrap();
        fs::write(root.join("docs/index.md"), "---\nframework: 2\n---\n").unwrap();
        let out = run(root, || {
            Ok(ToolOutput {
                stdout: String::new(),
                stderr: "  ✗ E-X  bad\n\n1 error, 0 warnings\n".into(),
                code: 1,
            })
        });
        assert_eq!(
            out,
            "Framework      v2 (run factory docs migrate --dry)\ndrift          1 error, 0 warnings\n  ✗ E-X  bad\n"
        );
        let out = run(root, || Err(crate::Error::Msg("no docs.mjs: x".into())));
        assert!(out.contains("drift          unknown: no docs.mjs"), "{out}");
    }
}
