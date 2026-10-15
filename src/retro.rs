//! The `## Numbers` section of a feature's `retro.md`, between markers the driver owns.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::report::{self, ROLE_ORDER};
use crate::db::{Db, RunRow};
use crate::docs::Docs;
use crate::error::io;
use crate::scheduler::write_atomic;
use crate::{Error, Result, ci, quota};

pub const OPEN: &str = "<!-- factory:numbers -->";
pub const CLOSE: &str = "<!-- /factory:numbers -->";
pub const NO_EARLIER: &str = "No earlier retro in this repo.";
const COHORT: usize = 3;
const THIS_RUN: &str = " (this run)";

/// `(insertions, deletions)` of a `git diff --shortstat` line; either part may be absent.
pub fn shortstat(text: &str) -> (u64, u64) {
    let mut out = (0, 0);
    for part in text.trim().split(',') {
        let mut words = part.split_whitespace();
        let (Some(n), Some(what)) = (words.next(), words.next()) else {
            continue;
        };
        let Ok(n) = n.parse::<u64>() else { continue };
        if what.starts_with("insertion") {
            out.0 = n;
        } else if what.starts_with("deletion") {
            out.1 = n;
        }
    }
    out
}

/// Three dots: commits the base gained since the branch left it do not count.
pub fn changed_lines(repo: &Path, base: &str) -> Result<(u64, u64)> {
    let out = ci::git(repo, &["diff", "--shortstat", &format!("{base}...HEAD")])?;
    if !out.ok() {
        return Err(Error::Msg(format!(
            "git diff --shortstat {base}...HEAD: {}",
            out.text()
        )));
    }
    Ok(shortstat(&out.stdout))
}

#[derive(Default)]
struct RoleRow {
    workers: u64,
    own: u64,
    /// `None` while no worker of the role had its subagents read.
    sub: Option<u64>,
    ms: Option<u64>,
    turns: Option<u64>,
}

impl RoleRow {
    fn weighted(&self) -> u64 {
        self.own + self.sub.unwrap_or(0)
    }

    fn add(&mut self, other: &RoleRow) {
        self.workers += other.workers;
        self.own += other.own;
        self.sub = add_opt(self.sub, other.sub);
        self.ms = add_opt(self.ms, other.ms);
        self.turns = add_opt(self.turns, other.turns);
    }

    fn cells(&self, name: &str, weighted: u64) -> String {
        let minutes = self.ms.map(|ms| (ms as f64 / 60_000.0).round() as u64);
        format!(
            "| {name} | {} | {} | {} | {weighted} | {} | {} |",
            self.workers,
            self.own,
            dash(self.sub),
            dash(minutes),
            dash(self.turns)
        )
    }
}

fn add_opt(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    }
}

fn dash<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "—".to_string(), |n| n.to_string())
}

fn role_rows(db: &Db, run_id: &str) -> Result<BTreeMap<String, RoleRow>> {
    let workers = db.workers(run_id)?;
    let per = report::per_worker(&workers, &[]);
    let mut rows: BTreeMap<String, RoleRow> = BTreeMap::new();
    for (w, p) in workers.iter().zip(&per) {
        let n = |v: Option<i64>| v.map(|v| v.max(0) as u64);
        let one = RoleRow {
            workers: 1,
            own: p.weighted_own,
            sub: p.weighted_sub,
            ms: n(w.duration_ms),
            turns: n(w.num_turns),
        };
        rows.entry(p.role.clone()).or_default().add(&one);
    }
    Ok(rows)
}

fn slices_of(db: &Db, run_id: &str) -> Result<u64> {
    Ok(db
        .events(run_id)?
        .iter()
        .filter(|e| e.kind == "slice_done")
        .count() as u64)
}

fn compare_row(db: &Db, run: &RunRow) -> Result<String> {
    let weighted = quota::weighted(db, &run.id)?;
    let slices = slices_of(db, &run.id)?;
    let lines = match (run.lines_added, run.lines_removed) {
        (Some(a), Some(d)) => Some(a + d),
        _ => None,
    };
    let per = |den: Option<u64>| match den {
        Some(d) if d > 0 => (weighted as f64 / d as f64).round().to_string(),
        _ => "—".to_string(),
    };
    let name = format!("{}{THIS_RUN}", run.id);
    let share = match (run.comment_lines, run.nonblank_lines) {
        (Some(c), Some(n)) if n > 0 => format!("{:.1}%", c as f64 * 100.0 / n as f64),
        _ => "—".to_string(),
    };
    Ok(format!(
        "| {name} | {} | {weighted} | {slices} | {} | {} | {} | {share} | {} |",
        run.feature,
        per(Some(slices)),
        dash(lines),
        per(lines),
        dash(run.id_hits)
    ))
}

/// Deterministic on the same store and tree: no time, no version but the run's own.
pub fn numbers(db: &Db, run_id: &str, root: &Path) -> Result<String> {
    let run = db
        .run_row(run_id)?
        .ok_or_else(|| Error::Msg(format!("no run {run_id} in the store")))?;
    let r = report::compute(db, run_id, None)?;
    let mut s = String::new();

    s.push_str("| role | workers | own | subagents | weighted | minutes | turns |\n");
    s.push_str("|---|---:|---:|---:|---:|---:|---:|\n");
    let rows = role_rows(db, run_id)?;
    let known = ROLE_ORDER.iter().filter_map(|r| rows.get_key_value(*r));
    let other = rows
        .iter()
        .filter(|(r, _)| !ROLE_ORDER.contains(&r.as_str()));
    let mut total = RoleRow::default();
    for (role, row) in known.chain(other) {
        s.push_str(&row.cells(role, row.weighted()));
        s.push('\n');
        total.add(row);
    }
    // Rounded once over the unrounded sum, as the report and the budget read it.
    let workers = db.workers(run_id)?;
    total.own = workers.iter().map(quota::weigh_own).sum::<f64>().round() as u64;
    if total.sub.is_some() {
        total.sub = Some(workers.iter().map(quota::weigh_sub).sum::<f64>().round() as u64);
    }
    s.push_str(&total.cells("total", r.weighted_tokens));
    s.push_str("\n\n");

    s.push_str(&format!(
        "Knobs: {}\n\n",
        report::knobs_text(r.knobs.as_ref())
    ));
    s.push_str(&format!(
        "Hypothesis: {}\n\n",
        r.hypothesis.as_deref().unwrap_or("none")
    ));
    s.push_str(&r.quality_line());
    s.push_str("\n\n");

    let pack = match r.pack_by_role.get("build") {
        Some(Some(p)) => {
            let mut parts: Vec<String> =
                p.sections.iter().map(|(s, n)| format!("{s} {n}")).collect();
            parts.push(format!("total {}", p.total));
            parts.push(format!("start_context {}", dash(p.start_context)));
            parts.join(" · ")
        }
        _ => "—".to_string(),
    };
    s.push_str(&format!("Pack (build): {pack}\n\n"));

    let earlier = tracked_rows(root, &run.feature)?;
    if earlier.is_empty() {
        s.push_str(&format!("Compared with: {NO_EARLIER}\n"));
    } else {
        s.push_str("Compared with:\n\n");
        s.push_str(
            "| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |\n",
        );
        s.push_str("|---|---|---:|---:|---:|---:|---:|---:|---:|\n");
        s.push_str(&compare_row(db, &run)?);
        s.push('\n');
        for row in &earlier {
            s.push_str(row);
            s.push('\n');
        }
    }
    Ok(s)
}

/// The `(this run)` row of each retro git tracks under `root`'s features and archive but `feature`'s own,
/// newest run date first, then the higher feature number; copied, not recounted.
fn tracked_rows(root: &Path, feature: &str) -> Result<Vec<String>> {
    let number = |name: &str| name.split('-').next().and_then(|n| n.parse::<u32>().ok());
    let own = number(feature);
    let listed = crate::docs::preflight::git(
        root,
        &[
            "ls-files",
            "-z",
            "--",
            "docs/features/*/retro.md",
            "docs/archive/*/retro.md",
        ],
    )
    .unwrap_or_default();
    let mut rows = Vec::new();
    for rel in listed.split('\0') {
        let ["docs", _, folder, "retro.md"] = rel.split('/').collect::<Vec<_>>()[..] else {
            continue;
        };
        let Some(n) = number(folder).filter(|n| Some(*n) != own) else {
            continue;
        };
        // Another feature's unreadable retro must not cost this one its Numbers.
        let Ok(text) = fs::read_to_string(root.join(rel)) else {
            continue;
        };
        if let Some(row) = this_run_row(&text) {
            let date = row
                .trim_start_matches(['|', ' '])
                .split(['-', ' '])
                .nth(1)
                .unwrap_or("")
                .to_string();
            rows.push((date, n, row));
        }
    }
    rows.sort_by(|a, b| (&b.0, b.1).cmp(&(&a.0, a.1)));
    Ok(rows.into_iter().take(COHORT).map(|r| r.2).collect())
}

/// Only between the markers: a retro's prose may hold other tables.
fn this_run_row(retro: &str) -> Option<String> {
    let from = retro.find(OPEN)? + OPEN.len();
    let to = from + retro[from..].find(CLOSE)?;
    let line = retro[from..to].lines().find(|l| l.contains(THIS_RUN))?;
    let row = line.replacen(THIS_RUN, "", 1).trim().to_string();
    // Seven cells: written before the comment-share and id-hits columns existed.
    if row.matches('|').count() == 8 {
        Some(format!("{row} — | — |"))
    } else {
        Some(row)
    }
}

/// A stray marker is refused: the next write would replace the prose up to it.
pub fn write(retro: &Path, section: &str) -> Result<bool> {
    let text = fs::read_to_string(retro).map_err(io(format!("read {}", retro.display())))?;
    let (opens, closes) = (text.matches(OPEN).count(), text.matches(CLOSE).count());
    let open = text.find(OPEN);
    let close = text.find(CLOSE);
    let out = match (open, close) {
        (Some(o), Some(c)) if opens == 1 && closes == 1 && o < c => {
            format!("{}{OPEN}\n{section}{}", &text[..o], &text[c..])
        }
        (None, None) => {
            let mut t = text.clone();
            if !t.is_empty() && !t.ends_with('\n') {
                t.push('\n');
            }
            t.push_str(&format!("\n## Numbers\n\n{OPEN}\n{section}{CLOSE}\n"));
            t
        }
        _ => {
            return Err(Error::Msg(format!(
                "{}: {opens} `{OPEN}` and {closes} `{CLOSE}` marker(s), not one pair in order; nothing written",
                retro.display()
            )));
        }
    };
    if out == text {
        return Ok(false);
    }
    write_atomic(retro, &out)?;
    Ok(true)
}

pub fn feature_dir(repo: &Path, feature: &str) -> Result<PathBuf> {
    if let Some(dir) = Docs::open(repo)
        .ok()
        .and_then(|d| d.status(feature).ok())
        .map(|s| repo.join(s.dir))
        .filter(|d| d.is_dir())
    {
        return Ok(dir);
    }
    let features = repo.join("docs/features");
    let prefix = format!("{feature}-");
    let mut found: Vec<PathBuf> = fs::read_dir(&features)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(Error::Msg(format!(
            "no folder for feature {feature}: `docs status {feature}` does not name one and nothing matches {}/{prefix}*",
            features.display()
        ))),
        _ => Err(Error::Msg(format!(
            "feature {feature} matches {} folders under {}",
            found.len(),
            features.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortstat_reads_either_part_or_nothing() {
        assert_eq!(
            shortstat(" 3 files changed, 10 insertions(+), 2 deletions(-)\n"),
            (10, 2)
        );
        assert_eq!(shortstat(" 1 file changed, 4 deletions(-)"), (0, 4));
        assert_eq!(shortstat(" 1 file changed, 1 insertion(+)"), (1, 0));
        assert_eq!(shortstat(""), (0, 0));
    }

    const PROSE: &str = "---\nscore:\n---\n\n# Retro\n\n## Cost vs catch\n\n| a | b |\n";

    #[test]
    fn write_appends_once_then_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("retro.md");
        fs::write(&p, PROSE).unwrap();
        assert!(write(&p, "one\n").unwrap());
        let first = fs::read(&p).unwrap();
        assert_eq!(
            String::from_utf8(first.clone()).unwrap(),
            format!("{PROSE}\n## Numbers\n\n{OPEN}\none\n{CLOSE}\n")
        );
        assert!(!write(&p, "one\n").unwrap());
        assert_eq!(fs::read(&p).unwrap(), first);
    }

    #[test]
    fn write_replaces_only_the_span_between_the_markers() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("retro.md");
        let before = "# Retro\n\nprose above\n\n## Numbers\n\n";
        let after = "\n\nprose below, no final newline";
        fs::write(&p, format!("{before}{OPEN}\nstale\nrows\n{CLOSE}{after}")).unwrap();
        assert!(write(&p, "fresh\n").unwrap());
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            format!("{before}{OPEN}\nfresh\n{CLOSE}{after}")
        );
        assert!(!write(&p, "fresh\n").unwrap());
    }

    #[test]
    fn write_refuses_markers_that_are_not_one_pair_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("retro.md");
        for text in [
            format!("{OPEN}\nprose the owner kept\n"),
            format!("{CLOSE}\nx\n{OPEN}\n"),
            format!("{OPEN}\na\n{CLOSE}\n{OPEN}\nb\n{CLOSE}\n"),
            format!("prose\n{CLOSE}\n"),
        ] {
            fs::write(&p, &text).unwrap();
            assert!(write(&p, "fresh\n").is_err(), "{text}");
            assert_eq!(fs::read_to_string(&p).unwrap(), text);
        }
    }

    fn tracked(root: &Path, dir: &str, body: impl AsRef<[u8]>) {
        let d = root.join(dir);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("retro.md"), body).unwrap();
        for args in [&["init", "-q"][..], &["add", "--", dir]] {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        }
    }

    fn numbers_block(rows: &str) -> String {
        format!("# Retro\n\n## Numbers\n\n{OPEN}\nCompared with:\n\n{rows}{CLOSE}\n")
    }

    #[test]
    fn a_seven_cell_row_is_padded_and_a_nine_cell_row_copied() {
        let dir = tempfile::tempdir().unwrap();
        tracked(
            dir.path(),
            "docs/features/002-old",
            numbers_block("| 002-20261013-e729 (this run) | 002 | 15 | 11 | 1 | 24 | 6 |\n"),
        );
        tracked(
            dir.path(),
            "docs/archive/007-new",
            numbers_block(
                "| 007-20261015-c8fa (this run) | 007 | 4 | 5 | 1 | 3 | 1 | 4.0% | 0 |\n| 002-20261013-e729 | 002 | 15 | 11 | 1 | 24 | 6 | — | — |\n",
            ),
        );
        assert_eq!(
            tracked_rows(dir.path(), "008").unwrap(),
            [
                "| 007-20261015-c8fa | 007 | 4 | 5 | 1 | 3 | 1 | 4.0% | 0 |",
                "| 002-20261013-e729 | 002 | 15 | 11 | 1 | 24 | 6 | — | — |",
            ]
        );
    }

    #[test]
    fn a_retro_without_a_row_between_the_markers_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        tracked(
            dir.path(),
            "docs/features/001-prose",
            format!(
                "| 001-20261010-aaaa (this run) | report |\n\n{OPEN}\nCompared with: {NO_EARLIER}\n{CLOSE}\n"
            ),
        );
        tracked(dir.path(), "docs/features/003-none", "# Retro\n");
        tracked(
            dir.path(),
            "docs/features/004-latin1",
            b"# Retro\n\ncaf\xe9\n",
        );
        let draft = dir.path().join("docs/features/009-draft");
        fs::create_dir_all(&draft).unwrap();
        fs::write(
            draft.join("retro.md"),
            numbers_block("| 009-20261020-dddd (this run) | 009 | 1 | 1 | 1 | 1 | 1 | — | — |\n"),
        )
        .unwrap();
        assert!(tracked_rows(dir.path(), "008").unwrap().is_empty());
        assert!(
            tracked_rows(&dir.path().join("missing"), "008")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn the_newest_three_win_a_tie_goes_to_the_higher_feature_and_the_own_is_left_out() {
        let dir = tempfile::tempdir().unwrap();
        for (dir_name, id) in [
            ("004-a", "004-20261013-0004"),
            ("005-b", "005-20261014-0005"),
            ("006-c", "006-20261014-0006"),
            ("002-d", "002-20261013-0002"),
            ("008-own", "008-20261016-0008"),
        ] {
            tracked(
                dir.path(),
                &format!("docs/features/{dir_name}"),
                numbers_block(&format!(
                    "| {id} (this run) | x | 1 | 1 | 1 | 1 | 1 | 1% | 0 |\n"
                )),
            );
        }
        let ids: Vec<String> = tracked_rows(dir.path(), "008")
            .unwrap()
            .iter()
            .map(|r| r[2..19].to_string())
            .collect();
        assert_eq!(
            ids,
            [
                "006-20261014-0006",
                "005-20261014-0005",
                "004-20261013-0004"
            ]
        );
    }
}
