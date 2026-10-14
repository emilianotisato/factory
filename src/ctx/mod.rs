//! Every call re-reads and re-parses: no daemon, no index, no cache.
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::SearcherBuilder;
use grep_searcher::sinks::Lossy;

use crate::error::io;
use crate::{Error, Result};

pub mod def;
pub mod lang;
pub mod outline;
pub mod print;
pub mod refs;
pub mod vue;
pub mod walk;

#[derive(Subcommand, Debug, Clone)]
pub enum Verb {
    /// Where a symbol is defined, with its body
    Def(DefArgs),
    /// Where a symbol is used, capped and collapsed per file
    Refs(RefsArgs),
    /// The definitions a file holds
    Outline(OutlineArgs),
}

#[derive(Args, Debug, Clone)]
pub struct DefArgs {
    /// The symbol: a class, function, method or type name
    pub symbol: String,
    /// Only definitions of this kind (`fn` takes methods too)
    #[arg(long, value_parser = ["class", "fn", "method", "type"])]
    pub kind: Option<String>,
    /// Location and signature only, no body
    #[arg(long)]
    pub no_body: bool,
    /// Search vendor directories and gitignored paths too
    #[arg(long)]
    pub vendor: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RefsArgs {
    /// The symbol, matched as a whole word
    pub symbol: String,
    /// Search this directory only
    #[arg(long = "in", value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// The most hits printed
    #[arg(long, default_value_t = 10)]
    pub max: usize,
    /// The most hits printed from one file
    #[arg(long, default_value_t = 3)]
    pub per_file: usize,
    /// Include test files (left out by default)
    #[arg(long)]
    pub tests: bool,
    /// Search vendor directories and gitignored paths too
    #[arg(long)]
    pub vendor: bool,
}

#[derive(Args, Debug, Clone)]
pub struct OutlineArgs {
    /// The files to outline; a directory stands for the source files
    /// directly in it
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// How many levels to print: 1 is the top level, 2 adds class members
    #[arg(long, default_value_t = 2)]
    pub depth: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Searched {
    pub files: usize,
    pub too_large: usize,
    pub binary: usize,
}

/// Exit 0 whenever the search ran, found or not; 2 on an unreadable root.
pub fn run(verb: &Verb, base: &Path, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let text = match verb {
        Verb::Def(a) => {
            if let Some(code) = unreadable(base, base, err)? {
                return Ok(code);
            }
            let opts = walk::WalkOpts { vendor: a.vendor };
            let found = def::find(base, &a.symbol, a.kind.as_deref(), &opts)?;
            print::defs(&a.symbol, &found, !a.no_body)
        }
        Verb::Refs(a) => {
            let root = a
                .dir
                .as_ref()
                .map_or_else(|| base.to_path_buf(), |d| base.join(d));
            if let Some(code) = unreadable(base, &root, err)? {
                return Ok(code);
            }
            let caps = refs::Caps {
                max: a.max,
                per_file: a.per_file,
                tests: a.tests,
            };
            let opts = walk::WalkOpts { vendor: a.vendor };
            let found = refs::find(base, &root, &a.symbol, caps, &opts)?;
            print::refs(&a.symbol, &found)
        }
        Verb::Outline(a) => match outline::list(base, &a.paths) {
            Ok(listing) => print::outline(&outline::find(base, &listing, a.depth)),
            Err((path, e)) => {
                writeln!(err, "factory ctx: cannot read {}: {e}", path.display())
                    .map_err(io("write stderr"))?;
                return Ok(2);
            }
        },
    };
    out.write_all(text.as_bytes()).map_err(io("write stdout"))?;
    Ok(0)
}

fn unreadable(base: &Path, root: &Path, err: &mut dyn Write) -> Result<Option<i32>> {
    match std::fs::read_dir(root) {
        Ok(_) => Ok(None),
        Err(e) => {
            writeln!(err, "factory ctx: cannot read {}: {e}", rel(base, root))
                .map_err(io("write stderr"))?;
            Ok(Some(2))
        }
    }
}

/// `^` anchors at each line start and no match spans a newline.
pub(crate) fn matcher(pattern: &str) -> Result<RegexMatcher> {
    RegexMatcherBuilder::new()
        .multi_line(true)
        .line_terminator(Some(b'\n'))
        .build(pattern)
        .map_err(|e| Error::Msg(format!("ctx: bad pattern `{pattern}`: {e}")))
}

/// As `rg -w` builds one: `\b` needs a word character at the name's edge, so it misses PHP's `$user`.
pub(crate) fn whole_word(escaped: &str) -> String {
    format!(r"(?:^|\W)(?:{escaped})(?:\W|$)")
}

pub(crate) fn matching_lines(m: &RegexMatcher, bytes: &[u8]) -> Vec<u64> {
    let mut lines = Vec::new();
    let mut searcher = SearcherBuilder::new().line_number(true).build();
    let _ = searcher.search_slice(
        m,
        bytes,
        Lossy(|n, _| {
            lines.push(n);
            Ok(true)
        }),
    );
    lines
}

pub(crate) fn rel(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
