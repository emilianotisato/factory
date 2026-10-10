//! The compact printer (PRD §14 FR-12): `path:line  kind  signature`, then
//! the context indented four spaces (the body for `def`, the line after the
//! hit for `refs`), hits separated by a blank line, and a trailer on the
//! last line saying what was cut. Plain text, no colour, no width cap: a
//! model reads it, not a terminal.
use std::fmt::Write;

use super::Searched;
use super::def::{self, BODY_LINES};
use super::refs;

/// `def`'s output; `body` is false under `--no-body`.
pub fn defs(name: &str, found: &def::Found, body: bool) -> String {
    let mut out = String::new();
    for d in &found.hits {
        block(&mut out, &d.path, d.line, d.kind, &d.signature);
        if body {
            for l in &d.body {
                indented(&mut out, l);
            }
        }
    }
    let n = found.hits.len();
    let mut trailer = if n == 0 {
        format!("no definition of {name} found")
    } else {
        plural(n, "definition", "definitions")
    };
    let _ = write!(
        trailer,
        " (searched {})",
        plural(found.searched.files, "file", "files")
    );
    let cut = found.hits.iter().filter(|d| d.cut).count();
    if body && cut > 0 {
        let what = if cut == 1 { "body" } else { "bodies" };
        let _ = write!(trailer, "; {cut} {what} cut at {BODY_LINES} lines");
    }
    skipped(&mut trailer, &found.searched);
    finish(out, &trailer)
}

/// `refs`' output.
pub fn refs(name: &str, found: &refs::Found) -> String {
    let mut out = String::new();
    for r in &found.shown {
        block(&mut out, &r.path, r.line, "ref", &r.text);
        if let Some(next) = &r.next {
            indented(&mut out, next);
        }
    }
    let mut trailer = if found.total == 0 {
        format!(
            "no references to {name} found (searched {})",
            plural(found.searched.files, "file", "files")
        )
    } else if found.more_hits == 0 {
        format!(
            "{} in {} (searched {})",
            plural(found.total, "hit", "hits"),
            plural(found.files, "file", "files"),
            plural(found.searched.files, "file", "files")
        )
    } else {
        format!(
            "{} more {} in {} (--max, --per-file to widen)",
            found.more_hits,
            if found.more_hits == 1 { "hit" } else { "hits" },
            plural(found.more_files, "file", "files")
        )
    };
    if found.test_hits > 0 {
        let _ = write!(trailer, "; {} in test files (--tests)", found.test_hits);
    }
    skipped(&mut trailer, &found.searched);
    finish(out, &trailer)
}

fn block(out: &mut String, path: &str, line: u64, kind: &str, text: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    let _ = writeln!(out, "{path}:{line}  {kind}  {text}");
}

fn indented(out: &mut String, line: &str) {
    if line.is_empty() {
        out.push('\n');
    } else {
        let _ = writeln!(out, "    {line}");
    }
}

fn skipped(trailer: &mut String, s: &Searched) {
    let mut parts = Vec::new();
    if s.binary > 0 {
        parts.push(plural(s.binary, "binary file", "binary files"));
    }
    if s.too_large > 0 {
        parts.push(format!(
            "{} over 2 MB",
            plural(s.too_large, "file", "files")
        ));
    }
    if !parts.is_empty() {
        let _ = write!(trailer, "; skipped {}", parts.join(", "));
    }
}

fn finish(mut out: String, trailer: &str) -> String {
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(trailer);
    out.push('\n');
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
