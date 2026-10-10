//! `factory ctx def` (PRD §14 FR-12): one definition regex per language run
//! through `grep-searcher` and `grep-regex` over the walk. The kind printed
//! is the keyword the line matched (`class`, `fn`, `function`, `type`, …);
//! slice 12 maps it to the `--kind` vocabulary and confirms each hit with
//! tree-sitter. Until then the body is a heuristic window, and a regex hit
//! inside a docblock is still printed.
use std::path::Path;

use super::walk::{self, Lang, WalkOpts};
use super::{Searched, matcher, matching_lines, rel};
use crate::Result;

/// The most lines a heuristic body takes, the match line included: the
/// median body the research measured (`ctx-research.md` §2).
pub const BODY_LINES: usize = 25;

/// One definition found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Def {
    pub path: String,
    pub line: u64,
    pub kind: &'static str,
    /// The match line, trimmed.
    pub signature: String,
    /// The lines after the match line, dedented by its indentation.
    pub body: Vec<String>,
    /// The body stopped at [`BODY_LINES`] before it ended.
    pub cut: bool,
}

/// The four regex tables: Vue files use TypeScript's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Table {
    Php,
    Ts,
    Go,
    Rust,
}

impl Table {
    const ALL: [Table; 4] = [Table::Php, Table::Ts, Table::Go, Table::Rust];

    fn of(lang: Lang) -> Table {
        match lang {
            Lang::Php => Table::Php,
            Lang::Ts | Lang::Vue => Table::Ts,
            Lang::Go => Table::Go,
            Lang::Rust => Table::Rust,
        }
    }

    /// The definition regexes, `NAME` already escaped, each anchored at the
    /// line start after whitespace and modifiers (slice 11 § Current code).
    fn patterns(self, name: &str) -> Vec<String> {
        let p = match self {
            Table::Php => vec![
                r"(abstract\s+|final\s+)?(class|interface|trait|enum)\s+NAME\b",
                r"(public|protected|private|static|\s)*function\s+&?NAME\s*\(",
            ],
            Table::Ts => vec![
                r"(export\s+)?(default\s+)?(abstract\s+)?(class|interface|type|enum)\s+NAME\b",
                r"(export\s+)?(async\s+)?function\s*\*?\s*NAME\s*[(<]",
                r"(export\s+)?(const|let|var)\s+NAME\s*(:[^=]+)?=\s*(async\s*)?(\(|[A-Za-z_$][\w$]*\s*=>)",
                r"(public|private|protected|static|readonly|async|\s)*NAME\s*\([^)]*\)\s*(:[^{]+)?\{",
            ],
            Table::Go => vec![r"func\s+(\([^)]*\)\s*)?NAME\s*[(\[]", r"type\s+NAME\b"],
            Table::Rust => vec![
                r"(pub(\([^)]*\))?\s+)?(async\s+)?(unsafe\s+)?(fn|struct|enum|trait|type|mod|const|static)\s+NAME\b",
                r"macro_rules!\s+NAME",
            ],
        };
        p.into_iter()
            .map(|p| format!(r"^\s*{}", p.replace("NAME", name)))
            .collect()
    }

    /// The keywords a match can carry, and the kind of a match with none
    /// (only TypeScript's class-member regex has no keyword).
    fn keywords(self) -> (&'static [&'static str], &'static str) {
        match self {
            Table::Php => (
                &["class", "interface", "trait", "enum", "function"],
                "function",
            ),
            Table::Ts => (
                &[
                    "class",
                    "interface",
                    "type",
                    "enum",
                    "function",
                    "const",
                    "let",
                    "var",
                ],
                "method",
            ),
            Table::Go => (&["func", "type"], "func"),
            Table::Rust => (
                &[
                    "fn",
                    "struct",
                    "enum",
                    "trait",
                    "type",
                    "mod",
                    "const",
                    "static",
                    "macro_rules",
                ],
                "fn",
            ),
        }
    }
}

/// What `def` found, in path then line order.
#[derive(Debug, Default)]
pub struct Found {
    pub hits: Vec<Def>,
    pub searched: Searched,
}

/// Every definition of `name` under `base`.
pub fn find(base: &Path, name: &str, opts: &WalkOpts) -> Result<Found> {
    let escaped = escape(name);
    let matchers = Table::ALL
        .iter()
        .map(|t| {
            let joined = t
                .patterns(&escaped)
                .iter()
                .map(|p| format!("(?:{p})"))
                .collect::<Vec<_>>()
                .join("|");
            matcher(&joined).map(|m| (*t, m))
        })
        .collect::<Result<Vec<_>>>()?;
    let walked = walk::walk(base, opts);
    let (per_file, binary) = walk::scan(&walked.files, |path, lang, bytes| {
        let table = Table::of(lang);
        let (_, m) = matchers.iter().find(|(t, _)| *t == table)?;
        let numbers = matching_lines(m, bytes);
        if numbers.is_empty() {
            return None;
        }
        let text = String::from_utf8_lossy(bytes);
        let lines: Vec<&str> = text.lines().collect();
        let defs = numbers
            .into_iter()
            .filter_map(|n| {
                let at = usize::try_from(n).ok()?.checked_sub(1)?;
                let line = *lines.get(at)?;
                let (end, cut) = body_end(&lines, at);
                let indent = indent(line);
                Some(Def {
                    path: rel(base, path),
                    line: n,
                    kind: kind_of(table, line, name),
                    signature: line.trim().to_string(),
                    body: lines[at + 1..end]
                        .iter()
                        .map(|l| dedent(l, indent))
                        .collect(),
                    cut,
                })
            })
            .collect::<Vec<_>>();
        Some(defs)
    });
    Ok(Found {
        hits: per_file.into_iter().flat_map(|(_, d)| d).collect(),
        searched: Searched {
            files: walked.files.len() - binary,
            too_large: walked.too_large,
            binary,
        },
    })
}

/// Escapes the regex metacharacters in a symbol name.
pub fn escape(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.chars() {
        if r"\.+*?()|[]{}^$#&-~".contains(c) {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

/// The last keyword of the table before `name` on a matched line.
fn kind_of(table: Table, line: &str, name: &str) -> &'static str {
    let (keywords, none) = table.keywords();
    let before = &line[..word_at(line, name).unwrap_or(line.len())];
    before
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter_map(|tok| keywords.iter().find(|k| **k == tok))
        .next_back()
        .copied()
        .unwrap_or(none)
}

/// The byte offset of the first whole-word `name` in `line`.
fn word_at(line: &str, name: &str) -> Option<usize> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(name).map(|(i, _)| i).find(|&i| {
        let before = line[..i].chars().next_back();
        let after = line[i + name.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

/// Where the heuristic body of the definition on line `at` ends (exclusive)
/// and whether [`BODY_LINES`] cut it. It goes on while a bracket the
/// definition opened is still open, or the next non-blank line is indented
/// deeper than the match, or that line opens with `{` (a brace on its own
/// line). So it stops at the first line whose indentation returns to the
/// match's, keeping the closing brace and a signature spread over lines.
pub fn body_end(lines: &[&str], at: usize) -> (usize, bool) {
    let base = indent(lines[at]);
    let mut depth = 0i64;
    let mut i = at;
    loop {
        depth += balance(lines[i]);
        let Some(next) = (i + 1..lines.len()).find(|&j| !lines[j].trim().is_empty()) else {
            return (i + 1, false);
        };
        let goes_on =
            depth > 0 || indent(lines[next]) > base || lines[next].trim_start().starts_with('{');
        if !goes_on {
            return (i + 1, false);
        }
        if next - at >= BODY_LINES {
            let mut end = at + BODY_LINES;
            while end > i + 1 && lines[end - 1].trim().is_empty() {
                end -= 1;
            }
            return (end, true);
        }
        i = next;
    }
}

fn balance(line: &str) -> i64 {
    line.chars()
        .map(|c| match c {
            '{' | '(' | '[' => 1,
            '}' | ')' | ']' => -1,
            _ => 0,
        })
        .sum()
}

pub(crate) fn indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// `line` without up to `n` leading spaces or tabs.
pub(crate) fn dedent(line: &str, n: usize) -> String {
    let strip = indent(line).min(n);
    line[strip..].trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(src: &str, at: usize) -> (usize, bool) {
        let lines: Vec<&str> = src.lines().collect();
        body_end(&lines, at)
    }

    #[test]
    fn a_body_ends_at_the_brace_that_returns_to_the_match_indent() {
        let php = "final class A\n{\n    public function b()\n    {\n        return 1;\n    }\n}\n\nclass C {}\n";
        assert_eq!(end(php, 0), (7, false));
        assert_eq!(end(php, 2), (6, false));
        let rs = "pub struct A;\n\npub fn b() {}\npub fn c(\n    x: u8,\n) -> u8 {\n    x\n}\n";
        assert_eq!(end(rs, 0), (1, false));
        assert_eq!(end(rs, 2), (3, false));
        assert_eq!(end(rs, 3), (8, false));
        let psr = "    public function d(\n        int $x,\n    ): int\n    {\n        return $x;\n    }\n";
        assert_eq!(end(psr, 0), (6, false));
        let arrow = "const f = (x) =>\n  x + 1\n\nconst g = 2\n";
        assert_eq!(end(arrow, 0), (2, false));
    }

    #[test]
    fn a_long_body_is_cut_at_25_lines() {
        let mut src = String::from("fn long() {\n");
        for i in 0..40 {
            src.push_str(&format!("    let x{i} = {i};\n"));
        }
        src.push_str("}\n");
        assert_eq!(end(&src, 0), (25, true));
    }

    #[test]
    fn the_kind_is_the_keyword_before_the_name() {
        assert_eq!(
            kind_of(Table::Php, "final class UserService", "UserService"),
            "class"
        );
        assert_eq!(
            kind_of(Table::Php, "  public static function &handle(", "handle"),
            "function"
        );
        assert_eq!(
            kind_of(Table::Ts, "export const useThing = () => {", "useThing"),
            "const"
        );
        assert_eq!(
            kind_of(Table::Ts, "  async load(id: string) {", "load"),
            "method"
        );
        assert_eq!(
            kind_of(Table::Ts, "export default function* gen<T>(", "gen"),
            "function"
        );
        assert_eq!(
            kind_of(Table::Go, "func (c *Client) Client() {", "Client"),
            "func"
        );
        assert_eq!(kind_of(Table::Go, "type Config struct {", "Config"), "type");
        assert_eq!(
            kind_of(Table::Rust, "pub(crate) async unsafe fn run(", "run"),
            "fn"
        );
        assert_eq!(
            kind_of(Table::Rust, "macro_rules! make {", "make"),
            "macro_rules"
        );
    }

    #[test]
    fn metacharacters_in_a_name_are_escaped() {
        assert_eq!(escape("a.b$c"), r"a\.b\$c");
        assert_eq!(escape("plain_name1"), "plain_name1");
    }
}
