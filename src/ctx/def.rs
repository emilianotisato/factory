//! `factory ctx def` (PRD §14 FR-12): one definition regex per language run
//! through `grep-searcher` and `grep-regex` over the walk finds the
//! candidates; each candidate file is then tagged with tree-sitter
//! (`lang.rs`) and a hit is kept only when a definition's name sits on its
//! line. The regexes take modifiers in any order and a member whose
//! parameters run onto the next line, so every shape `outline` lists is a
//! candidate (AD-40001). The body is the definition node's exact range, the
//! kind the tag's in the `--kind` vocabulary. A file whose tree holds a
//! syntax error keeps its textual hits with slice 11's heuristic body, so a
//! broken file never hides a definition.
use std::path::Path;

use super::lang::{Definition, Parsed, Unit, definitions, kind_matches, line_of, line_starts};
use super::walk::{self, Lang, WalkOpts};
use super::{Searched, matcher, matching_lines, rel};
use crate::Result;

/// The most lines a heuristic body takes, the match line included: the
/// median body the research measured (`ctx-research.md` §2). Only a file
/// that does not parse cleanly gets a heuristic body.
pub const BODY_LINES: usize = 25;

/// One definition found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Def {
    pub path: String,
    pub line: u64,
    pub kind: &'static str,
    /// The signature line, trimmed: the line of the definition's name, or
    /// the match line in a file that does not parse cleanly.
    pub signature: String,
    /// The lines after the signature line, dedented by its indentation;
    /// the node's whole range when it opens above the name (AD-9003).
    pub body: Vec<String>,
    /// A heuristic body stopped at [`BODY_LINES`] before it ended.
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
    /// line start after whitespace and modifiers in any order (slice 11 §
    /// Current code, widened by AD-40001). A TypeScript member whose
    /// parameters run onto the next line matches on its name's line; a Go
    /// type inside a `type ( … )` group matches as a name, then a type,
    /// which a struct field does too, and the tree tells them apart.
    fn patterns(self, name: &str) -> Vec<String> {
        let p = match self {
            Table::Php => vec![
                r"((abstract|final|readonly)\s+)*(class|interface|trait|enum)\s+NAME\b",
                r"((public|protected|private|static|abstract|final)\s+)*function\s+&?NAME\s*\(",
            ],
            Table::Ts => vec![
                r"((export|default|declare|abstract|const)\s+)*(class|interface|type|enum)\s+NAME\b",
                r"((export|default|declare|async)\s+)*function\s*\*?\s*NAME\s*[(<]",
                r"(export\s+)?(const|let|var)\s+NAME\s*(:[^=]+)?=\s*(async\s*)?(\(|[A-Za-z_$][\w$]*\s*=>)",
                r"((public|private|protected|static|readonly|async|override|abstract|get|set)\s+)*\*?\s*NAME\s*(<[^>]*>)?\s*\(([^)]*\)\s*(:[^{]+)?\{|[^)]*$)",
            ],
            Table::Go => vec![
                r"func\s+(\([^)]*\)\s*)?NAME\s*[(\[]",
                r"type\s+NAME\b",
                r"NAME(\[[^\]]*\])?\s+(=\s*)?[\w.*\[({]",
            ],
            Table::Rust => vec![
                r#"(pub(\([^)]*\))?\s+)?((default|const|async|unsafe|extern(\s+"[^"]*")?)\s+)*(fn|struct|enum|union|trait|type|mod|const|static(\s+mut)?)\s+NAME\b"#,
                r"macro_rules!\s+NAME",
            ],
        };
        p.into_iter()
            .map(|p| format!(r"^\s*{}", p.replace("NAME", name)))
            .collect()
    }

    /// The keywords a match can carry, and the kind of a match with none
    /// (TypeScript's class member, a Go type in a `type ( … )` group).
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
            Table::Go => (&["func", "type"], "type"),
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
                    "union",
                    "macro_rules",
                ],
                "fn",
            ),
        }
    }
}

/// What `def` found, in path then line order, and what it left out.
#[derive(Debug, Default)]
pub struct Found {
    pub hits: Vec<Def>,
    pub searched: Searched,
    /// Textual matches inside a comment or a string (a docblock mention).
    pub in_comments: usize,
    /// Textual matches on no definition's name (a call that reads like one).
    pub not_defs: usize,
    /// Definitions of a kind `--kind` left out.
    pub other_kind: usize,
    /// Files whose tree holds a syntax error: their textual matches kept.
    pub unparsed: usize,
}

/// One file's share of [`Found`].
#[derive(Debug, Default)]
struct InFile {
    hits: Vec<Def>,
    in_comments: usize,
    not_defs: usize,
    other_kind: usize,
    unparsed: bool,
}

/// Every definition of `name` under `base`, of kind `want` when given
/// (`fn` takes methods too).
pub fn find(base: &Path, name: &str, want: Option<&str>, opts: &WalkOpts) -> Result<Found> {
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
        let file = File {
            shown: rel(base, path),
            text: &text,
            name,
            want,
            numbers: &numbers,
        };
        Some(
            file.confirmed(path, lang)
                .unwrap_or_else(|| file.textual(table)),
        )
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
        found.hits.extend(f.hits);
        found.in_comments += f.in_comments;
        found.not_defs += f.not_defs;
        found.other_kind += f.other_kind;
        found.unparsed += usize::from(f.unparsed);
    }
    Ok(found)
}

/// One candidate file: its text and the lines the regexes matched.
struct File<'a> {
    shown: String,
    text: &'a str,
    name: &'a str,
    want: Option<&'a str>,
    numbers: &'a [u64],
}

impl File<'_> {
    /// The hits the file's definition tags confirm; none when its tree holds
    /// a syntax error or tagging fails. An `impl` block names a type it does
    /// not define, so it confirms nothing. A Vue file is tagged block by
    /// block; one without a `<script>` block has no definitions.
    fn confirmed(&self, path: &Path, lang: Lang) -> Option<InFile> {
        let text = self.text;
        let units = Unit::all(path, lang, text);
        let (defs, broken) = definitions(&units)?;
        if broken {
            return None;
        }
        let starts = line_starts(text);
        let mut parsed = None;
        let mut f = InFile::default();
        for &n in self.numbers {
            let Some(at) = usize::try_from(n).ok().and_then(|n| n.checked_sub(1)) else {
                continue;
            };
            let Some(&from) = starts.get(at) else {
                continue;
            };
            let to = starts.get(at + 1).copied().unwrap_or(text.len());
            let tag = defs.iter().find(|d| {
                d.kind != "impl"
                    && (from..to).contains(&d.name.start)
                    && text.get(d.name.clone()) == Some(self.name)
            });
            match tag {
                Some(d) if kind_matches(self.want, d.kind) => {
                    f.hits.push(self.node_def(&starts, d));
                }
                Some(_) => f.other_kind += 1,
                None => {
                    let inside = word_at(&text[from..to], self.name).is_some_and(|i| {
                        parsed
                            .get_or_insert_with(|| Parsed::of(&units))
                            .as_ref()
                            .is_some_and(|p| {
                                p.in_comment_or_string(from + i..from + i + self.name.len())
                            })
                    });
                    if inside {
                        f.in_comments += 1;
                    } else {
                        f.not_defs += 1;
                    }
                }
            }
        }
        Some(f)
    }

    /// A confirmed definition: the location and signature are the name's
    /// line, as `outline` prints them; the body is the node's other lines,
    /// the last one cut where the node ends. A node that opens above its
    /// name (PHP attributes, a TypeScript class decorator) prints its whole
    /// range as the body, the signature line included, so the attributes
    /// stay in place (AD-9003).
    fn node_def(&self, starts: &[usize], d: &Definition) -> Def {
        let text = self.text;
        let first = line_of(starts, d.range.start);
        let last = line_of(starts, d.range.end.max(d.range.start + 1) - 1);
        let at = line_of(starts, d.name.start).clamp(first, last);
        let line = |i: usize| {
            let to = starts.get(i + 1).copied().unwrap_or(text.len());
            &text[starts[i]..to]
        };
        let head = line(at);
        let indent = indent(head);
        let from = if at == first { first + 1 } else { first };
        let body = (from..=last)
            .map(|i| {
                let l = if i == last {
                    &text[starts[i]..d.range.end.max(starts[i])]
                } else {
                    line(i)
                };
                dedent(l, indent)
            })
            .collect();
        Def {
            path: self.shown.clone(),
            line: at as u64 + 1,
            kind: d.kind,
            signature: head.trim().to_string(),
            body,
            cut: false,
        }
    }

    /// Slice 11's output for a file that does not parse cleanly: every
    /// textual hit, the heuristic body, the keyword's kind. `--kind` still
    /// filters, but `fn` and `method` pass for each other: the keyword
    /// cannot tell them apart.
    fn textual(&self, table: Table) -> InFile {
        let lines: Vec<&str> = self.text.lines().collect();
        let mut f = InFile {
            unparsed: true,
            ..InFile::default()
        };
        for &n in self.numbers {
            let Some(at) = usize::try_from(n).ok().and_then(|n| n.checked_sub(1)) else {
                continue;
            };
            let Some(&line) = lines.get(at) else {
                continue;
            };
            let kind = vocabulary(table, kind_of(table, line, self.name));
            let either =
                matches!(kind, "fn" | "method") && matches!(self.want, Some("fn" | "method"));
            if !(kind_matches(self.want, kind) || either) {
                f.other_kind += 1;
                continue;
            }
            let (end, cut) = body_end(&lines, at);
            let indent = indent(line);
            f.hits.push(Def {
                path: self.shown.clone(),
                line: n,
                kind,
                signature: line.trim().to_string(),
                body: lines[at + 1..end]
                    .iter()
                    .map(|l| dedent(l, indent))
                    .collect(),
                cut,
            });
        }
        f
    }
}

/// A regex keyword in the `--kind` vocabulary, as close as a keyword gets.
fn vocabulary(table: Table, keyword: &'static str) -> &'static str {
    match keyword {
        "class" | "interface" | "trait" | "enum" | "struct" | "union" => "class",
        "const" | "static" if table == Table::Rust => "constant",
        "function" | "func" | "fn" | "const" | "let" | "var" => "fn",
        "mod" => "module",
        "macro_rules" => "macro",
        other => other,
    }
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
    words(line, name).next()
}

/// The byte offsets of every whole-word `name` in `line`.
pub(crate) fn words<'a>(line: &'a str, name: &'a str) -> impl Iterator<Item = usize> + 'a {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(name).map(|(i, _)| i).filter(move |&i| {
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
        assert_eq!(kind_of(Table::Go, "\tGrouped struct {", "Grouped"), "type");
        assert_eq!(
            kind_of(Table::Rust, r#"pub extern "C" fn ffi() {}"#, "ffi"),
            "fn"
        );
        assert_eq!(
            kind_of(Table::Php, "final readonly class Dto", "Dto"),
            "class"
        );
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
    fn a_keyword_maps_to_the_kind_vocabulary() {
        assert_eq!(vocabulary(Table::Php, "trait"), "class");
        assert_eq!(vocabulary(Table::Go, "func"), "fn");
        assert_eq!(vocabulary(Table::Ts, "const"), "fn");
        assert_eq!(vocabulary(Table::Rust, "const"), "constant");
        assert_eq!(vocabulary(Table::Rust, "mod"), "module");
        assert_eq!(vocabulary(Table::Ts, "method"), "method");
        assert_eq!(vocabulary(Table::Go, "type"), "type");
    }

    #[test]
    fn metacharacters_in_a_name_are_escaped() {
        assert_eq!(escape("a.b$c"), r"a\.b\$c");
        assert_eq!(escape("plain_name1"), "plain_name1");
    }
}
