//! PRD-005 slice 07: a done slice lands (FR-3). The driver commits the
//! slice's lane whole on the lane's base, under `refs/factory/<NNN>/slice-NN`,
//! and cherry-picks it onto the feature tip with the main tree's board files:
//! one commit per slice, one parent, today's subject. A driver killed at
//! either commit leaves the worker for the resume; a dirty path in the main
//! tree parks the run; a conflicting pick blocks the slice. No `log-*.md` is
//! ever written.
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

/// One slice's landed commit: `feat(NNN): slice-NN <title>`, no body, the
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

/// The `refs/factory/` refs left in the repo, one per line.
fn factory_refs(fx: &Fixture) -> String {
    fx.git(&["for-each-ref", "--format=%(refname)", "refs/factory/"])
}

/// The number of parents of `sha`.
fn parents(fx: &Fixture, sha: &str) -> usize {
    fx.git(&["rev-list", "--parents", "-n", "1", sha])
        .split_whitespace()
        .count()
        - 1
}

// ── one lane: one commit per slice, no markdown log ────────────────────────

#[test]
fn each_done_slice_lands_as_one_commit_from_its_lane() {
    let fx = Fixture::with_index("commits-chain", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done"}),
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
    assert_slice_commit(&fx, &dir, 1, "A", &["src/slice-01.txt"]);
    assert_slice_commit(&fx, &dir, 2, "B", &["src/slice-02.txt"]);
    assert_slice_commit(&fx, &dir, 3, "C", &["src/slice-03.txt"]);

    // Each slice's decision is in log.jsonl, ids allocated in landing order.
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

    // Nothing left in the main tree, and no lane commit waits on a ref.
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    assert_eq!(factory_refs(&fx), "");
    fx.git(&["fsck", "--strict"]);

    let k = kinds(&dir);
    let first = k.iter().position(|x| x == "slice_spawned").unwrap();
    assert_eq!(
        k[first..],
        [
            "slice_spawned",
            "slice_done",
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

// ── two lanes: landings interleave ─────────────────────────────────────────

#[test]
fn two_lanes_land_out_of_order_and_each_commit_takes_only_its_own_files() {
    let fx = Fixture::with_index("commits-two-lanes", &[("checkpoint_every", "10")]);
    // 01 and 02 start together; 02 finishes first and lands while 01 is
    // still writing in its own lane. 03 follows 02.
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 800}),
        json!({"outcome": "done", "delay_ms": 200}),
        json!({"outcome": "done"}),
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
    assert_eq!(subjects.len(), 7, "{subjects:?}");
    assert!(
        subjects.ends_with(&[
            "docs(001): checkpoint 1".to_string(),
            "fix(001): polish review fixes".to_string(),
            "docs(001): ship PRD-001".to_string()
        ]),
        "{subjects:?}"
    );
    // One commit per slice, one parent each: a pick, never a merge.
    for (n, title) in [(1, "A"), (2, "B"), (3, "C")] {
        let sha = assert_slice_commit(&fx, &dir, n, title, &[&format!("src/slice-{n:02}.txt")]);
        assert_eq!(parents(&fx, &sha), 1, "slice {n:02}");
    }
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    assert_eq!(factory_refs(&fx), "");
    fx.git(&["fsck", "--strict"]);
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_committed"), 3);
    assert_eq!(count(&k, "log_merged"), 0);
    assert_eq!(k.last().map(String::as_str), Some("run_finished"));
}

// ── a slice edited only through Bash lands whole ───────────────────────────

#[test]
fn a_slice_edited_only_through_bash_lands_every_path_byte_equal() {
    let fx = Fixture::with_index("commits-bash-only", &[("checkpoint_every", "10")]);
    fx.chain();
    fs::write(fx.path().join("src/old.txt"), "old\n").unwrap();
    fs::write(fx.path().join("src/edit.txt"), "one\ntwo\n").unwrap();
    fx.git(&["add", "src/old.txt", "src/edit.txt"]);
    fx.git(&["commit", "-q", "-m", "chore: files to edit"]);
    // No tool call names a file: a rename, a redirect, an inline rewrite, a
    // new directory and a symlink to it. The lane's result is copied out
    // before the worker returns, to compare with what lands.
    let sh = "git mv src/old.txt src/renamed.txt \
              && printf 'new\\n' > src/new.txt \
              && sed -i 's/two/TWO/' src/edit.txt \
              && mkdir -p gen/dir && printf 'inner\\n' > gen/dir/inner.txt \
              && ln -s dir gen/link \
              && mkdir -p \"$FACTORY_STUB_DIR/lane-copy\" \
              && cp -a src gen \"$FACTORY_STUB_DIR/lane-copy/\"";
    fx.write_plan(&[
        json!({"outcome": "done", "touch": [], "sh": sh}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 40);

    let sha = sha_of(&fx, "feat(001): slice-01 A");
    let stat = fx.git(&["show", "--stat=200", "--format=", &sha]);
    for p in [
        "renamed.txt",
        "src/new.txt",
        "src/edit.txt",
        "gen/dir/inner.txt",
        "gen/link",
    ] {
        assert!(stat.contains(p), "{p} not in the slice commit:\n{stat}");
    }
    assert_eq!(fx.git(&["status", "--porcelain"]), "");

    // The landed commit alone holds what the lane held.
    let alone = fx.stub_dir().join("alone");
    fx.git(&[
        "worktree",
        "add",
        "-q",
        "--detach",
        alone.to_str().unwrap(),
        &sha,
    ]);
    let copy = fx.stub_dir().join("lane-copy");
    for f in [
        "src/renamed.txt",
        "src/new.txt",
        "src/edit.txt",
        "gen/dir/inner.txt",
    ] {
        assert_eq!(
            fs::read(alone.join(f)).unwrap(),
            fs::read(copy.join(f)).unwrap(),
            "{f}"
        );
    }
    assert!(!alone.join("src/old.txt").exists());
    let link = fs::read_link(alone.join("gen/link")).unwrap();
    assert_eq!(link, fs::read_link(copy.join("gen/link")).unwrap());
    assert_eq!(link, Path::new("dir"));
    assert_eq!(fs::read(alone.join("src/edit.txt")).unwrap(), b"one\nTWO\n");
}

// ── a driver killed mid-landing leaves the worker for the resume ───────────

/// The `kill_at`-th `git commit` from now kills the driver (the hook's
/// grandparent) in its `hook`: a `pre-commit` kill fails that commit, a
/// `post-commit` one leaves it made. Hooks and the counter live in the
/// common dir, which every lane shares.
fn kill_at_commit(fx: &Fixture, hook: &str, kill_at: u32) {
    let hook = fx.path().join(".git/hooks").join(hook);
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\n\
             c=\"$(git rev-parse --git-common-dir)/commits-seen\"\n\
             n=$(( $(cat \"$c\" 2>/dev/null || echo 0) + 1 ))\n\
             echo \"$n\" > \"$c\"\n\
             [ \"$n\" = {kill_at} ] || exit 0\n\
             kill -9 \"$(ps -o ppid= -p \"$PPID\" | tr -d ' ')\"\n\
             exit 1\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

/// Slice 01 done, the driver killed at its `kill_at`-th commit, the run
/// resumed: one slice-01 commit, a settled worker, one `slice_done`.
fn killed_and_resumed(name: &str, hook: &str, kill_at: u32, after_kill: impl Fn(&Fixture)) {
    let fx = Fixture::with_index(name, &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done", "delay_ms": 3000}),
    ]);
    kill_at_commit(&fx, hook, kill_at);

    let dir = start(&fx);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "the driver to die at the commit", 20, || {
        !pid_alive(driver)
    });
    // Slice 01 is done and its worker is not settled.
    assert_eq!(fx.slice_status(1), "done");
    assert_eq!(state(&dir)["workers"][0]["outcome"], Value::Null);
    assert_eq!(count(&kinds(&dir), "slice_done"), 1);
    after_kill(&fx);

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until(&fx, &dir, "slice 01 to land", 20, || {
        count(&kinds(&dir), "slice_committed") == 1
    });
    let sha = sha_of(&fx, "feat(001): slice-01 A");
    assert!(files_of(&fx, &sha).contains(&slice_rel(&fx, 1)));
    // The landing comes before the outcome: wait for the settle too.
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
    assert_eq!(
        subjects(&fx)
            .iter()
            .filter(|s| *s == "feat(001): slice-01 A")
            .count(),
        1
    );
    assert_eq!(factory_refs(&fx), "");
}

#[test]
fn a_driver_killed_at_the_lane_commit_leaves_the_worker_for_the_resume() {
    // The first commit is the lane's: nothing is on a ref yet, and the
    // resume commits the lane before it lands it.
    killed_and_resumed("commit-kill-lane", "pre-commit", 1, |fx| {
        assert!(!subjects(fx).iter().any(|s| s == "feat(001): slice-01 A"));
        assert_eq!(factory_refs(fx), "");
    });
}

#[test]
fn a_driver_killed_at_the_landing_leaves_its_ref_for_the_resume() {
    // The second is the landing's: the lane commit is on its ref, the pick
    // half done in the main tree, and the resume lands the ref.
    killed_and_resumed("commit-kill-landing", "pre-commit", 2, |fx| {
        assert!(!subjects(fx).iter().any(|s| s == "feat(001): slice-01 A"));
        assert_eq!(factory_refs(fx), "refs/factory/001/slice-01\n");
    });
}

#[test]
fn a_driver_killed_after_the_landing_commit_records_it_on_the_resume() {
    // The landing is made but not recorded, its ref not yet deleted: the
    // resume finds it on the tip and picks nothing twice.
    killed_and_resumed("commit-kill-landed", "post-commit", 2, |fx| {
        assert!(subjects(fx).iter().any(|s| s == "feat(001): slice-01 A"));
        assert_eq!(factory_refs(fx), "refs/factory/001/slice-01\n");
    });
}

// ── a dirty main tree parks the run ────────────────────────────────────────

#[test]
fn a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit() {
    let fx = Fixture::with_index("commits-dirty", &[("checkpoint_every", "10")]);
    // 01 and 02 start together; 03 depends on 02. 01 is held by the owner's
    // file where it lands; 02 lands meanwhile, and 03 must not spawn.
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 1500}),
        json!({"outcome": "done", "delay_ms": 3000}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let dir = start(&fx);
    let owned = fx.path().join("src/slice-01.txt");
    fs::write(&owned, "owner\n").unwrap();

    let park = "slice 01 cannot land: src/slice-01.txt changed in the main tree";
    wait_until(&fx, &dir, "slice 01's landing to park", 20, || {
        driver_log(&dir).contains(park)
    });
    wait_phase(&fx, &dir, "waiting-owner", 10);
    wait_until(&fx, &dir, "slice 02 to land while 01 waits", 20, || {
        count(&kinds(&dir), "slice_committed") == 1
    });
    std::thread::sleep(Duration::from_millis(1000));
    assert_eq!(fs::read_to_string(&owned).unwrap(), "owner\n");
    assert_eq!(fx.slice_status(3), "pending");
    assert_eq!(fx.calls().len(), 2, "nothing spawns while a landing waits");
    assert_eq!(factory_refs(&fx), "refs/factory/001/slice-01\n");

    fs::remove_file(&owned).unwrap();
    wait_phase(&fx, &dir, "finished", 40);
    assert_eq!(
        driver_log(&dir).matches(park).count(),
        1,
        "{}",
        driver_log(&dir)
    );
    // Slice 02's landing already carried 01's board writes: 01's commit
    // holds its own files only.
    let sha = sha_of(&fx, "feat(001): slice-01 A");
    assert_eq!(
        files_of(&fx, &sha),
        [slice_rel(&fx, 1), "src/slice-01.txt".to_string()]
    );
    assert_eq!(
        fx.read("src/slice-01.txt"),
        fx.git(&["show", &format!("{sha}:src/slice-01.txt")])
    );
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    assert_eq!(factory_refs(&fx), "");
}

// ── a conflicting landing blocks the slice (until PRD-005 slice 08) ────────

#[test]
fn a_conflicting_landing_blocks_the_slice_and_keeps_its_ref() {
    let fx = Fixture::with_index("commits-conflict", &[("checkpoint_every", "10")]);
    fs::write(fx.path().join("shared.txt"), "1\n2\n3\n4\n").unwrap();
    fx.git(&["add", "shared.txt"]);
    fx.git(&["commit", "-q", "-m", "chore: a shared file"]);
    // 01 and 02 start together from one tip and rewrite the same line; 01
    // lands first.
    fx.write_plan(&[
        json!({"outcome": "done", "sh": "sed -i '3s/.*/one/' shared.txt"}),
        json!({"outcome": "done", "delay_ms": 1500, "sh": "sed -i '3s/.*/two/' shared.txt"}),
    ]);
    let dir = start(&fx);
    wait_until(&fx, &dir, "slice 02 to block", 30, || {
        fx.slice_status(2) == "blocked"
    });

    let sha = sha_of(&fx, "feat(001): slice-01 A");
    assert!(files_of(&fx, &sha).contains(&"shared.txt".to_string()));
    assert_eq!(fx.read("shared.txt"), "1\n2\none\n4\n");
    let blocks: Vec<Value> = fx
        .log_of("block")
        .into_iter()
        .filter(|e| e["slice"] == 2)
        .collect();
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert!(
        blocks[0]["title"].as_str().unwrap().contains("shared.txt"),
        "{blocks:?}"
    );
    assert!(
        fx.git(&[
            "rev-parse",
            "--verify",
            "refs/factory/001/slice-02^{commit}"
        ])
        .trim()
        .len()
            == 40
    );
    // The main tree is untouched: only the board moved.
    assert_eq!(
        fx.git(&["status", "--porcelain", "--", ".", ":!docs/features"]),
        ""
    );

    let out = factory(&fx, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 30);
}
