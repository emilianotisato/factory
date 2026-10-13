//! `factory skills update` (PRD-004 FR-4): brings a `behind`, `differs` or
//! `missing` repo's skills to the binary's bundle, deleting what the bundle
//! dropped. Refuses `none`, a source checkout, `ahead`, `hand-edited` and an
//! unreadable manifest with exit 1, writing nothing. Commits nothing: it
//! prints the line that would.
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::error::io;
use crate::skills::{self, LINKS_DIR, MANIFEST, SKILLS_DIR, Verdict, Version};
use crate::{Error, Result, paths};

#[derive(Subcommand, Debug, Clone)]
pub enum SkillsVerb {
    /// Re-render a behind, differs or missing repo's skills
    Update,
}

pub fn skills(verb: &SkillsVerb, out: &mut dyn Write) -> Result<i32> {
    let SkillsVerb::Update = verb;
    let root = paths::repo_root(Path::new(".")).unwrap_or_else(|_| PathBuf::from("."));
    update(&root, out)
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(io("stdout"))
}

/// The verdict decides; only `behind`, `differs` and `missing` write.
pub fn update(root: &Path, out: &mut dyn Write) -> Result<i32> {
    let verdict = skills::verdict(root);
    let (version, hand_edited, missing) = match &verdict {
        Verdict::None => {
            say(out, "no skills manifest → factory init")?;
            return Ok(1);
        }
        Verdict::SourceCheckout => {
            say(
                out,
                "source checkout: .claude/skills reads skills/ live, nothing to update",
            )?;
            return Ok(1);
        }
        Verdict::Unreadable(_) => {
            for l in verdict.lines() {
                say(out, &l)?;
            }
            return Ok(1);
        }
        Verdict::Checked {
            version,
            hand_edited,
            missing,
        } => (version, hand_edited, missing),
    };
    if let Version::Ahead { .. } = version {
        say(out, &verdict.lines()[0])?;
        return Ok(1);
    }
    if !hand_edited.is_empty() {
        say(out, &format!("hand-edited: {}", hand_edited.join(", ")))?;
        say(
            out,
            "restore each with git, then run factory skills update:",
        )?;
        for rel in hand_edited {
            say(out, &format!("  git checkout -- {SKILLS_DIR}/{rel}"))?;
        }
        return Ok(1);
    }
    if matches!(version, Version::Current(_)) && missing.is_empty() {
        say(out, "already current")?;
        return Ok(0);
    }

    // `render` rewrites the manifest: the old one is read first, or the files
    // the new bundle dropped are invisible.
    let old = skills::read_manifest(root)
        .map_err(|e| Error::Msg(format!("unreadable {MANIFEST}: {e}")))?
        .ok_or_else(|| Error::Msg(format!("{MANIFEST} vanished")))?;
    let rendered = skills::render(root)?;
    let new = skills::read_manifest(root)
        .map_err(|e| Error::Msg(format!("unreadable {MANIFEST}: {e}")))?
        .ok_or_else(|| Error::Msg(format!("{MANIFEST} vanished")))?;
    let removed = skills::remove_dropped(root, &old, &new)?;

    let v = env!("CARGO_PKG_VERSION");
    for l in verdict.lines() {
        say(out, &format!("was: {l}"))?;
    }
    say(
        out,
        &format!(
            "skills {v}: {SKILLS_DIR}/ ({} files, factory.toml), {LINKS_DIR}/ ({} links)",
            rendered.files.len(),
            rendered.links.len()
        ),
    )?;
    for r in &removed {
        say(out, &format!("removed {r}"))?;
    }
    // The dirs, not the files: `git add` of a dir stages the deletions too.
    say(
        out,
        &format!("commit: git add {SKILLS_DIR} {LINKS_DIR} && git commit -m \"chore: skills {v}\""),
    )?;
    Ok(0)
}
