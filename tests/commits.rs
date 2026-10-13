//! Slice 05 of PRD-001, on the v3 docs (PRD-002 slice 09): commits by
//! pathspec (PRD §7 FR-5). A `done` slice is committed from the paths its
//! worker's stream named plus its slice file, the feature's `state.toml` and
//! its `log.jsonl` — never `git add -A`, so two lanes committing out of order
//! each take only their own work; what the worker wrote without naming it is
//! listed in a `note` entry of `log.jsonl` and left in the tree. There is no
//! fragment and no merge: no `log-*.md` is ever written.
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

/// The feature's board and log, which every slice commit stages.
fn board_files() -> [String; 2] {
    [
        format!("{FEATURE_DIR}/state.toml"),
        format!("{FEATURE_DIR}/log.jsonl"),
    ]
}

/// The `Untracked leftovers` notes in `log.jsonl`.
fn leftover_notes(fx: &Fixture) -> Vec<Value> {
    fx.log_of("note")
        .into_iter()
        .filter(|e| e["title"] == "Untracked leftovers")
        .collect()
}

/// No `log-*.md` (fragment or merged log) in the feature folder.
fn assert_no_md_log(fx: &Fixture) {
    let md: Vec<String> = fs::read_dir(fx.path().join(FEATURE_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|f| (f.starts_with("log-") && f.ends_with(".md")) || f == "agent-log.md")
        .collect();
    assert!(md.is_empty(), "markdown logs in the feature folder: {md:?}");
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// One slice's commit as FR-5 asks for it: `feat(NNN): slice-NN <title>`, no body, the
/// repo's own author, exactly the named paths plus slice file, `state.toml`
/// and `log.jsonl`;
/// the `slice_committed` event and the worker row name the same sha.
fn assert_slice_commit(fx: &Fixture, dir: &Path, n: u8, title: &str, code: &[&str]) -> String {
    let sha = sha_of(fx, &format!("feat(001): slice-{n:02} {title}"));
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
    want.extend(board_files());
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

// ── tasks 1 and 2: one lane, three commits, leftovers, no markdown log ──────

#[test]
fn each_done_slice_is_committed_by_pathspec_and_no_markdown_log_is_written() {
    let fx = Fixture::with_index("commits-chain", &[("checkpoint_every", "10")]);
    fx.chain();
    // Slice 01 also leaves a file no tool call names (a generator, a script):
    // it is listed in log.jsonl and never committed (FR-5). The file its
    // `Bash` heredoc typed out is named like a `Write`, and committed.
    fx.write_plan(&[
        json!({"outcome": "done", "touch": ["src/lib.txt"], "stray": ["notes.txt"], "heredoc": ["src/gen/new.rs"]}),
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
            "feat(001): slice-01 A",
            "feat(001): slice-02 B",
            "feat(001): slice-03 C",
            "docs(001): checkpoint 1",
            "fix(001): polish review fixes",
            "docs(001): ship PRD-001",
        ]
    );
    assert_slice_commit(&fx, &dir, 1, "A", &["src/gen/new.rs", "src/lib.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/lib.txt"]);
    assert_slice_commit(&fx, &dir, 3, "C", &["src/lib.txt"]);

    // The stray file: on disk, in no commit, named in a log.jsonl note.
    assert!(fx.path().join("notes.txt").exists());
    assert!(
        !fx.git(&["log", "--all", "--format=", "--name-only"])
            .lines()
            .any(|l| l == "notes.txt"),
        "notes.txt was committed"
    );
    let notes = leftover_notes(&fx);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0]["slice"], 1);
    assert!(
        notes[0]["body"].as_str().unwrap().contains("- notes.txt"),
        "{notes:?}"
    );
    let leftovers: Vec<Value> = events(&dir)
        .into_iter()
        .filter(|e| e["kind"] == "untracked_leftovers")
        .collect();
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert_eq!(leftovers[0]["slice"], 1);
    assert_eq!(leftovers[0]["files"], json!(["notes.txt"]));
    let blob = leftovers[0]["blobs"]["notes.txt"].as_str().unwrap();
    assert_eq!(blob, fx.git(&["hash-object", "notes.txt"]).trim());
    // Each slice's decision is in log.jsonl, ids allocated in commit order.
    let ads: Vec<(String, u64)> = fx
        .log_of("decision")
        .iter()
        .map(|e| {
            (
                e["id"].as_str().unwrap().to_string(),
                e["slice"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        ads,
        [("AD-1".into(), 1), ("AD-2".into(), 2), ("AD-3".into(), 3)],
        "{ads:?}"
    );
    // After the full run: no fragment, no agent-log.md.
    assert_no_md_log(&fx);

    // The checkpoint commit stages the feature dir: no index render, no
    // fragment. HEAD itself is the ship commit, two commits later.
    let cp_files = files_of(&fx, &sha_of(&fx, "docs(001): checkpoint 1"));
    assert!(
        cp_files.contains(&format!("{FEATURE_DIR}/log.jsonl")),
        "{cp_files:?}"
    );
    assert!(
        !cp_files.iter().any(|f| f == "docs/index.md"),
        "{cp_files:?}"
    );
    assert!(
        !cp_files
            .iter()
            .any(|f| f.rsplit('/').next().unwrap().starts_with("log-slice-")),
        "{cp_files:?}"
    );
    assert!(
        !events(&dir).iter().any(|e| e["kind"] == "log_merged"),
        "log_merged is never emitted"
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
            "checkpoint_spawned",
            "checkpoint_done",
            "polish_spawned",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_green",
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
    assert!(
        at("feat(001): slice-02 B") < at("feat(001): slice-01 A"),
        "{subjects:?}"
    );
    assert_eq!(
        subjects.last().map(String::as_str),
        Some("docs(001): ship PRD-001")
    );
    assert_eq!(subjects.len(), 7, "{subjects:?}");
    assert!(
        subjects.ends_with(&[
            "docs(001): checkpoint 1".to_string(),
            "fix(001): polish review fixes".to_string(),
            "docs(001): ship PRD-001".to_string()
        ]),
        "{subjects:?}"
    );
    assert_slice_commit(&fx, &dir, 1, "A", &["src/a.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/b.txt"]);
    assert_slice_commit(&fx, &dir, 3, "C", &["src/c.txt"]);
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    fx.git(&["fsck", "--strict"]);
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_committed"), 3);
    assert_eq!(count(&k, "log_merged"), 0);
    assert_eq!(k.last().map(String::as_str), Some("run_finished"));
}

// ── polish: the commit lands before the worker's outcome is saved ───────────

#[test]
fn a_driver_killed_at_the_slice_commit_leaves_the_worker_for_the_resume() {
    let fx = Fixture::with_index("commit-kill", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done", "delay_ms": 3000}),
    ]);
    // The first `git commit` kills the driver (the hook's grandparent) and
    // fails; every later one passes.
    let hook = fx.path().join(".git/hooks/pre-commit");
    fs::write(
        &hook,
        "#!/bin/sh\n\
         marker=\"$(git rev-parse --git-dir)/killed-once\"\n\
         [ -e \"$marker\" ] && exit 0\n\
         touch \"$marker\"\n\
         kill -9 \"$(ps -o ppid= -p \"$PPID\" | tr -d ' ')\"\n\
         exit 1\n",
    )
    .unwrap();
    fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();

    let dir = start(&fx);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "the driver to die at the commit", 20, || {
        !pid_alive(driver)
    });
    // Slice 01 is done and uncommitted, and its worker is not settled.
    assert_eq!(fx.slice_status(1), "done");
    assert!(!subjects(&fx).iter().any(|s| s == "feat(001): slice-01 A"));
    assert_eq!(state(&dir)["workers"][0]["outcome"], Value::Null);
    assert_eq!(count(&kinds(&dir), "slice_done"), 1);

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until(&fx, &dir, "slice 01 to be committed", 20, || {
        count(&kinds(&dir), "slice_committed") == 1
    });
    let sha = sha_of(&fx, "feat(001): slice-01 A");
    assert!(files_of(&fx, &sha).contains(&slice_rel(&fx, 1)));
    // The commit lands before the outcome: wait for the settle too.
    wait_until(&fx, &dir, "slice 01's worker to be settled", 20, || {
        !state(&dir)["workers"][0]["outcome"].is_null()
    });
    assert_eq!(state(&dir)["workers"][0]["outcome"], "done");
    // The resume did not log the slice done a second time.
    let done_01 = events(&dir)
        .iter()
        .filter(|e| e["kind"] == "slice_done" && e["slice"] == 1)
        .count();
    assert_eq!(done_01, 1);

    let out = factory(&fx, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 30);
}

#[test]
fn a_resumed_driver_judges_an_orphans_leftovers_against_its_spawn_baseline() {
    let fx = Fixture::with_index("orphan-baseline", &[("checkpoint_every", "10")]);
    fx.chain();
    // Slice 01 leaves notes.txt untracked; slice 02's worker outlives a
    // killed driver, so the resumed one settles it.
    fx.write_plan(&[
        json!({"outcome": "done", "stray": ["notes.txt"]}),
        json!({"outcome": "done", "delay_ms": 3000}),
    ]);
    let dir = start(&fx);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    // `slice_spawned` is emitted once the worker is in state.json: a kill
    // before that leaves an orphan the resumed driver cannot settle.
    wait_until(&fx, &dir, "slice 02's worker to spawn", 20, || {
        count(&kinds(&dir), "slice_spawned") == 2 && fx.slice_status(2) == "building"
    });
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    wait_until(&fx, &dir, "the driver to die", 5, || !pid_alive(driver));
    wait_until(&fx, &dir, "the orphan to finish slice 02", 20, || {
        fx.slice_status(2) == "done"
    });

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until(&fx, &dir, "slice 02 to be committed", 20, || {
        count(&kinds(&dir), "slice_committed") == 2
    });
    // notes.txt was there before slice 02's worker: slice 01's leftover only.
    let leftovers: Vec<Value> = events(&dir)
        .into_iter()
        .filter(|e| e["kind"] == "untracked_leftovers")
        .collect();
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert_eq!(leftovers[0]["slice"], 1);
    let notes = leftover_notes(&fx);
    assert!(notes.iter().all(|e| e["slice"] == 1), "{notes:?}");

    let out = factory(&fx, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 30);
}

// ── AD-20007: tracked files no tool call named ──────────────────────────────

/// A tracked `src/extra.txt` a worker can change without naming it.
fn track_extra(fx: &Fixture) {
    fs::write(fx.path().join("src/extra.txt"), "extra\n").unwrap();
    fx.git(&["add", "src/extra.txt"]);
    fx.git(&["commit", "-q", "-m", "chore: track extra"]);
}

#[test]
fn a_lone_workers_unnamed_tracked_change_goes_in_its_slice_commit() {
    let fx = Fixture::with_index("commits-sweep-alone", &[("checkpoint_every", "10")]);
    fx.chain();
    track_extra(&fx);
    // Slice 01 changes a tracked file no tool call names (a `sed -i`, a
    // formatter); nothing else runs beside it.
    fx.write_plan(&[
        json!({"outcome": "done", "stray": ["src/extra.txt"]}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    assert_slice_commit(&fx, &dir, 1, "A", &["src/extra.txt", "src/lib.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/lib.txt"]);
    assert_eq!(count(&kinds(&dir), "untracked_leftovers"), 0);
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    assert!(
        driver_log(&dir).contains("changed 1 tracked file(s) no tool call named"),
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn a_listed_file_committed_since_and_changed_again_goes_in_the_next_commit() {
    let fx = Fixture::with_index("commits-relisted", &[("checkpoint_every", "10")]);
    fx.chain();
    // Run B of 2026-10-11: slice 02 left files, the owner committed them,
    // and polish's later edits to them went in no commit and on no list.
    fx.write_plan(&[
        json!({"outcome": "done", "stray": ["notes.txt"]}),
        json!({"outcome": "done", "commit": ["notes.txt"]}),
        json!({"outcome": "done", "stray": ["notes.txt"]}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    assert_eq!(count(&kinds(&dir), "untracked_leftovers"), 1);
    assert_slice_commit(&fx, &dir, 3, "C", &["notes.txt", "src/lib.txt"]);
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
}

#[test]
fn an_unnamed_tracked_change_beside_a_live_sibling_is_listed_not_committed() {
    let fx = Fixture::with_index("commits-sweep-sibling", &[("checkpoint_every", "10")]);
    track_extra(&fx);
    // 01 and 02 start together; 01 returns while 02 still runs, so whose
    // the tracked change is cannot be told: it is listed, not committed.
    fx.write_plan(&[
        json!({"outcome": "done", "touch": ["src/a.txt"], "stray": ["src/extra.txt"]}),
        json!({"outcome": "done", "delay_ms": 1500, "touch": ["src/b.txt"]}),
        json!({"outcome": "done", "touch": ["src/c.txt"]}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    assert_slice_commit(&fx, &dir, 1, "A", &["src/a.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/b.txt"]);
    assert_slice_commit(&fx, &dir, 3, "C", &["src/c.txt"]);
    let leftovers: Vec<Value> = events(&dir)
        .into_iter()
        .filter(|e| e["kind"] == "untracked_leftovers")
        .collect();
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert_eq!(leftovers[0]["slice"], 1);
    assert_eq!(leftovers[0]["files"], json!(["src/extra.txt"]));
    // In no commit after the one that tracked it, still modified.
    let touching = fx.git(&["log", "--format=%s", "--", "src/extra.txt"]);
    assert_eq!(touching.trim(), "chore: track extra");
    assert!(
        fx.git(&["status", "--porcelain"])
            .contains(" M src/extra.txt"),
        "{}",
        fx.git(&["status", "--porcelain"])
    );
    let notes = leftover_notes(&fx);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0]["slice"], 1);
    assert!(
        notes[0]["body"]
            .as_str()
            .unwrap()
            .contains("- src/extra.txt"),
        "{notes:?}"
    );
}

fn leftovers_of(dir: &Path) -> Vec<Value> {
    events(dir)
        .into_iter()
        .filter(|e| e["kind"] == "untracked_leftovers")
        .collect()
}

#[test]
fn a_lone_worker_claims_generated_files_by_git_adding_their_folder() {
    let fx = Fixture::with_index("commits-claim-dir", &[("checkpoint_every", "10")]);
    fx.chain();
    // A generator wrote gen/ and a scratch file; the worker staged gen/ only.
    fx.write_plan(&[
        json!({"outcome": "done", "stray": ["gen/a.txt", "gen/b.txt", "scratch.log"], "git_add": "gen"}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    assert_slice_commit(
        &fx,
        &dir,
        1,
        "A",
        &["gen/a.txt", "gen/b.txt", "src/lib.txt"],
    );
    let leftovers = leftovers_of(&dir);
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert_eq!(leftovers[0]["files"], json!(["scratch.log"]));
    assert_eq!(fx.git(&["status", "--porcelain"]), "?? scratch.log\n");
}

#[test]
fn git_add_all_claims_nothing_and_its_new_files_are_listed() {
    let fx = Fixture::with_index("commits-claim-all", &[("checkpoint_every", "10")]);
    fx.chain();
    // `git add -A` stages the scratch file too: staged-new is not tracked,
    // so even a lone worker's sweep leaves it out.
    fx.write_plan(&[
        json!({"outcome": "done", "stray": ["gen/a.txt", "scratch.log"], "git_add": "-A"}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    assert_slice_commit(&fx, &dir, 1, "A", &["src/lib.txt"]);
    let leftovers = leftovers_of(&dir);
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert_eq!(leftovers[0]["files"], json!(["gen/a.txt", "scratch.log"]));
    assert_eq!(
        fx.git(&["status", "--porcelain"]),
        "A  gen/a.txt\nA  scratch.log\n"
    );
}

#[test]
fn a_file_git_added_by_name_is_committed_beside_a_live_sibling() {
    let fx = Fixture::with_index("commits-claim-sibling", &[("checkpoint_every", "10")]);
    // 01 returns while 02 still runs: its folder claim waits for the owner,
    // its claim by name does not.
    fx.write_plan(&[
        json!({"outcome": "done", "touch": ["src/a.txt"], "stray": ["gen/a.txt", "gen/b.txt"], "git_add": "gen/a.txt"}),
        json!({"outcome": "done", "delay_ms": 1500, "touch": ["src/b.txt"]}),
        json!({"outcome": "done", "touch": ["src/c.txt"]}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    assert_slice_commit(&fx, &dir, 1, "A", &["gen/a.txt", "src/a.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/b.txt"]);
    let leftovers = leftovers_of(&dir);
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert_eq!(leftovers[0]["slice"], 1);
    assert_eq!(leftovers[0]["files"], json!(["gen/b.txt"]));
}
