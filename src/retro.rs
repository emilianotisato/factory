//! PRD-003 FR-25: the `## Numbers` section of a feature's `retro.md`,
//! between markers the driver owns. [`numbers`] builds it from the store
//! (the per-role table, the knobs, the quality line, the builders' pack and
//! the same repo's earlier runs); [`write`] puts it in the file and touches
//! nothing outside the markers, so a rerun on the same store is a no-op.
//! The driver writes it at ship; `factory retro <run>` regenerates it.
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
/// What the comparison says when the repo has no earlier finished run.
pub const NO_EARLIER: &str = "No earlier finished run in this repo.";
/// How many earlier runs the comparison names.
const COHORT: u32 = 3;

/// `(insertions, deletions)` of a `git diff --shortstat` line: either part
/// may be absent, and an empty diff prints nothing.
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

/// `git diff --shortstat <base>..HEAD` in `repo`.
pub fn changed_lines(repo: &Path, base: &str) -> Result<(u64, u64)> {
    let out = ci::git(repo, &["diff", "--shortstat", &format!("{base}..HEAD")])?;
    if !out.ok() {
        return Err(Error::Msg(format!(
            "git diff --shortstat {base}..HEAD: {}",
            out.text()
        )));
    }
    Ok(shortstat(&out.stdout))
}

/// One role's row of the table, summed over its workers.
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

/// The per-role rows from `--per-worker`'s figures, so a role's `weighted`
/// is exactly the sum of its rows' `weighted_own + weighted_sub`.
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

/// One comparison row.
fn compare_row(db: &Db, run: &RunRow, this: bool) -> Result<String> {
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
    let name = if this {
        format!("{} (this run)", run.id)
    } else {
        run.id.clone()
    };
    Ok(format!(
        "| {name} | {} | {weighted} | {slices} | {} | {} | {} |",
        run.feature,
        per(Some(slices)),
        dash(lines),
        per(lines)
    ))
}

/// The section between the markers, ending in a newline. Nothing in it
/// varies between two calls on the same store: no time, no version but the
/// run's own.
pub fn numbers(db: &Db, run_id: &str) -> Result<String> {
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
    // The total is the run's `weighted_tokens`, rounded once over the
    // unrounded sum as the report and the budget read it; the role rows,
    // each a sum of rounded workers, can add up to a few tokens more.
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

    let earlier = db.earlier_runs(&run.repo, &run.started_at, &run.id, COHORT)?;
    if earlier.is_empty() {
        s.push_str(&format!("Compared with: {NO_EARLIER}\n"));
    } else {
        s.push_str("Compared with:\n\n");
        s.push_str(
            "| run | feature | weighted | slices | per slice | lines changed | per line |\n",
        );
        s.push_str("|---|---|---:|---:|---:|---:|---:|\n");
        s.push_str(&compare_row(db, &run, true)?);
        s.push('\n');
        for e in &earlier {
            s.push_str(&compare_row(db, e, false)?);
            s.push('\n');
        }
    }
    Ok(s)
}

/// Put `section` between the markers in `retro`, replacing what is there,
/// or append `## Numbers` and the markers at the end of the file. Every
/// byte outside the markers stays. `true` when the file changed.
pub fn write(retro: &Path, section: &str) -> Result<bool> {
    let text = fs::read_to_string(retro).map_err(io(format!("read {}", retro.display())))?;
    let open = text.find(OPEN);
    let close = open.and_then(|o| text[o..].find(CLOSE).map(|c| o + c));
    let out = match (open, close) {
        (Some(o), Some(c)) => format!("{}{OPEN}\n{section}{}", &text[..o], &text[c..]),
        _ => {
            let mut t = text.clone();
            if !t.is_empty() && !t.ends_with('\n') {
                t.push('\n');
            }
            t.push_str(&format!("\n## Numbers\n\n{OPEN}\n{section}{CLOSE}\n"));
            t
        }
    };
    if out == text {
        return Ok(false);
    }
    write_atomic(retro, &out)?;
    Ok(true)
}

/// The feature folder of a stored run: `docs status NNN`'s `dir` in the
/// run's repo, else the one `docs/features/<NNN>-*` folder there.
pub fn feature_dir(repo: &Path, feature: &str) -> Result<PathBuf> {
    if let Some(dir) = Docs::locate(repo)
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

    fn insert(db: &Db, id: &str, repo: &str, started: &str, finished: Option<&str>) {
        db.conn()
            .execute(
                "INSERT INTO runs (id, repo, feature, started_at, finished_at, phase, budget_weighted_tokens)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
                rusqlite::params![
                    id,
                    repo,
                    &id[..3],
                    started,
                    finished,
                    if finished.is_some() { "finished" } else { "building" }
                ],
            )
            .unwrap();
    }

    #[test]
    fn numbers_compares_with_the_three_newest_earlier_runs_of_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("factory.db")).unwrap();
        insert(
            &db,
            "001-20261001-aaaa",
            "/r",
            "2026-10-01T00:00:00Z",
            Some("2026-10-01T01:00:00Z"),
        );
        let alone = numbers(&db, "001-20261001-aaaa").unwrap();
        assert!(
            alone.contains(&format!("Compared with: {NO_EARLIER}\n")),
            "{alone}"
        );

        insert(
            &db,
            "002-20261002-bbbb",
            "/r",
            "2026-10-02T00:00:00Z",
            Some("2026-10-02T01:00:00Z"),
        );
        insert(
            &db,
            "003-20261003-cccc",
            "/r",
            "2026-10-03T00:00:00Z",
            Some("2026-10-03T01:00:00Z"),
        );
        insert(
            &db,
            "004-20261004-dddd",
            "/r",
            "2026-10-04T00:00:00Z",
            Some("2026-10-04T01:00:00Z"),
        );
        // Another repo, a run not finished, and one that finished after this
        // run started: none of them is in the cohort.
        insert(
            &db,
            "005-20261004-eeee",
            "/other",
            "2026-10-04T02:00:00Z",
            Some("2026-10-04T03:00:00Z"),
        );
        insert(&db, "006-20261005-ffff", "/r", "2026-10-05T00:00:00Z", None);
        insert(
            &db,
            "007-20261006-gggg",
            "/r",
            "2026-10-06T00:00:00Z",
            Some("2026-10-07T09:00:00Z"),
        );
        insert(
            &db,
            "008-20261007-hhhh",
            "/r",
            "2026-10-07T00:00:00Z",
            Some("2026-10-07T05:00:00Z"),
        );
        let s = numbers(&db, "008-20261007-hhhh").unwrap();
        let rows: Vec<&str> = s
            .lines()
            .filter(|l| l.starts_with("| 0"))
            .map(|l| &l[2..19])
            .collect();
        assert_eq!(
            rows,
            [
                "008-20261007-hhhh",
                "004-20261004-dddd",
                "003-20261003-cccc",
                "002-20261002-bbbb"
            ],
            "{s}"
        );
        assert!(s.contains("| 008-20261007-hhhh (this run) | 008 |"), "{s}");
        assert!(!s.contains(NO_EARLIER), "{s}");
        assert_eq!(s, numbers(&db, "008-20261007-hhhh").unwrap());
    }
}
