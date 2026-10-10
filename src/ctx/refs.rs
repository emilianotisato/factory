//! `factory ctx refs` (PRD §14 FR-12): a word-boundary search over the walk,
//! collapsed per file and capped, so a symbol with three hundred callers
//! answers in ten lines and a trailer. Test files are left out unless
//! `--tests`: they are the noisiest callers in the transcripts. Each file
//! with hits is parsed once and a hit line is dropped when every match on
//! it lies inside a comment or a string, before the caps.
use std::path::Path;

use super::def::{dedent, escape, indent, words};
use super::lang::{Unit, line_starts};
use super::walk::{self, Lang, WalkOpts};
use super::{Searched, matcher, matching_lines, rel};
use crate::Result;

/// One reference shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    pub path: String,
    pub line: u64,
    /// The hit line, trimmed.
    pub text: String,
    /// The line after it, dedented by the hit's indentation; none when that
    /// line is blank or the file ends.
    pub next: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub max: usize,
    pub per_file: usize,
    pub tests: bool,
}

/// What `refs` found: the hits shown, then what the caps cut.
#[derive(Debug, Default)]
pub struct Found {
    pub shown: Vec<Ref>,
    pub total: usize,
    pub files: usize,
    pub more_hits: usize,
    pub more_files: usize,
    /// Hits in test files, left out without `--tests`.
    pub test_hits: usize,
    /// Hit lines inside comments or strings, dropped before the caps.
    pub dropped: usize,
    /// Files whose tree holds a syntax error: their hits kept unfiltered.
    pub unparsed: usize,
    pub searched: Searched,
}

/// One file's share of [`Found`].
struct InFile {
    test: bool,
    hits: usize,
    dropped: usize,
    unparsed: bool,
    first: Vec<Ref>,
}

/// The references to `name` under `root`, with paths relative to `base`.
pub fn find(base: &Path, root: &Path, name: &str, caps: Caps, opts: &WalkOpts) -> Result<Found> {
    let m = matcher(&format!(r"\b{}\b", escape(name)))?;
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

/// The hit lines holding a whole-word `name` outside every comment and
/// string, and whether the file's tree holds a syntax error (then every
/// line is kept: a broken tree misplaces strings). A Vue hit outside the
/// `<script>` block is kept.
fn code_lines(
    path: &Path,
    lang: Lang,
    text: &str,
    name: &str,
    numbers: Vec<u64>,
) -> (Vec<u64>, bool) {
    let Some(unit) = Unit::of(path, lang, text) else {
        return (numbers, false);
    };
    let Some(tree) = unit.parse().filter(|t| !t.root_node().has_error()) else {
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
                || at.any(|i| !unit.in_comment_or_string(&tree, from + i..from + i + name.len()))
        })
        .collect();
    (kept, false)
}

/// A test path, relative to the search root: `tests?/|spec|_test\.go|\.test\.|\.spec\.`
/// (slice 11 task 3).
pub fn is_test(path: &str) -> bool {
    ["test/", "tests/", "spec", "_test.go", ".test."]
        .iter()
        .any(|p| path.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_match_the_slice_pattern() {
        for p in [
            "tests/Feature/UserTest.php",
            "src/test/a.ts",
            "client_test.go",
            "a.test.ts",
            "a.spec.ts",
            "spec/models.ts",
        ] {
            assert!(is_test(p), "{p}");
        }
        for p in ["src/user.ts", "app/Services/UserService.php", "contest.go"] {
            assert!(!is_test(p), "{p}");
        }
    }
}
