//! Every prose asset is embedded from `assets/`, so a pack never depends on what is installed in the owner's home.
use std::fs;
use std::path::{Path, PathBuf};

use crate::ctx::{def, outline, print, walk};
use crate::docs::{Docs, Status, log};
use crate::error::io;
use crate::lanes;
use crate::paths;
use crate::scheduler::pad_list;
use crate::{Error, Result};

fn custom_lane_env(env: &[(String, String)]) -> impl Iterator<Item = &(String, String)> {
    env.iter().filter(|(k, _)| k.as_str() != lanes::ENV_LANE)
}

fn lane_export_assignments(env: &[(String, String)]) -> Option<String> {
    let v: Vec<_> = custom_lane_env(env).collect();
    if v.is_empty() {
        return None;
    }
    Some(
        v.iter()
            .map(|(k, val)| format!("{k}={}", paths::shell_word(val)))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn env_prefixed(env: &[(String, String)], cmd: &str) -> String {
    match lane_export_assignments(env) {
        Some(a) => format!("export {a}; {cmd}"),
        None => cmd.to_string(),
    }
}

fn lane_env_section(env: &[(String, String)]) -> Option<String> {
    let assignments = lane_export_assignments(env)?;
    Some(format!(
        "# Lane env\n\nexport {assignments}\n\n\
         Keep these exports active when running build/test commands in interactive shells."
    ))
}

fn push_lane_env(sections: &mut Vec<String>, lane_env: &[(String, String)]) {
    if let Some(s) = lane_env_section(lane_env) {
        sections.push(s);
    }
}

pub const CONTRACT: &str = include_str!("../assets/contract-slice.md");
pub const CONTRACT_FIX: &str = include_str!("../assets/contract-fix.md");
pub const CONTRACT_CHECKPOINT: &str = include_str!("../assets/contract-checkpoint.md");
pub const CONTRACT_POLISH: &str = include_str!("../assets/contract-polish.md");
pub const CONTRACT_CI_FIX: &str = include_str!("../assets/contract-ci-fix.md");
pub const CONTRACT_MERGE: &str = include_str!("../assets/contract-merge.md");
pub const PR_BODY: &str = include_str!("../assets/pr-body.md");
const AUTONOMY: &str = include_str!("../assets/autonomy.md");
const CUT: &str = "<!-- builder: cut -->";
const CUT_END: &str = "<!-- builder: end -->";
pub const RETRO: &str = include_str!("../assets/retro.md");
pub const LOOKUP: &str = include_str!("../assets/lookup.md");

/// The running binary's absolute path, so the verbs work from a build that is not installed.
pub fn lookup() -> String {
    LOOKUP.trim_end().replace(
        "`factory ctx ",
        &format!("`{} ctx ", crate::paths::factory_command()),
    )
}

/// The whole Autonomy file, its cut markers removed.
pub fn autonomy() -> String {
    autonomy_text(false)
}

/// The Autonomy a builder reads: the parts that are the orchestrator's or the driver's job left out.
pub fn builder_autonomy() -> String {
    autonomy_text(true)
}

fn autonomy_text(cut: bool) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in AUTONOMY.lines() {
        match line.trim() {
            CUT => inside = true,
            CUT_END => inside = false,
            _ if inside && cut => {}
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out.trim_end().to_string()
}

fn contract(text: &str) -> String {
    text.replace("{factory}", &crate::paths::factory_command())
}

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

pub const OUTLINE_CAP: usize = 8000;
pub const DEF_CAP: usize = 6000;

pub const TAIL_LINES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub path: PathBuf,
    pub prompt: String,
    pub files: Vec<String>,
    pub missing_sections: Vec<u32>,
    pub parts: Vec<Part>,
    /// The QA rows a checkpoint pack handed its worker; empty for every other pack.
    pub rows: Vec<QaRow>,
}

/// One handed QA row: `FR-<n>` when its FR has one row in the table, `FR-<n>.<k>` for the k-th of several.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QaRow {
    pub key: String,
    pub human: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Part {
    pub section: String,
    pub chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentBack {
    pub attempt: u32,
    pub missing_slices: Vec<u32>,
    pub unproven_rows: Vec<String>,
    pub prior_record: Option<u32>,
}

fn sent_back_section(sb: &SentBack, n: u32) -> String {
    let mut body = String::from("# Sent back\n\n");
    if let Some(k) = sb.prior_record {
        body.push_str(&format!("Prior record: `CP-{k}`.\n\n"));
    }
    if !sb.missing_slices.is_empty() {
        body.push_str(&format!(
            "Missing slices: {}.\n\n",
            pad_list(&sb.missing_slices)
        ));
    }
    if !sb.unproven_rows.is_empty() {
        body.push_str(&format!(
            "Previously unproven rows: {}.\n\n",
            sb.unproven_rows.join(", ")
        ));
    }
    body.push_str(&format!(
        "Prior evidence in `cp-{n}/` was moved aside because the lane was reset; re-drive and save fresh evidence for EVERY row of checkpoint {n}."
    ));
    body
}

/// The joining blank line counts toward the section before it, so the parts sum to the text after the header.
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

/// The `# Collected` body from a collect file: its plan, the leaks verdict (with its hits, when any), the second opinion's status
/// and pass lines, then the file's path. The findings stay in the file, so they do not anchor the local review.
pub fn collected(text: &str, file: &Path) -> String {
    let part = |heading: &str| {
        let marker = format!("\n## {heading}\n");
        text.find(&marker).map(|i| {
            let rest = &text[i + marker.len()..];
            rest.find("\n## ").map_or(rest, |end| &rest[..end]).trim()
        })
    };
    let plan = part("Plan").unwrap_or("(no plan in the file)");
    let leaks = part("Leaks");
    let verdict = leaks
        .and_then(|l| l.lines().find(|l| l.starts_with("verdict:")))
        .unwrap_or("verdict: (none in the file)");
    // Hits are fixes, not opinions that could anchor the review, so they travel with the verdict.
    let verdict = match leaks {
        Some(l) if verdict != "verdict: clean" => l,
        _ => verdict,
    };
    let marker = "\n## Second opinion\n";
    let opinion = text.find(marker).map_or("", |i| &text[i + marker.len()..]);
    let mut status: Vec<String> = Vec::new();
    let mut in_section = false;
    for line in opinion.lines() {
        if let Some(pass) = line.strip_prefix("### Pass ") {
            in_section = true;
            if pass.ends_with(" — counted") || pass.ends_with(" — unreviewed") {
                status.push(format!("- Pass {pass}"));
            }
        } else if line.starts_with("### ") {
            in_section = true;
        } else if !in_section && !line.trim().is_empty() {
            status.push(line.to_string());
        }
    }
    format!(
        "## Plan\n\n{plan}\n\n## Leaks\n\n{verdict}\n\n## Second opinion\n\n{}\n\nThe whole file, findings included: `{}`",
        status.join("\n"),
        file.display()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSection {
    pub id: String,
    /// Repo-relative: the log file the entry was read from.
    pub source: String,
    pub text: String,
}

impl Pack {
    /// `spawn` numbers the file so a respawn never overwrites the pack a dead worker read.
    pub fn for_slice(
        docs: &Docs,
        board: &Status,
        slice: u32,
        packs_dir: &Path,
        spawn: u32,
    ) -> Result<Pack> {
        Self::for_slice_with(docs, board, slice, packs_dir, spawn, None)
    }

    pub fn for_slice_with(
        docs: &Docs,
        board: &Status,
        slice: u32,
        packs_dir: &Path,
        spawn: u32,
        block: Option<&BlockSection>,
    ) -> Result<Pack> {
        Self::for_slice_with_env(docs, board, slice, packs_dir, spawn, block, &[])
    }

    pub fn for_slice_with_env(
        docs: &Docs,
        board: &Status,
        slice: u32,
        packs_dir: &Path,
        spawn: u32,
        block: Option<&BlockSection>,
        lane_env: &[(String, String)],
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
        sections.push(format!("# Autonomy\n\n{}", builder_autonomy()));
        push_lane_env(&mut sections, lane_env);
        sections.push(format!(
            "# Inner loop\n\n{}",
            env_prefixed(lane_env, inner.trim())
        ));
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
            rows: Vec::new(),
        })
    }
}

impl Pack {
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
        Self::for_fix_env(docs, board, n, attempt, gate_log, packs_dir, spawn, &[])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_fix_env(
        docs: &Docs,
        board: &Status,
        n: u32,
        attempt: u32,
        gate_log: &Path,
        packs_dir: &Path,
        spawn: u32,
        lane_env: &[(String, String)],
    ) -> Result<Pack> {
        let feature = format!("{:03}", board.number);
        let gate = docs.gate()?;
        let inner = docs.gate_inner()?;
        let log_text = read(gate_log)?;
        let contract = contract(CONTRACT_FIX)
            .replace("{NNN}", &feature)
            .replace("{n}", &n.to_string());

        let prefixed = env_prefixed(lane_env, inner.trim());
        let mut body = vec![
            format!(
                "# Gate failure\n\n<!-- checkpoint {n}, gate attempt {attempt}: {} -->\n\n\
                     Gate command: `{}`\nInner command: `{}`\n\n```\n{}\n```",
                gate_log.display(),
                gate.trim(),
                prefixed.trim(),
                tail(&log_text, TAIL_LINES).trim_end()
            ),
            log_section(docs, board)?,
            format!("# Contract\n\n{}", contract.trim_end()),
            format!("# Autonomy\n\n{}", autonomy()),
        ];
        push_lane_env(&mut body, lane_env);
        body.push(format!("# Inner loop\n\n{}", prefixed));
        let (text, parts) = assemble("<!-- factory-pack files: -->\n\n".to_string(), body);

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
            rows: Vec::new(),
        })
    }

    /// `sha` is read in the main tree, which shares the lanes' objects.
    #[allow(clippy::too_many_arguments)]
    pub fn for_merge(
        docs: &Docs,
        board: &Status,
        slice: u32,
        conflicted: &[String],
        sha: &str,
        packs_dir: &Path,
        spawn: u32,
        block: Option<&BlockSection>,
    ) -> Result<Pack> {
        Self::for_merge_env(
            docs,
            board,
            slice,
            conflicted,
            sha,
            packs_dir,
            spawn,
            block,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_merge_env(
        docs: &Docs,
        board: &Status,
        slice: u32,
        conflicted: &[String],
        sha: &str,
        packs_dir: &Path,
        spawn: u32,
        block: Option<&BlockSection>,
        lane_env: &[(String, String)],
    ) -> Result<Pack> {
        let entry = board
            .slices
            .iter()
            .find(|s| s.number == slice)
            .ok_or_else(|| Error::Msg(format!("slice {slice:02} is not on the board")))?;
        let slice_rel = format!("docs/features/{}", entry.file);
        let repo = docs.root();
        let slice_text = read(&repo.join(&slice_rel))?;
        let inner = docs.gate_inner()?;
        let feature = format!("{:03}", board.number);
        let nn = format!("{slice:02}");
        let message = git_text(repo, &["log", "-1", "--format=%B", sha])?;
        let stat = git_text(repo, &["show", "--stat", "--format=", sha])?;
        let contract = contract(CONTRACT_MERGE)
            .replace("{NNN}", &feature)
            .replace("{NN}", &nn)
            .replace("{N}", &slice.to_string());

        let mut files = vec![slice_rel.clone()];
        if let Some(b) = block
            && !files.contains(&b.source)
        {
            files.push(b.source.clone());
        }
        let mut sections = vec![
            format!(
                "# Conflict\n\n<!-- lane commit {sha} -->\n\nConflicted paths:\n\n```\n{}\n```\n\n\
                 The lane commit's message:\n\n```\n{}\n```\n\n\
                 What it changed (`git show --stat`):\n\n```\n{}\n```",
                conflicted.join("\n"),
                message.trim(),
                stat.trim()
            ),
            format!(
                "# Slice\n\n<!-- {slice_rel} -->\n\n{}",
                slice_text.trim_end()
            ),
        ];
        if let Some(b) = block {
            sections.push(format!(
                "# Block and answer\n\n<!-- {}: {} -->\n\n{}",
                b.source,
                b.id,
                b.text.trim_end()
            ));
        }
        sections.push(log_section(docs, board)?);
        sections.push(format!("# Contract\n\n{}", contract.trim_end()));
        sections.push(format!("# Autonomy\n\n{}", autonomy()));
        push_lane_env(&mut sections, lane_env);
        sections.push(format!(
            "# Inner loop\n\n{}",
            env_prefixed(lane_env, inner.trim())
        ));
        let (text, parts) = assemble(
            format!("<!-- factory-pack files: {} -->\n\n", files.join(",")),
            sections,
        );

        fs::create_dir_all(packs_dir).map_err(io(format!("mkdir {}", packs_dir.display())))?;
        let path = packs_dir.join(format!("merge-{nn}-{spawn}.md"));
        fs::write(&path, text).map_err(io(format!("write {}", path.display())))?;
        Ok(Pack {
            path,
            prompt: format!(
                "Resolve the conflicted landing of slice {nn} of PRD-{feature} as the appended system prompt says. Return the STATUS block."
            ),
            files,
            missing_sections: Vec::new(),
            parts,
            rows: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_checkpoint(
        docs: &Docs,
        board: &Status,
        n: u32,
        covers: &[u32],
        gate_log: &Path,
        cached: bool,
        leaks: &str,
        packs_dir: &Path,
        spawn: u32,
        sent_back: Option<&SentBack>,
    ) -> Result<Pack> {
        Self::for_checkpoint_env(
            docs,
            board,
            n,
            covers,
            gate_log,
            cached,
            leaks,
            packs_dir,
            spawn,
            sent_back,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_checkpoint_env(
        docs: &Docs,
        board: &Status,
        n: u32,
        covers: &[u32],
        gate_log: &Path,
        cached: bool,
        leaks: &str,
        packs_dir: &Path,
        spawn: u32,
        sent_back: Option<&SentBack>,
        lane_env: &[(String, String)],
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
        // `{covers}` is prose; the `--covers` flag needs one shell word.
        let covers_csv = covers
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let contract = contract(CONTRACT_CHECKPOINT)
            .replace("{NNN}", &feature)
            .replace("{n}", &n.to_string())
            .replace("{NN}", &format!("{last:02}"))
            .replace("{covers}", &covers_text)
            .replace("{covers csv}", &covers_csv)
            .replace("{feature dir}", &board.dir);

        let mut files = Vec::new();
        let mut slices = String::new();
        for c in covers {
            let Some(entry) = board.slices.iter().find(|s| s.number == *c) else {
                continue;
            };
            let rel = format!("docs/features/{}", entry.file);
            let slice_text = read(&repo.join(&rel))?;
            let prove = prove_section(&slice_text);
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
        let (qa_text, rows) = handed_qa(repo, board, covers, &prd_text)?;

        let mut sections = vec![
            format!(
                "# Gate transcript\n\n<!-- checkpoint {n}, {}: {} -->\n\n```\n{}\n```",
                if cached { "cached green" } else { "green" },
                gate_log.display(),
                tail(&log_text, TAIL_LINES).trim_end()
            ),
            format!("# Leaks\n\n{}", leaks.trim_end()),
            format!("# Covered slices\n\n{}", slices.trim_end()),
            format!(
                "# QA rows\n\n<!-- {prd_rel}: § QA plan -->\n\n{}{}",
                if board.all_done {
                    "Every slice is done, so this checkpoint is handed every row in the QA table, \
                     not only the rows its covered slices name.\n\n"
                } else {
                    ""
                },
                qa_text.trim_end()
            ),
        ];
        if let Some(sb) = sent_back {
            sections.push(sent_back_section(sb, n));
        }
        sections.push(log_section(docs, board)?);
        sections.push(format!("# Contract\n\n{}", contract.trim_end()));
        push_lane_env(&mut sections, lane_env);
        sections.push(format!("# Autonomy\n\n{}", autonomy()));

        let (text, parts) = assemble(
            format!("<!-- factory-pack files: {} -->\n\n", files.join(",")),
            sections,
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
            rows,
        })
    }

    /// The prompt is `/polish`: slash commands resolve inside `-p`, so the pack carries no skill body.
    #[allow(clippy::too_many_arguments)]
    pub fn for_polish(
        docs: &Docs,
        board: &Status,
        run_dir: &Path,
        default_branch: &str,
        feature_branch: &str,
        collected: &str,
        retro_template: &str,
        packs_dir: &Path,
        spawn: u32,
        lane_env: &[(String, String)],
    ) -> Result<Pack> {
        let feature = format!("{:03}", board.number);
        let inner = docs.gate_inner()?;
        let prd_rel = format!("{}/prd.md", board.dir);
        let repo = docs.root();
        let base_ref = crate::commit::resolve_base_ref(repo, Some(default_branch))
            .unwrap_or_else(|_| default_branch.to_string());
        let log = std::process::Command::new("git")
            .args(["log", "--oneline", &format!("{base_ref}..HEAD")])
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
        let mut sections = vec![
            branch,
            format!("# Collected\n\n{}", collected.trim_end()),
            log_section(docs, board)?,
            retro,
            pr_body,
            format!("# Contract\n\n{}", contract.trim_end()),
            format!("# Autonomy\n\n{}", autonomy()),
        ];
        push_lane_env(&mut sections, lane_env);
        sections.push(format!(
            "# Inner loop\n\n{}",
            env_prefixed(lane_env, inner.trim())
        ));
        let (text, parts) = assemble(
            format!(
                "<!-- factory-pack files: {} -->\n<!-- run dir: {} -->\n\n",
                files.join(","),
                run_dir.display()
            ),
            sections,
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
            rows: Vec::new(),
        })
    }

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
        Self::for_ci_fix_env(
            docs,
            board,
            round,
            attempt,
            check,
            log_text,
            packs_dir,
            spawn,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_ci_fix_env(
        docs: &Docs,
        board: &Status,
        round: u32,
        attempt: u32,
        check: &str,
        log_text: &str,
        packs_dir: &Path,
        spawn: u32,
        lane_env: &[(String, String)],
    ) -> Result<Pack> {
        let feature = format!("{:03}", board.number);
        let gate = docs.gate()?;
        let inner = docs.gate_inner()?;
        let contract = contract(CONTRACT_CI_FIX)
            .replace("{NNN}", &feature)
            .replace("{round}", &round.to_string())
            .replace("{check}", check);

        let mut body = vec![
            format!(
                "# CI failure\n\n<!-- round {round}, attempt {attempt}, check: {check} -->\n\n\
                     Failing check: `{check}`\nGate command: `{}`\nInner command: `{}`\n\n```\n{}\n```",
                gate.trim(),
                inner.trim(),
                tail(log_text, TAIL_LINES).trim_end()
            ),
            log_section(docs, board)?,
            format!("# Contract\n\n{}", contract.trim_end()),
            format!("# Autonomy\n\n{}", autonomy()),
        ];
        push_lane_env(&mut body, lane_env);
        body.push(format!(
            "# Inner loop\n\n{}",
            env_prefixed(lane_env, inner.trim())
        ));
        let (text, parts) = assemble("<!-- factory-pack files: -->\n\n".to_string(), body);

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
            rows: Vec::new(),
        })
    }
}

pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let skip = lines.len().saturating_sub(n);
    lines[skip..].join("\n")
}

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

pub fn progress_line(slice_text: &str) -> Option<&str> {
    slice_text
        .lines()
        .find(|l| l.trim_start().starts_with("**Progress**"))
}

pub fn source_before_depends(slice_text: &str) -> &str {
    let end = slice_text
        .find("**Depends on**")
        .unwrap_or(slice_text.len());
    let head = &slice_text[..end];
    head.find("**Source**:").map(|i| &head[i..]).unwrap_or("")
}

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

/// The QA rows a checkpoint covering `covers` is handed: the rows the covered slices' Prove sections name, or every
/// row once every slice is done.
pub fn handed_qa(
    repo: &Path,
    board: &Status,
    covers: &[u32],
    prd_text: &str,
) -> Result<(String, Vec<QaRow>)> {
    if board.all_done {
        return Ok(qa_rows(prd_text, None));
    }
    let mut named: Vec<String> = Vec::new();
    for c in covers {
        let Some(entry) = board.slices.iter().find(|s| s.number == *c) else {
            continue;
        };
        let slice_text = read(&repo.join(format!("docs/features/{}", entry.file)))?;
        for id in fr_ids(prove_section(&slice_text)) {
            if !named.contains(&id) {
                named.push(id);
            }
        }
    }
    let mut incomplete: Vec<String> = Vec::new();
    for s in &board.slices {
        if s.status.as_deref() == Some("done") {
            continue;
        }
        let slice_text = read(&repo.join(format!("docs/features/{}", s.file)))?;
        for id in fr_ids(prove_section(&slice_text))
            .into_iter()
            .chain(fr_ids(source_before_depends(&slice_text)))
        {
            if !incomplete.contains(&id) {
                incomplete.push(id);
            }
        }
    }
    let deferred: Vec<String> = named
        .iter()
        .filter(|id| incomplete.contains(id))
        .cloned()
        .collect();
    named.retain(|id| !incomplete.contains(id));
    let (mut qa_text, rows) = qa_rows(prd_text, Some(&named[..]));
    if !deferred.is_empty() {
        qa_text.push_str(&format!(
            "\n(deferred until remaining slice(s) finish: {})",
            deferred.join(", ")
        ));
    }
    Ok((qa_text, rows))
}

/// The QA plan rows handed to a checkpoint, each printed with its key: the rows whose first cell names an id in
/// `named`, or every row when `named` is `None`.
pub fn qa_rows(prd: &str, named: Option<&[String]>) -> (String, Vec<QaRow>) {
    let lines: Vec<&str> = prd.lines().collect();
    let row_id = |line: &str| -> Option<String> {
        line.strip_prefix("| FR-")?;
        fr_ids(cells(line).first()?).into_iter().next()
    };
    let Some(first) = lines.iter().position(|l| row_id(l).is_some()) else {
        let any_table = lines.iter().any(|l| l.starts_with("|---"));
        return (
            String::from(if any_table {
                "(the PRD has no `| FR-` row: 0 rows handed)"
            } else {
                "(the PRD has no QA plan table)"
            }),
            Vec::new(),
        );
    };
    let header_start = lines[..first]
        .iter()
        .rposition(|l| !l.starts_with('|'))
        .map_or(0, |i| i + 1);
    let header = &lines[header_start..first];
    let human_col = header
        .iter()
        .find_map(|h| cells(h).iter().position(|c| *c == "Human?"))
        .unwrap_or(3);
    let body: Vec<(&str, String)> = lines
        .iter()
        .filter_map(|l| row_id(l).map(|id| (*l, id)))
        .collect();

    let mut out: Vec<String> = header.iter().map(|h| h.to_string()).collect();
    let mut rows = Vec::new();
    for (i, (line, id)) in body.iter().enumerate() {
        if named.is_some_and(|n| !n.contains(id)) {
            continue;
        }
        let total = body.iter().filter(|(_, other)| other == id).count();
        let key = if total == 1 {
            id.clone()
        } else {
            let k = body[..=i].iter().filter(|(_, other)| other == id).count();
            format!("{id}.{k}")
        };
        let human = cells(line).get(human_col).is_some_and(|c| is_yes(c));
        out.push(format!(
            "{line} key: `{key}`{}",
            if human {
                " (deferred: Human? = yes)"
            } else {
                ""
            }
        ));
        rows.push(QaRow { key, human });
    }
    if rows.is_empty() {
        out.push(String::new());
        out.push("(no covered slice's Prove section names a row: 0 rows handed)".to_string());
    }
    (out.join("\n"), rows)
}

/// A `Human?` cell reads yes when its first word is `yes`, as in `yes`, `**yes**: routing is judgment` or
/// `yes → qa-guide.md`.
fn is_yes(cell: &str) -> bool {
    cell.trim_start_matches(['*', '_', ' '])
        .split(|c: char| !c.is_ascii_alphabetic())
        .next()
        .is_some_and(|w| w.eq_ignore_ascii_case("yes"))
}

fn cells(row: &str) -> Vec<&str> {
    let row = row.trim();
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = row.strip_suffix('|').unwrap_or(row);
    row.split('|').map(str::trim).collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recon {
    pub text: String,
    pub outlined: Vec<String>,
}

impl Recon {
    /// A cited file that does not exist yet is planned, not an error.
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
            let found = def::find(repo, repo, name, None, &opts)?;
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

fn git_text(repo: &Path, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(io(format!("run git {}", args.join(" "))))?;
    if !out.status.success() {
        return Err(Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Only the top `**Source**` line: task-level ones repeat its set by convention.
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

pub fn cut_section(prd: &str, n: u32) -> Option<&str> {
    let prefix = format!("## {n}.");
    cut_where(prd, |line| {
        line.starts_with(&prefix) || line.trim_end() == format!("## {n}")
    })
}

/// The `## ` section whose title, past any `N.` number, is `title`, ignoring case; heading line included.
pub fn cut_titled<'a>(prd: &'a str, title: &str) -> Option<&'a str> {
    let titled = |line: &str| {
        line.strip_prefix("## ").is_some_and(|h| {
            let h = h.trim();
            let h = h
                .split_once(". ")
                .filter(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                .map_or(h, |(_, rest)| rest);
            h.trim().eq_ignore_ascii_case(title)
        })
    };
    cut_where(prd, titled)
}

/// From the first line `starts` matches up to the next `## ` heading, heading line included.
fn cut_where(prd: &str, starts: impl Fn(&str) -> bool) -> Option<&str> {
    let mut start = None;
    let mut offset = 0;
    for line in prd.split_inclusive('\n') {
        if let Some(s) = start
            && line.starts_with("## ")
        {
            return Some(&prd[s..offset]);
        }
        if start.is_none() && starts(line) {
            start = Some(offset);
        }
        offset += line.len();
    }
    start.map(|s| &prd[s..])
}

#[cfg(test)]
#[path = "../tests/common/passage.rs"]
mod passage;

#[cfg(test)]
mod tests {
    use super::passage::assert_unique_passage;
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

    const QA_TABLE: &str = "# PRD\n\n| Step | What |\n|---|---|\n| 1 | a |\n\n## 8. QA plan\n\n\
        | FR | Proof | Evidence | Human? |\n|---|---|---|---|\n\
        | FR-6 | run a | transcript | no |\n\
        | FR-6 | run b | transcript | no |\n\
        | FR-7 | the owner reads it back | read-back | yes |\n\
        | FR-6 | run c | transcript | no |\n\
        | FR-8 after merge | run d | transcript | no |\n";

    fn keys(rows: &[QaRow]) -> Vec<&str> {
        rows.iter().map(|r| r.key.as_str()).collect()
    }

    fn body_rows(text: &str) -> Vec<&str> {
        text.lines().filter(|l| l.starts_with("| FR-")).collect()
    }

    #[test]
    fn one_fr_with_three_rows_gets_three_keys() {
        let (text, rows) = qa_rows(QA_TABLE, None);
        assert_eq!(keys(&rows), ["FR-6.1", "FR-6.2", "FR-7", "FR-6.3", "FR-8"]);
        let human: Vec<&str> = rows
            .iter()
            .filter(|r| r.human)
            .map(|r| r.key.as_str())
            .collect();
        assert_eq!(human, ["FR-7"]);
        assert!(
            text.contains("| FR-6 | run c | transcript | no | key: `FR-6.3`\n"),
            "{text}"
        );
        assert!(
            text.contains("| yes | key: `FR-7` (deferred: Human? = yes)"),
            "{text}"
        );
        assert!(!text.contains("| Step"), "{text}");

        let named = ["FR-6".to_string(), "FR-8".to_string()];
        let (_, rows) = qa_rows(QA_TABLE, Some(&named));
        assert_eq!(keys(&rows), ["FR-6.1", "FR-6.2", "FR-6.3", "FR-8"]);
    }

    #[test]
    fn a_human_cell_reads_yes_by_its_first_word() {
        for cell in [
            "yes",
            "Yes",
            "**yes**: routing is judgment",
            "yes → `qa-guide.md`",
        ] {
            assert!(is_yes(cell), "{cell}");
        }
        for cell in ["no", "", "yesterday", "no — yes later", "n/a"] {
            assert!(!is_yes(cell), "{cell}");
        }
    }

    #[test]
    fn a_prd_with_no_fr_rows_hands_nothing() {
        let prd = "# PRD\n\n| Step | What |\n|---|---|\n| 1 | a |\n";
        let (text, rows) = qa_rows(prd, None);
        assert!(rows.is_empty());
        assert_eq!(text, "(the PRD has no `| FR-` row: 0 rows handed)");
        let (text, rows) = qa_rows("# PRD\n\nno table\n", None);
        assert!(rows.is_empty());
        assert_eq!(text, "(the PRD has no QA plan table)");
    }

    #[test]
    fn source_before_depends_ignores_fr_ids_after_the_depends_line() {
        let text = "**Source**: §4 FR-2\n**Depends on**: Slice 02 (so FR-4 ships)\n";
        assert_eq!(fr_ids(source_before_depends(text)), ["FR-2"]);
        assert!(fr_ids(prove_section(text)).is_empty());
    }

    #[test]
    fn a_pending_slices_prove_defers_the_row_at_an_earlier_checkpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        let prd = "# PRD\n\n| FR | Proof | Evidence | Human? |\n|---|---|---|---|\n\
            | FR-3 | run t | transcript | no |\n";
        fs::write(repo.join(dir).join("prd.md"), prd).unwrap();
        fs::write(
            repo.join(dir).join("slice-02.md"),
            "# S\n\n### 3. Prove\n§8, the FR-3 row.\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-04.md"),
            "# S\n\n**Source**: §4 FR-3\n\n### 3. Prove\n§8, the FR-3 row.\n",
        )
        .unwrap();
        let board = Status {
            dir: dir.to_string(),
            all_done: false,
            slices: vec![
                crate::docs::Slice {
                    number: 2,
                    file: "001-x/slice-02.md".into(),
                    status: Some("done".into()),
                    ..Default::default()
                },
                crate::docs::Slice {
                    number: 4,
                    file: "001-x/slice-04.md".into(),
                    status: Some("pending".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (text, rows) = handed_qa(repo, &board, &[2], prd).unwrap();
        assert!(rows.is_empty(), "{rows:?}");
        assert!(
            text.contains("(deferred until remaining slice(s) finish: FR-3)"),
            "{text}"
        );
    }

    #[test]
    fn a_pending_slice_mentions_an_fr_only_after_depends_and_does_not_defer_it() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        let prd = "# PRD\n\n| FR | Proof | Evidence | Human? |\n|---|---|---|---|\n\
            | FR-4 | run t | transcript | no |\n";
        fs::write(repo.join(dir).join("prd.md"), prd).unwrap();
        fs::write(
            repo.join(dir).join("slice-02.md"),
            "# S\n\n### 3. Prove\n§8, the FR-4 row.\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-03.md"),
            "# S\n\n**Source**: §6 FR-2\n**Depends on**: Slice 02 (so FR-4 lands)\n",
        )
        .unwrap();
        let board = Status {
            dir: dir.to_string(),
            all_done: false,
            slices: vec![
                crate::docs::Slice {
                    number: 2,
                    file: "001-x/slice-02.md".into(),
                    status: Some("done".into()),
                    ..Default::default()
                },
                crate::docs::Slice {
                    number: 3,
                    file: "001-x/slice-03.md".into(),
                    status: Some("pending".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (text, rows) = handed_qa(repo, &board, &[2], prd).unwrap();
        assert_eq!(keys(&rows), ["FR-4"], "{rows:?}");
        assert!(!text.contains("deferred until remaining"), "{text}");
    }

    #[test]
    fn an_incomplete_slice_immediately_after_covers_also_defers_its_fr() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        let prd = "# PRD\n\n| FR | Proof | Evidence | Human? |\n|---|---|---|---|\n\
            | FR-1 | run a | transcript | no |\n| FR-2 | run b | transcript | no |\n";
        fs::write(repo.join(dir).join("prd.md"), prd).unwrap();
        fs::write(
            repo.join(dir).join("slice-01.md"),
            "# S\n\n### 3. Prove\n§8, the FR-1 row.\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-02.md"),
            "# S\n\n### 3. Prove\n§8, the FR-2 row.\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-03.md"),
            "# S\n\n**Source**: [`prd.md`](prd.md) → §4 (FR-2). **Depends on**: Slice 02.\n",
        )
        .unwrap();
        let board = Status {
            dir: dir.to_string(),
            all_done: false,
            slices: vec![
                crate::docs::Slice {
                    number: 1,
                    file: "001-x/slice-01.md".into(),
                    status: Some("done".into()),
                    ..Default::default()
                },
                crate::docs::Slice {
                    number: 2,
                    file: "001-x/slice-02.md".into(),
                    status: Some("done".into()),
                    ..Default::default()
                },
                crate::docs::Slice {
                    number: 3,
                    file: "001-x/slice-03.md".into(),
                    status: Some("pending".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (text, rows) = handed_qa(repo, &board, &[1, 2], prd).unwrap();
        assert_eq!(keys(&rows), ["FR-1"], "{text}");
        assert!(
            text.contains("(deferred until remaining slice(s) finish: FR-2)"),
            "{text}"
        );
    }

    #[test]
    fn incomplete_slices_after_the_covers_are_the_ones_that_defer() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        let prd = "# PRD\n\n| FR | Proof | Evidence | Human? |\n|---|---|---|---|\n\
            | FR-1 | run t | transcript | no |\n| FR-2 | run u | transcript | no |\n";
        fs::write(repo.join(dir).join("prd.md"), prd).unwrap();
        fs::write(
            repo.join(dir).join("slice-01.md"),
            "# S\n\n### 3. Prove\n§8, the FR-1 and FR-2 rows.\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-03.md"),
            "# S\n\n**Source**: [`prd.md`](prd.md) → §_(n)_\n",
        )
        .unwrap();
        let board = Status {
            dir: dir.to_string(),
            all_done: false,
            slices: vec![
                crate::docs::Slice {
                    number: 1,
                    file: "001-x/slice-01.md".into(),
                    status: Some("done".into()),
                    ..Default::default()
                },
                crate::docs::Slice {
                    number: 3,
                    file: "001-x/slice-03.md".into(),
                    status: Some("building".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (text, rows) = handed_qa(repo, &board, &[1], prd).unwrap();
        assert_eq!(keys(&rows), ["FR-1", "FR-2"], "{text}");
        assert!(!text.contains("deferred until remaining"), "{text}");
    }

    #[test]
    fn the_last_checkpoint_hands_every_row_even_when_slices_still_cite_it() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        let prd = "# PRD\n\n| FR | Proof | Evidence | Human? |\n|---|---|---|---|\n\
            | FR-3 | run t | transcript | no |\n";
        fs::write(repo.join(dir).join("prd.md"), prd).unwrap();
        fs::write(
            repo.join(dir).join("slice-02.md"),
            "# S\n\n### 3. Prove\n§8, the FR-3 row.\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-04.md"),
            "# S\n\n### 3. Prove\n§8, the FR-3 row.\n",
        )
        .unwrap();
        let board = Status {
            dir: dir.to_string(),
            all_done: true,
            slices: vec![
                crate::docs::Slice {
                    number: 2,
                    file: "001-x/slice-02.md".into(),
                    status: Some("done".into()),
                    ..Default::default()
                },
                crate::docs::Slice {
                    number: 4,
                    file: "001-x/slice-04.md".into(),
                    status: Some("pending".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (text, rows) = handed_qa(repo, &board, &[2], prd).unwrap();
        assert_eq!(keys(&rows), ["FR-3"], "{rows:?}");
        assert!(!text.contains("deferred until remaining"), "{text}");
    }

    /// The first slice's Prove section names one row; the second's names none.
    fn checkpoint_rows(all_done: bool, covers: &[u32]) -> (Vec<QaRow>, String) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        fs::write(repo.join("docs/project.toml"), "[docs]\nframework = 3\n").unwrap();
        fs::write(repo.join(dir).join("prd.md"), QA_TABLE).unwrap();
        fs::write(
            repo.join(dir).join("slice-01-a.md"),
            "# S\n\n### 3. Prove\n`prd.md` §8, the FR-8 row.\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-02-b.md"),
            "# S\n\n### 3. Prove\nnone: no user-visible change.\n",
        )
        .unwrap();
        let docs = Docs::open(repo).unwrap();
        let slice = |number: u32, file: &str| crate::docs::Slice {
            number,
            file: format!("001-x/{file}"),
            status: Some("done".to_string()),
            ..Default::default()
        };
        let board = Status {
            number: 1,
            dir: dir.to_string(),
            all_done,
            slices: vec![slice(1, "slice-01-a.md"), slice(2, "slice-02-b.md")],
            ..Default::default()
        };
        let log = repo.join("gate.log");
        fs::write(&log, "ok\n").unwrap();
        let cp = Pack::for_checkpoint(
            &docs,
            &board,
            1,
            covers,
            &log,
            false,
            "clean\n",
            &repo.join("packs"),
            1,
            None,
        )
        .unwrap();
        let text = section_text(&cp, "qa_rows");
        (cp.rows, text)
    }

    #[test]
    fn the_last_checkpoint_is_handed_every_row_in_the_table() {
        let (rows, text) = checkpoint_rows(true, &[1]);
        assert_eq!(keys(&rows), ["FR-6.1", "FR-6.2", "FR-7", "FR-6.3", "FR-8"]);
        assert_eq!(body_rows(&text).len(), 5, "{text}");
        assert!(text.contains("handed every row in the QA table"), "{text}");
    }

    #[test]
    fn an_earlier_checkpoint_is_handed_only_its_covered_rows() {
        let (rows, text) = checkpoint_rows(false, &[1]);
        assert_eq!(keys(&rows), ["FR-8"]);
        assert_eq!(
            body_rows(&text),
            ["| FR-8 after merge | run d | transcript | no | key: `FR-8`"]
        );
        assert!(!text.contains("handed every row"), "{text}");
    }

    #[test]
    fn a_covered_slice_naming_no_row_hands_nothing() {
        let (rows, text) = checkpoint_rows(false, &[2]);
        assert!(rows.is_empty(), "{rows:?}");
        assert!(body_rows(&text).is_empty(), "{text}");
        assert!(
            text.contains("| FR | Proof | Evidence | Human? |\n|---|---|---|---|\n"),
            "{text}"
        );
        assert!(text.contains("0 rows handed"), "{text}");
    }

    #[test]
    fn a_checkpoint_pack_passes_its_covers_as_one_word() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        fs::write(repo.join("docs/project.toml"), "[docs]\nframework = 3\n").unwrap();
        fs::write(repo.join(dir).join("prd.md"), "# PRD\n").unwrap();
        let docs = Docs::open(repo).unwrap();
        let board = Status {
            number: 1,
            dir: dir.to_string(),
            ..Default::default()
        };
        let log = repo.join("gate.log");
        fs::write(&log, "ok\n").unwrap();
        let packs = repo.join("packs");
        let cp = Pack::for_checkpoint(
            &docs,
            &board,
            1,
            &[1, 2, 4],
            &log,
            false,
            "clean\n",
            &packs,
            1,
            None,
        )
        .unwrap();
        let text = fs::read_to_string(&cp.path).unwrap();
        assert!(text.contains("--slice 04 --covers 1,2,4 --title"), "{text}");
        assert!(text.contains("**Covers:** slices 01, 02, 04"), "{text}");
    }

    #[test]
    fn sent_back_section_names_missing_slices_and_every_row() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        fs::write(repo.join("docs/project.toml"), "[docs]\nframework = 3\n").unwrap();
        fs::write(repo.join(dir).join("prd.md"), "# PRD\n").unwrap();
        let docs = Docs::open(repo).unwrap();
        let board = Status {
            number: 1,
            dir: dir.to_string(),
            ..Default::default()
        };
        let log = repo.join("gate.log");
        fs::write(&log, "ok\n").unwrap();
        let sb = SentBack {
            attempt: 2,
            missing_slices: vec![2],
            unproven_rows: vec!["FR-1".into()],
            prior_record: Some(1),
        };
        let cp = Pack::for_checkpoint(
            &docs,
            &board,
            1,
            &[1, 2],
            &log,
            false,
            "clean\n",
            &repo.join("packs"),
            2,
            Some(&sb),
        )
        .unwrap();
        let text = fs::read_to_string(&cp.path).unwrap();
        assert!(text.contains("# Sent back"), "{text}");
        assert!(text.contains("Missing slices: 02"), "{text}");
        assert!(text.contains("Previously unproven rows: FR-1"), "{text}");
        assert!(
            text.contains("re-drive and save fresh evidence for EVERY row of checkpoint 1"),
            "{text}"
        );
        assert_eq!(
            sections_of(&cp),
            [
                "gate_transcript",
                "leaks",
                "covered_slices",
                "qa_rows",
                "sent_back",
                "log",
                "contract",
                "autonomy"
            ]
        );
    }

    #[test]
    fn a_merge_pack_leads_with_the_conflict_and_fills_the_contract() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(repo)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        fs::write(
            repo.join("docs/project.toml"),
            "[docs]\nframework = 3\ninner = \"./gate.sh --inner\"\n",
        )
        .unwrap();
        fs::write(repo.join(dir).join("prd.md"), "# PRD\n").unwrap();
        fs::write(
            repo.join(dir).join("slice-02-b.md"),
            "# Slice 02 — b\n\nrewrite line three\n",
        )
        .unwrap();
        fs::write(repo.join("shared.txt"), "1\n2\ntwo\n4\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "feat(001): slice-02 B"]);
        let docs = Docs::open(repo).unwrap();
        let board = Status {
            number: 1,
            dir: dir.to_string(),
            slices: vec![crate::docs::Slice {
                number: 2,
                file: "001-x/slice-02-b.md".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let pack = Pack::for_merge(
            &docs,
            &board,
            2,
            &["shared.txt".to_string()],
            "HEAD",
            &repo.join("packs"),
            1,
            None,
        )
        .unwrap();
        let text = fs::read_to_string(&pack.path).unwrap();
        let body = text.split_once("\n\n").unwrap().1;
        assert!(body.starts_with("# Conflict\n"), "{text}");
        assert!(text.contains("```\nshared.txt\n```"), "{text}");
        assert!(text.contains("feat(001): slice-02 B"), "{text}");
        assert!(
            text.lines()
                .any(|l| l.trim_start().starts_with("shared.txt") && l.ends_with("| 4 ++++")),
            "{text}"
        );
        assert!(text.contains("rewrite line three"), "{text}");
        assert!(text.contains("slice 02 of PRD-001"), "{text}");
        assert!(
            text.contains("docs log add 001 --kind block --slice 2 "),
            "{text}"
        );
        assert!(!text.contains("{NNN}") && !text.contains("{N}"), "{text}");
        assert!(pack.path.ends_with("merge-02-1.md"));
        assert_eq!(
            sections_of(&pack),
            [
                "conflict",
                "slice",
                "log",
                "contract",
                "autonomy",
                "inner_loop"
            ]
        );

        let job = crate::worker::Job::Merge {
            slice: 2,
            attempt: 1,
        };
        assert_eq!(job.role(), "merge");
        assert_eq!(job.slice(), Some(2));
    }

    fn sections_of(pack: &Pack) -> Vec<&str> {
        let text = fs::read_to_string(&pack.path).unwrap();
        let (_, body) = text.split_once("\n\n").unwrap();
        let sum: usize = pack.parts.iter().map(|p| p.chars).sum();
        assert_eq!(sum, body.chars().count(), "{}", pack.path.display());
        pack.parts.iter().map(|p| p.section.as_str()).collect()
    }

    #[test]
    fn env_prefixed_export_prefixes_custom_lane_vars_and_skips_factory_lane_only() {
        let env = vec![
            (lanes::ENV_LANE.to_string(), "2".to_string()),
            (
                "CARGO_TARGET_DIR".to_string(),
                "/abs/lane-2/target".to_string(),
            ),
        ];
        let cmd = "cargo test -q";
        let got = env_prefixed(&env, cmd);
        assert!(got.starts_with("export CARGO_TARGET_DIR=/abs/lane-2/target; "));
        assert_eq!(
            got,
            format!("export CARGO_TARGET_DIR=/abs/lane-2/target; {cmd}")
        );
        let lane_only = vec![(lanes::ENV_LANE.to_string(), "1".to_string())];
        assert_eq!(env_prefixed(&lane_only, cmd), cmd);
    }

    #[test]
    fn slice_pack_carries_lane_env_and_prefixed_inner_loop() {
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
            "# PRD\n\n## 1. FR-1\n\nrule\n",
        )
        .unwrap();
        fs::write(
            repo.join(dir).join("slice-01-a.md"),
            "# Slice 01\n\n**Source**: §1\n",
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
        let lane_env = vec![
            (lanes::ENV_LANE.to_string(), "2".to_string()),
            (
                "CARGO_TARGET_DIR".to_string(),
                "/tmp/lane-2/target".to_string(),
            ),
        ];
        let pack =
            Pack::for_slice_with_env(&docs, &board, 1, &repo.join("packs"), 1, None, &lane_env)
                .unwrap();
        let text = fs::read_to_string(&pack.path).unwrap();
        assert!(text.contains("# Lane env\n\nexport CARGO_TARGET_DIR=/tmp/lane-2/target"));
        assert!(text.contains("export CARGO_TARGET_DIR=/tmp/lane-2/target; ./gate.sh --inner"));
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

        let lane_env = vec![(
            "CARGO_TARGET_DIR".to_string(),
            "/tmp/lane/target".to_string(),
        )];
        let fix = Pack::for_fix_env(&docs, &board, 1, 1, &log, &packs, 1, &lane_env).unwrap();
        let fix_text = fs::read_to_string(&fix.path).unwrap();
        assert!(
            fix_text.contains("export CARGO_TARGET_DIR=/tmp/lane/target; ./gate.sh --inner"),
            "{fix_text}"
        );
        assert_eq!(
            sections_of(&fix),
            [
                "gate_failure",
                "log",
                "contract",
                "autonomy",
                "lane_env",
                "inner_loop"
            ]
        );
        let cp = Pack::for_checkpoint(
            &docs,
            &board,
            1,
            &[1],
            &log,
            false,
            "clean\n",
            &packs,
            1,
            None,
        )
        .unwrap();
        assert_eq!(
            sections_of(&cp),
            [
                "gate_transcript",
                "leaks",
                "covered_slices",
                "qa_rows",
                "log",
                "contract",
                "autonomy"
            ]
        );
        let polish_env = vec![(
            "CARGO_TARGET_DIR".to_string(),
            "/tmp/lane/target".to_string(),
        )];
        let polish = Pack::for_polish(
            &docs,
            &board,
            repo,
            "main",
            "feat/x",
            "collect failed: x",
            "# Retro",
            &packs,
            1,
            &polish_env,
        )
        .unwrap();
        let polish_text = fs::read_to_string(&polish.path).unwrap();
        assert!(polish_text.contains("# Lane env"));
        assert!(
            polish_text.contains("export CARGO_TARGET_DIR=/tmp/lane/target; ./gate.sh --inner")
        );
        assert_eq!(
            sections_of(&polish),
            [
                "branch",
                "collected",
                "log",
                "retro",
                "pr_body",
                "contract",
                "autonomy",
                "lane_env",
                "inner_loop"
            ]
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

    #[test]
    fn leaks_hits_travel_with_their_verdict_and_a_clean_scan_is_one_line() {
        let file = Path::new("/runs/r1/collect.md");
        let with = |leaks: &str| {
            format!(
                "<!-- head: abc -->\n\n## Plan\n\nsize: local review only\n\n## Leaks\n\n{leaks}\n## Second opinion\n\nskipped by size\n"
            )
        };
        let hits = "verdict: 1 fix hit\n\nfix: name the behaviour\n  src/a.rs:3 // slice 2\n";
        let section = collected(&with(hits), file);
        assert!(
            section.contains(&format!(
                "## Leaks\n\n{}\n\n## Second opinion",
                hits.trim_end()
            )),
            "{section}"
        );
        let section = collected(&with("verdict: clean\n\nno hits\n"), file);
        assert!(
            section.contains("## Leaks\n\nverdict: clean\n\n## Second opinion"),
            "{section}"
        );
    }

    #[test]
    fn collected_strips_the_claims_index_and_keeps_pass_status() {
        let file = Path::new("/runs/r1/collect.md");
        let text = "<!-- head: abc -->\n\n## Plan\n\nsize\n\n## Leaks\n\nverdict: clean\n\n## Second opinion\n\nexit: 0\nlog: /data/reviews/x\n\n### Claims index\n\n| severity | file:line | pass | claim |\n|---|---|---|---|\n| high | src/c.rs:3 | 2 | high claim pass2 |\n\n### Pass 1 — a (400 lines, 3 s, grok) — counted\n\nprose\n\n### Pass 2 — b (300 lines, 1 s, grok) — counted\n\nmore\n";
        let section = collected(text, file);
        assert!(section.contains("exit: 0\nlog: /data/reviews/x\n"));
        assert!(section.contains("- Pass 1 — a (400 lines, 3 s, grok) — counted"));
        assert!(section.contains("- Pass 2 — b (300 lines, 1 s, grok) — counted"));
        assert!(!section.contains("### Claims index"));
        assert!(!section.contains("high claim pass2"));
    }

    #[test]
    fn the_collected_section_keeps_the_status_lines_and_leaves_the_findings_in_the_file() {
        let file = Path::new("/runs/r1/collect.md");
        let text = "<!-- head: abc -->\n\n## Plan\n\nchanged: 2 files, 70 lines against main...HEAD\n- a.rs (60 lines)\n- b.rs (10 lines)\nsize: local review and second opinion\nsecond opinion: reviewed, 1 of 2 passes counted\n\n## Leaks\n\nverdict: clean\n\nno hits\n\n## Second opinion\n\nexit: 0\nlog: /data/reviews/x\n\n### Pass 1 — a.rs (60 lines, 3 s, grok) — counted\n\n## Bugs\n\na.rs:4 drops the error\n\n### Pass 2 — b.rs (10 lines, 1 s, grok) — unreviewed\n\nunreviewed (exit 1, no text)\n";
        let section = collected(text, file);
        assert_eq!(
            section,
            "## Plan\n\nchanged: 2 files, 70 lines against main...HEAD\n- a.rs (60 lines)\n- b.rs (10 lines)\nsize: local review and second opinion\nsecond opinion: reviewed, 1 of 2 passes counted\n\n## Leaks\n\nverdict: clean\n\n## Second opinion\n\nexit: 0\nlog: /data/reviews/x\n- Pass 1 — a.rs (60 lines, 3 s, grok) — counted\n- Pass 2 — b.rs (10 lines, 1 s, grok) — unreviewed\n\nThe whole file, findings included: `/runs/r1/collect.md`"
        );
        assert!(!section.contains("drops the error"), "{section}");
        assert!(!section.contains("## Bugs"), "{section}");
    }

    #[test]
    fn every_role_reads_the_plan_stays_in_docs() {
        let tmp = tempfile::tempdir().unwrap();
        for pack in &every_role_pack(tmp.path()) {
            let text = fs::read_to_string(&pack.path).unwrap();
            assert_eq!(
                text.matches("\n## The plan stays in docs/\n").count(),
                1,
                "{}",
                pack.path.display()
            );
        }
    }

    fn section_text(pack: &Pack, name: &str) -> String {
        let text = fs::read_to_string(&pack.path).unwrap();
        let (_, body) = text.split_once("\n\n").unwrap();
        let mut chars = body.chars();
        for part in &pack.parts {
            let section: String = chars.by_ref().take(part.chars).collect();
            if part.section == name {
                return section;
            }
        }
        panic!("no {name} in {}", pack.path.display())
    }

    #[test]
    fn a_builder_reads_autonomy_without_the_drivers_and_the_orchestrators_parts() {
        let tmp = tempfile::tempdir().unwrap();
        let all = every_role_pack(tmp.path());
        for pack in &all {
            let text = fs::read_to_string(&pack.path).unwrap();
            assert!(!text.contains(CUT), "{}", pack.path.display());
            assert!(!text.contains(CUT_END), "{}", pack.path.display());
            assert!(!text.contains("<!-- builder"), "{}", pack.path.display());
        }
        let builder = section_text(&all[0], "autonomy");
        let checkpoint = section_text(&all[3], "autonomy");
        for cut in [
            "## Mapping",
            "**Slice hand-back:**",
            "**CI:**",
            "**Flaky:**",
            "**Not this branch's:**",
            "**Notify**",
            "## The owner answers mid-run",
            "## Resuming",
        ] {
            assert!(!builder.contains(cut), "{cut}");
            assert_unique_passage("checkpoint autonomy", &checkpoint, cut);
        }
        for kept in [
            "\n## The plan stays in docs/\n",
            "5. **Stop.**",
            "**Gate:**",
        ] {
            assert_unique_passage("builder autonomy", &builder, kept);
        }
        assert!(
            checkpoint.chars().count() >= builder.chars().count() + 2500,
            "{} vs {}",
            checkpoint.chars().count(),
            builder.chars().count()
        );
        for autonomy in [&builder, &checkpoint] {
            assert_unique_passage("autonomy", autonomy, "Several tool calls side by side");
            assert!(!autonomy.contains("Read, Grep or Bash"));
        }
    }

    fn every_role_pack(repo: &Path) -> [Pack; 6] {
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(repo)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        let dir = "docs/features/001-x";
        fs::create_dir_all(repo.join(dir)).unwrap();
        fs::write(
            repo.join("docs/project.toml"),
            "[docs]\nframework = 3\ngate = \"./gate.sh\"\ninner = \"./gate.sh --inner\"\n",
        )
        .unwrap();
        fs::write(repo.join(dir).join("prd.md"), "# PRD\n\n## 4. FR-1\n\nx\n").unwrap();
        fs::write(
            repo.join(dir).join("slice-01-a.md"),
            "# Slice 01 — a\n\n**Source**: §4\n",
        )
        .unwrap();
        fs::write(repo.join("shared.txt"), "1\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "feat(001): slice-01 A"]);
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
        fs::write(&log, "ok\n").unwrap();

        [
            Pack::for_slice(&docs, &board, 1, &packs, 1).unwrap(),
            Pack::for_fix(&docs, &board, 1, 1, &log, &packs, 1).unwrap(),
            Pack::for_merge(
                &docs,
                &board,
                1,
                &["shared.txt".to_string()],
                "HEAD",
                &packs,
                1,
                None,
            )
            .unwrap(),
            Pack::for_checkpoint(
                &docs,
                &board,
                1,
                &[1],
                &log,
                false,
                "clean\n",
                &packs,
                1,
                None,
            )
            .unwrap(),
            Pack::for_polish(
                &docs,
                &board,
                repo,
                "main",
                "feat/x",
                "collect failed: x",
                "# Retro",
                &packs,
                1,
                &[],
            )
            .unwrap(),
            Pack::for_ci_fix(&docs, &board, 1, 1, "test", "red\n", &packs, 1).unwrap(),
        ]
    }
}
