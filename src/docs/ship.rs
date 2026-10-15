use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::board::{self, Feature, Kind};
use super::{scaffold, state};
use crate::error::io;
use crate::{Error, Result};

/// A `state.toml` that does not parse is refused here: read past, every slice reads "not done".
fn live(root: &Path, reference: &str) -> Result<Feature> {
    let b = board::scan(root)?;
    let f = b.resolve(reference)?;
    if let Err(e) = &f.state {
        return Err(Error::Msg(e.clone()));
    }
    if f.kind == Kind::Archive {
        return Err(Error::Msg(format!(
            "PRD-{:03} is already in docs/archive/",
            f.number
        )));
    }
    Ok(f.clone())
}

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

fn git_out(root: &Path, args: &[&str]) -> Result<String> {
    // A tracked file named `*` must not widen `git rm` to the files the archive keeps.
    let o = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(io("run git"))?;
    if !o.status.success() {
        return Err(Error::Msg(format!(
            "git {}: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&o.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

fn say(out: &mut dyn Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes()).map_err(io("write stdout"))
}

/// On a shipped feature a no-op; the closeout's rerun relies on it.
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
    // The closeout forwards stdout lines that start with ⚠.
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

pub fn archive(root: &Path, reference: &str, out: &mut dyn Write) -> Result<()> {
    let f = live(root, reference)?;
    all_done(&f, "Archiving is feature-granular.")?;
    to_archive(root, &[f], out)
}

const KEPT: [&str; 4] = ["prd.md", "state.toml", "log.jsonl", "retro.md"];
const BANNER: &str =
    "> **History.** The code, `docs/reference` and `docs/decisions.md` win over this document.";

/// Every feature is checked before any moves. Staging comes last: a file edited after `git add` shows `AM`,
/// and an `M` under `docs/archive/` fails the check the hook runs.
pub fn to_archive(root: &Path, batch: &[Feature], out: &mut dyn Write) -> Result<()> {
    let base = root.join("docs/archive");
    let mut seen = std::collections::HashSet::new();
    let batch: Vec<&Feature> = batch.iter().filter(|f| seen.insert(f.number)).collect();
    for f in &batch {
        let dir = rel(root, &f.dir);
        // A cloned repo can point one of these outside itself; the banner and the `~/` rewrite would follow it.
        for p in std::iter::once(f.dir.clone()).chain(KEPT.iter().map(|k| f.dir.join(k))) {
            if fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(Error::Msg(format!(
                    "refusing: {} is a symlink",
                    rel(root, &p)
                )));
            }
        }
        let status = git_out(root, &["status", "--porcelain", "--ignored", "--", &dir])?;
        if status.lines().any(|l| !l.starts_with("!! ")) {
            return Err(Error::Msg(format!(
                "refusing: {dir} has uncommitted changes; commit them first"
            )));
        }
        // `git mv` carries ignored files along, and a `.pyc` holds a home path.
        let own_lock = format!("!! {dir}/{}", state::LOCK);
        if let Some(l) = status.lines().find(|l| *l != own_lock) {
            return Err(Error::Msg(format!(
                "refusing: {dir} holds ignored files the archive would keep, such as {}; remove them first (git clean -ndX -- {dir} lists them)",
                &l[3..]
            )));
        }
        let to = base.join(&f.dir_name);
        if to.exists() {
            return Err(Error::Msg(format!("{} already exists", rel(root, &to))));
        }
        for p in KEPT.iter().map(|k| f.dir.join(k)).filter(|p| p.is_file()) {
            read(&p)?;
        }
    }
    for f in batch {
        let mut text = String::new();
        let dir = rel(root, &f.dir);
        let to = base.join(&f.dir_name);
        let prd = f.dir.join("prd.md");
        if prd.is_file() {
            let raw = read(&prd)?;
            write(&prd, &with_banner(&raw))?;
        }
        for name in KEPT {
            let p = f.dir.join(name);
            if p.is_file() {
                let raw = read(&p)?;
                let tilde = crate::paths::home_paths_to_tilde(&raw);
                if tilde != raw {
                    write(&p, &tilde)?;
                }
            }
        }
        let tracked = git_out(root, &["ls-files", "-z", "--", &dir])?;
        let dropped: Vec<&str> = tracked
            .split('\0')
            .filter(|p| !p.is_empty())
            .filter(|p| !KEPT.iter().any(|k| *p == format!("{dir}/{k}")))
            .collect();
        if !dropped.is_empty() {
            let mut args = vec!["rm", "-q", "--"];
            args.extend(&dropped);
            git_out(root, &args)?;
        }
        fs::create_dir_all(&base).map_err(io(format!("create {}", base.display())))?;
        git_out(root, &["mv", &dir, &rel(root, &to)])?;
        state::set(&to, &[("status", "archived")])?;
        let kept: Vec<String> = KEPT
            .iter()
            .map(|k| to.join(k))
            .filter(|p| p.is_file())
            .map(|p| rel(root, &p))
            .collect();
        let mut args = vec!["add", "--"];
        args.extend(kept.iter().map(String::as_str));
        git_out(root, &args)?;
        text.push_str(&format!(
            "PRD-{:03} → {}, archived, staged.\n",
            f.number,
            rel(root, &to)
        ));
        if dropped.is_empty() {
            text.push_str("  dropped nothing\n");
        }
        for p in dropped {
            text.push_str(&format!("  dropped {p}\n"));
        }
        say(out, &text)?;
    }
    Ok(())
}

/// The banner goes right after the line where the frontmatter parser closes the block, or on top without one; never twice.
fn with_banner(prd: &str) -> String {
    if prd.contains(BANNER) {
        return prd.to_string();
    }
    let at = prd
        .starts_with("---\n")
        .then(|| prd[3..].find("\n---"))
        .flatten()
        .map(|i| {
            let close = i + 4;
            prd[close..].find('\n').map_or(prd.len(), |j| close + j + 1)
        })
        .unwrap_or(0);
    let head = &prd[..at];
    let sep = if head.is_empty() || head.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    format!(
        "{head}{sep}{BANNER}\n\n{}",
        prd[at..].trim_start_matches('\n')
    )
}

fn read(p: &Path) -> Result<String> {
    fs::read_to_string(p).map_err(io(format!("read {}", p.display())))
}

fn write(p: &Path, text: &str) -> Result<()> {
    fs::write(p, text).map_err(io(format!("write {}", p.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_lands_after_the_frontmatter_once() {
        let prd = "---\nnumber: 001\n---\n\n# Title\n";
        let once = with_banner(prd);
        assert_eq!(
            once,
            format!("---\nnumber: 001\n---\n{BANNER}\n\n# Title\n")
        );
        assert_eq!(with_banner(&once), once);
        assert_eq!(with_banner("# Title\n"), format!("{BANNER}\n\n# Title\n"));
    }

    #[test]
    fn the_banner_follows_an_empty_or_loosely_closed_frontmatter() {
        assert_eq!(
            with_banner("---\n---\n# Title\n"),
            format!("---\n---\n{BANNER}\n\n# Title\n")
        );
        assert_eq!(
            with_banner("---\nnumber: 001\n--- \n# Title\n\n---\nmore\n"),
            format!("---\nnumber: 001\n--- \n{BANNER}\n\n# Title\n\n---\nmore\n")
        );
    }
}
