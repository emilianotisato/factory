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
use crate::docs::{Docs, Status, log};
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
/// The retro template (PRD-002 FR-19): the heading and the prose sections,
/// no frontmatter. The numbers live in the store and the `## Numbers` section.
pub const RETRO: &str = include_str!("../assets/retro.md");
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

/// A contract with each `{factory}` spelled as [`lookup`] spells the `ctx`
/// verbs: the running binary's absolute path (PRD-002 FR-18).
fn contract(text: &str) -> String {
    text.replace("{factory}", &crate::paths::factory_command())
}

/// The `# Log` section: the feature's `log.jsonl` as `factory docs log show
/// NNN` prints it, byte for byte, or "none yet" for an empty log. It takes
/// the place of the `agent-log.md` a worker used to open (PRD-002 FR-18).
fn log_section(docs: &Docs, board: &Status) -> Result<String> {
    let (entries, _) = log::entries(&docs.root().join(&board.dir))?;
    let text = if entries.is_empty() {
        "none yet".to_string()
    } else {
        log::render(&entries, log::Filter::All, board.number)
    };
    Ok(format!(
        "# Log\n\n<!-- factory docs log show {:03} -->\n\n{}",
        board.number,
        text.trim_end()
    ))
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
    /// Each section the pack wrote, in order, with its size (PRD-003 §4
    /// FR-22): what the worker's starting context is made of.
    pub parts: Vec<Part>,
}

/// One `# Heading` section of a written pack: its name in lower snake case
/// and its characters, heading through the blank line before the next one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Part {
    pub section: String,
    pub chars: usize,
}

/// `header` then `sections`, each starting at its `# Heading` line, joined
/// by a blank line that counts toward the section before it; the final
/// newline counts toward the last. The header is no part, so the parts sum
/// to the text after it.
fn assemble(header: String, sections: Vec<String>) -> (String, Vec<Part>) {
    let mut text = header;
    let mut parts = Vec::new();
    let last = sections.len().saturating_sub(1);
    for (i, mut section) in sections.into_iter().enumerate() {
        section.push_str(if i == last { "\n" } else { "\n\n" });
        let heading = section.lines().next().unwrap_or("");
        parts.push(Part {
            section: heading
                .trim_start_matches('#')
                .trim()
                .to_lowercase()
                .replace(' ', "_"),
            chars: section.chars().count(),
        });
        text.push_str(&section);
    }
    (text, parts)
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
        let mut prd_sections = String::new();
        let mut missing = Vec::new();
        for n in &cited {
            match cut_section(&prd_text, *n) {
                Some(text) => {
                    prd_sections.push_str(text.trim_end());
                    prd_sections.push_str("\n\n");
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
        let contract = contract(CONTRACT)
            .replace("{NNN}", &feature)
            .replace("{NN}", &nn)
            .replace("{N}", &slice.to_string());

        let mut sections = vec![format!(
            "# Slice\n\n<!-- {slice_rel} -->\n\n{}",
            slice_text.trim_end()
        )];
        if let Some(b) = block {
            sections.push(format!(
                "# Block and answer\n\n<!-- {}: {} -->\n\n{}",
                b.source,
                b.id,
                b.text.trim_end()
            ));
        }
        sections.push(format!(
            "# PRD sections\n\n<!-- {prd_rel}: {} -->\n\n{}",
            if cited.is_empty() {
                "the slice cites no section".to_string()
            } else {
                cited
                    .iter()
                    .map(|n| format!("§{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            prd_sections.trim_end()
        ));
        sections.push(log_section(docs, board)?);
        sections.push(format!("# Outline\n\n{}", recon.text.trim_end()));
        sections.push(format!("# Lookup\n\n{}", lookup()));
        sections.push(format!("# Contract\n\n{}", contract.trim_end()));
        sections.push(format!("# Autonomy\n\n{}", AUTONOMY.trim_end()));
        sections.push(format!("# Inner loop\n\n{}", inner.trim()));
        let (text, parts) = assemble(
            format!("<!-- factory-pack files: {} -->\n\n", files.join(",")),
            sections,
        );

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
            parts,
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
        let contract = contract(CONTRACT_FIX)
            .replace("{NNN}", &feature)
            .replace("{n}", &n.to_string());

        let (text, parts) = assemble(
            "<!-- factory-pack files: -->\n\n".to_string(),
            vec![
                format!(
                    "# Gate failure\n\n<!-- checkpoint {n}, gate attempt {attempt}: {} -->\n\n\
                     Gate command: `{}`\nInner command: `{}`\n\n```\n{}\n```",
                    gate_log.display(),
                    gate.trim(),
                    inner.trim(),
                    tail(&log_text, TAIL_LINES).trim_end()
                ),
                log_section(docs, board)?,
                format!("# Contract\n\n{}", contract.trim_end()),
                format!("# Autonomy\n\n{}", AUTONOMY.trim_end()),
                format!("# Inner loop\n\n{}", inner.trim()),
            ],
        );

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
            parts,
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
        let contract = contract(CONTRACT_CHECKPOINT)
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

        let (text, parts) = assemble(
            format!("<!-- factory-pack files: {} -->\n\n", files.join(",")),
            vec![
                format!(
                    "# Gate transcript\n\n<!-- checkpoint {n}, {}: {} -->\n\n```\n{}\n```",
                    if cached { "cached green" } else { "green" },
                    gate_log.display(),
                    tail(&log_text, TAIL_LINES).trim_end()
                ),
                format!("# Covered slices\n\n{}", slices.trim_end()),
                format!(
                    "# QA rows\n\n<!-- {prd_rel}: § QA plan -->\n\n{}",
                    qa_rows(&prd_text, &named).trim_end()
                ),
                log_section(docs, board)?,
                format!("# Contract\n\n{}", contract.trim_end()),
                format!("# Autonomy\n\n{}", AUTONOMY.trim_end()),
            ],
        );

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
            parts,
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
        let contract = contract(CONTRACT_POLISH)
            .replace("{NNN}", &feature)
            .replace("{feature dir}", &board.dir)
            .replace("{date}", &crate::docs::scaffold::today());
        let files = vec![prd_rel.clone()];

        let branch = format!(
            "# Branch\n\nDefault branch: `{default_branch}` · feature branch: `{feature_branch}`\n\n\
             Commits this branch adds (`git log --oneline {default_branch}..HEAD`):\n\n```\n{log}\n```"
        );
        let retro = format!(
            "# Retro\n\n<!-- {}/retro.md -->\n\n\
             Write the prose sections. After you return, the driver writes the `## Numbers` \
             section and the feature's row in its store; put no number of its in the file. \
             The template:\n\n````markdown\n{}\n````",
            board.dir,
            retro_template.replace("{NNN}", &feature).trim_end()
        );
        let pr_body = format!(
            "# PR body\n\n<!-- write: {} -->\n\nWrite the PR body to `{}` (outside the repo) in this shape:\n\n{}",
            body_path.display(),
            body_path.display(),
            PR_BODY.trim_end()
        );
        // Two header lines: the polish stub reads the run dir from the second.
        let (text, parts) = assemble(
            format!(
                "<!-- factory-pack files: {} -->\n<!-- run dir: {} -->\n\n",
                files.join(","),
                run_dir.display()
            ),
            vec![
                branch,
                log_section(docs, board)?,
                retro,
                pr_body,
                format!("# Contract\n\n{}", contract.trim_end()),
                format!("# Autonomy\n\n{}", AUTONOMY.trim_end()),
            ],
        );

        fs::create_dir_all(packs_dir).map_err(io(format!("mkdir {}", packs_dir.display())))?;
        let path = packs_dir.join(format!("polish-{spawn}.md"));
        fs::write(&path, text).map_err(io(format!("write {}", path.display())))?;
        Ok(Pack {
            path,
            prompt: "/polish".to_string(),
            files,
            missing_sections: Vec::new(),
            parts,
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
        let contract = contract(CONTRACT_CI_FIX)
            .replace("{NNN}", &feature)
            .replace("{round}", &round.to_string())
            .replace("{check}", check);

        let (text, parts) = assemble(
            "<!-- factory-pack files: -->\n\n".to_string(),
            vec![
                format!(
                    "# CI failure\n\n<!-- round {round}, attempt {attempt}, check: {check} -->\n\n\
                     Failing check: `{check}`\nGate command: `{}`\nInner command: `{}`\n\n```\n{}\n```",
                    gate.trim(),
                    inner.trim(),
                    tail(log_text, TAIL_LINES).trim_end()
                ),
                log_section(docs, board)?,
                format!("# Contract\n\n{}", contract.trim_end()),
                format!("# Autonomy\n\n{}", AUTONOMY.trim_end()),
                format!("# Inner loop\n\n{}", inner.trim()),
            ],
        );

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
            parts,
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
        // The trailers name the running binary, as `# Lookup` does.
        let factory = crate::paths::factory_command();
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
        let (defs, defs_cut) = cap(&defs, DEF_CAP, &format!("{factory} ctx def <symbol>"));
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
        let (outlines, outlines_cut) = cap(
            &outlines,
            OUTLINE_CAP,
            &format!("{factory} ctx outline <path>"),
        );
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

    /// The parts' chars against the pack file: its text after the header
    /// comment lines and the blank line that ends them.
    fn sections_of(pack: &Pack) -> Vec<&str> {
        let text = fs::read_to_string(&pack.path).unwrap();
        let (_, body) = text.split_once("\n\n").unwrap();
        let sum: usize = pack.parts.iter().map(|p| p.chars).sum();
        assert_eq!(sum, body.chars().count(), "{}", pack.path.display());
        pack.parts.iter().map(|p| p.section.as_str()).collect()
    }

    #[test]
    fn every_pack_kind_measures_the_sections_it_wrote() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        fs::write(repo.join("gate.sh"), "").unwrap();
        fs::write(
            repo.join("docs/project.toml"),
            "[docs]\nframework = 3\ngate = \"./gate.sh\"\ninner = \"./gate.sh --inner\"\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("prd.md"),
            "# PRD — é\n\n## 4. FR-1\n\nthe rule — ünïcode\n\n## 9. QA plan\n\n| FR-1 | run → ok | t | no |\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-01-a.md"),
            "# Slice 01 — a\n\n**Source**: §4\n\n### 2. Prove\nFR-1 row\n",
        )
        .unwrap();
        let docs = Docs::open(repo).unwrap();
        let board = Status {
            number: 1,
            dir: dir.to_string(),
            slices: vec![crate::docs::Slice {
                number: 1,
                file: "001-x/slice-01-a.md".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let packs = repo.join("packs");
        let log = repo.join("gate.log");
        fs::write(&log, "ok — done\n").unwrap();

        let slice = Pack::for_slice(&docs, &board, 1, &packs, 1).unwrap();
        assert_eq!(
            sections_of(&slice),
            [
                "slice",
                "prd_sections",
                "log",
                "outline",
                "lookup",
                "contract",
                "autonomy",
                "inner_loop"
            ]
        );
        let block = BlockSection {
            id: "BLOCK-1".to_string(),
            source: format!("{dir}/log.jsonl"),
            text: "### BLOCK-1 — x\n\n**Resolution**: y".to_string(),
        };
        let answered = Pack::for_slice_with(&docs, &board, 1, &packs, 2, Some(&block)).unwrap();
        assert_eq!(
            sections_of(&answered)[..3],
            ["slice", "block_and_answer", "prd_sections"]
        );

        let fix = Pack::for_fix(&docs, &board, 1, 1, &log, &packs, 1).unwrap();
        assert_eq!(
            sections_of(&fix),
            ["gate_failure", "log", "contract", "autonomy", "inner_loop"]
        );
        let cp = Pack::for_checkpoint(&docs, &board, 1, &[1], &log, false, &packs, 1).unwrap();
        assert_eq!(
            sections_of(&cp),
            [
                "gate_transcript",
                "covered_slices",
                "qa_rows",
                "log",
                "contract",
                "autonomy"
            ]
        );
        let polish =
            Pack::for_polish(&docs, &board, repo, "main", "feat/x", "# Retro", &packs, 1).unwrap();
        assert_eq!(
            sections_of(&polish),
            ["branch", "log", "retro", "pr_body", "contract", "autonomy"]
        );
        let ci = Pack::for_ci_fix(&docs, &board, 1, 1, "test", "red — ✗\n", &packs, 1).unwrap();
        assert_eq!(
            sections_of(&ci),
            ["ci_failure", "log", "contract", "autonomy", "inner_loop"]
        );
        let text = fs::read_to_string(&slice.path).unwrap();
        assert!(
            text.contains("# Log\n\n<!-- factory docs log show 001 -->\n\nnone yet\n\n# Outline"),
            "{text}"
        );
    }
}
