//! The context pack a worker reads as its appended system prompt (PRD §4
//! FR-2): the slice file, the PRD sections its `**Source**` line cites, the
//! slice contract, the autonomy rules and the inner-loop command. Nothing
//! else. A checkpoint's workers (§6 FR-4) get the same layout with the slice
//! replaced: the fix worker reads the gate failure, the checkpoint worker
//! the green transcript, the covered slices and their QA rows. All prose
//! assets are this repo's own copies under `assets/`, so a pack never
//! depends on what is installed under the owner's home (§19).
use std::fs;
use std::path::{Path, PathBuf};

use crate::docs::{Docs, Status};
use crate::error::io;
use crate::{Error, Result};

pub const CONTRACT: &str = include_str!("../assets/contract-slice.md");
pub const CONTRACT_FIX: &str = include_str!("../assets/contract-fix.md");
pub const CONTRACT_CHECKPOINT: &str = include_str!("../assets/contract-checkpoint.md");
pub const AUTONOMY: &str = include_str!("../assets/autonomy.md");

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
