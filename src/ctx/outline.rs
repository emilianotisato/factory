//! `factory ctx outline` (PRD §14 FR-12): the definitions a file holds, from
//! its grammar's `tags.scm`, as the Agentless skeleton: one line each,
//! `line  kind  signature`, a member indented under its class, trait, `impl`
//! or module. Bodies, properties, constants, macros, imports and the
//! definitions local to a function are left out. A directory stands for the
//! source files directly in it.
use std::io;
use std::path::{Path, PathBuf};

use super::lang::{Definition, Unit, line_of, line_starts};
use super::walk::{self, Lang, MAX_FILE_BYTES};
use super::{Searched, rel};

/// The files a call names, in argument order, each directory's sorted.
#[derive(Debug, Default)]
pub struct Listing {
    pub files: Vec<(PathBuf, Lang)>,
    /// Named files of no outlined language.
    pub unknown: usize,
    pub too_large: usize,
}

/// One outline line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 1 at the top level.
    pub depth: usize,
    pub line: u64,
    pub kind: &'static str,
    /// The name's line, trimmed.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: String,
    pub entries: Vec<Entry>,
    /// The tree holds a syntax error: what was tagged is printed.
    pub broken: bool,
}

#[derive(Debug, Default)]
pub struct Found {
    pub files: Vec<File>,
    /// Definitions deeper than `--depth`.
    pub hidden: usize,
    pub unknown: usize,
    pub searched: Searched,
}

/// Expands `paths` under `base`; the first one that cannot be read is the
/// error, with the path as given.
pub fn list(base: &Path, paths: &[PathBuf]) -> Result<Listing, (PathBuf, io::Error)> {
    let mut listing = Listing::default();
    for p in paths {
        let full = base.join(p);
        let fail = |e| (p.clone(), e);
        if std::fs::metadata(&full).map_err(fail)?.is_dir() {
            let mut inside = Vec::new();
            for entry in std::fs::read_dir(&full).map_err(fail)? {
                let path = entry.map_err(fail)?.path();
                let hidden = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'));
                if let Some(lang) = Lang::from_path(&path).filter(|_| !hidden && path.is_file()) {
                    inside.push((path, lang));
                }
            }
            inside.sort_by(|a, b| a.0.cmp(&b.0));
            for f in inside {
                listing.add(f);
            }
        } else {
            std::fs::File::open(&full).map_err(fail)?;
            match Lang::from_path(&full) {
                Some(lang) => listing.add((full, lang)),
                None => listing.unknown += 1,
            }
        }
    }
    Ok(listing)
}

impl Listing {
    fn add(&mut self, file: (PathBuf, Lang)) {
        if std::fs::metadata(&file.0).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
            self.too_large += 1;
        } else {
            self.files.push(file);
        }
    }
}

/// The outline of every listed file, `depth` levels deep, paths relative
/// to `base`.
pub fn find(base: &Path, listing: &Listing, depth: usize) -> Found {
    let (per_file, binary) = walk::scan(&listing.files, |path, lang, bytes| {
        let text = String::from_utf8_lossy(bytes);
        Some(outline(rel(base, path), path, lang, &text, depth))
    });
    let mut found = Found {
        unknown: listing.unknown,
        searched: Searched {
            files: listing.files.len() - binary,
            too_large: listing.too_large,
            binary,
        },
        ..Found::default()
    };
    for (_, (file, hidden)) in per_file {
        found.files.push(file);
        found.hidden += hidden;
    }
    found
}

/// A definition still open while the ones after it are read.
struct Open {
    end: usize,
    depth: usize,
    /// What lies inside is not outlined: a function's locals, a field's.
    buries: bool,
}

/// One file's outline and how many definitions `max` hid.
fn outline(path: String, file: &Path, lang: Lang, text: &str, max: usize) -> (File, usize) {
    let mut out = File {
        path,
        entries: Vec::new(),
        broken: false,
    };
    let Some(unit) = Unit::of(file, lang, text) else {
        return (out, 0);
    };
    let Some((defs, broken)) = unit.definitions() else {
        out.broken = true;
        return (out, 0);
    };
    out.broken = broken;
    let starts = line_starts(text);
    let mut open: Vec<Open> = Vec::new();
    let mut hidden = 0;
    for d in &defs {
        while open.last().is_some_and(|o| d.range.start >= o.end) {
            open.pop();
        }
        let local = open.last().is_some_and(|o| o.buries);
        let depth = open.last().map_or(1, |o| o.depth + 1);
        let shown = outlined(text, d);
        open.push(Open {
            end: d.range.end,
            depth,
            buries: local || shown != Some(true),
        });
        if local || shown.is_none() {
            continue;
        }
        if depth > max {
            hidden += 1;
            continue;
        }
        out.entries.push(Entry {
            depth,
            line: line_of(&starts, d.name.start) as u64 + 1,
            kind: d.kind,
            text: text.get(d.line.clone()).unwrap_or_default().to_string(),
        });
    }
    (out, hidden)
}

/// Whether a definition is outlined (`Some`) and holds members (`true`).
/// A module counts only with a body: PHP's `namespace X;` is a header.
fn outlined(text: &str, d: &Definition) -> Option<bool> {
    match d.kind {
        "class" | "impl" => Some(true),
        "module" => text
            .get(d.range.clone())
            .is_some_and(|t| t.contains('{'))
            .then_some(true),
        "fn" | "method" | "type" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(path: &str, lang: Lang, src: &str, max: usize) -> Vec<String> {
        let (file, _) = outline(path.into(), Path::new(path), lang, src, max);
        file.entries
            .iter()
            .map(|e| format!("{}{} {}", "  ".repeat(e.depth - 1), e.line, e.kind))
            .collect()
    }

    #[test]
    fn members_nest_and_a_function_keeps_its_locals() {
        let ts = "export class A {\n  b() {\n    function local() {}\n  }\n}\nfunction c() {\n  const d = () => 1\n}\ntype E = string\n";
        assert_eq!(
            lines("a.ts", Lang::Ts, ts, 2),
            ["1 class", "  2 method", "6 fn", "9 type"]
        );
        assert_eq!(
            lines("a.ts", Lang::Ts, ts, 1),
            ["1 class", "6 fn", "9 type"]
        );
        let rs = "mod m {\n    pub fn f() {}\n    const C: u8 = 1;\n}\nmod n;\nmacro_rules! k { () => {} }\n";
        assert_eq!(lines("a.rs", Lang::Rust, rs, 2), ["1 module", "  2 fn"]);
        let php = "<?php\nnamespace App;\n\nenum Suit { case A; }\n";
        assert_eq!(lines("a.php", Lang::Php, php, 2), ["4 class"]);
    }
}
