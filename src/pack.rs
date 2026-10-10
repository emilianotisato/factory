//! The context pack a worker reads as its appended system prompt (PRD §4
//! FR-2): the slice file, the PRD sections its `**Source**` line cites, the
//! slice contract, the autonomy rules and the inner-loop command. Nothing
//! else. Both prose assets are this repo's own copies under `assets/`, so a
//! pack never depends on what is installed under the owner's home (§19).
use std::fs;
use std::path::{Path, PathBuf};

use crate::docs::{Docs, Status};
use crate::error::io;
use crate::{Error, Result};

pub const CONTRACT: &str = include_str!("../assets/contract-slice.md");
pub const AUTONOMY: &str = include_str!("../assets/autonomy.md");

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
        if start.is_some() && line.starts_with("## ") {
            return Some(&prd[start.unwrap()..offset]);
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
