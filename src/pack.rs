//! The context pack a worker reads as its appended system prompt (PRD §4
//! FR-2): the slice file, the PRD sections its `**Source**` line cites, the
//! slice contract, the autonomy rules and the inner-loop command. Nothing
//! else. A checkpoint's workers (§6 FR-4) get the same layout with the slice
//! replaced: the fix worker reads the gate failure, the checkpoint worker
//! the green transcript, the covered slices and their QA rows. All prose
//! assets are this repo's own copies under `assets/`, so a pack never
//! depends on what is installed under the owner's home (§19).
//!
//! Slice 13 (§14 FR-12 decision 3) adds `# Outline` and `# Lookup` to a
//! slice's pack, after the PRD sections: the `factory ctx outline` of every
//! file the slice's `Current code / environment` cites and the `def` of
//! every symbol it cites, run in-process, and the three verbs with their
//! one rule.
use std::fs;
use std::path::{Path, PathBuf};

use crate::ctx::{def, outline, print, walk};
use crate::docs::{Docs, Status};
use crate::error::io;
use crate::{Error, Result};

pub const CONTRACT: &str = include_str!("../assets/contract-slice.md");
pub const CONTRACT_FIX: &str = include_str!("../assets/contract-fix.md");
pub const CONTRACT_CHECKPOINT: &str = include_str!("../assets/contract-checkpoint.md");
pub const CONTRACT_POLISH: &str = include_str!("../assets/contract-polish.md");
pub const CONTRACT_CI_FIX: &str = include_str!("../assets/contract-ci-fix.md");
/// The PR body shape `/deliver` fills (`deliver/pr-body.md`), this repo's copy.
pub const PR_BODY: &str = include_str!("../assets/pr-body.md");
pub const AUTONOMY: &str = include_str!("../assets/autonomy.md");
/// The `# Lookup` section: the three `ctx` verbs, an example each, the rule.
pub const LOOKUP: &str = include_str!("../assets/lookup.md");

/// [`LOOKUP`] with each `factory ctx` command spelled the way a worker's
/// shell can run it: the running binary's absolute path, so the verbs work
/// from a build that is not installed (`paths::factory_command`).
pub fn lookup() -> String {
    LOOKUP.trim_end().replace(
        "`factory ctx ",
        &format!("`{} ctx ", crate::paths::factory_command()),
    )
}

/// The most characters of outlines `# Outline` carries (§14 decision 3).
pub const OUTLINE_CAP: usize = 8000;
/// The most characters of `def` answers `# Outline` carries.
pub const DEF_CAP: usize = 6000;

/// The most lines of a gate transcript a pack carries.
pub const TAIL_LINES: usize = 200;

/// A written pack and what to say to the worker about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    /// `runs/<id>/packs/slice-NN-<spawn>.md`.
    pub path: PathBuf,
    /// The one-line `-p` prompt.
    pub prompt: String,
    /// The repo-relative files the pack holds, slice first — the list the
    /// header line carries for slice 13's hook.
    pub files: Vec<String>,
    /// `§n` sections the slice cites that `prd.md` does not have (§4: a
    /// warning event, not a crash).
    pub missing_sections: Vec<u32>,
}

/// The `# Block and answer` section of an answered slice's pack (PRD §5
/// FR-3): the `BLOCK-n` entry with its `**Resolution**` line, verbatim from
/// the log file it lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSection {
    pub id: String,
    /// The repo-relative file the entry was read from.
    pub source: String,
    pub text: String,
}

impl Pack {
    /// Write the pack for `slice` (its board number) into `packs_dir`.
    /// `spawn` numbers the file so a respawn never overwrites the record of
    /// the pack a dead worker read.
    pub fn for_slice(
        docs: &Docs,
        board: &Status,
        slice: u32,
        packs_dir: &Path,
        spawn: u32,
    ) -> Result<Pack> {
        Self::for_slice_with(docs, board, slice, packs_dir, spawn, None)
    }

    /// [`Pack::for_slice`] with, for an answered slice, the block and its
    /// answer as a `# Block and answer` section right after the slice.
    pub fn for_slice_with(
        docs: &Docs,
        board: &Status,
        slice: u32,
        packs_dir: &Path,
        spawn: u32,
        block: Option<&BlockSection>,
    ) -> Result<Pack> {
        let entry = board
            .slices
            .iter()
            .find(|s| s.number == slice)
            .ok_or_else(|| Error::Msg(format!("slice {slice:02} is not on the board")))?;
        let slice_rel = format!("docs/features/{}", entry.file);
        let prd_rel = format!("{}/prd.md", board.dir);
        let repo = docs.root();
        let slice_text = read(&repo.join(&slice_rel))?;
        let prd_text = read(&repo.join(&prd_rel))?;
        let inner = docs.gate_inner()?;
        let feature = format!("{:03}", board.number);
        let nn = format!("{slice:02}");

        let cited = cited_sections(&slice_text);
        let mut sections = String::new();
        let mut missing = Vec::new();
        for n in &cited {
            match cut_section(&prd_text, *n) {
                Some(text) => {
                    sections.push_str(text.trim_end());
                    sections.push_str("\n\n");
                }
                None => missing.push(*n),
            }
        }
        let mut files = vec![slice_rel.clone(), prd_rel.clone()];
        if let Some(b) = block
            && !files.contains(&b.source)
        {
            files.push(b.source.clone());
        }
        let recon = Recon::of(repo, &slice_text)?;
        for f in &recon.outlined {
            if !files.contains(f) {
                files.push(f.clone());
            }
        }
        let contract = CONTRACT
            .replace("{NNN}", &feature)
            .replace("{NN}", &nn)
            .replace("{seed}", &format!("{slice}01"));

        let mut text = format!("<!-- factory-pack files: {} -->\n\n", files.join(","));
        text.push_str(&format!("# Slice\n\n<!-- {slice_rel} -->\n\n"));
        text.push_str(slice_text.trim_end());
        if let Some(b) = block {
            text.push_str(&format!(
                "\n\n# Block and answer\n\n<!-- {}: {} -->\n\n",
                b.source, b.id
            ));
            text.push_str(b.text.trim_end());
        }
        text.push_str(&format!(
            "\n\n# PRD sections\n\n<!-- {prd_rel}: {} -->\n\n",
            if cited.is_empty() {
                "the slice cites no section".to_string()
            } else {
                cited
                    .iter()
                    .map(|n| format!("§{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
        text.push_str(sections.trim_end());
        text.push_str("\n\n# Outline\n\n");
        text.push_str(recon.text.trim_end());
        text.push_str("\n\n# Lookup\n\n");
        text.push_str(&lookup());
        text.push_str("\n\n# Contract\n\n");
        text.push_str(contract.trim_end());
        text.push_str("\n\n# Autonomy\n\n");
        text.push_str(AUTONOMY.trim_end());
        text.push_str("\n\n# Inner loop\n\n");
        text.push_str(inner.trim());
        text.push('\n');

        fs::create_dir_all(packs_dir).map_err(io(format!("mkdir {}", packs_dir.display())))?;
        let path = packs_dir.join(format!("slice-{nn}-{spawn}.md"));
        fs::write(&path, text).map_err(io(format!("write {}", path.display())))?;
        Ok(Pack {
            path,
            prompt: format!(
                "Build slice {nn} of PRD-{feature} as the appended system prompt says. Return the STATUS block."
            ),
            files,
            missing_sections: missing,
        })
    }
}

impl Pack {
    /// The fix worker's pack at checkpoint `n` (FR-4): `# Gate failure` — the
    /// last [`TAIL_LINES`] of `gate_log`, the gate command and the inner
    /// command — then the fix contract, the autonomy rules and the inner loop.
    #[allow(clippy::too_many_arguments)]
    pub fn for_fix(
        docs: &Docs,
        board: &Status,
        n: u32,
        attempt: u32,
        gate_log: &Path,
        packs_dir: &Path,
        spawn: u32,
    ) -> Result<Pack> {
        let feature = format!("{:03}", board.number);
        let gate = docs.gate()?;
        let inner = docs.gate_inner()?;
        let log_text = read(gate_log)?;
        let contract = CONTRACT_FIX
            .replace("{NNN}", &feature)
            .replace("{n}", &n.to_string())
            .replace("{seed}", &format!("9{n}01"));

        let mut text = String::from("<!-- factory-pack files: -->\n\n");
        text.push_str(&format!(
            "# Gate failure\n\n<!-- checkpoint {n}, gate attempt {attempt}: {} -->\n\n",
            gate_log.display()
        ));
        text.push_str(&format!(
            "Gate command: `{}`\nInner command: `{}`\n\n",
            gate.trim(),
            inner.trim()
        ));
        text.push_str("```\n");
        text.push_str(tail(&log_text, TAIL_LINES).trim_end());
        text.push_str("\n```");
        text.push_str("\n\n# Contract\n\n");
        text.push_str(contract.trim_end());
        text.push_str("\n\n# Autonomy\n\n");
        text.push_str(AUTONOMY.trim_end());
        text.push_str("\n\n# Inner loop\n\n");
        text.push_str(inner.trim());
        text.push('\n');

        fs::create_dir_all(packs_dir).map_err(io(format!("mkdir {}", packs_dir.display())))?;
        let path = packs_dir.join(format!("fix-{n}-{spawn}.md"));
        fs::write(&path, text).map_err(io(format!("write {}", path.display())))?;
        Ok(Pack {
            path,
            prompt: format!(
                "Fix the red gate at checkpoint {n} of PRD-{feature} as the appended system prompt says. Return the STATUS block."
            ),
            files: Vec::new(),
            missing_sections: Vec::new(),
        })
    }

    /// The checkpoint worker's pack (FR-4): `# Gate transcript` (the green
    /// log), `# Covered slices` (each covered slice's Prove task and Progress
    /// line), `# QA rows` (the PRD § QA plan rows those Prove tasks name, or
    /// every row when they name none), the checkpoint contract and the
    /// autonomy rules.
    #[allow(clippy::too_many_arguments)]
    pub fn for_checkpoint(
        docs: &Docs,
        board: &Status,
        n: u32,
        covers: &[u32],
        gate_log: &Path,
        cached: bool,
        packs_dir: &Path,
        spawn: u32,
    ) -> Result<Pack> {
        let feature = format!("{:03}", board.number);
        let prd_rel = format!("{}/prd.md", board.dir);
        let repo = docs.root();
        let prd_text = read(&repo.join(&prd_rel))?;
        let log_text = read(gate_log)?;
        let last = covers.iter().copied().max().unwrap_or(0);
        let covers_text = covers
            .iter()
            .map(|c| format!("{c:02}"))
            .collect::<Vec<_>>()
            .join(", ");
        let contract = CONTRACT_CHECKPOINT
            .replace("{NNN}", &feature)
            .replace("{n}", &n.to_string())
            .replace("{NN}", &format!("{last:02}"))
            .replace("{covers}", &covers_text)
            .replace("{feature dir}", &board.dir);

        let mut files = Vec::new();
        let mut slices = String::new();
        let mut named: Vec<String> = Vec::new();
        for c in covers {
            let Some(entry) = board.slices.iter().find(|s| s.number == *c) else {
                continue;
            };
            let rel = format!("docs/features/{}", entry.file);
            let slice_text = read(&repo.join(&rel))?;
            let prove = prove_section(&slice_text);
            for id in fr_ids(prove) {
                if !named.contains(&id) {
                    named.push(id);
                }
            }
            slices.push_str(&format!(
                "## slice-{c:02} — {}\n\n<!-- {rel} -->\n\n",
                entry.title.as_deref().unwrap_or("")
            ));
            match progress_line(&slice_text) {
                Some(p) => slices.push_str(&format!("{p}\n\n")),
                None => slices.push_str("**Progress**: (none)\n\n"),
            }
            slices.push_str(prove.trim_end());
            slices.push_str("\n\n");
            files.push(rel);
        }
        files.push(prd_rel.clone());

        let mut text = format!("<!-- factory-pack files: {} -->\n\n", files.join(","));
        text.push_str(&format!(
            "# Gate transcript\n\n<!-- checkpoint {n}, {}: {} -->\n\n",
            if cached { "cached green" } else { "green" },
            gate_log.display()
        ));
        text.push_str("```\n");
        text.push_str(tail(&log_text, TAIL_LINES).trim_end());
        text.push_str("\n```");
        text.push_str("\n\n# Covered slices\n\n");
        text.push_str(slices.trim_end());
        text.push_str(&format!(
            "\n\n# QA rows\n\n<!-- {prd_rel}: § QA plan -->\n\n"
        ));
        text.push_str(qa_rows(&prd_text, &named).trim_end());
        text.push_str("\n\n# Contract\n\n");
        text.push_str(contract.trim_end());
        text.push_str("\n\n# Autonomy\n\n");
        text.push_str(AUTONOMY.trim_end());
        text.push('\n');

        fs::create_dir_all(packs_dir).map_err(io(format!("mkdir {}", packs_dir.display())))?;
        let path = packs_dir.join(format!("checkpoint-{n}-{spawn}.md"));
        fs::write(&path, text).map_err(io(format!("write {}", path.display())))?;
        Ok(Pack {
            path,
            prompt: format!(
                "Run checkpoint {n} of PRD-{feature} as the appended system prompt says. Return the STATUS block."
            ),
            files,
            missing_sections: Vec::new(),
        })
    }

    /// The polish worker's pack (FR-6): `# Branch` (the default and feature
    /// branch names, `git log --oneline <default>..HEAD`), `# Retro` (the
    /// template and which keys the driver fills), `# PR body` (the shape and
    /// the run-dir path to write), the polish contract and the autonomy rules.
    /// The prompt is `/polish`: slash commands resolve inside `-p` (slice 03,
    /// PRD §20), so the pack carries no skill body.
    #[allow(clippy::too_many_arguments)]
    pub fn for_polish(
        docs: &Docs,
        board: &Status,
        run_dir: &Path,
        default_branch: &str,
        feature_branch: &str,
        retro_template: &str,
        packs_dir: &Path,
        spawn: u32,
    ) -> Result<Pack> {
        let feature = format!("{:03}", board.number);
        let prd_rel = format!("{}/prd.md", board.dir);
        let repo = docs.root();
        let log = std::process::Command::new("git")
            .args(["log", "--oneline", &format!("{default_branch}..HEAD")])
            .current_dir(repo)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "(no commits listed)".to_string());
        let body_path = run_dir.join("pr-body.md");
        let contract = CONTRACT_POLISH
            .replace("{NNN}", &feature)
            .replace("{feature dir}", &board.dir)
            .replace("{date}", &crate::clock::today())
            .replace("{seed}", "9901");
        let files = vec![prd_rel.clone()];

        let mut text = format!(
            "<!-- factory-pack files: {} -->\n<!-- run dir: {} -->\n\n",
            files.join(","),
            run_dir.display()
        );
        text.push_str(&format!(
            "# Branch\n\nDefault branch: `{default_branch}` · feature branch: `{feature_branch}`\n\n\
             Commits this branch adds (`git log --oneline {default_branch}..HEAD`):\n\n```\n{log}\n```"
        ));
        text.push_str(&format!(
            "\n\n# Retro\n\n<!-- {}/retro.md -->\n\n\
             Write the prose sections. The driver fills `gate_runs`, `gate_red`, `gate_minutes`, \
             `ci_cycles`, `slices`, `checkpoints`, `build_minutes`, `polish_minutes`, `out_tokens`, \
             `cache_read_tokens`, `start_context` and `peak_context` from its store after you return; \
             `score` is the owner's. The template:\n\n````markdown\n{}\n````",
            board.dir,
            retro_template.replace("{NNN}", &feature).trim_end()
        ));
        text.push_str(&format!(
            "\n\n# PR body\n\n<!-- write: {} -->\n\nWrite the PR body to `{}` (outside the repo) in this shape:\n\n{}",
            body_path.display(),
            body_path.display(),
            PR_BODY.trim_end()
        ));
        text.push_str("\n\n# Contract\n\n");
        text.push_str(contract.trim_end());
        text.push_str("\n\n# Autonomy\n\n");
        text.push_str(AUTONOMY.trim_end());
        text.push('\n');

        fs::create_dir_all(packs_dir).map_err(io(format!("mkdir {}", packs_dir.display())))?;
        let path = packs_dir.join(format!("polish-{spawn}.md"));
        fs::write(&path, text).map_err(io(format!("write {}", path.display())))?;
        Ok(Pack {
            path,
            prompt: "/polish".to_string(),
            files,
            missing_sections: Vec::new(),
        })
    }

    /// The CI fix worker's pack (FR-6): `# CI failure` — the check's name,
    /// the gate and inner commands, the last [`TAIL_LINES`] of
    /// `gh run view --log-failed` — then the CI fix contract, the autonomy
    /// rules and the inner loop.
    #[allow(clippy::too_many_arguments)]
    pub fn for_ci_fix(
        docs: &Docs,
        board: &Status,
        round: u32,
        attempt: u32,
        check: &str,
        log_text: &str,
        packs_dir: &Path,
        spawn: u32,
    ) -> Result<Pack> {
        let feature = format!("{:03}", board.number);
        let gate = docs.gate()?;
        let inner = docs.gate_inner()?;
        let contract = CONTRACT_CI_FIX
            .replace("{NNN}", &feature)
            .replace("{round}", &round.to_string())
            .replace("{check}", check)
            .replace("{seed}", &format!("8{round}01"));

        let mut text = String::from("<!-- factory-pack files: -->\n\n");
        text.push_str(&format!(
            "# CI failure\n\n<!-- round {round}, attempt {attempt}, check: {check} -->\n\n"
        ));
        text.push_str(&format!(
            "Failing check: `{check}`\nGate command: `{}`\nInner command: `{}`\n\n",
            gate.trim(),
            inner.trim()
        ));
        text.push_str("```\n");
        text.push_str(tail(log_text, TAIL_LINES).trim_end());
        text.push_str("\n```");
        text.push_str("\n\n# Contract\n\n");
        text.push_str(contract.trim_end());
        text.push_str("\n\n# Autonomy\n\n");
        text.push_str(AUTONOMY.trim_end());
        text.push_str("\n\n# Inner loop\n\n");
        text.push_str(inner.trim());
        text.push('\n');

        fs::create_dir_all(packs_dir).map_err(io(format!("mkdir {}", packs_dir.display())))?;
        let path = packs_dir.join(format!("ci-fix-{round}-{spawn}.md"));
        fs::write(&path, text).map_err(io(format!("write {}", path.display())))?;
        Ok(Pack {
            path,
            prompt: format!(
                "Fix the red CI check `{check}` on the PR of PRD-{feature} as the appended system prompt says. Return the STATUS block."
            ),
            files: Vec::new(),
            missing_sections: Vec::new(),
        })
    }
}

/// The last `n` lines of `text`.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let skip = lines.len().saturating_sub(n);
    lines[skip..].join("\n")
}

/// The slice's `### k. Prove` task: its heading through the line before the
/// next `### ` or `## `; empty when the slice has none.
pub fn prove_section(slice_text: &str) -> &str {
    let mut start = None;
    let mut offset = 0;
    for line in slice_text.split_inclusive('\n') {
        if let Some(s) = start
            && (line.starts_with("### ") || line.starts_with("## "))
        {
            return &slice_text[s..offset];
        }
        if start.is_none() && line.starts_with("### ") && line.contains("Prove") {
            start = Some(offset);
        }
        offset += line.len();
    }
    start.map(|s| &slice_text[s..]).unwrap_or("")
}

/// The slice's `**Progress**:` line.
pub fn progress_line(slice_text: &str) -> Option<&str> {
    slice_text
        .lines()
        .find(|l| l.trim_start().starts_with("**Progress**"))
}

/// Every `FR-n` in `text`, once each, in order.
pub fn fr_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("FR-") {
        rest = &rest[i + 3..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let id = format!("FR-{digits}");
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// The § QA plan table of `prd`: its two header lines and the `| FR-n |`
/// rows whose id is in `named` — every row when `named` is empty or none
/// matches. A note when the PRD has no table.
pub fn qa_rows(prd: &str, named: &[String]) -> String {
    let rows: Vec<&str> = prd
        .lines()
        .filter(|l| l.starts_with("| FR") || l.starts_with("|---"))
        .collect();
    if rows.is_empty() {
        return String::from("(the PRD has no QA plan table)");
    }
    let header: Vec<&str> = rows
        .iter()
        .copied()
        .take_while(|l| !l.starts_with("| FR-"))
        .collect();
    let body: Vec<&str> = rows
        .iter()
        .copied()
        .filter(|l| l.starts_with("| FR-"))
        .collect();
    let picked: Vec<&str> = body
        .iter()
        .copied()
        .filter(|l| {
            let id = l.trim_start_matches('|').trim();
            let id = id.split('|').next().unwrap_or("").trim();
            named.iter().any(|n| n == id)
        })
        .collect();
    let mut out: Vec<&str> = header;
    if picked.is_empty() {
        out.extend(body);
    } else {
        out.extend(picked);
    }
    out.join("\n")
}

/// A slice pack's `# Outline` body and the files it outlined (§14 FR-12
/// decision 3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recon {
    pub text: String,
    /// Repo-relative, in print order: the header line's additions.
    pub outlined: Vec<String>,
}

impl Recon {
    /// The `def` of each symbol the slice's `Current code / environment`
    /// cites, then the outline of each file it cites and of each file a
    /// printed `def` hit lives in; [`DEF_CAP`] and [`OUTLINE_CAP`] with a
    /// trailer when cut. A planned file (absent) and a `def` with no hit
    /// are skipped; so is a file in no outlined language.
    pub fn of(repo: &Path, slice_text: &str) -> Result<Recon> {
        let (paths, symbols) = cited_in(current_code(slice_text));
        let opts = walk::WalkOpts { vendor: false };
        let mut defs = String::new();
        let mut def_files: Vec<String> = Vec::new();
        for name in &symbols {
            if defs.len() >= DEF_CAP {
                break;
            }
            let found = def::find(repo, name, None, &opts)?;
            if found.hits.is_empty() {
                continue;
            }
            for h in &found.hits {
                if !def_files.contains(&h.path) {
                    def_files.push(h.path.clone());
                }
            }
            defs.push_str(&format!(
                "<!-- factory ctx def {name} -->\n\n{}\n",
                print::defs(name, &found, true).trim_end()
            ));
        }
        let (defs, defs_cut) = cap(&defs, DEF_CAP, "factory ctx def <symbol>");
        if defs_cut {
            def_files.retain(|f| defs.contains(&format!("{f}:")));
        }

        let mut outlines = String::new();
        let mut outlined: Vec<String> = Vec::new();
        for p in paths.iter().chain(&def_files) {
            if outlined.contains(p) || !repo.join(p).is_file() {
                continue;
            }
            let Ok(listing) = outline::list(repo, &[PathBuf::from(p)]) else {
                continue;
            };
            if listing.files.is_empty() {
                continue;
            }
            let found = outline::find(repo, &listing, 2);
            if !outlines.is_empty() {
                outlines.push('\n');
            }
            outlines.push_str(&print::outline(&found));
            outlined.push(p.clone());
        }
        let (outlines, outlines_cut) = cap(&outlines, OUTLINE_CAP, "factory ctx outline <path>");
        if outlines_cut {
            outlined.retain(|f| outlines.contains(&format!("== {f} ==")));
        }

        let mut text = String::new();
        if outlined.is_empty() && defs.is_empty() {
            text.push_str(
                "<!-- the slice's Current code cites no existing source file and no defined symbol -->\n",
            );
        }
        if !outlined.is_empty() {
            text.push_str(&format!(
                "<!-- factory ctx outline {} -->\n\n{}\n",
                outlined.join(" "),
                outlines.trim_end()
            ));
        }
        if !defs.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&defs);
        }
        Ok(Recon { text, outlined })
    }
}

/// `text` whole when it fits in `max` bytes; else cut at the last line
/// that fits, with a trailer saying how to get the rest.
fn cap(text: &str, max: usize, verb: &str) -> (String, bool) {
    if text.len() <= max {
        return (text.to_string(), false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let keep = text[..end].rfind('\n').map_or(0, |i| i + 1);
    (
        format!(
            "{}[cut at {max} chars: run `{verb}` for the rest]\n",
            &text[..keep]
        ),
        true,
    )
}

/// The slice's `## Current code / environment` section, to the next `## `.
pub fn current_code(slice_text: &str) -> &str {
    let mut start = None;
    let mut offset = 0;
    for line in slice_text.split_inclusive('\n') {
        if let Some(s) = start
            && line.starts_with("## ")
        {
            return &slice_text[s..offset];
        }
        if start.is_none() && line.starts_with("## Current code") {
            start = Some(offset + line.len());
        }
        offset += line.len();
    }
    start.map_or("", |s| &slice_text[s..])
}

/// The repo-relative paths (`` `src/a.rs` ``, `` `src/a.rs:12` ``,
/// `` `src/a.rs:12-40` ``) and the symbol names (`` `name()` ``,
/// `` `Type::name` ``, by their last segment) backticked in `text`, in
/// order, once each. Absolute, home, variable and `..` paths are not the
/// repo's and are left out.
pub fn cited_in(text: &str) -> (Vec<String>, Vec<String>) {
    let mut paths: Vec<String> = Vec::new();
    let mut symbols: Vec<String> = Vec::new();
    for span in text.split('`').skip(1).step_by(2) {
        let span = span.trim();
        if let Some(p) = cited_path(span) {
            if !paths.contains(&p) {
                paths.push(p);
            }
        } else if let Some(s) = cited_symbol(span)
            && !symbols.contains(&s)
        {
            symbols.push(s);
        }
    }
    (paths, symbols)
}

fn cited_path(span: &str) -> Option<String> {
    let path = match span.rsplit_once(':') {
        Some((p, lines))
            if !lines.is_empty()
                && lines
                    .split('-')
                    .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) =>
        {
            p
        }
        _ => span,
    };
    let path = path.strip_prefix("./").unwrap_or(path);
    let name = path.rsplit('/').next()?;
    let ok = !path.is_empty()
        && !path.starts_with(['/', '~', '$'])
        && !path.split('/').any(|seg| seg.is_empty() || seg == "..")
        && path
            .chars()
            .all(|c| c.is_alphanumeric() || "/._-@+".contains(c))
        && name.rsplit_once('.').is_some_and(|(stem, ext)| {
            !stem.is_empty() && !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric())
        });
    ok.then(|| path.to_string())
}

fn cited_symbol(span: &str) -> Option<String> {
    let call = span.strip_suffix("()");
    let body = call.unwrap_or(span);
    let segs: Vec<&str> = body.split("::").collect();
    let ident = |s: &str| {
        s.starts_with(|c: char| c.is_alphabetic() || c == '_')
            && s.chars().all(|c| c.is_alphanumeric() || c == '_')
    };
    if !segs.iter().all(|s| ident(s)) || (call.is_none() && segs.len() < 2) {
        return None;
    }
    segs.last().map(|s| s.to_string())
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(io(format!("read {}", path.display())))
}

/// The `§n` numbers on the slice's first `**Source**:` line, in order, once
/// each. A slice's task-level `**Source**` lines repeat the top one's set by
/// convention, and a section cited only there is read from the top line's
/// cut anyway.
pub fn cited_sections(slice_text: &str) -> Vec<u32> {
    let Some(line) = slice_text
        .lines()
        .find(|l| l.trim_start().starts_with("**Source**"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find('§') {
        rest = &rest[i + '§'.len_utf8()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse::<u32>()
            && !out.contains(&n)
        {
            out.push(n);
        }
    }
    out
}

/// `prd.md` from its `## n.` heading to the line before the next `## `.
pub fn cut_section(prd: &str, n: u32) -> Option<&str> {
    let prefix = format!("## {n}.");
    let mut start = None;
    let mut offset = 0;
    for line in prd.split_inclusive('\n') {
        if let Some(s) = start
            && line.starts_with("## ")
        {
            return Some(&prd[s..offset]);
        }
        if start.is_none() && (line.starts_with(&prefix) || line.trim_end() == format!("## {n}")) {
            start = Some(offset);
        }
        offset += line.len();
    }
    start.map(|s| &prd[s..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cited_sections_reads_the_top_source_line_once_each() {
        let text = "# T\n\n**Source**: [`prd.md`](prd.md) → §4 FR-2 bullets 1, 5; §3 FR-1 and §4 again.\n\n### 1. x\n**Source**: §20\n";
        assert_eq!(cited_sections(text), vec![4, 3]);
        assert_eq!(cited_sections("**Source**: §_(n)_.\n"), Vec::<u32>::new());
        assert_eq!(cited_sections("no source line\n"), Vec::<u32>::new());
    }

    #[test]
    fn current_code_cites_paths_and_symbols_once_each() {
        let slice = "# S\n\n## Current code / environment\n\n\
            - `src/a.rs:12` and `src/a.rs:3-9`, `./lib/b.php`, `Cargo.toml`\n\
            - `~/.claude/settings.json:51`, `/etc/x.conf`, `$HOME/a.sh`, `../up.rs`\n\
            - `helper()`, `Pack::for_slice`, `Pack::for_slice()`, `a::b::c`\n\
            - `file:line`, `name`, `--settings '<json>'`, `x.y()`, `` `src/c.go` ``\n\n\
            ## Tasks\n\n`src/not_here.rs` and `later()`\n";
        let (paths, symbols) = cited_in(current_code(slice));
        assert_eq!(
            paths,
            vec!["src/a.rs", "lib/b.php", "Cargo.toml", "src/c.go"]
        );
        assert_eq!(symbols, vec!["helper", "for_slice", "c"]);
        assert_eq!(current_code("# no such section\n"), "");
    }

    #[test]
    fn cap_cuts_at_a_line_with_a_trailer() {
        let text = "aaaa\nbbbb\ncccc\n";
        assert_eq!(cap(text, 100, "v"), (text.to_string(), false));
        let (cut, was) = cap(text, 12, "factory ctx outline <path>");
        assert!(was);
        assert_eq!(
            cut,
            "aaaa\nbbbb\n[cut at 12 chars: run `factory ctx outline <path>` for the rest]\n"
        );
    }

    #[test]
    fn cut_section_stops_at_the_next_heading() {
        let prd = "# P\n\n## 1. Why\n\nbecause\n\n### 1.1 sub\n\nmore\n\n## 2. Users\n\nthem\n";
        assert_eq!(
            cut_section(prd, 1),
            Some("## 1. Why\n\nbecause\n\n### 1.1 sub\n\nmore\n\n")
        );
        assert_eq!(cut_section(prd, 2), Some("## 2. Users\n\nthem\n"));
        assert_eq!(cut_section(prd, 3), None);
        assert_eq!(cut_section(prd, 11), None);
    }
}
