//! Slice 05 of PRD-001: commits by pathspec and log merge (PRD §7 FR-5, §4
//! FR-2). A `done` slice is committed from the paths its worker's stream
//! named plus its slice file and fragment — never `git add -A`, so two lanes
//! committing out of order each take only their own work; what the worker
//! wrote without naming it is listed in the fragment and left in the tree.
//! When the lanes drain, the fragments are merged into `agent-log.md` and
//! committed; a refused merge is a `BLOCK-n` the run parks on until the owner
//! fixes the fragment and answers.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::db::Db;
use factory::run::pid_alive;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const FEATURE_DIR: &str = "docs/features/001-fixture";
const AGENT_LOG: &str = "docs/features/001-fixture/agent-log.md";

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN).args(args).output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn data_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn only_run_dir(fx: &Fixture) -> PathBuf {
    let mut v: Vec<PathBuf> = fs::read_dir(data_dir(fx).join("runs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    v.sort();
    assert_eq!(v.len(), 1, "{v:?}");
    v.pop().unwrap()
}

fn run_id(dir: &Path) -> String {
    dir.file_name().unwrap().to_str().unwrap().to_string()
}

fn state(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn events(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect(l))
        .collect()
}

fn kinds(dir: &Path) -> Vec<String> {
    events(dir)
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect()
}

fn count(kinds: &[String], kind: &str) -> usize {
    kinds.iter().filter(|k| *k == kind).count()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

fn driver_log(dir: &Path) -> String {
    fs::read_to_string(dir.join("driver.log")).unwrap_or_default()
}

/// Polls `ok`; on timeout, dumps the run so a failure in the detached
/// driver can be read from the test output.
fn wait_until(fx: &Fixture, dir: &Path, what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        if Instant::now() >= deadline {
            let statuses: Vec<String> = (1..=3)
                .map(|n| format!("{n:02}={}", fx.slice_status(n)))
                .collect();
            panic!(
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- slices {statuses:?}\n--- git log\n{}\n--- driver.log\n{}",
                state(dir),
                kinds(dir),
                fx.git(&["log", "--format=%h %s"]),
                driver_log(dir)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_phase(fx: &Fixture, dir: &Path, phase: &str, secs: u64) {
    wait_until(fx, dir, &format!("phase {phase}"), secs, || {
        state(dir)["phase"] == phase
    });
}

fn seq_first(dir: &Path, kind: &str) -> u64 {
    events(dir)
        .iter()
        .find(|e| e["kind"] == kind)
        .map(|e| e["seq"].as_u64().unwrap())
        .unwrap_or_else(|| panic!("no {kind}"))
}

fn start(fx: &Fixture) -> PathBuf {
    let out = factory(fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    only_run_dir(fx)
}

/// Subjects, oldest first.
fn subjects(fx: &Fixture) -> Vec<String> {
    fx.git(&["log", "--reverse", "--format=%s"])
        .lines()
        .map(str::to_string)
        .collect()
}

fn sha_of(fx: &Fixture, subject: &str) -> String {
    let out = fx.git(&["log", "--format=%H", "--grep", &format!("^{subject}$")]);
    let shas: Vec<&str> = out.lines().collect();
    assert_eq!(shas.len(), 1, "commits titled {subject:?}: {shas:?}");
    shas[0].to_string()
}

/// The files a commit touches, sorted.
fn files_of(fx: &Fixture, sha: &str) -> Vec<String> {
    let mut v: Vec<String> = fx
        .git(&["show", "--format=", "--name-only", sha])
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    v.sort();
    v
}

fn slice_rel(fx: &Fixture, n: u8) -> String {
    fx.slice_path(n)
        .strip_prefix(fx.path())
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn fragment(n: u8) -> String {
    format!("{FEATURE_DIR}/log-slice-{n:02}.md")
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// One slice's commit as FR-5 asks for it: `feat: <title>`, no body, the
/// repo's own author, exactly the named paths plus slice file and fragment;
/// the `slice_committed` event and the worker row name the same sha.
fn assert_slice_commit(fx: &Fixture, dir: &Path, n: u8, title: &str, code: &[&str]) -> String {
    let sha = sha_of(fx, &format!("feat: {title}"));
    assert_eq!(
        fx.git(&["log", "-1", "--format=%b", &sha]),
        "\n",
        "{title}: body"
    );
    assert_eq!(
        fx.git(&["log", "-1", "--format=%an <%ae>", &sha]).trim(),
        "Fixture <fixture@example.invalid>"
    );
    let mut want: Vec<String> = code.iter().map(|s| s.to_string()).collect();
    want.push(slice_rel(fx, n));
    want.push(fragment(n));
    assert_eq!(files_of(fx, &sha), sorted(want.clone()), "{title}: files");
    let ev = events(dir)
        .into_iter()
        .find(|e| e["kind"] == "slice_committed" && e["slice"] == u64::from(n))
        .unwrap_or_else(|| panic!("no slice_committed for slice {n}"));
    assert_eq!(ev["sha"], sha);
    let named: Vec<String> = ev["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(sorted(named), sorted(want));
    let rows = db(fx).workers(&run_id(dir)).unwrap();
    let row = rows
        .iter()
        .find(|r| r.slice == Some(u32::from(n)) && r.outcome.as_deref() == Some("done"))
        .unwrap_or_else(|| panic!("no done row for slice {n}: {rows:?}"));
    assert_eq!(row.commit_sha.as_deref(), Some(sha.as_str()));
    sha
}

// ── tasks 1, 2 and 4: one lane, three commits, leftovers, merge ─────────────

#[test]
fn each_done_slice_is_committed_by_pathspec_and_the_logs_merge_at_the_end() {
    let fx = Fixture::with_index("commits-chain", &[("checkpoint_every", "10")]);
    fx.chain();
    // Slice 01 also leaves a file no tool call names (a heredoc, a generator):
    // it is listed in the fragment and never committed (FR-5).
    fx.write_plan(&[
        json!({"outcome": "done", "touch": ["src/lib.txt"], "stray": ["notes.txt"]}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    assert_eq!(
        subjects(&fx),
        [
            "chore: fixture repo",
            "chore: chain the fixture slices",
            "feat: A",
            "feat: B",
            "feat: C",
            "docs: checkpoint 1"
        ]
    );
    assert_slice_commit(&fx, &dir, 1, "A", &["src/lib.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/lib.txt"]);
    assert_slice_commit(&fx, &dir, 3, "C", &["src/lib.txt"]);

    // The stray file: on disk, in no commit, named in the (now merged) log.
    assert!(fx.path().join("notes.txt").exists());
    assert!(
        !fx.git(&["log", "--all", "--format=", "--name-only"])
            .lines()
            .any(|l| l == "notes.txt"),
        "notes.txt was committed"
    );
    let log = fx.read(AGENT_LOG);
    assert!(log.contains("## Untracked leftovers"), "{log}");
    assert!(log.contains("- notes.txt"), "{log}");
    let leftovers: Vec<Value> = events(&dir)
        .into_iter()
        .filter(|e| e["kind"] == "untracked_leftovers")
        .collect();
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert_eq!(leftovers[0]["slice"], 1);
    assert_eq!(leftovers[0]["files"], json!(["notes.txt"]));
    for n in 1..=3 {
        assert!(
            !fx.path().join(fragment(n)).exists(),
            "fragment {n} survived the merge"
        );
    }
    for ad in ["AD-101", "AD-201", "AD-301"] {
        assert!(
            log.contains(&format!("### {ad} —")),
            "{ad} missing from\n{log}"
        );
    }

    // The merge lands in the checkpoint commit (slice 06): agent-log.md,
    // what `render --write` changed (here prd.md's slice table; the index
    // was already current) and the fragments' removal.
    let merge = fx.git(&["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(
        fx.git(&["log", "-1", "--format=%s", &merge]).trim(),
        "docs: checkpoint 1"
    );
    let merged_files = files_of(&fx, &merge);
    for f in [
        AGENT_LOG.to_string(),
        format!("{FEATURE_DIR}/prd.md"),
        fragment(1),
        fragment(2),
        fragment(3),
    ] {
        assert!(
            merged_files.contains(&f),
            "{f} not in the merge commit: {merged_files:?}"
        );
    }
    let ev = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "log_merged")
        .expect("log_merged");
    assert!(
        ev.get("sha").is_none(),
        "the sha is the checkpoint commit's (`checkpoint_done`): {ev}"
    );
    assert_eq!(
        ev["fragments"],
        json!([fragment(1), fragment(2), fragment(3)])
    );

    // The tree after: only the leftover is unaccounted for, and git agrees.
    assert_eq!(fx.git(&["status", "--porcelain"]), "?? notes.txt\n");
    fx.git(&["fsck", "--strict"]);

    let k = kinds(&dir);
    let first = k.iter().position(|x| x == "slice_spawned").unwrap();
    assert_eq!(
        k[first..],
        [
            "slice_spawned",
            "slice_done",
            "untracked_leftovers",
            "slice_committed",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_run",
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
            "run_finished",
        ]
    );
}

// ── task 3: two lanes, commits interleave ────────────────────────────────────

#[test]
fn two_lanes_commit_out_of_order_and_each_commit_takes_only_its_own_files() {
    let fx = Fixture::with_index("commits-two-lanes", &[("checkpoint_every", "10")]);
    // 01 and 02 start together; 02 finishes first and commits while 01 is
    // still writing in the same tree. 03 follows 02.
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 800, "touch": ["src/a.txt"]}),
        json!({"outcome": "done", "delay_ms": 200, "touch": ["src/b.txt"]}),
        json!({"outcome": "done", "touch": ["src/c.txt"]}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    let subjects = subjects(&fx);
    let at = |s: &str| {
        subjects
            .iter()
            .position(|x| x == s)
            .unwrap_or_else(|| panic!("{s}: {subjects:?}"))
    };
    assert!(at("feat: B") < at("feat: A"), "{subjects:?}");
    assert_eq!(
        subjects.last().map(String::as_str),
        Some("docs: checkpoint 1")
    );
    assert_eq!(subjects.len(), 5, "{subjects:?}");
    assert_slice_commit(&fx, &dir, 1, "A", &["src/a.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/b.txt"]);
    assert_slice_commit(&fx, &dir, 3, "C", &["src/c.txt"]);
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    fx.git(&["fsck", "--strict"]);
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_committed"), 3);
    assert_eq!(count(&k, "log_merged"), 1);
    assert_eq!(k.last().map(String::as_str), Some("run_finished"));
}

// ── task 4: a refused merge parks the run on a BLOCK in agent-log.md ────────

#[test]
fn a_refused_merge_blocks_in_agent_log_and_the_answer_retries_it() {
    let fx = Fixture::with_index("commits-clash", &[("checkpoint_every", "10")]);
    // Slice 02's builder ignores its range and reuses 01's AD number.
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done", "ad": "AD-101"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "waiting-owner", 40);

    // Three slice commits, a green gate, then the merge refuses (slice 06
    // runs it inside the checkpoint); the fragments are still there.
    assert_eq!(
        sorted(subjects(&fx)),
        sorted(
            ["chore: fixture repo", "feat: A", "feat: B", "feat: C"]
                .map(String::from)
                .to_vec()
        )
    );
    for n in 1..=3 {
        assert!(fx.path().join(fragment(n)).exists(), "fragment {n} gone");
    }
    let log = fx.read(AGENT_LOG);
    assert!(log.starts_with("# Agent log — PRD-001\n"), "{log}");
    let heading = "### BLOCK-1 — log merge refused: AD-101 in log-slice-02.md is already in log-slice-01.md · checkpoint · open";
    assert!(log.contains(heading), "{log}");
    assert!(log.contains("**Recommended:** 1"), "{log}");
    let refused: Vec<Value> = events(&dir)
        .into_iter()
        .filter(|e| e["kind"] == "merge_refused")
        .collect();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["block_id"], "BLOCK-1");
    assert!(
        refused[0]["detail"]
            .as_str()
            .unwrap()
            .contains("AD-101 in log-slice-02.md is already in log-slice-01.md"),
        "{:?}",
        refused[0]
    );
    let k = kinds(&dir);
    assert_eq!(k.last().map(String::as_str), Some("run_waiting"));
    assert_eq!(count(&k, "log_merged"), 0);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    assert!(pid_alive(driver), "driver gone");
    assert!(
        driver_log(&dir).contains(
            "parked (waiting-owner): checkpoint blocked — log merge refused: \
             AD-101 in log-slice-02.md is already in log-slice-01.md: BLOCK-1 in agent-log.md"
        ),
        "{}",
        driver_log(&dir)
    );
    let open = &fx.docs_json("status")["openBlocks"];
    assert_eq!(open.as_array().map(Vec::len), Some(1), "{open}");
    assert_eq!(open[0]["n"], 1);
    assert_eq!(open[0]["phase"], "checkpoint");

    // The owner renumbers the clashing entry and answers; the checkpoint
    // resumes (the green gate is cached), the merge runs again and the
    // checkpoint commit lands.
    let frag = fx.path().join(fragment(2));
    let fixed = fs::read_to_string(&frag)
        .unwrap()
        .replace("AD-101", "AD-201");
    fs::write(&frag, fixed).unwrap();
    let out = factory(&fx, &["answer", "BLOCK-1", "renumbered to AD-201"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.starts_with("BLOCK-1 resolved (docs/features/001-fixture/agent-log.md line "),
        "{text}"
    );
    wait_phase(&fx, &dir, "finished", 40);

    assert_eq!(
        fx.git(&["log", "-1", "--format=%s"]).trim(),
        "docs: checkpoint 1"
    );
    let merged_files = files_of(&fx, "HEAD");
    assert!(merged_files.contains(&fragment(2)), "{merged_files:?}");
    assert!(
        merged_files.contains(&AGENT_LOG.to_string()),
        "{merged_files:?}"
    );
    for n in 1..=3 {
        assert!(
            !fx.path().join(fragment(n)).exists(),
            "fragment {n} survived the merge"
        );
    }
    let log = fx.read(AGENT_LOG);
    for ad in ["AD-101", "AD-201", "AD-301"] {
        assert!(
            log.contains(&format!("### {ad} —")),
            "{ad} missing from\n{log}"
        );
    }
    assert!(log.contains("· checkpoint · resolved"), "{log}");
    assert!(log.contains("**Resolution ("), "{log}");
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    fx.git(&["fsck", "--strict"]);

    let k = kinds(&dir);
    assert!(seq_first(&dir, "block_answered") < seq_first(&dir, "log_merged"));
    assert!(seq_first(&dir, "log_merged") < seq_first(&dir, "checkpoint_done"));
    assert_eq!(count(&k, "run_waiting"), 1);
    assert_eq!(count(&k, "merge_refused"), 1);
    assert_eq!(k.last().map(String::as_str), Some("run_finished"));
}
