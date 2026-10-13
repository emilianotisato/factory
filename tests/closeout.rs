//! Slice 09 of PRD-001: the closeout (PRD §8 FR-6). When every slice is
//! done and the last checkpoint is in, the driver spawns the polish worker
//! on `/polish`, commits its fixes, ships the feature (`docs ship`, the
//! retro's numbers from the store), pushes, opens the draft PR with the
//! body the worker wrote, and watches CI: a fix worker per red check (three
//! rounds), one rerun for a flaky one, a block otherwise. Every `gh` here is
//! the stub in `tests/stubs/`; `origin` is the fixture's bare repo.
#[path = "harness/mod.rs"]
mod harness;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::db::Db;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const FEATURE_DIR: &str = "docs/features/001-fixture";
const LOG: &str = "docs/features/001-fixture/log.jsonl";
const STATE: &str = "docs/features/001-fixture/state.toml";
const RETRO: &str = "docs/features/001-fixture/retro.md";
const PR_URL: &str = "https://example.invalid/pr/1";
const RUN_FIELDS: &str = "databaseId,name,workflowName,conclusion,headSha,status,attempt";
const CI_FIX: &str = "Fix the red CI check `test` on the PR of PRD-001 as the appended system prompt says. Return the STATUS block.";
/// The run row's twelve numbers the `retros` row copies at ship (FR-19).
const RETRO_NUMBERS: [&str; 12] = [
    "gate_runs",
    "gate_red",
    "gate_minutes",
    "ci_cycles",
    "slices",
    "checkpoints",
    "build_minutes",
    "polish_minutes",
    "out_tokens",
    "cache_read_tokens",
    "start_context",
    "peak_context",
];

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
    let mut dirs: Vec<PathBuf> = fs::read_dir(data_dir(fx).join("runs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    dirs.sort();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs.remove(0)
}

fn run_id(dir: &Path) -> String {
    dir.file_name().unwrap().to_str().unwrap().to_string()
}

fn state(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn events(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
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

fn of_kind(dir: &Path, kind: &str) -> Vec<Value> {
    events(dir)
        .into_iter()
        .filter(|e| e["kind"] == kind)
        .collect()
}

/// The kinds from the first `from` on.
fn kinds_from(dir: &Path, from: &str) -> Vec<String> {
    let all = kinds(dir);
    let i = all
        .iter()
        .position(|k| k == from)
        .unwrap_or_else(|| panic!("no {from} in {all:?}"));
    all[i..].to_vec()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

fn driver_log(dir: &Path) -> String {
    fs::read_to_string(dir.join("driver.log")).unwrap_or_default()
}

fn wait_until(fx: &Fixture, dir: &Path, what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        if Instant::now() >= deadline {
            panic!(
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- git log\n{}\n--- gh.log\n{:?}\n--- driver.log\n{}\n--- log.jsonl\n{}",
                state(dir),
                kinds(dir),
                fx.git(&["log", "--format=%h %s"]),
                fx.gh_log(),
                driver_log(dir),
                fs::read_to_string(fx.path().join(LOG)).unwrap_or_default()
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

/// The first `closeout_blocked` event, once it is there.
fn wait_block(fx: &Fixture, dir: &Path, secs: u64) -> Value {
    wait_until(fx, dir, "closeout_blocked", secs, || {
        !of_kind(dir, "closeout_blocked").is_empty()
    });
    of_kind(dir, "closeout_blocked").remove(0)
}

fn start(fx: &Fixture) -> PathBuf {
    let out = factory(fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    only_run_dir(fx)
}

fn stop(fx: &Fixture, dir: &Path) {
    let out = factory(fx, &["stop", &run_id(dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(fx, dir, "finished", 60);
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

fn done() -> Value {
    json!({"outcome": "done"})
}

fn step(outcome: &str) -> Value {
    json!({"outcome": outcome})
}

/// The five steps every fixture run takes before the closeout.
fn built() -> Vec<Value> {
    vec![
        done(),
        done(),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]
}

fn plan_with(tail: &[Value]) -> Vec<Value> {
    let mut v = built();
    v.extend_from_slice(tail);
    v
}

fn prompts(fx: &Fixture) -> Vec<String> {
    fx.calls()
        .iter()
        .map(|c| c["prompt"].as_str().unwrap_or("").to_string())
        .collect()
}

/// The pack file a call was given, and its text.
fn pack_of(call: &Value) -> (String, String) {
    let argv: Vec<&str> = call["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let i = argv
        .iter()
        .position(|a| *a == "--append-system-prompt-file")
        .unwrap_or_else(|| panic!("no pack flag in {argv:?}"));
    let path = PathBuf::from(argv[i + 1]);
    let name = path.file_name().unwrap().to_str().unwrap().to_string();
    (name, fs::read_to_string(&path).unwrap())
}

fn roles(fx: &Fixture, dir: &Path) -> Vec<String> {
    db(fx)
        .workers(&run_id(dir))
        .unwrap()
        .iter()
        .map(|w| w.role.clone())
        .collect()
}

/// The `gh` calls whose argv starts with `verb` (`pr create`, `run rerun`).
fn gh_calls(fx: &Fixture, verb: &str) -> Vec<String> {
    fx.gh_log()
        .into_iter()
        .filter(|l| l == verb || l.starts_with(&format!("{verb} ")))
        .collect()
}

/// The frontmatter of a fixture file as `(key, value)` pairs.
/// `factory report <run> --json`, parsed.
fn report_json(fx: &Fixture, dir: &Path) -> Value {
    let out = factory(fx, &["report", &run_id(dir), "--json"]);
    serde_json::from_str(&stdout(&out)).expect("one JSON object")
}

fn origin_head(fx: &Fixture, branch: &str) -> Option<String> {
    let origin = fx.path().parent().unwrap().join("origin.git");
    let out = fx
        .command("git")
        .args([
            "-C",
            origin.to_str().unwrap(),
            "rev-parse",
            "--verify",
            "-q",
        ])
        .arg(format!("refs/heads/{branch}"))
        .output()
        .unwrap();
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn head(fx: &Fixture) -> String {
    fx.git(&["rev-parse", "HEAD"]).trim().to_string()
}

/// The `block` entry `id` of the feature's log: its `phase` is `phase` and
/// no `answer` refers to it yet (open).
fn block_entry(fx: &Fixture, id: &str, phase: &str) -> Value {
    let all = fx.log_entries();
    let entry = all
        .iter()
        .find(|e| e["kind"] == "block" && e["id"] == id)
        .unwrap_or_else(|| panic!("{id} missing from {all:#?}"))
        .clone();
    assert_eq!(entry["phase"], phase, "{entry}");
    assert!(
        !answered_in_log(fx, id),
        "{id} is not an open {phase} block: {all:#?}"
    );
    entry
}

/// The body of the `block` entry `id`.
fn block_body(fx: &Fixture, id: &str) -> String {
    fx.log_of("block")
        .into_iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no block {id} in {:#?}", fx.log_entries()))["body"]
        .as_str()
        .unwrap()
        .to_string()
}

/// An `answer` entry refers to `id`: the block is resolved.
fn answered_in_log(fx: &Fixture, id: &str) -> bool {
    fx.log_of("answer").iter().any(|e| e["ref"] == id)
}

/// Each line fails the first `gh` call whose argv starts with it, once.
fn gh_fail(fx: &Fixture, prefixes: &[&str]) {
    let dir = fx.path().join("scenario");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("gh-fail"), prefixes.join("\n") + "\n").unwrap();
}

/// One poll a second: a pending check costs a second, not a minute.
const FAST: [(&str, &str); 1] = [("FACTORY_CI_INTERVAL", "1")];

/// `factory run 001` with the driver's environment extended by `env`.
fn start_with(fx: &Fixture, env: &[(&str, &str)]) -> PathBuf {
    let out = fx
        .command(BIN)
        .args(["run", "001"])
        .envs(env.iter().copied())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    only_run_dir(fx)
}

fn answer(fx: &Fixture, id: &str, text: &str) {
    let out = factory(fx, &["answer", id, text]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
}

/// The `closeout_blocked` event of `id`, once it is there.
fn wait_block_id(fx: &Fixture, dir: &Path, id: &str, secs: u64) -> Value {
    wait_until(fx, dir, id, secs, || {
        of_kind(dir, "closeout_blocked")
            .iter()
            .any(|e| e["block_id"] == id)
    });
    of_kind(dir, "closeout_blocked")
        .into_iter()
        .find(|e| e["block_id"] == id)
        .unwrap()
}

/// Parked: `n` `run_waiting` events so far.
fn wait_parked(fx: &Fixture, dir: &Path, n: usize, secs: u64) {
    wait_until(fx, dir, &format!("run_waiting #{n}"), secs, || {
        of_kind(dir, "run_waiting").len() >= n
    });
}

fn short(sha: &str) -> &str {
    &sha[..7]
}

#[test]
fn a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr() {
    let fx = Fixture::new("closeout-green");
    fx.chain();
    fx.write_plan(&plan_with(&[step("polish")]));
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 90);

    // One polish worker after the last checkpoint, prompted with `/polish`.
    let p = prompts(&fx);
    assert_eq!(p.len(), 6, "{p:?}");
    assert_eq!(p[5], "/polish");
    assert_eq!(
        roles(&fx, &dir),
        [
            "build",
            "build",
            "checkpoint",
            "build",
            "checkpoint",
            "polish"
        ]
    );
    let polish = &db(&fx).workers(&run_id(&dir)).unwrap()[5];
    assert_eq!(polish.role, "polish");
    assert_eq!(polish.slice, None);
    assert_eq!(polish.checkpoint, None);
    assert_eq!(polish.outcome.as_deref(), Some("done"), "{polish:?}");
    let w = &state(&dir)["workers"][5];
    assert_eq!(w["role"], "polish");
    assert_eq!(w["outcome"], "done");

    // The pack: branch, retro template, PR body shape, contract, autonomy.
    let (name, pack) = pack_of(&fx.calls()[5]);
    assert_eq!(name, "polish-6.md");
    assert!(
        pack.starts_with(&format!(
            "<!-- factory-pack files: {FEATURE_DIR}/prd.md -->\n<!-- run dir: {} -->\n\n# Branch\n",
            dir.display()
        )),
        "{pack}"
    );
    for want in [
        "Default branch: `main` · feature branch: `feat/fixture`",
        "Commits this branch adds (`git log --oneline main..HEAD`):",
        " feat(001): slice-03 C\n",
        " docs(001): checkpoint 2\n",
        "# Retro",
        &format!("<!-- {FEATURE_DIR}/retro.md -->"),
        "````markdown\n# Retro — PRD-001\n",
        "# PR body",
        &format!("<!-- write: {} -->", dir.join("pr-body.md").display()),
        "## PRD-{NNN} — {title}",
        "# Contract",
        "You are the polish worker of PRD-001",
        "run `/polish` **Unattended**",
        "docs retro 001 --set score=4`",
        "--kind block --phase polish",
        "# Autonomy",
    ] {
        assert!(
            pack.contains(want),
            "{want:?} missing from the polish pack:\n{pack}"
        );
    }
    assert!(!pack.contains("# Gate transcript"), "{pack}");
    assert!(!pack.contains("\nscore:"), "{pack}");
    harness::assert_v3_contract(&pack);
    // FR-18: the slice and checkpoint packs of the same run call the tool too.
    for i in [0, 1, 2, 3, 4] {
        harness::assert_v3_contract(&pack_of(&fx.calls()[i]).1);
    }
    // The `# Log` section is `log show 001`, byte for byte: the polish worker
    // writes no entry, so the log at the end is the log it was handed.
    let from = pack
        .find("# Log\n\n<!-- factory docs log show 001 -->\n\n")
        .expect("# Log");
    let body = &pack[from + "# Log\n\n<!-- factory docs log show 001 -->\n\n".len()..];
    let shown = fx.docs(&["log", "show", "001"]);
    assert!(shown.contains("## Decisions"), "{shown}");
    assert_eq!(
        &body[..body.find("\n\n# Retro\n").unwrap()],
        shown.trim_end()
    );

    // Its commit, then the ship commit: HEAD is `docs: ship PRD-001`.
    assert_eq!(
        subjects(&fx)[2..],
        [
            "feat(001): slice-01 A",
            "feat(001): slice-02 B",
            "docs(001): checkpoint 1",
            "feat(001): slice-03 C",
            "docs(001): checkpoint 2",
            "fix(001): polish review fixes",
            "docs(001): ship PRD-001",
        ]
    );
    assert_eq!(
        fx.git(&["log", "-1", "--format=%s"]).trim(),
        "docs(001): ship PRD-001"
    );
    let polish_sha = sha_of(&fx, "fix(001): polish review fixes");
    assert_eq!(
        files_of(&fx, &polish_sha),
        [
            RETRO.to_string(),
            STATE.to_string(),
            "src/lib.txt".to_string()
        ]
    );
    assert!(fx.read("src/lib.txt").contains("polished by spawn 6\n"));
    // The ship commit stages the feature dir: the shipped state and the
    // filled retro; no index render, no agent log.
    let ship_files = files_of(&fx, &sha_of(&fx, "docs(001): ship PRD-001"));
    for f in [STATE, RETRO] {
        assert!(
            ship_files.contains(&f.to_string()),
            "{f} not in {ship_files:?}"
        );
    }
    for f in [
        "docs/index.md".to_string(),
        format!("{FEATURE_DIR}/agent-log.md"),
    ] {
        assert!(!ship_files.contains(&f), "{f} in {ship_files:?}");
    }
    assert_eq!(fx.git(&["status", "--porcelain"]).trim(), "", "tree dirty");

    // The PRD is shipped and reviewed; the retro has its twelve numbers.
    let board = fx.state();
    assert_eq!(board.status, "shipped");
    assert!(
        board
            .shipped
            .as_deref()
            .is_some_and(|d| d.starts_with("20")),
        "shipped: {:?}",
        board.shipped
    );
    assert!(
        board
            .reviewed
            .as_deref()
            .is_some_and(|d| d.starts_with("20")),
        "reviewed: {:?}",
        board.reviewed
    );
    // FR-19: the numbers are the store's `retros` row, not frontmatter.
    assert!(!fx.read(RETRO).starts_with("---"), "{}", fx.read(RETRO));
    let id = run_id(&dir);
    let repo = db(&fx).run_row(&id).unwrap().unwrap().repo;
    let rr = db(&fx).retro(&repo, 1).unwrap().expect("a retros row");
    assert_eq!(rr.run_id.as_deref(), Some(id.as_str()));
    assert_eq!((rr.slug.as_str(), rr.score), ("fixture", None));
    assert!(
        rr.shipped_at
            .as_deref()
            .is_some_and(|d| d.starts_with("20"))
    );
    assert_eq!(
        (
            rr.slices,
            rr.checkpoints,
            rr.gate_runs,
            rr.gate_red,
            rr.ci_cycles
        ),
        (Some(3), Some(2), Some(2), Some(0), Some(0))
    );
    let rep = report_json(&fx, &dir);
    let row_numbers = [
        rr.gate_runs.map(|v| v as f64),
        rr.gate_red.map(|v| v as f64),
        rr.gate_minutes,
        rr.ci_cycles.map(|v| v as f64),
        rr.slices.map(|v| v as f64),
        rr.checkpoints.map(|v| v as f64),
        rr.build_minutes,
        rr.polish_minutes,
        rr.out_tokens.map(|v| v as f64),
        rr.cache_read_tokens.map(|v| v as f64),
        rr.start_context.map(|v| v as f64),
        rr.peak_context.map(|v| v as f64),
    ];
    for (key, got) in RETRO_NUMBERS.iter().zip(row_numbers) {
        let want = rep[key].as_f64().unwrap_or_else(|| panic!("{key}: {rep}"));
        let got = got.unwrap_or_else(|| panic!("{key} is null in the row"));
        assert!(
            (got - want).abs() < 0.005,
            "{key}: row {got}, report {want}"
        );
    }
    assert!(
        fx.read(RETRO).contains("## Cost vs catch"),
        "the worker's prose"
    );
    // PRD-003 FR-25: the changed lines at ship, and the `## Numbers` section
    // after the prose, its weighted figures `--per-worker`'s summed per role.
    let ship_sha = sha_of(&fx, "docs(001): ship PRD-001");
    let stat = fx.git(&["diff", "--shortstat", &format!("main..{ship_sha}^")]);
    let row = db(&fx).run_row(&run_id(&dir)).unwrap().unwrap();
    assert_eq!(
        (row.lines_added, row.lines_removed),
        (
            Some(factory::retro::shortstat(&stat).0),
            Some(factory::retro::shortstat(&stat).1)
        ),
        "{stat}"
    );
    assert!(row.lines_added.unwrap() > 0, "{stat}");
    let text = fx.read(RETRO);
    assert!(
        text.find("## Cost vs catch") < text.find("## Numbers\n\n<!-- factory:numbers -->\n"),
        "{text}"
    );
    let numbers = section(&text);
    assert_eq!(
        role_weighted(numbers),
        per_worker_weighted(&fx, &run_id(&dir))
    );
    assert!(
        numbers.contains("Compared with: No earlier finished run in this repo.\n"),
        "{numbers}"
    );
    assert!(numbers.contains("Hypothesis: none\n"), "{numbers}");
    assert!(numbers.contains("\nquality: gate_red 0 · "), "{numbers}");
    let before = fs::read(fx.path().join(RETRO)).unwrap();
    let out = factory(&fx, &["retro", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(
        stdout(&out),
        format!(
            "retro: {} unchanged\nscore: none\n",
            fx.path().join(RETRO).display()
        )
    );
    assert_eq!(fs::read(fx.path().join(RETRO)).unwrap(), before);
    fx.docs(&["retro", "001", "--set", "score=4"]);
    let out = factory(&fx, &["retro", &run_id(&dir)]);
    assert!(
        stdout(&out).ends_with(" unchanged\nscore: 4\n"),
        "{}",
        stdout(&out)
    );
    let body = fs::read_to_string(dir.join("pr-body.md")).unwrap();
    assert!(body.starts_with("## PRD-001 — Fixture\n"), "{body}");

    // Pushed to the fixture's origin, then exactly one draft PR, then green CI.
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));
    assert_eq!(
        gh_calls(&fx, "pr create"),
        [format!(
            "pr create --draft --base main --title \"PRD-001: Fixture\" --body-file {} --head feat/fixture",
            dir.join("pr-body.md").display()
        )]
    );
    assert_eq!(
        gh_calls(&fx, "pr checks"),
        ["pr checks 1 --json name,state,bucket,link,workflow"]
    );
    assert_eq!(
        gh_calls(&fx, "pr view"),
        [
            "pr view feat/fixture --json url,state",
            "pr view 1 --json headRefOid"
        ]
    );
    assert!(gh_calls(&fx, "pr edit").is_empty());
    assert!(gh_calls(&fx, "run rerun").is_empty());
    assert_eq!(state(&dir)["pr_url"], PR_URL);
    assert_eq!(
        db(&fx)
            .run_row(&run_id(&dir))
            .unwrap()
            .unwrap()
            .pr_url
            .as_deref(),
        Some(PR_URL)
    );

    // The events: closeout from the polish spawn to the end of the run.
    assert_eq!(
        kinds_from(&dir, "polish_spawned"),
        [
            "polish_spawned",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_green",
            "run_finished"
        ]
    );
    let polished = &of_kind(&dir, "polish_done")[0];
    assert_eq!(polished["sha"], polish_sha);
    assert_eq!(polished["files"].as_array().unwrap().len(), 3, "{polished}");
    assert_eq!(of_kind(&dir, "pr_opened")[0]["url"], PR_URL);
    assert_eq!(of_kind(&dir, "ci_green")[0]["head"], head(&fx));
    let last = events(&dir).pop().unwrap();
    assert_eq!(last["kind"], "run_finished");
    assert_eq!(last["pr_url"], PR_URL);
    assert_eq!(last["slices"], 3);
    assert_eq!(last["checkpoints"], 2);
    assert_eq!(last["ci_cycles"], 0);
    assert!(last["duration_minutes"].as_f64().is_some(), "{last}");
    assert!(
        driver_log(&dir).contains(&format!("finished (every slice done) — PR {PR_URL}")),
        "{}",
        driver_log(&dir)
    );
    let out = factory(&fx, &["report", &run_id(&dir)]);
    assert!(
        stdout(&out).contains("ci_cycles: 0\nslices: 3\n"),
        "{}",
        stdout(&out)
    );

    // No `retro.md`: `factory retro` refuses, naming the path.
    fs::remove_file(fx.path().join(RETRO)).unwrap();
    let out = factory(&fx, &["retro", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains(&fx.path().join(RETRO).display().to_string()),
        "{}",
        stdout(&out)
    );
}

/// PRD-003 FR-25: two runs of feature 001 in one repo, the board reset
/// from `main` between them. The second run's section compares with the
/// first, per slice and per changed line; the first's, regenerated, still
/// has no earlier run, since its cohort is fixed by its `started_at`.
#[test]
fn a_second_run_of_the_repo_compares_with_the_first() {
    let fx = Fixture::new("retro-compare");
    fx.chain();
    let plan = plan_with(&[step("polish")]);
    fx.write_plan(&plan);
    let first_dir = start(&fx);
    wait_phase(&fx, &first_dir, "finished", 90);
    let first = run_id(&first_dir);

    // The board as `main` has it: PRD ready, slices pending, no checkpoints.
    fx.git(&["rm", "-r", "-q", "docs"]);
    fx.git(&["checkout", "main", "--", "docs"]);
    fx.git(&["commit", "-q", "-m", "chore: reset the board"]);
    fx.write_plan(&[plan.clone(), plan].concat());
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let mut dirs: Vec<PathBuf> = fs::read_dir(data_dir(&fx).join("runs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| *p != first_dir)
        .collect();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    let second_dir = dirs.remove(0);
    wait_phase(&fx, &second_dir, "finished", 90);
    let second = run_id(&second_dir);

    let store = db(&fx);
    let (a, b) = (
        store.run_row(&first).unwrap().unwrap(),
        store.run_row(&second).unwrap().unwrap(),
    );
    assert_eq!(a.repo, b.repo);
    let report = factory(&fx, &["report", &first, "--json"]);
    let weighted = serde_json::from_slice::<Value>(&report.stdout).unwrap()["weighted_tokens"]
        .as_u64()
        .unwrap();
    let lines = a.lines_added.unwrap() + a.lines_removed.unwrap();
    assert!(lines > 0, "{a:?}");
    let text = fx.read(RETRO);
    let numbers = section(&text);
    let rows: Vec<&str> = numbers
        .lines()
        .skip_while(|l| *l != "Compared with:")
        .filter(|l| l.starts_with("| 001-"))
        .collect();
    assert_eq!(rows.len(), 2, "{numbers}");
    assert!(
        rows[0].starts_with(&format!("| {second} (this run) | 001 | ")),
        "{numbers}"
    );
    let per_slice = (weighted as f64 / 3.0).round();
    let per_line = (weighted as f64 / lines as f64).round();
    assert_eq!(
        rows[1],
        format!("| {first} | 001 | {weighted} | 3 | {per_slice} | {lines} | {per_line} |"),
        "{numbers}"
    );

    let out = factory(&fx, &["retro", &first]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(
        stdout(&out).ends_with(" updated\nscore: none\n"),
        "{}",
        stdout(&out)
    );
    let text = fx.read(RETRO);
    let numbers = section(&text);
    assert!(
        numbers.contains("Compared with: No earlier finished run in this repo.\n"),
        "{numbers}"
    );
    assert!(!numbers.contains(&second), "{numbers}");
}

/// The text between the `## Numbers` markers.
fn section(retro: &str) -> &str {
    let open = "<!-- factory:numbers -->\n";
    let from = retro.find(open).expect("no numbers marker") + open.len();
    let to = retro
        .find("<!-- /factory:numbers -->")
        .expect("no end marker");
    &retro[from..to]
}

/// The section's role rows: role → weighted (the fifth column).
fn role_weighted(numbers: &str) -> BTreeMap<String, u64> {
    numbers
        .lines()
        .take_while(|l| !l.is_empty())
        .skip(2)
        .map(|l| l.split('|').map(str::trim).collect::<Vec<_>>())
        .filter(|cells| cells[1] != "total")
        .map(|cells| (cells[1].to_string(), cells[5].parse().expect(cells[5])))
        .collect()
}

/// `factory report <id> --json --per-worker`: `weighted_own + weighted_sub`
/// summed per role.
fn per_worker_weighted(fx: &Fixture, id: &str) -> BTreeMap<String, u64> {
    let out = factory(fx, &["report", id, "--json", "--per-worker"]);
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut by_role = BTreeMap::new();
    for w in r["per_worker"].as_array().unwrap() {
        *by_role
            .entry(w["role"].as_str().unwrap().to_string())
            .or_insert(0) +=
            w["weighted_own"].as_u64().unwrap() + w["weighted_sub"].as_u64().unwrap_or(0);
    }
    by_role
}

/// AD-904: a polish worker that leaves out `skip` (`retro`, `body` or both)
/// still gets its run shipped and its PR opened, from templates — and each
/// stand-in is flagged: one `template_stand_in` event naming the file, which
/// `factory wait` relays, and one first line in the PR body. Returns the run
/// dir, the PR body and the cockpit's replayed lines.
fn stood_in_run(name: &str, skip: &[&str]) -> (Fixture, PathBuf, String, String) {
    let fx = Fixture::new(name);
    fx.chain();
    fx.write_plan(&plan_with(&[json!({"outcome": "polish", "skip": skip})]));
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(
        subjects(&fx)[subjects(&fx).len() - 2..],
        ["fix(001): polish review fixes", "docs(001): ship PRD-001"]
    );
    assert_eq!(fx.git(&["status", "--porcelain"]).trim(), "", "tree dirty");
    let body_path = dir.join("pr-body.md");
    assert_eq!(
        gh_calls(&fx, "pr create"),
        [format!(
            "pr create --draft --base main --title \"PRD-001: Fixture\" --body-file {} --head feat/fixture",
            body_path.display()
        )]
    );
    let body = fs::read_to_string(&body_path).unwrap();
    let out = factory(&fx, &["wait", &run_id(&dir), "--replay"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    (fx, dir, body, stdout(&out))
}

fn stand_in_relay(file: &str) -> String {
    format!(
        "stand-in: {file} is a template, the polish worker wrote none — the PR body's first line says so"
    )
}

#[test]
fn a_retro_the_polish_worker_left_out_is_a_flagged_stand_in() {
    let (fx, dir, body, relayed) = stood_in_run("closeout-stand-in-retro", &["retro"]);
    // The retro is the template, committed with the polish and filled at ship.
    let polish_sha = sha_of(&fx, "fix(001): polish review fixes");
    assert!(files_of(&fx, &polish_sha).contains(&RETRO.to_string()));
    assert!(!fx.read(RETRO).contains("Stub retro"), "{}", fx.read(RETRO));
    assert!(
        fx.read(RETRO).starts_with("# Retro — PRD-001\n"),
        "{}",
        fx.read(RETRO)
    );
    // One event, naming the retro; the body is the worker's, flagged first.
    let stood = of_kind(&dir, "template_stand_in");
    assert_eq!(stood.len(), 1, "{stood:?}");
    assert_eq!(stood[0]["what"], "retro");
    assert_eq!(stood[0]["path"], RETRO);
    assert_eq!(
        body.lines().next().unwrap(),
        format!(
            "**Template stand-in:** the retro (`{RETRO}`) is factory's template — the polish worker wrote none."
        )
    );
    assert!(
        body.contains("\n\n## PRD-001 — Fixture\n\nStub PR body"),
        "{body}"
    );
    assert_eq!(body.matches("**Template stand-in").count(), 1, "{body}");
    assert!(relayed.contains(&stand_in_relay(RETRO)), "{relayed}");
    assert_eq!(
        kinds_from(&dir, "polish_spawned"),
        [
            "polish_spawned",
            "template_stand_in",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_green",
            "run_finished"
        ]
    );
}

#[test]
fn a_pr_body_the_polish_worker_left_out_is_a_flagged_stand_in() {
    let (fx, dir, body, relayed) = stood_in_run("closeout-stand-in-body", &["body"]);
    assert!(fx.read(RETRO).contains("Stub retro"), "the worker's retro");
    let path = dir.join("pr-body.md").display().to_string();
    let stood = of_kind(&dir, "template_stand_in");
    assert_eq!(stood.len(), 1, "{stood:?}");
    assert_eq!(stood[0]["what"], "pr_body");
    assert_eq!(stood[0]["path"], path.as_str());
    assert!(
        body.starts_with(
            "**Template stand-in:** this PR body is factory's template — the polish worker wrote none.\n\n## PRD-001 — fixture\n"
        ),
        "{body}"
    );
    assert!(relayed.contains(&stand_in_relay(&path)), "{relayed}");
}

#[test]
fn a_polish_worker_that_wrote_neither_gets_both_flagged() {
    let (_fx, dir, body, relayed) = stood_in_run("closeout-stand-in-both", &["retro", "body"]);
    let path = dir.join("pr-body.md").display().to_string();
    let stood = of_kind(&dir, "template_stand_in");
    let named: Vec<(&str, &str)> = stood
        .iter()
        .map(|e| (e["what"].as_str().unwrap(), e["path"].as_str().unwrap()))
        .collect();
    assert_eq!(named, [("retro", RETRO), ("pr_body", path.as_str())]);
    assert_eq!(
        body.lines().next().unwrap(),
        format!(
            "**Template stand-ins:** this PR body and the retro (`{RETRO}`) are factory's templates — the polish worker wrote neither."
        )
    );
    assert_eq!(body.matches("**Template stand-in").count(), 1, "{body}");
    for file in [RETRO, path.as_str()] {
        assert!(relayed.contains(&stand_in_relay(file)), "{relayed}");
    }
}

#[test]
fn a_slice_not_done_at_ship_blocks_the_closeout_in_phase_polish() {
    let fx = Fixture::new("closeout-ship-refused");
    fx.chain();
    // The stub polish worker flips slice 02 back to `building` before it
    // returns: the polish commit lands, `docs ship` refuses.
    fx.write_plan(&plan_with(&[
        json!({"outcome": "polish", "slices": {"02": "building"}}),
    ]));
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "polish");
    assert_eq!(blocked["block_id"], "BLOCK-1");
    let q = blocked["question"].as_str().unwrap();
    assert!(
        q.starts_with("docs ship PRD-001 refused: refusing: 1 slice(s) not done"),
        "{q}"
    );
    // One `block` entry in phase polish, its body quoting the refusal.
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:#?}");
    let entry = block_entry(&fx, "BLOCK-1", "polish");
    assert_eq!(entry["title"], q);
    let log = entry["body"].as_str().unwrap();
    assert!(
        log.contains("**Happened:** the polish commit is in and `docs ship 001` refused"),
        "{log}"
    );
    assert!(
        log.contains("\n  refusing: 1 slice(s) not done (02)."),
        "{log}"
    );
    assert!(log.contains("**Recommended:** 1"), "{log}");

    assert_eq!(
        kinds_from(&dir, "polish_spawned")[..4],
        [
            "polish_spawned",
            "polish_done",
            "closeout_blocked",
            "run_waiting"
        ]
    );
    assert!(subjects(&fx).contains(&"fix(001): polish review fixes".to_string()));
    assert!(!subjects(&fx).contains(&"docs(001): ship PRD-001".to_string()));
    assert_eq!(fx.state().status, "ready");
    assert!(gh_calls(&fx, "pr create").is_empty(), "{:?}", fx.gh_log());
    assert_eq!(origin_head(&fx, "feat/fixture"), None, "nothing pushed");
    stop(&fx, &dir);
}

#[test]
fn a_red_check_gets_one_fix_round_then_green() {
    let fx = Fixture::new("closeout-ci-red-green");
    fx.chain();
    fx.script_ci(&["red:test", "green"]);
    fx.write_plan(&plan_with(&[step("polish"), step("fix")]));
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 120);

    // One fix worker, role `fix` with no checkpoint, on the CI pack.
    let p = prompts(&fx);
    assert_eq!(p.len(), 7, "{p:?}");
    assert_eq!(p[6], CI_FIX);
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "fix"]);
    let fixer = &db(&fx).workers(&run_id(&dir)).unwrap()[6];
    assert_eq!(fixer.checkpoint, None);
    assert_eq!(fixer.slice, None);
    assert_eq!(fixer.outcome.as_deref(), Some("done"));
    let (name, pack) = pack_of(&fx.calls()[6]);
    assert_eq!(name, "ci-fix-1-7.md");
    for want in [
        "<!-- factory-pack files: -->\n\n# CI failure",
        "# CI failure\n\n<!-- round 1, attempt 1, check: test -->",
        "Failing check: `test`",
        "Gate command: `./gate.sh`",
        "FAIL test: expected 2 got 3",
        "# Contract",
        "fix round 1, unattended",
        "--kind block --phase ci",
        "# Autonomy",
        "# Inner loop",
    ] {
        assert!(
            pack.contains(want),
            "{want:?} missing from the CI fix pack:\n{pack}"
        );
    }
    harness::assert_v3_contract(&pack);

    // Its commit is `fix(001): CI test`, pushed; the report counts the cycle.
    assert_eq!(
        subjects(&fx)[7..],
        [
            "fix(001): polish review fixes",
            "docs(001): ship PRD-001",
            "fix(001): CI test"
        ]
    );
    let fix_sha = sha_of(&fx, "fix(001): CI test");
    assert_eq!(files_of(&fx, &fix_sha), ["src/lib.txt".to_string()]);
    assert!(fx.read("src/lib.txt").contains("fixed by spawn 7\n"));
    // FR-19: the CI round leaves the retro as ship wrote it.
    let ship_sha = sha_of(&fx, "docs(001): ship PRD-001");
    assert_eq!(
        fx.git(&["show", &format!("{ship_sha}:{RETRO}")]),
        fx.read(RETRO)
    );
    assert_eq!(fx.state().status, "shipped");
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));
    // FR-24: one red round, no rerun, one fix cycle.
    let out = factory(&fx, &["report", &run_id(&dir), "--json"]);
    let r: Value = serde_json::from_str(&stdout(&out)).expect("one JSON object");
    assert_eq!(
        (&r["ci_red_rounds"], &r["ci_reruns"], &r["ci_cycles"]),
        (&json!(1), &json!(0), &json!(1)),
        "{r}"
    );

    // Two `pr checks`, one run list, one log read, no rerun, one PR.
    assert_eq!(gh_calls(&fx, "pr checks").len(), 2, "{:?}", fx.gh_log());
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert_eq!(
        gh_calls(&fx, "run list"),
        [format!(
            "run list --commit {} --workflow CI --limit 100 --json {RUN_FIELDS}",
            sha_of(&fx, "docs(001): ship PRD-001")
        )]
    );
    assert_eq!(
        gh_calls(&fx, "run view"),
        [
            format!("run view 1000 --json {RUN_FIELDS}"),
            "run view 1000 --log-failed".to_string()
        ]
    );
    assert!(gh_calls(&fx, "run rerun").is_empty());
    assert!(gh_calls(&fx, "pr edit").is_empty());

    assert_eq!(
        kinds_from(&dir, "polish_spawned"),
        [
            "polish_spawned",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_red",
            "ci_fix_spawned",
            "ci_fix_committed",
            "ci_green",
            "run_finished",
        ]
    );
    let red = &of_kind(&dir, "ci_red")[0];
    assert_eq!(red["check"], "test");
    assert_eq!(red["round"], 0);
    let spawned = &of_kind(&dir, "ci_fix_spawned")[0];
    assert_eq!(spawned["round"], 1);
    assert_eq!(spawned["attempt"], 1);
    let committed = &of_kind(&dir, "ci_fix_committed")[0];
    assert_eq!(committed["round"], 1);
    assert_eq!(committed["sha"], fix_sha);
    let last = events(&dir).pop().unwrap();
    assert_eq!(last["kind"], "run_finished");
    assert_eq!(last["ci_cycles"], 1);
    assert_eq!(last["pr_url"], PR_URL);
}

#[test]
fn a_flaky_check_is_rerun_once_then_green() {
    let fx = Fixture::new("closeout-ci-flaky");
    fx.chain();
    fx.script_ci(&["flaky:test", "green"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 90);

    assert_eq!(prompts(&fx).len(), 6, "no fix worker: {:?}", prompts(&fx));
    assert!(!roles(&fx, &dir).contains(&"fix".to_string()));
    assert_eq!(gh_calls(&fx, "run rerun"), ["run rerun 1000 --failed"]);
    assert_eq!(gh_calls(&fx, "pr checks").len(), 2);
    assert!(!subjects(&fx).iter().any(|s| s.starts_with("fix(001): CI")));
    assert_eq!(report_json(&fx, &dir)["ci_cycles"], 0);
    assert_eq!(
        kinds_from(&dir, "pr_opened"),
        [
            "pr_opened",
            "ci_red",
            "ci_rerun",
            "ci_green",
            "run_finished"
        ]
    );
    let rerun = &of_kind(&dir, "ci_rerun")[0];
    assert_eq!(rerun["check"], "test");
    assert_eq!(rerun["run_id"], 1000);
    assert_eq!(rerun["head"], head(&fx));
}

#[test]
fn a_flaky_check_red_after_its_rerun_blocks_and_the_answer_resumes_at_the_pr() {
    let fx = Fixture::new("closeout-ci-flaky-twice");
    fx.chain();
    fx.script_ci(&["flaky:test", "flaky:test"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "ci");
    assert_eq!(blocked["block_id"], "BLOCK-1");
    assert_eq!(
        blocked["question"],
        "flaky check test red again after its rerun"
    );
    block_entry(&fx, "BLOCK-1", "ci");
    wait_phase(&fx, &dir, "waiting-owner", 30);
    assert_eq!(gh_calls(&fx, "run rerun").len(), 1);
    assert_eq!(gh_calls(&fx, "pr checks").len(), 2);
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert!(gh_calls(&fx, "pr edit").is_empty());
    assert_eq!(prompts(&fx).len(), 6, "no fix worker for a flaky check");
    assert_eq!(
        kinds_from(&dir, "pr_opened"),
        [
            "pr_opened",
            "ci_red",
            "ci_rerun",
            "ci_red",
            "closeout_blocked",
            "run_waiting"
        ]
    );

    // The answer resumes at the PR: the body is refreshed, no second PR, and
    // the checks (now green) finish the run.
    let out = factory(&fx, &["answer", "BLOCK-1", "rerun it once more"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 60);
    assert_eq!(gh_calls(&fx, "pr create").len(), 1, "{:?}", fx.gh_log());
    assert_eq!(
        gh_calls(&fx, "pr edit"),
        [format!(
            "pr edit 1 --body-file {}",
            dir.join("pr-body.md").display()
        )]
    );
    assert_eq!(gh_calls(&fx, "pr checks").len(), 3);
    assert_eq!(
        kinds_from(&dir, "closeout_blocked"),
        [
            "closeout_blocked",
            "run_waiting",
            "block_answered",
            "ci_green",
            "run_finished"
        ]
    );
    // The block entry stands as written; an `answer` entry resolves it.
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:#?}");
    assert_eq!(blocks[0]["id"], "BLOCK-1");
    assert_eq!(blocks[0]["phase"], "ci");
    assert_eq!(
        blocks[0]["title"],
        "flaky check test red again after its rerun"
    );
    assert!(answered_in_log(&fx, "BLOCK-1"), "{:#?}", fx.log_entries());
    assert_eq!(events(&dir).pop().unwrap()["pr_url"], PR_URL);
}

#[test]
fn three_fix_rounds_on_a_red_check_block_the_closeout() {
    let fx = Fixture::new("closeout-ci-red-four");
    fx.chain();
    fx.script_ci(&["red:test", "red:test", "red:test", "red:test"]);
    fx.write_plan(&plan_with(&[done(), step("fix"), step("fix"), step("fix")]));
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 150);
    assert_eq!(blocked["phase"], "ci");
    assert_eq!(blocked["question"], "CI red after 3 fix rounds: test");
    block_entry(&fx, "BLOCK-1", "ci");

    assert_eq!(roles(&fx, &dir)[5..], ["polish", "fix", "fix", "fix"]);
    assert_eq!(
        subjects(&fx)[8..],
        [
            "docs(001): ship PRD-001",
            "fix(001): CI test",
            "fix(001): CI test",
            "fix(001): CI test"
        ]
    );
    assert_eq!(report_json(&fx, &dir)["ci_cycles"], 3);
    assert_eq!(gh_calls(&fx, "pr checks").len(), 4);
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert!(gh_calls(&fx, "run rerun").is_empty());
    let rounds: Vec<u64> = of_kind(&dir, "ci_fix_spawned")
        .iter()
        .map(|e| e["round"].as_u64().unwrap())
        .collect();
    assert_eq!(rounds, [1, 2, 3]);
    let names: Vec<String> = fx.calls()[6..].iter().map(|c| pack_of(c).0).collect();
    assert_eq!(names, ["ci-fix-1-7.md", "ci-fix-2-8.md", "ci-fix-3-9.md"]);
    assert_eq!(
        origin_head(&fx, "feat/fixture"),
        Some(head(&fx)),
        "every fix pushed"
    );
    stop(&fx, &dir);
}

#[test]
fn a_red_check_at_a_foreign_head_blocks_without_a_fix_worker() {
    let fx = Fixture::new("closeout-ci-foreign-head");
    fx.chain();
    fx.script_ci(&["red:test@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "ci");
    assert_eq!(
        blocked["question"],
        "CI red at a head this branch does not have: test"
    );
    block_entry(&fx, "BLOCK-1", "ci");
    assert_eq!(prompts(&fx).len(), 6, "{:?}", prompts(&fx));
    assert!(gh_calls(&fx, "run rerun").is_empty());
    assert_eq!(
        gh_calls(&fx, "run view"),
        [format!("run view 1000 --json {RUN_FIELDS}")]
    );
    assert!(!subjects(&fx).iter().any(|s| s.starts_with("fix(001): CI")));
    stop(&fx, &dir);
}

#[test]
fn a_red_check_beside_another_workflows_green_is_not_flaky() {
    let fx = Fixture::new("closeout-ci-other-green");
    fx.chain();
    fx.script_ci(&["red:test pass:lint/Lint", "pass:test pass:lint/Lint"]);
    fx.write_plan(&plan_with(&[step("polish"), step("fix")]));
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 120);

    // `Lint` green at the head says nothing of `test`: only another run of
    // `CI` passing there would make it flaky. A fix round, no rerun.
    assert!(gh_calls(&fx, "run rerun").is_empty(), "{:?}", fx.gh_log());
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "fix"]);
    assert_eq!(
        gh_calls(&fx, "run list"),
        [format!(
            "run list --commit {} --workflow CI --limit 100 --json {RUN_FIELDS}",
            sha_of(&fx, "docs(001): ship PRD-001")
        )]
    );
    assert_eq!(
        kinds_from(&dir, "pr_opened"),
        [
            "pr_opened",
            "ci_red",
            "ci_fix_spawned",
            "ci_fix_committed",
            "ci_green",
            "run_finished"
        ]
    );
}

#[test]
fn two_red_workflows_get_one_fix_round_with_both_runs_logs() {
    let fx = Fixture::new("closeout-ci-two-red");
    fx.chain();
    fx.script_ci(&["red:test red:lint/Lint", "pass:test pass:lint/Lint"]);
    fx.write_plan(&plan_with(&[step("polish"), step("fix")]));
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 120);

    // Both checks in one round; each run comes from its check's job link —
    // `CI`'s 1000 and `Lint`'s 1100 — and both logs reach the worker.
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "fix"]);
    assert_eq!(of_kind(&dir, "ci_red")[0]["check"], "test, lint");
    assert_eq!(
        gh_calls(&fx, "run view"),
        [
            format!("run view 1000 --json {RUN_FIELDS}"),
            format!("run view 1100 --json {RUN_FIELDS}"),
            "run view 1000 --log-failed".to_string(),
            "run view 1100 --log-failed".to_string(),
        ]
    );
    let (name, pack) = pack_of(&fx.calls()[6]);
    assert_eq!(name, "ci-fix-1-7.md");
    for want in [
        "Failing check: `test, lint`",
        "--- run 1000 (`CI`), failed: test\n",
        "FAIL test: expected 2 got 3",
        "--- run 1100 (`Lint`), failed: lint\n",
        "FAIL lint: expected 2 got 3",
    ] {
        assert!(pack.contains(want), "{want:?} missing from:\n{pack}");
    }
    assert!(
        subjects(&fx).contains(&"fix(001): CI test, lint".to_string()),
        "{:?}",
        subjects(&fx)
    );
    assert!(gh_calls(&fx, "run rerun").is_empty());
    assert_eq!(of_kind(&dir, "ci_green")[0]["head"], head(&fx));
}

#[test]
fn a_timeout_or_a_runner_that_never_started_blocks_at_once_and_proceed_ends_the_watch() {
    let fx = Fixture::new("closeout-ci-infra");
    fx.chain();
    // `lint` is still running: the block does not wait for it.
    fx.script_ci(&["timed_out:test startup_failure:build/Build pending:lint/Lint"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "ci");
    assert_eq!(
        blocked["question"],
        "CI did not run to a verdict: test, build"
    );
    block_entry(&fx, "BLOCK-1", "ci");
    let log = block_body(&fx, "BLOCK-1");
    for want in [
        "  test\tTIMED_OUT\thttps://example.invalid/actions/runs/1000/job/5000\n",
        "  build\tSTARTUP_FAILURE\thttps://example.invalid/actions/runs/1100/job/5001\n",
        "answer `proceed` to take the checks over",
    ] {
        assert!(log.contains(want), "{want:?} missing from\n{log}");
    }
    wait_parked(&fx, &dir, 1, 30);
    assert_eq!(prompts(&fx).len(), 6, "no fix worker");
    assert_eq!(gh_calls(&fx, "pr checks").len(), 1);
    // Only to see whether a rerun is already running.
    assert_eq!(
        gh_calls(&fx, "run view"),
        [
            format!("run view 1000 --json {RUN_FIELDS}"),
            format!("run view 1100 --json {RUN_FIELDS}")
        ]
    );
    assert!(gh_calls(&fx, "run list").is_empty());
    assert!(gh_calls(&fx, "run rerun").is_empty());
    assert_eq!(
        kinds_from(&dir, "pr_opened"),
        ["pr_opened", "closeout_blocked", "run_waiting"]
    );

    answer(&fx, "BLOCK-1", "proceed");
    wait_phase(&fx, &dir, "finished", 60);
    assert_eq!(
        gh_calls(&fx, "pr checks").len(),
        1,
        "no watch after proceed"
    );
    assert!(of_kind(&dir, "ci_green").is_empty());
    assert_eq!(
        kinds_from(&dir, "block_answered"),
        ["block_answered", "run_finished"]
    );
    assert_eq!(events(&dir).pop().unwrap()["pr_url"], PR_URL);
}

#[test]
fn a_refused_run_list_blocks_instead_of_a_blind_fix_round() {
    let fx = Fixture::new("closeout-ci-run-list-refused");
    fx.chain();
    gh_fail(&fx, &["run list"]);
    fx.script_ci(&["red:test"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "ci");
    assert_eq!(
        blocked["question"],
        "gh run list refused: stub gh: scripted failure: run list"
    );
    block_entry(&fx, "BLOCK-1", "ci");
    assert_eq!(prompts(&fx).len(), 6, "no fix worker: {:?}", prompts(&fx));
    assert!(gh_calls(&fx, "run rerun").is_empty());
    assert!(!subjects(&fx).iter().any(|s| s.starts_with("fix(001): CI")));
    stop(&fx, &dir);
}

#[test]
fn no_checks_right_after_the_push_are_waited_on_for_the_grace() {
    let fx = Fixture::new("closeout-ci-none-then-green");
    fx.chain();
    fx.script_ci(&["none", "none", "green"]);
    fx.write_plan(&built());
    let dir = start_with(&fx, &FAST);
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(gh_calls(&fx, "pr checks").len(), 3, "{:?}", fx.gh_log());
    assert_eq!(of_kind(&dir, "ci_green").len(), 1);
    assert_eq!(
        driver_log(&dir)
            .matches("ci: waiting — no checks reported yet")
            .count(),
        1,
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn after_a_fix_push_the_watch_waits_for_the_pr_to_reach_the_pushed_head() {
    let fx = Fixture::new("closeout-ci-lag");
    fx.chain();
    // Poll 2: the PR still shows the ship commit and its red.
    fx.script_ci(&["red:test", "lag", "green"]);
    fx.write_plan(&plan_with(&[step("polish"), step("fix")]));
    let dir = start_with(&fx, &FAST);
    wait_phase(&fx, &dir, "finished", 120);

    // One fix round: the old red the PR still showed was not a second one.
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "fix"]);
    assert_eq!(of_kind(&dir, "ci_red").len(), 1);
    assert_eq!(
        gh_calls(&fx, "pr view"),
        [
            "pr view feat/fixture --json url,state",
            "pr view 1 --json headRefOid",
            "pr view 1 --json headRefOid",
            "pr view 1 --json headRefOid"
        ]
    );
    assert_eq!(gh_calls(&fx, "pr checks").len(), 2, "{:?}", fx.gh_log());
    assert_eq!(of_kind(&dir, "ci_green")[0]["head"], head(&fx));
    let ship = sha_of(&fx, "docs(001): ship PRD-001");
    let fix = head(&fx);
    assert!(
        driver_log(&dir).contains(&format!(
            "ci: waiting — the PR is still at {}, the push of {} has not reached it",
            short(&ship),
            short(&fix)
        )),
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn after_a_rerun_the_stale_red_is_waited_on_not_judged() {
    let fx = Fixture::new("closeout-ci-rerun-stale");
    fx.chain();
    // Poll 2: the rerun has not registered, the old red row is back.
    fx.script_ci(&["flaky:test", "stale", "green"]);
    fx.write_plan(&built());
    let dir = start_with(&fx, &FAST);
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(gh_calls(&fx, "run rerun"), ["run rerun 1000 --failed"]);
    assert_eq!(gh_calls(&fx, "pr checks").len(), 3);
    assert!(of_kind(&dir, "closeout_blocked").is_empty());
    assert_eq!(
        kinds_from(&dir, "pr_opened"),
        [
            "pr_opened",
            "ci_red",
            "ci_rerun",
            "ci_green",
            "run_finished"
        ]
    );
    assert!(
        driver_log(&dir).contains("ci: waiting — the rerun of `test` has not registered"),
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn a_refused_rerun_blocks_instead_of_a_fix_round() {
    let fx = Fixture::new("closeout-ci-rerun-refused");
    fx.chain();
    gh_fail(&fx, &["run rerun"]);
    fx.script_ci(&["flaky:test"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "ci");
    assert_eq!(
        blocked["question"],
        "gh run rerun 1000 refused: stub gh: scripted failure: run rerun"
    );
    block_entry(&fx, "BLOCK-1", "ci");
    assert_eq!(gh_calls(&fx, "run rerun"), ["run rerun 1000 --failed"]);
    assert!(of_kind(&dir, "ci_rerun").is_empty());
    assert_eq!(prompts(&fx).len(), 6, "no fix worker: {:?}", prompts(&fx));
    assert!(!subjects(&fx).iter().any(|s| s.starts_with("fix(001): CI")));
    stop(&fx, &dir);
}

#[test]
fn a_second_flaky_check_at_the_same_head_gets_its_own_rerun() {
    let fx = Fixture::new("closeout-ci-two-flaky");
    fx.chain();
    fx.script_ci(&[
        "flaky:test pass:lint",
        "pass:test flaky:lint",
        "pass:test pass:lint",
    ]);
    fx.write_plan(&built());
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(
        gh_calls(&fx, "run rerun"),
        ["run rerun 1000 --failed", "run rerun 1001 --failed"]
    );
    let reruns: Vec<Value> = of_kind(&dir, "ci_rerun")
        .iter()
        .map(|e| json!([e["check"], e["run_id"]]))
        .collect();
    assert_eq!(reruns, [json!(["test", 1000]), json!(["lint", 1001])]);
    assert!(of_kind(&dir, "closeout_blocked").is_empty());
    assert_eq!(prompts(&fx).len(), 6, "no fix worker");
    assert_eq!(of_kind(&dir, "ci_green").len(), 1);
}

#[test]
fn a_stop_during_a_pending_check_ends_the_watch_within_seconds() {
    let fx = Fixture::new("closeout-ci-stop");
    fx.chain();
    // The default interval: the next poll is a minute away.
    fx.script_ci(&["pending:test"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    wait_until(&fx, &dir, "the watch waiting", 90, || {
        driver_log(&dir).contains("ci: waiting — `test` running")
    });
    let asked = Instant::now();
    stop(&fx, &dir);
    assert!(
        asked.elapsed() < Duration::from_secs(20),
        "the stop took {:?}",
        asked.elapsed()
    );
    assert_eq!(of_kind(&dir, "run_stopped").len(), 1);
    assert!(of_kind(&dir, "ci_green").is_empty());
    assert_eq!(gh_calls(&fx, "pr checks").len(), 1);
}

#[test]
fn a_check_pending_past_the_timeout_blocks_and_proceed_ends_the_watch() {
    let fx = Fixture::new("closeout-ci-timeout");
    fx.chain();
    fx.script_ci(&["pending:test"; 60]);
    fx.write_plan(&built());
    let dir = start_with(
        &fx,
        &[("FACTORY_CI_INTERVAL", "1"), ("FACTORY_CI_TIMEOUT", "3")],
    );
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "ci");
    assert_eq!(blocked["question"], "CI still pending after 3s");
    block_entry(&fx, "BLOCK-1", "ci");
    let log = block_body(&fx, "BLOCK-1");
    assert!(log.contains(": `test` running. The last table:\n"), "{log}");
    wait_parked(&fx, &dir, 1, 30);
    let polls = gh_calls(&fx, "pr checks").len();
    assert!((3..=6).contains(&polls), "{polls} polls in 3s");
    assert!(of_kind(&dir, "ci_green").is_empty());

    answer(&fx, "BLOCK-1", "`proceed`");
    wait_phase(&fx, &dir, "finished", 60);
    assert_eq!(
        gh_calls(&fx, "pr checks").len(),
        polls,
        "no watch after proceed"
    );
    assert!(of_kind(&dir, "ci_green").is_empty());
    assert_eq!(
        kinds_from(&dir, "block_answered"),
        ["block_answered", "run_finished"]
    );
}

#[test]
fn proceed_to_the_flaky_block_finishes_without_another_watch() {
    let fx = Fixture::new("closeout-ci-flaky-proceed");
    fx.chain();
    fx.script_ci(&["flaky:test", "flaky:test"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(
        blocked["question"],
        "flaky check test red again after its rerun"
    );
    wait_parked(&fx, &dir, 1, 30);
    answer(&fx, "BLOCK-1", "2. proceed");
    wait_phase(&fx, &dir, "finished", 60);
    assert_eq!(gh_calls(&fx, "pr checks").len(), 2, "{:?}", fx.gh_log());
    assert!(of_kind(&dir, "ci_green").is_empty());
    assert_eq!(
        kinds_from(&dir, "block_answered"),
        ["block_answered", "run_finished"]
    );
    assert!(
        driver_log(&dir).contains("ci: BLOCK-1 answered `proceed` — the checks are the owner's"),
        "{}",
        driver_log(&dir)
    );
    assert_eq!(events(&dir).pop().unwrap()["pr_url"], PR_URL);
}

#[test]
fn proceed_to_a_refused_pr_create_finishes_with_the_owners_pr() {
    let fx = Fixture::new("closeout-pr-create-proceed");
    fx.chain();
    gh_fail(&fx, &["pr create"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "pr");
    assert_eq!(
        blocked["question"],
        "gh pr create refused: stub gh: scripted failure: pr create"
    );
    wait_parked(&fx, &dir, 1, 30);
    answer(&fx, "BLOCK-1", "proceed");
    wait_phase(&fx, &dir, "finished", 60);
    assert_eq!(gh_calls(&fx, "pr create").len(), 1, "{:?}", fx.gh_log());
    assert!(gh_calls(&fx, "pr checks").is_empty());
    assert!(state(&dir)["pr_url"].is_null());
    assert_eq!(
        kinds_from(&dir, "block_answered"),
        ["block_answered", "run_finished"]
    );
    assert!(events(&dir).pop().unwrap()["pr_url"].is_null());
}

#[test]
fn an_answer_to_the_cap_block_gives_three_fresh_fix_rounds() {
    let fx = Fixture::new("closeout-ci-cap-answered");
    fx.chain();
    fx.script_ci(&["red:test"; 5]);
    fx.write_plan(&plan_with(&[
        done(),
        step("fix"),
        step("fix"),
        step("fix"),
        step("fix"),
    ]));
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 150);
    assert_eq!(blocked["question"], "CI red after 3 fix rounds: test");
    wait_parked(&fx, &dir, 1, 30);
    answer(&fx, "BLOCK-1", "fixed the fixture by hand");
    wait_phase(&fx, &dir, "finished", 120);

    // The cap counts the rounds since the answer: a fourth round runs.
    let rounds: Vec<u64> = of_kind(&dir, "ci_fix_spawned")
        .iter()
        .map(|e| e["round"].as_u64().unwrap())
        .collect();
    assert_eq!(rounds, [1, 2, 3, 4]);
    let reds: Vec<u64> = of_kind(&dir, "ci_red")
        .iter()
        .map(|e| e["round"].as_u64().unwrap())
        .collect();
    assert_eq!(reds, [0, 1, 2, 3, 0]);
    let names: Vec<String> = fx.calls()[6..].iter().map(|c| pack_of(c).0).collect();
    assert_eq!(
        names,
        [
            "ci-fix-1-7.md",
            "ci-fix-2-8.md",
            "ci-fix-3-9.md",
            "ci-fix-4-10.md"
        ]
    );
    assert_eq!(report_json(&fx, &dir)["ci_cycles"], 4);
    assert_eq!(of_kind(&dir, "ci_green")[0]["head"], head(&fx));
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));
}

#[test]
fn the_push_refuses_a_head_off_the_feature_branch_and_the_pr_names_its_head() {
    let fx = Fixture::new("closeout-push-off-branch");
    fx.chain();
    gh_fail(&fx, &["pr create"]);
    fx.write_plan(&built());
    let dir = start(&fx);
    let first = wait_block_id(&fx, &dir, "BLOCK-1", 90);
    assert_eq!(first["phase"], "pr");
    wait_parked(&fx, &dir, 1, 30);

    // HEAD moves off the feature branch while the run is parked.
    fx.git(&["switch", "-c", "stray"]);
    answer(&fx, "BLOCK-1", "gh is fixed");
    let second = wait_block_id(&fx, &dir, "BLOCK-2", 60);
    assert_eq!(second["phase"], "pr");
    assert_eq!(
        second["question"],
        "push stopped: HEAD is on `stray`, not the feature branch `feat/fixture`"
    );
    block_entry(&fx, "BLOCK-2", "pr");
    assert!(
        block_body(&fx, "BLOCK-2")
            .contains("1. put HEAD back on the feature branch (`git switch feat/fixture`"),
        "{}",
        block_body(&fx, "BLOCK-2")
    );
    assert_eq!(origin_head(&fx, "stray"), None, "nothing pushed from stray");
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    wait_parked(&fx, &dir, 2, 30);

    fx.git(&["switch", "feat/fixture"]);
    answer(&fx, "BLOCK-2", "back on the branch");
    wait_phase(&fx, &dir, "finished", 60);
    let create = format!(
        "pr create --draft --base main --title \"PRD-001: Fixture\" --body-file {} --head feat/fixture",
        dir.join("pr-body.md").display()
    );
    assert_eq!(gh_calls(&fx, "pr create"), [create.clone(), create]);
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));
    assert_eq!(origin_head(&fx, "stray"), None);
    assert_eq!(state(&dir)["pr_url"], PR_URL);
}

#[test]
fn a_head_off_the_feature_branch_blocks_the_polish_before_a_worker_runs() {
    let fx = Fixture::new("closeout-polish-off-branch");
    fx.chain();
    fx.write_plan(&plan_with(&[step("dead"), step("dead"), step("polish")]));
    let dir = start(&fx);
    let first = wait_block_id(&fx, &dir, "BLOCK-1", 90);
    assert_eq!(first["question"], "Two polish workers died");
    wait_parked(&fx, &dir, 1, 30);

    // The owner answers from another branch: the closeout, not the
    // driver's tick, owns the check and asks with a block (AD-50001).
    let branched = head(&fx);
    fx.git(&["switch", "-c", "stray"]);
    answer(&fx, "BLOCK-1", "login fixed");
    let second = wait_block_id(&fx, &dir, "BLOCK-2", 60);
    assert_eq!(second["phase"], "polish");
    assert_eq!(
        second["question"],
        "polish stopped: HEAD is on `stray`, not the feature branch `feat/fixture`"
    );
    block_entry(&fx, "BLOCK-2", "polish");
    wait_parked(&fx, &dir, 2, 30);
    assert_eq!(
        roles(&fx, &dir)[5..],
        ["polish", "polish"],
        "no polish worker on stray"
    );

    fx.git(&["switch", "feat/fixture"]);
    answer(&fx, "BLOCK-2", "back on the branch");
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "polish", "polish"]);
    assert_eq!(
        fx.git(&["rev-parse", "stray"]).trim(),
        branched,
        "nothing committed on stray"
    );
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));
    assert_eq!(origin_head(&fx, "stray"), None);
}

/// A tracked `src/extra.txt` a closeout worker can change without naming it.
fn track_extra(fx: &Fixture) {
    fs::write(fx.path().join("src/extra.txt"), "extra\n").unwrap();
    fx.git(&["add", "src/extra.txt"]);
    fx.git(&["commit", "-q", "-m", "chore: track extra"]);
}

#[test]
fn closeout_workers_commit_their_unnamed_tracked_changes_and_list_the_rest() {
    let fx = Fixture::new("closeout-unnamed");
    fx.chain();
    track_extra(&fx);
    fx.script_ci(&["red:test", "green"]);
    // A formatter's edit to a tracked file, and a scratch file: the polish
    // worker leaves both, then the CI fix worker the same again (AD-50002).
    let stray = json!(["src/extra.txt", "scratch.txt"]);
    fx.write_plan(&plan_with(&[
        json!({"outcome": "polish", "stray": stray}),
        json!({"outcome": "fix", "stray": stray}),
    ]));
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 120);

    let polish = files_of(&fx, &sha_of(&fx, "fix(001): polish review fixes"));
    assert!(polish.contains(&"src/extra.txt".to_string()), "{polish:?}");
    assert!(!polish.contains(&"scratch.txt".to_string()), "{polish:?}");
    let fix = files_of(&fx, &sha_of(&fx, "fix(001): CI test"));
    assert!(fix.contains(&"src/extra.txt".to_string()), "{fix:?}");
    assert!(!fix.contains(&"scratch.txt".to_string()), "{fix:?}");
    // Listed once, by the worker that left it; never committed.
    let left = of_kind(&dir, "closeout_leftovers");
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!(left[0]["role"], "polish");
    assert_eq!(left[0]["files"], json!(["scratch.txt"]));
    assert_eq!(fx.git(&["status", "--porcelain"]).trim(), "?? scratch.txt");
    assert!(
        driver_log(&dir).contains("polish: worker changed 1 tracked file(s) no tool call named"),
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn a_shipped_board_without_its_ship_commit_ships_again() {
    let fx = Fixture::new("closeout-ship-uncommitted");
    fx.chain();
    fx.write_plan(&plan_with(&[step("dead"), step("dead")]));
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["question"], "Two polish workers died");
    wait_parked(&fx, &dir, 1, 30);

    // `docs ship` ran and its commit did not: what a driver killed between
    // the two leaves behind.
    fx.docs(&["ship", "001"]);
    assert_eq!(fx.state().status, "shipped");
    assert_ne!(
        fx.git(&["status", "--porcelain"]).trim(),
        "",
        "the ship is staged, not committed"
    );
    answer(&fx, "BLOCK-1", "the board is shipped, go on");
    wait_phase(&fx, &dir, "finished", 90);

    assert_eq!(
        roles(&fx, &dir)[5..],
        ["polish", "polish"],
        "no third polish"
    );
    assert_eq!(
        fx.git(&["log", "-1", "--format=%s"]).trim(),
        "docs(001): ship PRD-001"
    );
    let ship = sha_of(&fx, "docs(001): ship PRD-001");
    assert!(files_of(&fx, &ship).contains(&STATE.to_string()));
    assert!(
        fx.git(&["show", &format!("HEAD:{STATE}")])
            .contains("status = \"shipped\""),
        "HEAD holds the ship"
    );
    assert_eq!(fx.git(&["status", "--porcelain"]).trim(), "", "tree dirty");
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));
    assert!(
        driver_log(&dir).contains("closeout — PRD shipped but not committed, resumed at ship"),
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn proceed_to_the_dead_polish_block_ships_without_a_third_worker() {
    let fx = Fixture::new("closeout-polish-proceed");
    fx.chain();
    fx.write_plan(&plan_with(&[step("dead"), step("dead")]));
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["question"], "Two polish workers died");
    assert!(
        block_body(&fx, "BLOCK-1")
            .contains("commit, and answer this block with `proceed` — the driver ships from there"),
        "{}",
        block_body(&fx, "BLOCK-1")
    );
    wait_parked(&fx, &dir, 1, 30);
    answer(&fx, "BLOCK-1", "proceed");
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(
        roles(&fx, &dir)[5..],
        ["polish", "polish"],
        "no third polish"
    );
    assert_eq!(subjects(&fx)[7..], ["docs(001): ship PRD-001"]);
    assert_eq!(fx.state().status, "shipped");
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert_eq!(events(&dir).pop().unwrap()["kind"], "run_finished");
}

#[test]
fn two_dead_polish_workers_block_and_the_answer_spawns_a_third() {
    let fx = Fixture::new("closeout-polish-dead");
    fx.chain();
    fx.write_plan(&plan_with(&[step("dead"), step("dead"), step("polish")]));
    let dir = start(&fx);
    let blocked = wait_block(&fx, &dir, 90);
    assert_eq!(blocked["phase"], "polish");
    assert_eq!(blocked["question"], "Two polish workers died");
    block_entry(&fx, "BLOCK-1", "polish");
    wait_phase(&fx, &dir, "waiting-owner", 30);
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "polish"]);
    assert_eq!(
        kinds_from(&dir, "polish_spawned"),
        [
            "polish_spawned",
            "worker_dead",
            "polish_spawned",
            "worker_dead",
            "closeout_blocked",
            "run_waiting",
        ]
    );
    // Each death is a `note` entry (the old `## Traps` line).
    let notes = fx.log_of("note");
    assert_eq!(
        notes
            .iter()
            .filter(|e| e["title"] == "Dead polish worker")
            .count(),
        2,
        "{notes:#?}"
    );
    assert!(!subjects(&fx).contains(&"fix(001): polish review fixes".to_string()));
    assert!(gh_calls(&fx, "pr create").is_empty());

    let out = factory(&fx, &["answer", "BLOCK-1", "try once more"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "polish", "polish"]);
    assert_eq!(prompts(&fx)[7], "/polish");
    assert_eq!(
        subjects(&fx)[7..],
        ["fix(001): polish review fixes", "docs(001): ship PRD-001"]
    );
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert_eq!(events(&dir).pop().unwrap()["kind"], "run_finished");
}

/// What in `text` (one source file) asks GitHub to merge a PR or mark one
/// ready, or `None`. Every way the driver could: the `gh` subcommand as one
/// string, as an args array (read with whitespace removed, so rustfmt's
/// one-arg-per-line split reads as one), the REST merge endpoint, the
/// GraphQL mutations, and a `pr create` args array without `--draft`, which
/// opens the PR ready (K3).
fn merges_or_readies(text: &str) -> Option<&'static str> {
    let spaced = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let packed: String = text.split_whitespace().collect();
    for (form, hay) in [
        ("pr merge", &spaced),
        ("pr ready", &spaced),
        ("\"pr\",\"merge\"", &packed),
        ("\"pr\",\"ready\"", &packed),
        ("/merge", &spaced),
        ("mergePullRequest", &packed),
        ("markPullRequestReadyForReview", &packed),
        ("enablePullRequestAutoMerge", &packed),
    ] {
        if hay.contains(form) {
            return Some(form);
        }
    }
    let create = "\"pr\",\"create\"";
    for (at, _) in packed.match_indices(create) {
        let args = &packed[at..];
        let args = &args[..args.find(']').unwrap_or(args.len())];
        if !args.contains("\"--draft\"") {
            return Some("pr create without --draft");
        }
    }
    None
}

#[test]
fn the_driver_never_merges_or_readies_a_pr() {
    // The scan catches each form, and passes the one `pr create` the
    // driver runs (`ci::pr_create`, one arg per line).
    for (text, form) in [
        ("gh(repo, &[\"pr\", \"merge\", pr])", "\"pr\",\"merge\""),
        (
            "gh(\n    repo,\n    &[\n        \"pr\",\n        \"ready\",\n    ],\n)",
            "\"pr\",\"ready\"",
        ),
        ("sh(\"gh pr  merge 1 --squash\")", "pr merge"),
        ("format!(\"repos/{o}/pulls/{n}/merge\")", "/merge"),
        ("\"mutation { mergePullRequest }\"", "mergePullRequest"),
        (
            "&[\"pr\", \"create\", \"--base\", base]",
            "pr create without --draft",
        ),
    ] {
        assert_eq!(merges_or_readies(text), Some(form), "{text}");
    }
    let draft = "&[\n \"pr\",\n \"create\",\n \"--draft\",\n \"--base\",\n base,\n]";
    assert_eq!(merges_or_readies(draft), None);

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src];
    let mut hits = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                // No exception: the worker deny rules cover any `gh`, so no
                // source names a merge, not even to forbid it.
                if let Some(form) = merges_or_readies(&fs::read_to_string(&p).unwrap()) {
                    hits.push((p, form));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the driver asks GitHub to merge or ready a PR: {hits:?}"
    );
}

/// PRD-002 FR-18: every `{factory} docs …` line the slice contract names
/// runs as written on the fixture, in the contract's order, and the log
/// then holds what the worker wrote through it. The stub `claude` reads no
/// contract, so only this catches a flag the tool does not take.
#[test]
fn the_slice_contracts_commands_run_on_the_fixture() {
    let fx = Fixture::new("closeout-contract-commands");
    let contract = factory::pack::CONTRACT;
    let lines: Vec<String> = contract
        .split('`')
        .filter(|span| span.starts_with("{factory} docs "))
        .map(|span| {
            span.replace("{factory}", BIN)
                .replace("{NNN}", "001")
                .replace("{N}", "1")
                .replace("decide|fyi", "decide")
        })
        .collect();
    assert_eq!(lines.len(), 7, "{lines:?}");
    for line in &lines {
        let mut child = fx
            .command("sh")
            .args(["-c", line])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"**Chose:** a\n**Over:** b\n**Because:** c\n**Undo:** d\n")
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{line}\n{}{}",
            stdout(&out),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let kinds: Vec<String> = fx
        .log_entries()
        .iter()
        .map(|e| e["kind"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(kinds, ["decision", "note", "block"], "{kinds:?}");
    assert_eq!(fx.log_entries()[0]["flag"], "decide");
    assert_eq!(fx.state().slices[&1].status, "blocked");
}
