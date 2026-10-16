//! Test files are left out by default: they are the noisiest callers.
use std::path::Path;

use super::def::{dedent, escape, indent, words};
use super::lang::{Parsed, Unit, line_starts};
use super::walk::{self, Lang, WalkOpts};
use super::{Searched, matcher, matching_lines, rel, whole_word};
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    pub path: String,
    pub line: u64,
    pub text: String,
    pub next: Option<String>,
}

pub const MAX: usize = 10;
pub const PER_FILE: usize = 3;

#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub max: usize,
    pub per_file: usize,
    pub tests: bool,
}

#[derive(Debug, Default)]
pub struct Found {
    pub shown: Vec<Ref>,
    pub total: usize,
    pub files: usize,
    pub more_hits: usize,
    pub more_files: usize,
    pub test_hits: usize,
    pub dropped: usize,
    pub unparsed: usize,
    pub searched: Searched,
}

struct InFile {
    test: bool,
    hits: usize,
    dropped: usize,
    unparsed: bool,
    first: Vec<Ref>,
}

pub fn find(base: &Path, root: &Path, name: &str, caps: Caps, opts: &WalkOpts) -> Result<Found> {
    let m = matcher(&whole_word(&escape(name)))?;
    let walked = walk::walk(root, opts);
    let (per_file, binary) = walk::scan(&walked.files, |path, lang, bytes| {
        let numbers = matching_lines(&m, bytes);
        if numbers.is_empty() {
            return None;
        }
        let test = is_test(&path.strip_prefix(root).unwrap_or(path).to_string_lossy());
        if test && !caps.tests {
            return Some(InFile {
                test,
                hits: numbers.len(),
                dropped: 0,
                unparsed: false,
                first: Vec::new(),
            });
        }
        let text = String::from_utf8_lossy(bytes);
        let all = numbers.len();
        let (kept, unparsed) = code_lines(path, lang, &text, name, numbers);
        let lines: Vec<&str> = text.lines().collect();
        let first = kept
            .iter()
            .take(caps.per_file)
            .filter_map(|&n| {
                let at = usize::try_from(n).ok()?.checked_sub(1)?;
                let hit = *lines.get(at)?;
                Some(Ref {
                    path: rel(base, path),
                    line: n,
                    text: hit.trim().to_string(),
                    next: lines
                        .get(at + 1)
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| dedent(l, indent(hit))),
                })
            })
            .collect::<Vec<_>>();
        Some(InFile {
            test,
            hits: kept.len(),
            dropped: all - kept.len(),
            unparsed,
            first,
        })
    });
    let mut found = Found {
        searched: Searched {
            files: walked.files.len() - binary,
            too_large: walked.too_large,
            binary,
        },
        ..Found::default()
    };
    for (_, f) in per_file {
        if f.test && !caps.tests {
            found.test_hits += f.hits;
            continue;
        }
        found.dropped += f.dropped;
        found.unparsed += usize::from(f.unparsed);
        if f.hits == 0 {
            continue;
        }
        found.total += f.hits;
        found.files += 1;
        let room = caps.max.saturating_sub(found.shown.len());
        let shown = f.first.len().min(room);
        found.shown.extend(f.first.into_iter().take(shown));
        if shown < f.hits {
            found.more_files += 1;
        }
    }
    found.more_hits = found.total - found.shown.len();
    Ok(found)
}

/// A broken tree misplaces strings, so every line is kept; a Vue hit outside `<script>` is kept too.
fn code_lines(
    path: &Path,
    lang: Lang,
    text: &str,
    name: &str,
    numbers: Vec<u64>,
) -> (Vec<u64>, bool) {
    let units = Unit::all(path, lang, text);
    if units.is_empty() {
        return (numbers, false);
    }
    let Some(parsed) = Parsed::of(&units).filter(|p| !p.has_error()) else {
        return (numbers, true);
    };
    let starts = line_starts(text);
    let kept = numbers
        .into_iter()
        .filter(|&n| {
            let at = usize::try_from(n).ok().and_then(|n| n.checked_sub(1));
            let Some(from) = at.and_then(|at| starts.get(at).copied()) else {
                return true;
            };
            let to = at
                .and_then(|at| starts.get(at + 1).copied())
                .unwrap_or(text.len());
            let mut at = words(&text[from..to], name).peekable();
            at.peek().is_none()
                || at.any(|i| !parsed.in_comment_or_string(from + i..from + i + name.len()))
        })
        .collect();
    (kept, false)
}

pub fn is_test(path: &str) -> bool {
    let mut parts = path.split(['/', '\\']).filter(|p| !p.is_empty());
    let file = parts.next_back().unwrap_or("");
    parts.any(|d| matches!(d, "test" | "tests" | "spec" | "specs" | "__tests__"))
        || file.ends_with("_test.go")
        || file.contains(".test.")
        || file.contains(".spec.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirs_match_by_segment_and_test_files_by_name() {
        for p in [
            "tests/Feature/UserTest.php",
            "src/test/a.ts",
            "client_test.go",
            "a.test.ts",
            "a.spec.ts",
            "spec/models.ts",
            "web/__tests__/a.ts",
            "specs/a.ts",
            "pkg/test/c.go",
        ] {
            assert!(is_test(p), "{p}");
        }
        for p in [
            "src/user.ts",
            "app/Services/UserService.php",
            "contest.go",
            "app/Services/Inspector.php",
            "src/respect.rs",
            "docs/specification/x.ts",
            "latest/a.ts",
            "contest/a.go",
            "src/attest.go",
        ] {
            assert!(!is_test(p), "{p}");
        }
    }
}
