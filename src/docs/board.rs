//! The v3 board (PRD-002 FR-13): computed from the folders, `project.toml`
//! and each feature's `state.toml`, never from `docs/index.md`. A port of
//! `docs.mjs:158 scanFolder`, `:1374 featureState` and `:1343 statusCmd`.
//! The log fields (`checkpoints`, `unverified`, `openBlocks`, `log`) come
//! from the feature's `log.jsonl` (FR-14); the foundations (`gate`, `harness`,
//! `rubric`) from `gate.rs`.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use super::frontmatter::{self, Frontmatter};
use super::gate;
use super::log::{self, Flag};
use super::project::{self, Project};
use super::state::{self, State};
use super::{Building, Checkpoint, LogSummary, OpenBlock, Slice, SliceRef, Status};
use crate::error::io;
use crate::{Error, Result};

/// Live or archived, as `kind` prints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Live,
    Archive,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Live => "live",
            Kind::Archive => "archive",
        }
    }
}

/// One `NNN-slug/` folder.
#[derive(Debug, Clone)]
pub struct Feature {
    pub kind: Kind,
    pub number: u32,
    pub slug: String,
    pub dir_name: String,
    pub dir: PathBuf,
    /// `prd.md`'s frontmatter and text; `None` when there is no `prd.md`.
    pub prd: Option<(Frontmatter, String)>,
    /// `state.toml`, or why it did not parse.
    pub state: std::result::Result<Option<State>, String>,
    pub slices: Vec<SliceFile>,
}

/// One `slice-NN-subject.md`.
#[derive(Debug, Clone)]
pub struct SliceFile {
    pub number: u32,
    pub file: String,
    pub title: String,
    /// The v2 frontmatter, for a folder with no `state.toml`.
    pub fm: Frontmatter,
}

pub struct Board {
    pub root: PathBuf,
    pub project: Option<Project>,
    pub live: Vec<Feature>,
    pub archived: Vec<Feature>,
}

impl Feature {
    pub fn state(&self) -> Option<&State> {
        self.state.as_ref().ok().and_then(Option::as_ref)
    }

    /// `computedStatus` (`docs.mjs:367`): `state.toml` when there is one,
    /// else the v2 `prd.md` frontmatter. An archive folder is `retired` or
    /// `archived`, nothing else.
    pub fn status(&self) -> Option<String> {
        let own = match self.state() {
            Some(s) => Some(s.status.clone()),
            None => self
                .prd
                .as_ref()
                .and_then(|(fm, _)| fm.get("status"))
                .and_then(frontmatter::Value::text),
        };
        match self.kind {
            Kind::Archive if own.as_deref() == Some("retired") => Some("retired".into()),
            Kind::Archive => Some("archived".into()),
            Kind::Live => own,
        }
    }

    /// The board's slices: the `[slices.N]` entries, each with its file when
    /// there is one. A file with no entry is skipped (`check` reports it).
    /// A folder with no `state.toml` reads the v2 slice frontmatter.
    pub fn board_slices(&self) -> Vec<Slice> {
        let rel = |f: &SliceFile| format!("{}/{}", self.dir_name, f.file);
        let Some(state) = self.state() else {
            return self
                .slices
                .iter()
                .map(|f| Slice {
                    number: f.number,
                    file: rel(f),
                    title: Some(f.title.clone()),
                    status: f.fm.get("status").and_then(frontmatter::Value::text),
                    lane: f
                        .fm
                        .get("lane")
                        .and_then(frontmatter::Value::as_i64)
                        .map(|l| l as u32),
                    depends_on: f
                        .fm
                        .get("depends_on")
                        .and_then(frontmatter::Value::as_list)
                        .unwrap_or_default()
                        .iter()
                        .filter_map(frontmatter::Value::as_i64)
                        .map(|d| d as u32)
                        .collect(),
                })
                .collect();
        };
        state
            .slices
            .iter()
            .map(|(n, s)| {
                let file = self.slices.iter().find(|f| f.number == *n);
                Slice {
                    number: *n,
                    file: file.map(rel).unwrap_or_default(),
                    title: file.map(|f| f.title.clone()),
                    status: Some(s.status.clone()),
                    lane: s.lane,
                    depends_on: s.depends_on.clone(),
                }
            })
            .collect()
    }

    /// `summary` from the v3 PRD frontmatter, trimmed.
    pub fn summary(&self) -> Option<String> {
        let (fm, _) = self.prd.as_ref()?;
        let s = fm.str("summary")?.trim();
        (!s.is_empty()).then(|| s.to_string())
    }
}

/// Every feature folder under `docs/features/` and `docs/archive/`.
pub fn scan(root: &Path) -> Result<Board> {
    let docs = root.join("docs");
    Ok(Board {
        root: root.to_path_buf(),
        project: project::load(root)?,
        live: scan_folder(&docs.join("features"), Kind::Live)?,
        archived: scan_folder(&docs.join("archive"), Kind::Archive)?,
    })
}

/// `^(\d{3})-(.+)$` → number and slug.
pub fn folder_name(name: &str) -> Option<(u32, &str)> {
    let (n, slug) = name.split_at_checked(3)?;
    let slug = slug.strip_prefix('-')?;
    (n.bytes().all(|b| b.is_ascii_digit()) && !slug.is_empty())
        .then(|| n.parse().ok().map(|n| (n, slug)))
        .flatten()
}

/// One of `features/` or `archive/`; a `state.toml` that does not parse is
/// kept as its error on the feature, never a reason to fail.
pub fn scan_folder(base: &Path, kind: Kind) -> Result<Vec<Feature>> {
    let Ok(rd) = fs::read_dir(base) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let Some((number, slug)) = folder_name(&name) else {
            continue;
        };
        let dir = base.join(&name);
        let prd = match fs::read_to_string(dir.join("prd.md")) {
            Ok(text) => Some((frontmatter::parse(&text), text)),
            Err(_) => None,
        };
        let state = state::read(&dir).map_err(|e| e.to_string());
        out.push(Feature {
            kind,
            number,
            slug: slug.to_string(),
            dir_name: name.clone(),
            slices: slice_files(&dir)?,
            dir,
            prd,
            state,
        });
    }
    Ok(out)
}

fn slice_files(dir: &Path) -> Result<Vec<SliceFile>> {
    let rd = fs::read_dir(dir).map_err(io(format!("read {}", dir.display())))?;
    let mut out = Vec::new();
    for e in rd.filter_map(|e| e.ok()) {
        let file = e.file_name().to_string_lossy().into_owned();
        let Some(number) = state::slice_number(&file) else {
            continue;
        };
        let text =
            fs::read_to_string(e.path()).map_err(io(format!("read {}", e.path().display())))?;
        out.push(SliceFile {
            number,
            title: slice_title(&text, &file),
            fm: frontmatter::parse(&text),
            file,
        });
    }
    out.sort_by_key(|f| f.number);
    Ok(out)
}

/// The first H1 minus `Slice NN — ` (`docs.mjs:207`), else the file name.
fn slice_title(text: &str, file: &str) -> String {
    let h1 = text
        .lines()
        .find_map(|l| l.strip_prefix('#').filter(|r| r.starts_with([' ', '\t'])))
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let Some(h1) = h1 else {
        return file.to_string();
    };
    let strip = || -> Option<&str> {
        let rest = h1.get(..5).filter(|p| p.eq_ignore_ascii_case("slice"))?;
        let rest = h1[rest.len()..]
            .strip_prefix(char::is_whitespace)?
            .trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            return None;
        }
        let rest = rest[digits..].trim_start();
        let rest = rest.strip_prefix(['-', '–', '—'])?;
        Some(rest.trim_start())
    };
    strip().unwrap_or(h1).to_string()
}

impl Board {
    pub fn all(&self) -> impl Iterator<Item = &Feature> {
        self.live.iter().chain(&self.archived)
    }

    /// `resolveFeature`: a number, a slug, a folder name or a path to one.
    pub fn resolve(&self, reference: &str) -> Result<&Feature> {
        let r = reference.trim_end_matches('/');
        let r = r.rsplit('/').next().unwrap_or(r);
        let n = (!r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()))
            .then(|| r.parse::<u32>().ok())
            .flatten();
        self.all()
            .find(|f| match n {
                Some(n) => f.number == n,
                None => f.slug == r || f.dir_name == r,
            })
            .ok_or_else(|| Error::Msg(format!("no feature matching \"{reference}\"")))
    }

    fn lanes(&self) -> u32 {
        self.project.as_ref().map_or(1, |p| p.lanes)
    }

    fn checkpoint_every(&self) -> u32 {
        self.project.as_ref().map_or(2, |p| p.checkpoint_every)
    }

    fn framework(&self) -> u32 {
        self.project.as_ref().map_or(0, |p| p.framework)
    }
}

/// One feature's board, `docs status NNN --json`.
pub fn status(root: &Path, feature: &str) -> Result<Status> {
    let board = scan(root)?;
    let f = board.resolve(feature)?;
    feature_state(&board, f)
}

pub fn feature_state(board: &Board, f: &Feature) -> Result<Status> {
    if let Err(e) = &f.state {
        return Err(Error::Msg(e.clone()));
    }
    let slices = f.board_slices();
    let is = |s: &Slice, st: &str| s.status.as_deref() == Some(st);
    let done: Vec<u32> = slices
        .iter()
        .filter(|s| is(s, "done"))
        .map(|s| s.number)
        .collect();
    let runnable = |s: &Slice| is(s, "pending") && s.depends_on.iter().all(|d| done.contains(d));
    let as_ref = |s: &Slice, status: bool| SliceRef {
        number: s.number,
        file: s.file.clone(),
        status: if status { s.status.clone() } else { None },
    };
    let next = slices
        .iter()
        .find(|s| is(s, "building"))
        .or_else(|| slices.iter().find(|s| runnable(s)))
        .map(|s| as_ref(s, true));
    let ready: Vec<SliceRef> = slices
        .iter()
        .filter(|s| runnable(s))
        .map(|s| as_ref(s, false))
        .collect();
    let held: Vec<u32> = slices
        .iter()
        .filter(|s| is(s, "building") || is(s, "blocked"))
        .filter_map(|s| s.lane)
        .collect();
    let lanes = board.lanes();
    let free_lanes: Vec<u32> = (1..=lanes).filter(|l| !held.contains(l)).collect();
    let all_done = !slices.is_empty() && slices.iter().all(|s| is(s, "done"));
    let (entries, _) = log::entries(&f.dir)?;
    let of = |k: log::Kind| entries.iter().filter(move |e| e.kind == k);
    let checkpoints: Vec<Checkpoint> = of(log::Kind::Checkpoint)
        .filter_map(|e| {
            Some(Checkpoint {
                n: e.n()?,
                after: e.slice,
                covers: e.covers.clone(),
            })
        })
        .collect();
    // PRD-005 FR-7: every done slice no checkpoint covered. An entry with no
    // `covers` predates the field and keeps the old high-water rule.
    let covered = |n: u32| {
        checkpoints.iter().any(|c| {
            if c.covers.is_empty() {
                c.after.is_some_and(|a| a >= n)
            } else {
                c.covers.contains(&n)
            }
        })
    };
    let unverified: Vec<u32> = done.iter().copied().filter(|n| !covered(*n)).collect();
    let checkpoint_due =
        !unverified.is_empty() && (unverified.len() as u32 >= board.checkpoint_every() || all_done);
    let open_blocks: Vec<OpenBlock> = log::select(&entries, log::Filter::Open)
        .into_iter()
        .filter_map(|e| {
            Some(OpenBlock {
                n: e.n()?,
                slice: e.slice,
                phase: e.phase.clone(),
                title: Some(e.title.clone()),
            })
        })
        .collect();
    let decisions = || of(log::Kind::Decision).filter(|e| e.flag.is_some());
    // `decide` counts reviewed decisions too, as v2 does (`docs.mjs:1428`):
    // the ship step stores it on the run row (`closeout.rs:599`).
    let summary = LogSummary {
        decisions: (of(log::Kind::Decision).count() + of(log::Kind::Ask).count()) as u32,
        flagged: decisions().filter_map(|e| e.n()).collect(),
        decide: decisions()
            .filter(|e| e.flag == Some(Flag::Decide))
            .filter_map(|e| e.n())
            .collect(),
        blocks: of(log::Kind::Block).count() as u32,
    };
    let text = f.prd.as_ref().map(|(_, t)| t.as_str()).unwrap_or("");
    Ok(Status {
        number: f.number,
        slug: f.slug.clone(),
        dir: format!(
            "docs/{}/{}",
            if f.kind == Kind::Live {
                "features"
            } else {
                "archive"
            },
            f.dir_name
        ),
        kind: Some(f.kind.name().into()),
        status: f.status(),
        reviewed: f
            .state()
            .and_then(|s| s.reviewed.clone())
            .map_or(Value::Null, Value::String),
        gate: gate::status(
            &board.root,
            board.project.as_ref().and_then(|p| p.gate.as_deref()),
        ),
        qa_plan: has_qa_plan(text),
        harness_line: harness_line(text),
        harness: gate::harness(&board.root),
        rubric: gate::rubric(&board.root),
        sliced: !slices.is_empty(),
        all_done,
        checkpoint_every: board.checkpoint_every(),
        checkpoint_due,
        spawnable: if checkpoint_due {
            0
        } else {
            free_lanes.len().min(ready.len()) as u32
        },
        next,
        ready,
        building: slices
            .iter()
            .filter(|s| is(s, "building"))
            .map(|s| Building {
                number: s.number,
                file: s.file.clone(),
                lane: s.lane,
            })
            .collect(),
        lanes,
        free_lanes,
        blocked: slices
            .iter()
            .filter(|s| is(s, "blocked"))
            .map(|s| s.number)
            .collect(),
        open_blocks,
        open_opens: open_opens(text).into_iter().map(Value::from).collect(),
        checkpoints,
        unverified,
        log: summary,
        retro: Value::Null,
        branch: branch(&board.root),
        slices,
    })
}

fn branch(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `^#{2,3}\s+(?:\d+\.\s*)?QA plan\b`, case-insensitive (`docs.mjs:193`).
pub fn has_qa_plan(text: &str) -> bool {
    text.lines().any(|l| {
        let hashes = l.len() - l.trim_start_matches('#').len();
        if !(2..=3).contains(&hashes) {
            return false;
        }
        let rest = &l[hashes..];
        let Some(rest) = rest.strip_prefix(char::is_whitespace) else {
            return false;
        };
        let rest = rest.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let rest = match rest[digits..].strip_prefix('.') {
            Some(r) if digits > 0 => r.trim_start(),
            _ => rest,
        };
        rest.get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case("qa plan"))
            && !rest[7..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// `^\*\*Harness\*\*:\s*(.+)$` (`docs.mjs:194`).
pub fn harness_line(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let rest = l.strip_prefix("**Harness**:")?.trim_start();
        (!rest.is_empty()).then(|| rest.to_string())
    })
}

/// `openOpens` (`docs.mjs:238-254`): decision-log bullets whose OPEN is not
/// settled, by strike-through or a bold verdict word.
pub fn open_opens(text: &str) -> Vec<u32> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<u32> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some((struck, n)) = open_bullet(line) else {
            continue;
        };
        let mut block = line.to_string();
        for l in &lines[i + 1..] {
            if l.trim().is_empty() || is_bullet(l) || l.starts_with('#') {
                break;
            }
            block.push(' ');
            block.push_str(l);
        }
        let tag = format!("OPEN-{n}");
        let after = &block[block.find(&tag).unwrap_or(0)..];
        if !struck && !settled(&strip_question(after)) && !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// `^\s*[-*]\s` .
fn is_bullet(l: &str) -> bool {
    let t = l.trim_start();
    (t.starts_with('-') || t.starts_with('*')) && t[1..].starts_with(char::is_whitespace)
}

/// `^\s*[-*]\s+(~~)?\*\*(~~)?OPEN-(\d+)` → (struck through, n).
fn open_bullet(line: &str) -> Option<(bool, u32)> {
    if !is_bullet(line) {
        return None;
    }
    let rest = line.trim_start()[1..].trim_start();
    let (a, rest) = match rest.strip_prefix("~~") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let rest = rest.strip_prefix("**")?;
    let (b, rest) = match rest.strip_prefix("~~") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let rest = rest.strip_prefix("OPEN-")?;
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    Some((a || b, rest[..digits].parse().ok()?))
}

/// `after.replace(/^OPEN-\d+[^*]*\*\*/, '**')`: drop the bold question so a
/// verdict word inside it does not count.
fn strip_question(after: &str) -> String {
    let Some(rest) = after.strip_prefix("OPEN-") else {
        return after.to_string();
    };
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return after.to_string();
    }
    let tail = &rest[digits..];
    match tail.find('*') {
        Some(i) if tail[i..].starts_with("**") => format!("**{}", &tail[i + 2..]),
        _ => after.to_string(),
    }
}

const VERDICTS: [&str; 11] = [
    "closed",
    "decided",
    "settled",
    "resolved",
    "superseded",
    "cerrado",
    "cerrada",
    "decidido",
    "decidida",
    "resuelto",
    "resuelta",
];

/// `SETTLED` (`docs.mjs:238`): `\*\*[^*]{0,25}\b(verdict)\b`, case-insensitive.
fn settled(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    for p in 0..chars.len().saturating_sub(1) {
        if chars[p] != '*' || chars[p + 1] != '*' {
            continue;
        }
        let seg: Vec<char> = chars[p + 2..]
            .iter()
            .take_while(|c| **c != '*')
            .copied()
            .collect();
        for k in 0..=seg.len().min(25) {
            if k > 0 && word(seg[k - 1]) {
                continue;
            }
            let lower: String = seg[k..].iter().collect::<String>().to_lowercase();
            if VERDICTS
                .iter()
                .any(|v| lower.starts_with(v) && !lower[v.len()..].starts_with(|c: char| word(c)))
            {
                return true;
            }
        }
    }
    false
}

/// `docs status --json` with no feature: `{ framework, live, archived }`.
pub fn summary_json(board: &Board) -> Value {
    let brief = |f: &Feature| {
        let slices = f.board_slices();
        let count = |st: &str| {
            slices
                .iter()
                .filter(|s| s.status.as_deref() == Some(st))
                .count()
        };
        json!({
            "number": f.number,
            "slug": f.slug,
            "status": f.status(),
            "slices": slices.len(),
            "done": count("done"),
            "blocked": count("blocked"),
        })
    };
    json!({
        "framework": board.framework(),
        "live": board.live.iter().map(brief).collect::<Vec<_>>(),
        "archived": board.archived.iter().map(brief).collect::<Vec<_>>(),
    })
}

/// `docs status` with no feature: one line per live feature and its summary
/// under it, then the archive count and the retired numbers.
pub fn print_summary(board: &Board, out: &mut dyn Write) -> Result<()> {
    let mut text = String::new();
    if board.live.is_empty() {
        text.push_str("no live features\n");
    }
    for f in &board.live {
        let slices = f.board_slices();
        let count = |st: &str| {
            slices
                .iter()
                .filter(|s| s.status.as_deref() == Some(st))
                .count()
        };
        let mut line = format!(
            "{:03}  {:<12} {}",
            f.number,
            f.status().unwrap_or_else(|| "?".into()),
            f.slug
        );
        if !slices.is_empty() {
            line.push_str(&format!(
                " · slices {}/{} done",
                count("done"),
                slices.len()
            ));
            if count("blocked") > 0 {
                line.push_str(&format!(", {} blocked", count("blocked")));
            }
        }
        if let Err(e) = &f.state {
            line.push_str(&format!(" · state.toml unreadable: {e}"));
        }
        text.push_str(&line);
        text.push('\n');
        for l in f.summary().iter().flat_map(|s| s.lines()) {
            if !l.is_empty() {
                text.push_str("     ");
            }
            text.push_str(l);
            text.push('\n');
        }
    }
    let retired: Vec<String> = board
        .archived
        .iter()
        .filter(|f| f.status().as_deref() == Some("retired"))
        .map(|f| format!("{:03}", f.number))
        .collect();
    text.push_str(&format!(
        "\narchived: {} · retired: {}\n",
        board.archived.len(),
        if retired.is_empty() {
            "none".into()
        } else {
            retired.join(", ")
        }
    ));
    out.write_all(text.as_bytes()).map_err(io("write stdout"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_lose_their_slice_prefix() {
        assert_eq!(
            slice_title("---\n---\n# Slice 03 — C thing\n", "f.md"),
            "C thing"
        );
        assert_eq!(slice_title("# slice 3 - x\n", "f.md"), "x");
        assert_eq!(slice_title("# Other\n", "f.md"), "Other");
        assert_eq!(slice_title("no heading\n", "f.md"), "f.md");
    }

    #[test]
    fn the_prd_regexes_match_as_docs_mjs_does() {
        assert!(has_qa_plan("x\n## 12. QA plan\n"));
        assert!(has_qa_plan("### qa Plan\n"));
        assert!(!has_qa_plan("# QA plan\n"));
        assert!(!has_qa_plan("## QA planning\n"));
        assert_eq!(
            harness_line("**Harness**:  none — gate-only\n").as_deref(),
            Some("none — gate-only")
        );
        assert_eq!(harness_line("**Harness**:\n"), None);
    }

    #[test]
    fn open_opens_skips_settled_and_struck_bullets() {
        let text = "\
- **OPEN-1** — still open? Options.
- ~~**OPEN-2** — struck~~
- **~~OPEN-3~~ — struck inside**
- **OPEN-4 — is it closed?** **Closed 2026-09-11:** yes.
- **OPEN-5 — q?**
  **Decided 2026-10-11:** on the next line.
- **OPEN-6 — closed in the question only?** still open.
- **OPEN-1** — again
Inline OPEN-7 mention.
";
        assert_eq!(open_opens(text), vec![1, 6]);
    }

    #[test]
    fn folder_names_need_three_digits() {
        assert_eq!(folder_name("001-fixture"), Some((1, "fixture")));
        assert_eq!(folder_name("01-x"), None);
        assert_eq!(folder_name("001-"), None);
        assert_eq!(folder_name("abc-x"), None);
    }
}
