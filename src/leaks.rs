//! One table of rules run over the lines and paths a diff adds: plan ids, history and cites that belong in the docs, not the code.
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::docs::project;
use crate::error::io;
use crate::{Error, Result, ci};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Fix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Comment,
    String,
    Name,
    Path,
}

#[derive(Debug, Clone, Copy)]
pub enum Detector {
    Pattern(&'static str),
    /// An item and a number at an identifier or camelCase boundary, see [`name_hit`].
    Name,
}

pub struct Rule {
    pub id: &'static str,
    pub title: &'static str,
    pub looks: &'static [Look],
    pub detector: Detector,
    pub severity: Severity,
    pub fix: &'static str,
}

macro_rules! ids {
    () => {
        r"\b(?:PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK)-\d+|\bslice[- ]?\d+"
    };
}

pub const RULES: [Rule; 9] = [
    Rule {
        id: "id-comment",
        title: "a plan id in a comment",
        looks: &[Look::Comment],
        detector: Detector::Pattern(concat!("(?i)", ids!(), "|§")),
        severity: Severity::Fix,
        fix: "Delete the id. Keep the fact as one short line only if the code cannot show it.",
    },
    Rule {
        id: "id-name",
        title: "a plan id in a name or path",
        looks: &[Look::Name, Look::Path],
        detector: Detector::Name,
        severity: Severity::Fix,
        fix: "Rename after the behavior and update every reference.",
    },
    Rule {
        id: "id-string",
        title: "a plan id in a string",
        looks: &[Look::String],
        detector: Detector::Pattern(concat!("(?i)", ids!())),
        severity: Severity::Fix,
        fix: "Rewrite the string without the id, after a grep of tests/ and src/ for every match on it.",
    },
    Rule {
        id: "history",
        title: "history in a comment",
        looks: &[Look::Comment],
        detector: Detector::Pattern(
            r"(?i)\b(?:until slice|for now|previously|no longer|added in|used to|not built yet|a later slice|before PRD|since slice|as corrected|confirmed on)\b",
        ),
        severity: Severity::Fix,
        fix: "Delete it. A comment says what the code is; git log says what it was.",
    },
    Rule {
        id: "line-cite",
        title: "a file:line cite",
        looks: &[Look::Comment, Look::String],
        detector: Detector::Pattern(r"\w[\w.-]*\.(?:md|mjs|js|ts|rs|go|php|py):\d+(?:-\d+)?"),
        severity: Severity::Fix,
        fix: "Drop the cite; in a string, rewrite it after a grep of tests/ and src/. Keep the fact as one short line only if the code cannot show it.",
    },
    Rule {
        id: "section-cite",
        title: "a skill or asset section cite",
        looks: &[Look::Comment, Look::String],
        detector: Detector::Pattern(r"/\w+ §|\w+\.md §|(?i:prd) §"),
        severity: Severity::Fix,
        fix: "Drop the cite; in a string, rewrite it after a grep of tests/ and src/. Keep the fact as one short line only if the code cannot show it.",
    },
    Rule {
        id: "step-marker",
        title: "a task or step marker",
        looks: &[Look::Comment],
        detector: Detector::Pattern(
            r"\b[Tt]asks? \d|/// Task \d+:|── (?i:task \d+|prd-|fr-|slice \d)",
        ),
        severity: Severity::Fix,
        fix: "Delete the marker. A banner stays only when it names no task or plan id.",
    },
    Rule {
        id: "finding-id",
        title: "a review-finding id in a comment",
        looks: &[Look::Comment],
        // `L1` alone is a cache level, so the L row skips it.
        detector: Detector::Pattern(
            r"\b(?:C|Cn|K|Ln|S|X[a-d])\d{1,2}\b|\bL(?:[02-9]\d?|1\d)\b|finding [A-Z]+\d",
        ),
        severity: Severity::Fix,
        fix: "Delete the finding id. Keep the fact as one short line only if the code cannot show it.",
    },
    Rule {
        id: "plan-words",
        title: "plan words in a comment",
        looks: &[Look::Comment],
        detector: Detector::Pattern(
            r"(?i)the spec says|the PRD says|qa-guide row|slice-entry|checkpoint's transcript",
        ),
        severity: Severity::Fix,
        fix: "Drop the reference to the plan. Keep the fact as one short line only if the code cannot show it.",
    },
];

const ID_RULES: [&str; 3] = ["id-comment", "id-name", "id-string"];

const ITEMS: [&str; 11] = [
    "prd", "fr", "ad", "cp", "open", "oq", "dec", "block", "ask", "slice", "section",
];

static COMPILED: LazyLock<Vec<Option<Regex>>> = LazyLock::new(|| {
    RULES
        .iter()
        .map(|r| match r.detector {
            Detector::Pattern(p) => Some(Regex::new(p).expect("rule pattern compiles")),
            Detector::Name => None,
        })
        .collect()
});

static IDENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*").expect("ident pattern"));

fn matches(rule: usize, text: &str) -> bool {
    match &COMPILED[rule] {
        Some(re) => re.is_match(text),
        None => name_hit(text),
    }
}

/// An item at an identifier or camelCase boundary, then `-`, `_` or nothing, then digits that do not run on into a
/// lowercase word.
fn name_hit(name: &str) -> bool {
    let c: Vec<char> = name.chars().collect();
    let starts = |i: usize| {
        i == 0
            || !c[i - 1].is_ascii_alphanumeric()
            || (c[i - 1].is_ascii_lowercase() && c[i].is_ascii_uppercase())
    };
    (0..c.len()).filter(|&i| starts(i)).any(|i| {
        ITEMS.iter().any(|item| {
            let end = i + item.len();
            let same = end <= c.len()
                && c[i..end]
                    .iter()
                    .zip(item.chars())
                    .all(|(a, b)| a.eq_ignore_ascii_case(&b));
            if !same {
                return false;
            }
            let mut j = end;
            if matches!(c.get(j), Some('-' | '_')) {
                j += 1;
            }
            let digits = c[j..].iter().take_while(|ch| ch.is_ascii_digit()).count();
            digits > 0 && !c.get(j + digits).is_some_and(char::is_ascii_lowercase)
        })
    })
}

/// How a file's lines tell comments and strings from code.
#[derive(Debug, Clone, Copy)]
struct Syntax {
    /// `//`, `/* … */`, and a block comment's middle line by its leading `*`.
    slashes: bool,
    /// `#` at the line start or after a space.
    hash: bool,
    /// `#[` is an attribute, not a comment.
    hash_attr: bool,
    quotes: &'static str,
    /// `'x'` is a char literal and `'a` a lifetime, never a string.
    char_literals: bool,
    /// `r"…"` and `r#"…"#`.
    raw_strings: bool,
}

const C_LIKE: Syntax = Syntax {
    slashes: true,
    hash: false,
    hash_attr: false,
    quotes: "\"'`",
    char_literals: false,
    raw_strings: false,
};
const HASH: Syntax = Syntax {
    slashes: false,
    hash: true,
    hash_attr: false,
    quotes: "\"'",
    char_literals: false,
    raw_strings: false,
};

fn syntax(path: &str) -> Option<Syntax> {
    let ext = Path::new(path).extension()?.to_str()?;
    Some(match ext {
        "rs" => Syntax {
            quotes: "\"",
            char_literals: true,
            raw_strings: true,
            ..C_LIKE
        },
        "go" => Syntax {
            quotes: "\"`",
            char_literals: true,
            ..C_LIKE
        },
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx" | "vue" => C_LIKE,
        "php" => Syntax {
            hash: true,
            hash_attr: true,
            quotes: "\"'",
            ..C_LIKE
        },
        "py" | "sh" | "bash" | "toml" | "yaml" | "yml" => HASH,
        "json" | "jsonl" => Syntax {
            hash: false,
            quotes: "\"",
            ..HASH
        },
        _ => return None,
    })
}

#[derive(Debug, Default, PartialEq)]
struct Parts {
    code: String,
    strings: Vec<String>,
    comment: Option<String>,
}

impl Parts {
    fn is_comment(&self) -> bool {
        self.comment.is_some() && self.code.trim().is_empty() && self.strings.is_empty()
    }
}

fn ident(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn split(line: &str, syn: Syntax) -> Parts {
    let mut parts = Parts::default();
    let trimmed = line.trim_start();
    if syn.slashes && (trimmed == "*" || trimmed.starts_with("* ") || trimmed.starts_with("*/")) {
        parts.comment = Some(trimmed.to_string());
        return parts;
    }
    let c: Vec<char> = line.chars().collect();
    let rest = |i: usize| c[i..].iter().collect::<String>();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if syn.slashes && ch == '/' && matches!(c.get(i + 1), Some('/' | '*')) {
            parts.comment = Some(rest(i));
            break;
        }
        if syn.hash
            && ch == '#'
            && (i == 0 || c[i - 1].is_whitespace())
            && !(syn.hash_attr && c.get(i + 1) == Some(&'['))
        {
            parts.comment = Some(rest(i));
            break;
        }
        if syn.raw_strings
            && ch == 'r'
            && (i == 0 || !ident(c[i - 1]) || (c[i - 1] == 'b' && (i == 1 || !ident(c[i - 2]))))
        {
            let hashes = c[i + 1..].iter().take_while(|&&h| h == '#').count();
            let open = i + 1 + hashes;
            if c.get(open) == Some(&'"') {
                let close: Vec<char> = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let body = open + 1;
                let end = (body..c.len())
                    .find(|&k| c[k..].starts_with(&close))
                    .unwrap_or(c.len());
                parts.strings.push(c[body..end].iter().collect());
                parts.code.push(' ');
                i = (end + close.len()).min(c.len());
                continue;
            }
        }
        if syn.char_literals && ch == '\'' {
            if c.get(i + 1) == Some(&'\\') {
                let end = (i + 3..c.len()).find(|&k| c[k] == '\'').unwrap_or(c.len());
                parts.code.push(' ');
                i = end + 1;
                continue;
            }
            if c.get(i + 2) == Some(&'\'') {
                parts.code.push(' ');
                i += 3;
                continue;
            }
        }
        if syn.quotes.contains(ch) {
            let mut s = String::new();
            let mut j = i + 1;
            while j < c.len() && c[j] != ch {
                if c[j] == '\\' && j + 1 < c.len() {
                    s.push(c[j]);
                    j += 1;
                }
                s.push(c[j]);
                j += 1;
            }
            parts.strings.push(s);
            parts.code.push(' ');
            i = j + 1;
            continue;
        }
        parts.code.push(ch);
        i += 1;
    }
    parts
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Strings are not read; comments, names and paths still are.
    Strings,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allow {
    pub path: String,
    pub scope: Scope,
}

impl Allow {
    fn covers(&self, path: &str) -> bool {
        let dir = self.path.trim_end_matches('/');
        path == self.path || path.strip_prefix(dir).is_some_and(|r| r.starts_with('/'))
    }
}

fn scope_of(allow: &[Allow], path: &str) -> Option<Scope> {
    let covering = allow.iter().filter(|a| a.covers(path));
    covering.map(|a| a.scope).max_by_key(|s| *s == Scope::All)
}

/// `[leaks] allow` of `docs/project.toml`; no file or no table is an empty list.
pub fn allowlist(root: &Path) -> Result<Vec<Allow>> {
    let p = project::path(root);
    let text = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", p.display()))(e)),
    };
    let table: toml::Table = text.parse().map_err(|source| Error::Toml {
        what: format!("read {}", p.display()),
        source,
    })?;
    let Some(allow) = table.get("leaks").and_then(|l| l.get("allow")) else {
        return Ok(Vec::new());
    };
    let bad = |what: &str, entry: &toml::Value| {
        Error::Msg(format!(
            "{}: [leaks] allow entry {entry}: {what}",
            p.display()
        ))
    };
    let Some(items) = allow.as_array() else {
        return Err(bad("`allow` is not a list", allow));
    };
    items
        .iter()
        .map(|entry| {
            let Some(t) = entry.as_table() else {
                return Err(bad("not a `{ path = … }` table", entry));
            };
            if let Some(k) = t.keys().find(|k| !matches!(k.as_str(), "path" | "scope")) {
                return Err(bad(&format!("unknown key `{k}`"), entry));
            }
            let path = t
                .get("path")
                .and_then(toml::Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| bad("no `path` string", entry))?;
            let scope = match t.get("scope").map(|s| s.as_str()) {
                None | Some(Some("strings")) => Scope::Strings,
                Some(Some("all")) => Scope::All,
                _ => return Err(bad("scope is not `strings` or `all`", entry)),
            };
            Ok(Allow {
                path: path.trim_start_matches("./").to_string(),
                scope,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub path: String,
    pub line: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub rule: &'static str,
    pub path: String,
    /// `None` for a hit on the path itself.
    pub line: Option<u64>,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub hits: Vec<Hit>,
    pub comment_lines: u64,
    pub added_lines: u64,
}

fn rule(id: &str) -> &'static Rule {
    RULES
        .iter()
        .find(|r| r.id == id)
        .expect("a rule of the table")
}

impl Report {
    pub fn fix_hits(&self) -> usize {
        self.hits
            .iter()
            .filter(|h| rule(h.rule).severity == Severity::Fix)
            .count()
    }

    pub fn id_hits(&self) -> usize {
        self.hits
            .iter()
            .filter(|h| ID_RULES.contains(&h.rule))
            .count()
    }

    /// Comment lines over non-blank added lines, as a percentage.
    pub fn share(&self) -> f64 {
        if self.added_lines == 0 {
            0.0
        } else {
            self.comment_lines as f64 * 100.0 / self.added_lines as f64
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for r in &RULES {
            let hits: Vec<&Hit> = self.hits.iter().filter(|h| h.rule == r.id).collect();
            if hits.is_empty() {
                continue;
            }
            out.push_str(&format!("{}: {}\n  fix: {}\n", r.id, r.title, r.fix));
            for h in hits {
                match h.line {
                    Some(n) => out.push_str(&format!("  {}:{n}  {}\n", h.path, h.text)),
                    None => out.push_str(&format!("  {}\n", h.path)),
                }
            }
            out.push('\n');
        }
        out.push_str(&format!(
            "comment share: {:.1}% ({} of {} added lines)\n",
            self.share(),
            self.comment_lines,
            self.added_lines
        ));
        match self.fix_hits() {
            0 => out.push_str("clean\n"),
            1 => out.push_str("1 fix hit\n"),
            n => out.push_str(&format!("{n} fix hits\n")),
        }
        out
    }
}

fn in_docs(path: &str) -> bool {
    path.starts_with("docs/")
}

/// The rules over lines and paths already read; `scan` is this plus git.
pub fn check(allow: &[Allow], lines: &[Added], paths: &[String]) -> Report {
    let mut report = Report::default();
    for a in lines {
        if in_docs(&a.path) {
            continue;
        }
        let Some(syn) = syntax(&a.path) else { continue };
        let scope = scope_of(allow, &a.path);
        if scope == Some(Scope::All) || a.text.trim().is_empty() {
            continue;
        }
        let parts = split(&a.text, syn);
        report.added_lines += 1;
        report.comment_lines += u64::from(parts.is_comment());
        let strings = scope != Some(Scope::Strings);
        for (i, r) in RULES.iter().enumerate() {
            let hit = r.looks.iter().any(|look| match look {
                Look::Comment => parts.comment.as_deref().is_some_and(|t| matches(i, t)),
                Look::String => strings && parts.strings.iter().any(|s| matches(i, s)),
                Look::Name => IDENT.find_iter(&parts.code).any(|w| matches(i, w.as_str())),
                Look::Path => false,
            });
            if hit {
                report.hits.push(Hit {
                    rule: r.id,
                    path: a.path.clone(),
                    line: Some(a.line),
                    text: a.text.trim().to_string(),
                });
            }
        }
    }
    for p in paths {
        if in_docs(p) || scope_of(allow, p) == Some(Scope::All) {
            continue;
        }
        let name = p.replace('§', "section");
        for (i, r) in RULES.iter().enumerate() {
            if r.looks.contains(&Look::Path) && matches(i, &name) {
                report.hits.push(Hit {
                    rule: r.id,
                    path: p.clone(),
                    line: None,
                    text: String::new(),
                });
            }
        }
    }
    report
}

/// Every `+` line of a `-U0` diff, numbered from its hunk's `+start`.
pub fn parse_diff(text: &str) -> Vec<Added> {
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let (mut old_left, mut new_left, mut next) = (0u64, 0u64, 0u64);
    for line in text.lines() {
        if old_left > 0 || new_left > 0 {
            match line.chars().next() {
                Some('+') => {
                    if let Some(p) = &path {
                        out.push(Added {
                            path: p.clone(),
                            line: next,
                            text: line[1..].trim_end_matches('\r').to_string(),
                        });
                    }
                    next += 1;
                    new_left = new_left.saturating_sub(1);
                }
                Some('-') => old_left = old_left.saturating_sub(1),
                Some(' ') => {
                    old_left = old_left.saturating_sub(1);
                    new_left = new_left.saturating_sub(1);
                    next += 1;
                }
                _ => {}
            }
            continue;
        }
        if let Some(p) = line.strip_prefix("+++ ") {
            let p = p.trim_end_matches('\r').trim_matches('"');
            path = p.strip_prefix("b/").map(str::to_string);
        } else if let Some(h) = line.strip_prefix("@@ ") {
            let mut ranges = h.split_whitespace();
            let count = |r: Option<&str>, sign: char| -> (u64, u64) {
                let r = r.and_then(|r| r.strip_prefix(sign)).unwrap_or("0");
                let (start, n) = r.split_once(',').unwrap_or((r, "1"));
                (start.parse().unwrap_or(0), n.parse().unwrap_or(0))
            };
            old_left = count(ranges.next(), '-').1;
            (next, new_left) = count(ranges.next(), '+');
        }
    }
    out
}

/// The new path of every added or renamed row of a `--name-status -z` listing.
pub fn parse_name_status(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fields = text.split('\0').filter(|f| !f.is_empty());
    while let Some(status) = fields.next() {
        let two = status.starts_with('R') || status.starts_with('C');
        let first = fields.next();
        let new = if two { fields.next() } else { first };
        if (status.starts_with('A') || status.starts_with('R'))
            && let Some(p) = new
        {
            out.push(p.to_string());
        }
    }
    out
}

pub enum Input<'a> {
    /// `<ref>...HEAD`: what the branch added since it left the ref.
    Base(&'a str),
    /// The whole tree at `HEAD`, against the empty tree.
    All,
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = ci::git(repo, args)?;
    if !out.ok() {
        return Err(Error::Msg(format!(
            "git {}: {}",
            args.join(" "),
            out.text()
        )));
    }
    Ok(out.stdout)
}

/// Lines and paths the input adds outside `docs/`, run through the rules and the repo's allowlist.
pub fn scan(repo: &Path, input: Input) -> Result<Report> {
    let allow = allowlist(repo)?;
    // A SHA-256 repo has another empty tree, so ask git for it.
    let range: Vec<String> = match input {
        Input::Base(base) => vec![format!("{base}...HEAD")],
        Input::All => vec![
            git(repo, &["hash-object", "-t", "tree", "/dev/null"])?
                .trim()
                .to_string(),
            "HEAD".to_string(),
        ],
    };
    let range: Vec<&str> = range.iter().map(String::as_str).collect();
    let pathspec = ["--", ".", ":(exclude)docs"];
    let common = [
        "-c",
        "core.quotepath=off",
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--no-relative",
        "-M",
    ];
    let diff: Vec<&str> = common
        .iter()
        .copied()
        .chain(["-U0", "--src-prefix=a/", "--dst-prefix=b/"])
        .chain(range.iter().copied())
        .chain(pathspec)
        .collect();
    let names: Vec<&str> = common
        .iter()
        .copied()
        .chain(["--name-status", "-z"])
        .chain(range.iter().copied())
        .chain(pathspec)
        .collect();
    let lines = parse_diff(&git(repo, &diff)?);
    let paths = parse_name_status(&git(repo, &names)?);
    Ok(check(&allow, &lines, &paths))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits(path: &str, text: &str) -> Vec<&'static str> {
        hits_with(&[], path, text)
    }

    fn hits_with(allow: &[Allow], path: &str, text: &str) -> Vec<&'static str> {
        let line = Added {
            path: path.to_string(),
            line: 1,
            text: text.to_string(),
        };
        check(allow, &[line], &[])
            .hits
            .into_iter()
            .map(|h| h.rule)
            .collect()
    }

    fn path_hits(allow: &[Allow], path: &str) -> Vec<&'static str> {
        check(allow, &[], &[path.to_string()])
            .hits
            .into_iter()
            .map(|h| h.rule)
            .collect()
    }

    #[test]
    fn id_comment() {
        assert_eq!(hits("a.rs", "    // FR-3: guard"), ["id-comment"]);
        assert_eq!(hits("a.rs", "x(); // see § Caps"), ["id-comment"]);
        assert!(hits("a.rs", "    // the fixture's own numbering").is_empty());
    }

    #[test]
    fn id_name() {
        for line in [
            "fn prd_003() {}",
            "it('x', () => slice2Allowlists());",
            "let section_14 = 1;",
        ] {
            assert_eq!(hits("a.ts", line), ["id-name"], "{line}");
        }
        assert_eq!(path_hits(&[], "tests/fixture/prd-004/x.jsonl"), ["id-name"]);
        assert_eq!(path_hits(&[], "shots/fr157-switch.png"), ["id-name"]);
        assert!(
            hits(
                "a.rs",
                "fn the_prd_scaffold_frontmatter_has_the_seven_keys() {}"
            )
            .is_empty()
        );
        assert!(hits("a.rs", "let version_3_store = open(dir);").is_empty());
        assert!(path_hits(&[], "tests/fixture/log-without-covers/x.jsonl").is_empty());
    }

    #[test]
    fn id_string() {
        assert_eq!(
            hits("a.ts", "const t = 'FR-157: Support switches…';"),
            ["id-string"]
        );
        assert!(hits("a.rs", r#"let g = "checkpoint";"#).is_empty());
        assert!(hits("a.rs", r#"let f = format!("PRD-{feature}");"#).is_empty());
        assert!(hits("a.rs", r#"say("run polish §5");"#).is_empty());
    }

    #[test]
    fn history() {
        assert_eq!(
            hits("a.go", "\t// until slice 4"),
            ["id-comment", "history"]
        );
        assert_eq!(hits("a.py", "# kept for now"), ["history"]);
        assert!(hits("a.rs", r#"bail!("the lane is no longer running");"#).is_empty());
    }

    #[test]
    fn line_cite() {
        assert_eq!(hits("a.rs", "// docs.mjs:1930"), ["line-cite"]);
        assert_eq!(
            hits("a.rs", r#"let s = "measure.py:42-67";"#),
            ["line-cite"]
        );
        assert!(hits("a.rs", "// docs.mjs parses the board").is_empty());
    }

    #[test]
    fn section_cite() {
        assert_eq!(
            hits("a.rs", r#"let s = "see autonomy.md § Caps";"#),
            ["section-cite"]
        );
        assert_eq!(
            hits("a.rs", "// as /deliver §3 runs it"),
            ["id-comment", "section-cite"]
        );
        assert!(hits("a.rs", r#"let s = "SKILL.md";"#).is_empty());
        assert!(hits("a.rs", r#"say("run polish §5");"#).is_empty());
    }

    #[test]
    fn step_marker() {
        assert_eq!(hits("a.rs", "// ── task 3 (a):"), ["step-marker"]);
        assert_eq!(hits("a.rs", "/// Task 2: wire it"), ["step-marker"]);
        assert!(hits("a.rs", "// ── polish ──").is_empty());
    }

    #[test]
    fn finding_id() {
        assert_eq!(hits("a.rs", "// C3: the race"), ["finding-id"]);
        assert_eq!(hits("a.rs", "// see L11 and Ln6"), ["finding-id"]);
        assert!(hits("a.rs", "// L1 holds the hot set").is_empty());
        assert!(hits("a.rs", "// c3 in lowercase is a name").is_empty());
    }

    #[test]
    fn plan_words() {
        assert_eq!(hits("a.rs", "// the spec says so"), ["plan-words"]);
        assert!(hits("a.rs", "// the parser says so").is_empty());
    }

    #[test]
    fn markdown_and_docs_are_never_read() {
        assert!(hits("README.md", "FR-3: guard, until slice 4").is_empty());
        assert!(hits("docs/a.rs", "// FR-3: guard").is_empty());
        assert!(path_hits(&[], "docs/features/006-x/slice-01-a.md").is_empty());
        assert!(hits("Makefile", "# FR-3").is_empty());
    }

    #[test]
    fn the_split_keeps_markers_inside_strings_and_chars() {
        let rs = syntax("a.rs").unwrap();
        let p = split(r#"if c == '"' { s.push_str("// no"); } // FR-3"#, rs);
        assert_eq!(p.strings, ["// no"]);
        assert_eq!(p.comment.as_deref(), Some("// FR-3"));
        let p = split(r##"let j = r#"{"a": "b"}"#; // x"##, rs);
        assert_eq!(p.strings, [r#"{"a": "b"}"#]);
        assert_eq!(p.comment.as_deref(), Some("// x"));
        let p = split("fn f<'a>(x: &'a str) -> &'a str { x }", rs);
        assert!(p.strings.is_empty() && p.comment.is_none());
        assert!(split("   * a block's middle line", rs).is_comment());
        assert!(!split("    *count += 1;", rs).is_comment());

        let php = syntax("a.php").unwrap();
        assert!(split("#[Attribute]", php).comment.is_none());
        assert!(split("# a note", php).is_comment());
        let sh = syntax("a.sh").unwrap();
        assert!(split(r#"echo "$#" ${#x}"#, sh).comment.is_none());
        let json = syntax("a.jsonl").unwrap();
        assert_eq!(split(r#"{"t": "// x"}"#, json).strings, ["t", "// x"]);
    }

    #[test]
    fn the_share_is_comment_lines_over_non_blank_lines() {
        let lines = ["// a note", "let x = 1;", "   "].map(|t| Added {
            path: "a.rs".into(),
            line: 1,
            text: t.into(),
        });
        let r = check(&[], &lines, &[]);
        let text = r.render();
        assert!(
            text.contains("comment share: 50.0% (1 of 2 added lines)"),
            "{text}"
        );
        assert!(text.ends_with("clean\n"), "{text}");
    }

    #[test]
    fn the_report_groups_hits_under_their_rule_and_fix_line() {
        let lines = [
            Added {
                path: "a.rs".into(),
                line: 7,
                text: "    // FR-3: guard".into(),
            },
            Added {
                path: "a.rs".into(),
                line: 9,
                text: "    // until slice 4".into(),
            },
        ];
        let r = check(&[], &lines, &["tests/fixture/prd-9/x.jsonl".into()]);
        assert_eq!((r.fix_hits(), r.id_hits()), (4, 3));
        let text = r.render();
        let expect = format!(
            "id-comment: a plan id in a comment\n  fix: {}\n  a.rs:7  // FR-3: guard\n  a.rs:9  // until slice 4\n\nid-name: a plan id in a name or path\n  fix: {}\n  tests/fixture/prd-9/x.jsonl\n\nhistory:",
            rule("id-comment").fix,
            rule("id-name").fix
        );
        assert!(text.starts_with(&expect), "{text}");
        assert!(text.ends_with("4 fix hits\n"), "{text}");
    }

    #[test]
    fn a_strings_entry_drops_only_the_string_check() {
        let allow = [Allow {
            path: "tests/lanes.rs".into(),
            scope: Scope::Strings,
        }];
        assert!(hits_with(&allow, "tests/lanes.rs", r#"let b = "BLOCK-1";"#).is_empty());
        assert_eq!(
            hits_with(&allow, "tests/lanes.rs", "// BLOCK-1 again"),
            ["id-comment"]
        );
        assert_eq!(
            hits_with(&allow, "tests/lanes2.rs", r#"let b = "BLOCK-1";"#),
            ["id-string"]
        );
    }

    #[test]
    fn an_all_entry_drops_every_rule_under_its_directory() {
        let allow = [Allow {
            path: "tests/fixture/v2/".into(),
            scope: Scope::All,
        }];
        let under = "tests/fixture/v2/slice-3/a.rs";
        assert!(hits_with(&allow, under, "// FR-3: guard").is_empty());
        assert!(hits_with(&allow, under, "fn prd_003() {}").is_empty());
        assert!(path_hits(&allow, under).is_empty());
        let beside = "tests/fixture/v2b/slice-3/a.rs";
        assert_eq!(hits_with(&allow, beside, "// FR-3: guard"), ["id-comment"]);
        assert_eq!(path_hits(&allow, beside), ["id-name"]);
    }

    fn project(text: &str) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("docs")).unwrap();
        fs::write(project::path(d.path()), text).unwrap();
        d
    }

    #[test]
    fn the_allowlist_reads_paths_and_scopes() {
        let d = project(
            "[docs]\nframework = 3\n\n[leaks]\nallow = [\n  { path = \"tests/lanes.rs\" },\n  { path = \"tests/fixture/v2/\", scope = \"all\" },\n]\n",
        );
        assert_eq!(
            allowlist(d.path()).unwrap(),
            [
                Allow {
                    path: "tests/lanes.rs".into(),
                    scope: Scope::Strings
                },
                Allow {
                    path: "tests/fixture/v2/".into(),
                    scope: Scope::All
                },
            ]
        );
        assert!(
            allowlist(project("[docs]\nframework = 3\n").path())
                .unwrap()
                .is_empty()
        );
        assert!(
            allowlist(tempfile::tempdir().unwrap().path())
                .unwrap()
                .is_empty()
        );

        let d = project("[leaks]\nallow = [{ path = \"x\", scope = \"names\" }]\n");
        let e = allowlist(d.path()).unwrap_err().to_string();
        assert!(e.contains("\"names\"") && e.contains("scope"), "{e}");
    }

    #[test]
    fn the_diff_parser_numbers_added_lines_from_the_hunk() {
        let diff = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -3,0 +4,2 @@ fn x\n+one\n++++ b/fake\n@@ -9 +10 @@\n-old\n+new\ndiff --git a/gone.rs b/gone.rs\n--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n";
        let got: Vec<(String, u64, String)> = parse_diff(diff)
            .into_iter()
            .map(|a| (a.path, a.line, a.text))
            .collect();
        let want = [
            ("a.rs", 4, "one"),
            ("a.rs", 5, "+++ b/fake"),
            ("a.rs", 10, "new"),
        ]
        .map(|(p, n, t)| (p.to_string(), n, t.to_string()));
        assert_eq!(got, want);
    }

    #[test]
    fn name_status_keeps_added_and_renamed_paths() {
        let z = "A\0new.rs\0M\0kept.rs\0R087\0old/x.rs\0tests/fixture/prd-9/x.rs\0D\0gone.rs\0";
        assert_eq!(parse_name_status(z), ["new.rs", "tests/fixture/prd-9/x.rs"]);
    }
}
