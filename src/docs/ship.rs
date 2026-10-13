//! `factory docs ship NNN` and `archive NNN` (PRD-002 FR-15), ported from
//! `docs.mjs:1789-1865`. Both write `state.toml` only: v3 has no index row
//! to move and nothing to render.
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::board::{self, Feature, Kind};
use super::{scaffold, state};
use crate::error::io;
use crate::{Error, Result};

/// The live feature `reference` names; an archived one is refused.
fn live(root: &Path, reference: &str) -> Result<Feature> {
    let b = board::scan(root)?;
    let f = b.resolve(reference)?;
    if f.kind == Kind::Archive {
        return Err(Error::Msg(format!(
            "PRD-{:03} is already in docs/archive/",
            f.number
        )));
    }
    Ok(f.clone())
}

/// `refusing: …` while any slice is not `done` (`docs.mjs:1797-1800`).
fn all_done(f: &Feature, why: &str) -> Result<()> {
    let open: Vec<String> = f
        .board_slices()
        .iter()
        .filter(|s| s.status.as_deref() != Some("done"))
        .map(|s| format!("{:02}", s.number))
        .collect();
    if open.is_empty() {
        return Ok(());
    }
    Err(Error::Msg(format!(
        "refusing: {} slice(s) not done ({}). {why}",
        open.len(),
        open.join(", ")
    )))
}

/// `git <args>` in `root`; false outside git or on failure.
fn git(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

fn say(out: &mut dyn Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes()).map_err(io("write stdout"))
}

/// `ship NNN`: `status = "shipped"` and `shipped = <today>`, staged. A
/// shipped feature is a no-op the closeout relies on (`closeout.rs:115`).
pub fn ship(root: &Path, reference: &str, out: &mut dyn Write) -> Result<()> {
    let f = live(root, reference)?;
    let nnn = format!("{:03}", f.number);
    if f.status().as_deref() == Some("shipped") {
        return say(out, &format!("PRD-{nnn} is already shipped\n"));
    }
    all_done(
        &f,
        "Set them done first, or this claims work that did not land.",
    )?;
    // The closeout forwards stdout lines that start with ⚠ (closeout.rs:587).
    if f.state().and_then(|s| s.reviewed.as_deref()).is_none() {
        say(
            out,
            &format!(
                "⚠ PRD-{nnn} has no `reviewed` date — it has not been through /polish.\n  Shipping anyway; run /polish {nnn} before the PR if that was not deliberate.\n"
            ),
        )?;
    }
    state::set(
        &f.dir,
        &[("status", "shipped"), ("shipped", &scaffold::today())],
    )?;
    git(root, &["add", "--", &rel(root, &state::path(&f.dir))]);
    say(out, &format!("PRD-{nnn} → shipped, staged.\n"))
}

/// `archive NNN`: the folder moves to `docs/archive/`, its `state.toml`
/// with it, set `archived` and staged.
pub fn archive(root: &Path, reference: &str, out: &mut dyn Write) -> Result<()> {
    let f = live(root, reference)?;
    let nnn = format!("{:03}", f.number);
    all_done(&f, "Archiving is feature-granular.")?;
    let base = root.join("docs/archive");
    let to = base.join(&f.dir_name);
    if to.exists() {
        return Err(Error::Msg(format!("{} already exists", rel(root, &to))));
    }
    fs::create_dir_all(&base).map_err(io(format!("create {}", base.display())))?;
    if !git(root, &["mv", &rel(root, &f.dir), &rel(root, &to)]) {
        fs::rename(&f.dir, &to).map_err(io(format!("move {}", f.dir.display())))?;
    }
    state::set(&to, &[("status", "archived")])?;
    git(root, &["add", "--", &rel(root, &state::path(&to))]);
    say(
        out,
        &format!("PRD-{nnn} → {}, archived, staged.\n", rel(root, &to)),
    )
}
