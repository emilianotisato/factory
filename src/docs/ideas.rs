//! Ideas, one markdown file per open idea under `docs/ideas/` (PRD-002
//! FR-20). A file there means the idea is open; leaving is deleting, so there
//! is no state beyond `open | owed`. `add` takes no lock: every idea is its own
//! file, created with `O_EXCL`, so two worktrees never write the same one.
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use super::frontmatter::{self, Value};
use super::state::is_date;
use crate::error::io;
use crate::{Error, Result};

pub const DIR: &str = "docs/ideas";
/// The folder's rules, written by `init`; never an idea.
pub const README: &str = "README.md";
pub const STATES: [&str; 2] = ["open", "owed"];
const SLUG_MAX: usize = 50;

/// One idea's fields and body, as written and as read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Idea {
    pub title: String,
    pub state: String,
    pub added: String,
    pub origin: Option<String>,
    pub retrigger: Option<String>,
    pub priority: Option<String>,
    pub body: String,
}

/// Why a file in the folder is not an idea; `check` reports each as its code.
#[derive(Debug, Clone, PartialEq)]
pub enum Bad {
    Name(String),
    Parse(String),
    Field(String),
}

impl Bad {
    pub fn reason(&self) -> &str {
        match self {
            Bad::Name(m) | Bad::Parse(m) | Bad::Field(m) => m,
        }
    }
}

/// `idea list --json`: the per-repo contract the cross-repo scanner reads.
/// Absent optionals serialize as `null`, so every object has every key.
#[derive(Debug, Clone, Serialize)]
pub struct IdeaRow {
    pub id: String,
    pub path: String,
    pub title: String,
    pub state: String,
    pub added: String,
    pub origin: Option<String>,
    pub retrigger: Option<String>,
    pub priority: Option<String>,
    pub first_sentence: String,
    pub body_chars: usize,
}

fn refuse<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Msg(msg.into()))
}

/// The title lowercased, every run of other characters one `-`, cut to 50.
/// ASCII only, so the stem always matches the name `check` accepts.
pub fn slug(title: &str) -> Result<String> {
    let mut s = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.is_empty() && !s.ends_with('-') {
            s.push('-');
        }
    }
    s.truncate(SLUG_MAX);
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        return refuse(format!(
            "title \"{title}\" has no letters or digits to make a slug from"
        ));
    }
    Ok(s)
}

/// `^\d{4}-\d{2}-\d{2}-[a-z0-9]+(-[a-z0-9]+)*$`; a `-2` clash suffix is one
/// more segment.
pub fn id_ok(stem: &str) -> bool {
    let b = stem.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    b.len() > 11
        && digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && b[10] == b'-'
        && stem[11..].split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// One `key: "value"` line the frontmatter parser reads back unchanged:
/// single quotes when the value holds `"`, and a refusal when it holds both
/// quote kinds or a newline, since the parser has no escapes.
fn line(key: &str, value: &str) -> Result<String> {
    if value.contains(['\n', '\r']) {
        return refuse(format!("--{key} holds a newline; it must fit on one line"));
    }
    let q = match (value.contains('"'), value.contains('\'')) {
        (true, true) => {
            return refuse(format!(
                "--{key} holds both \" and ', which one frontmatter line cannot quote"
            ));
        }
        (true, false) => '\'',
        _ => '"',
    };
    Ok(format!("{key}: {q}{value}{q}\n"))
}

fn state_ok(state: &str) -> std::result::Result<(), String> {
    if STATES.contains(&state) {
        return Ok(());
    }
    Err(format!(
        "state \"{state}\" is not one of {}",
        STATES.join(" | ")
    ))
}

fn priority_ok(priority: Option<&str>) -> std::result::Result<(), String> {
    match priority {
        Some(p) if p != "high" => Err(format!("priority \"{p}\" is not high (absent is normal)")),
        _ => Ok(()),
    }
}

/// The file text: the frontmatter in FR-20's key order with absent keys
/// omitted, a blank line, the body ending in one newline. Slice 08 writes
/// migrated ideas through this too.
pub fn render(idea: &Idea) -> Result<String> {
    state_ok(&idea.state)
        .and_then(|()| priority_ok(idea.priority.as_deref()))
        .map_err(Error::Msg)?;
    if !is_date(&idea.added) {
        return refuse(format!(
            "added \"{}\" is not a date (YYYY-MM-DD)",
            idea.added
        ));
    }
    let mut s = String::from("---\n");
    s.push_str(&line("title", &idea.title)?);
    s.push_str(&line("state", &idea.state)?);
    s.push_str(&line("added", &idea.added)?);
    for (k, v) in [
        ("origin", &idea.origin),
        ("retrigger", &idea.retrigger),
        ("priority", &idea.priority),
    ] {
        if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
            s.push_str(&line(k, v)?);
        }
    }
    s.push_str("---\n");
    let body = idea.body.trim_end_matches('\n');
    if !body.is_empty() {
        s.push('\n');
        s.push_str(body);
        s.push('\n');
    }
    Ok(s)
}

/// Write `idea` as `docs/ideas/<added>-<slug>.md`, or `-2`, `-3`, … when the
/// name is taken, and return the id. Every refusal comes before a write.
pub fn add(root: &Path, idea: &Idea) -> Result<String> {
    let slug = slug(&idea.title)?;
    let text = render(idea)?;
    let dir = root.join(DIR);
    fs::create_dir_all(&dir).map_err(io(format!("mkdir {}", dir.display())))?;
    for n in 1.. {
        let id = match n {
            1 => format!("{}-{slug}", idea.added),
            _ => format!("{}-{slug}-{n}", idea.added),
        };
        let path = dir.join(format!("{id}.md"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(text.as_bytes())
                    .map_err(io(format!("write {}", path.display())))?;
                return Ok(id);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io(format!("create {}", path.display()))(e)),
        }
    }
    unreachable!("the suffix loop only ends by returning")
}

/// The files in `docs/ideas/` that should be ideas, sorted: all but the
/// README and dotfiles (a `.gitkeep`). `(repo-relative path, path)`.
pub fn files(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = fs::read_dir(root.join(DIR)) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name != README && !name.starts_with('.')).then(|| (format!("{DIR}/{name}"), e.path()))
        })
        .collect();
    out.sort();
    out
}

fn text_of(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::text).filter(|s| !s.trim().is_empty())
}

/// One idea file, named, parsed and checked against the vocabulary: the one
/// reader `idea list`, `new --from-idea` and `check` share, so a file one
/// skips is a file the others refuse or report.
pub fn read(path: &Path) -> std::result::Result<Idea, Bad> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = name.strip_suffix(".md");
    if !stem.is_some_and(id_ok) {
        return Err(Bad::Name(format!("\"{name}\" is not YYYY-MM-DD-slug.md")));
    }
    let text = fs::read_to_string(path).map_err(|e| Bad::Parse(e.to_string()))?;
    let fm = frontmatter::parse(&text);
    if fm.data.is_none() {
        return Err(Bad::Parse(
            fm.error.unwrap_or_else(|| "no frontmatter".into()),
        ));
    }
    let mut why = Vec::new();
    let mut required = |k: &str| {
        let v = text_of(fm.get(k));
        if v.is_none() {
            why.push(format!("missing {k}"));
        }
        v.unwrap_or_default()
    };
    let title = required("title");
    let state = required("state");
    let added = required("added");
    let priority = text_of(fm.get("priority"));
    if !state.is_empty()
        && let Err(e) = state_ok(&state)
    {
        why.push(e);
    }
    if let Err(e) = priority_ok(priority.as_deref()) {
        why.push(e);
    }
    if !added.is_empty() && !is_date(&added) {
        why.push(format!("added \"{added}\" is not a date (YYYY-MM-DD)"));
    }
    if !why.is_empty() {
        return Err(Bad::Field(why.join("; ")));
    }
    Ok(Idea {
        title,
        state,
        added,
        origin: text_of(fm.get("origin")),
        retrigger: text_of(fm.get("retrigger")),
        priority,
        body: fm.body.trim_start_matches('\n').to_string(),
    })
}

/// The body's first non-empty line, up to and including the first `.`, `?`
/// or `!` followed by a space; the whole line when there is none.
pub fn first_sentence(body: &str) -> String {
    let Some(line) = body.lines().map(str::trim).find(|l| !l.is_empty()) else {
        return String::new();
    };
    match [". ", "? ", "! "].iter().filter_map(|p| line.find(p)).min() {
        Some(i) => line[..=i].to_string(),
        None => line.to_string(),
    }
}

/// The open ideas sorted by id, and the files skipped as `(path, reason)`.
pub fn list(root: &Path) -> (Vec<IdeaRow>, Vec<(String, String)>) {
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for (rel, path) in files(root) {
        match read(&path) {
            Ok(i) => rows.push(IdeaRow {
                id: rel[DIR.len() + 1..rel.len() - 3].to_string(),
                path: rel,
                first_sentence: first_sentence(&i.body),
                body_chars: i.body.chars().count(),
                title: i.title,
                state: i.state,
                added: i.added,
                origin: i.origin,
                retrigger: i.retrigger,
                priority: i.priority,
            }),
            Err(b) => skipped.push((rel, b.reason().to_string())),
        }
    }
    (rows, skipped)
}

/// `<id>` or `<id>.md` → the id and its repo-relative path, refusing an id
/// with no file behind it.
pub fn locate(root: &Path, id: &str) -> Result<(String, String)> {
    let id = id.strip_suffix(".md").unwrap_or(id);
    let rel = format!("{DIR}/{id}.md");
    if id.is_empty() || id.contains('/') || id == "README" || !root.join(&rel).is_file() {
        return refuse(format!("no idea \"{id}\": {rel} does not exist"));
    }
    Ok((id.to_string(), rel))
}

/// An idea to build on: located and read, or refused with why.
pub fn open(root: &Path, id: &str) -> Result<(String, String, Idea)> {
    let (id, rel) = locate(root, id)?;
    match read(&root.join(&rel)) {
        Ok(i) => Ok((id, rel, i)),
        Err(b) => refuse(format!("{rel}: {} — fix it first", b.reason())),
    }
}

fn git(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(io(format!("run git {}", args.join(" "))))
}

/// Delete the idea and commit that one path with the reason as the body, so
/// it lives in `git log -- docs/ideas/`. Refuses a file git does not hold as
/// committed: an edit would be lost, and an untracked file has no history to
/// put the reason in.
pub fn drop(root: &Path, id: &str, because: &str) -> Result<String> {
    let (id, rel) = locate(root, id)?;
    if because.trim().is_empty() {
        return refuse("--because is empty: the reason is what the commit keeps");
    }
    let st = git(root, &["status", "--porcelain", "--", &rel])?;
    if !st.status.success() {
        return refuse(format!(
            "git status failed: {}",
            String::from_utf8_lossy(&st.stderr).trim()
        ));
    }
    let st = String::from_utf8_lossy(&st.stdout);
    if st.starts_with("??") {
        return refuse(format!(
            "{rel} is not committed yet: delete it by hand, there is no history to keep the reason in"
        ));
    }
    if !st.trim().is_empty() {
        return refuse(format!(
            "{rel} has uncommitted edits: commit or revert them first"
        ));
    }
    let path = root.join(&rel);
    fs::remove_file(&path).map_err(io(format!("delete {}", path.display())))?;
    let msg = format!("docs: drop idea {id}");
    let o = git(
        root,
        &["commit", "-q", "-m", &msg, "-m", because, "--", &rel],
    )?;
    if !o.status.success() {
        // Put the file back: a refused commit (a hook) must not leave the
        // idea deleted with its reason nowhere.
        let _ = git(root, &["checkout", "--", &rel]);
        return refuse(format!(
            "git commit failed, {rel} restored: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_ascii_kebab_and_at_most_fifty() {
        assert_eq!(slug("  Hello, World!  ").unwrap(), "hello-world");
        assert_eq!(slug("Café au lait").unwrap(), "caf-au-lait");
        let long = slug(&"word ".repeat(14)).unwrap();
        assert!(long.len() <= 50 && !long.ends_with('-'), "{long}");
        assert!(slug("!!!").is_err());
    }

    #[test]
    fn ids_match_the_check_pattern() {
        assert!(id_ok("2026-10-13-same"));
        assert!(id_ok("2026-10-13-same-2"));
        assert!(!id_ok("2026-10-13-"));
        assert!(!id_ok("2026-10-13-Same"));
        assert!(!id_ok("2026-10-13-a--b"));
        assert!(!id_ok("notes"));
    }

    #[test]
    fn the_first_sentence_ends_at_its_mark() {
        assert_eq!(first_sentence("\nOne. Two.\n\nThree."), "One.");
        assert_eq!(first_sentence("Why? Because."), "Why?");
        assert_eq!(first_sentence("v1.2 is out"), "v1.2 is out");
        assert_eq!(first_sentence(""), "");
    }

    #[test]
    fn a_rendered_idea_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let idea = Idea {
            title: "Say \"hi\"".into(),
            state: "owed".into(),
            added: "2026-10-13".into(),
            origin: Some("it's the owner".into()),
            retrigger: None,
            priority: Some("high".into()),
            body: "Body. More.\n\nTwo.\n\n\n".into(),
        };
        let p = dir.path().join("2026-10-13-say-hi.md");
        fs::write(&p, render(&idea).unwrap()).unwrap();
        let back = read(&p).unwrap();
        assert_eq!(
            back,
            Idea {
                body: "Body. More.\n\nTwo.\n".into(),
                ..idea
            }
        );
        let both = Idea {
            title: "a\"b'c".into(),
            ..back.clone()
        };
        assert!(render(&both).is_err());
        let nl = Idea {
            origin: Some("a\nb".into()),
            ..back
        };
        assert!(render(&nl).is_err());
    }
}
