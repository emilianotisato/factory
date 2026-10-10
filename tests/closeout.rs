//! Slice 09 of PRD-001: the closeout (PRD §8 FR-6). When every slice is
//! done and the last checkpoint is in, the driver spawns the polish worker
//! on `/polish`, commits its fixes, ships the feature (`docs ship`, the
//! retro's numbers from the store), pushes, opens the draft PR with the
//! body the worker wrote, and watches CI: a fix worker per red check (three
//! rounds), one rerun for a flaky one, a block otherwise. Every `gh` here is
//! the stub in `tests/stubs/`; `origin` is the fixture's bare repo.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::db::Db;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const FEATURE_DIR: &str = "docs/features/001-fixture";
const AGENT_LOG: &str = "docs/features/001-fixture/agent-log.md";
const PRD: &str = "docs/features/001-fixture/prd.md";
const RETRO: &str = "docs/features/001-fixture/retro.md";
const PR_URL: &str = "https://example.invalid/pr/1";
const CI_FIX: &str = "Fix the red CI check `test` on the PR of PRD-001 as the appended system prompt says. Return the STATUS block.";
const RETRO_KEYS: [&str; 12] = [
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
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- git log\n{}\n--- gh.log\n{:?}\n--- driver.log\n{}\n--- agent-log.md\n{}",
                state(dir),
                kinds(dir),
                fx.git(&["log", "--format=%h %s"]),
                fx.gh_log(),
                driver_log(dir),
                fs::read_to_string(fx.path().join(AGENT_LOG)).unwrap_or_default()
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
fn frontmatter(fx: &Fixture, rel: &str) -> Vec<(String, String)> {
    let text = fx.read(rel);
    let body = text.strip_prefix("---\n").expect(rel);
    let end = body.find("\n---").expect(rel);
    body[..end]
        .lines()
        .map(|l| {
            let (k, v) = l.split_once(':').unwrap_or((l, ""));
            (k.trim().to_string(), v.trim().to_string())
        })
        .collect()
}

fn fm_value(fx: &Fixture, rel: &str, key: &str) -> String {
    frontmatter(fx, rel)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("no `{key}:` in {rel}"))
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

fn block_heading(fx: &Fixture, id: &str, phase: &str) -> String {
    let log = fx.read(AGENT_LOG);
    let needle = format!("### {id} — ");
    let i = log
        .find(&needle)
        .unwrap_or_else(|| panic!("{needle} missing from\n{log}"));
    let line = log[i..].lines().next().unwrap().to_string();
    assert!(
        line.ends_with(&format!(" · {phase} · open")),
        "{line} is not an open {phase} block"
    );
    line
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
        " feat: C\n",
        " docs: checkpoint 2\n",
        "# Retro",
        &format!("<!-- {FEATURE_DIR}/retro.md -->"),
        "````markdown\n---\nscore:\ngate_runs:\n",
        "# Retro — PRD-001",
        "# PR body",
        &format!("<!-- write: {} -->", dir.join("pr-body.md").display()),
        "## PRD-{NNN} — {title}",
        "# Contract",
        "You are the polish worker of PRD-001",
        "run `/polish` **Unattended**",
        "**Your AD numbers start at AD-9901**",
        "`### BLOCK-n — {what} · polish · open`",
        "# Autonomy",
    ] {
        assert!(
            pack.contains(want),
            "{want:?} missing from the polish pack:\n{pack}"
        );
    }
    assert!(!pack.contains("# Gate transcript"), "{pack}");

    // Its commit, then the ship commit: HEAD is `docs: ship PRD-001`.
    assert_eq!(
        subjects(&fx)[2..],
        [
            "feat: A",
            "feat: B",
            "docs: checkpoint 1",
            "feat: C",
            "docs: checkpoint 2",
            "polish: review fixes",
            "docs: ship PRD-001",
        ]
    );
    assert_eq!(
        fx.git(&["log", "-1", "--format=%s"]).trim(),
        "docs: ship PRD-001"
    );
    let polish_sha = sha_of(&fx, "polish: review fixes");
    assert_eq!(
        files_of(&fx, &polish_sha),
        [
            PRD.to_string(),
            RETRO.to_string(),
            "src/lib.txt".to_string()
        ]
    );
    assert!(fx.read("src/lib.txt").contains("polished by spawn 6\n"));
    let ship_files = files_of(&fx, &sha_of(&fx, "docs: ship PRD-001"));
    for f in ["docs/index.md", PRD, RETRO] {
        assert!(
            ship_files.contains(&f.to_string()),
            "{f} not in {ship_files:?}"
        );
    }
    assert_eq!(fx.git(&["status", "--porcelain"]).trim(), "", "tree dirty");

    // The PRD is shipped and reviewed; the retro has its twelve numbers.
    assert_eq!(fm_value(&fx, PRD, "status"), "shipped");
    assert!(fm_value(&fx, PRD, "reviewed").starts_with("20"), "reviewed");
    let retro = frontmatter(&fx, RETRO);
    assert_eq!(retro[0], ("score".to_string(), String::new()), "{retro:?}");
    for key in RETRO_KEYS {
        let v = &retro
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("{key}"))
            .1;
        assert!(v.parse::<f64>().is_ok(), "{key}: {v:?} is not a number");
    }
    assert_eq!(fm_value(&fx, RETRO, "slices"), "3");
    assert_eq!(fm_value(&fx, RETRO, "checkpoints"), "2");
    assert_eq!(fm_value(&fx, RETRO, "gate_runs"), "2");
    assert_eq!(fm_value(&fx, RETRO, "gate_red"), "0");
    assert_eq!(fm_value(&fx, RETRO, "ci_cycles"), "0");
    assert!(
        fx.read(RETRO).contains("## Cost vs catch"),
        "the worker's prose"
    );
    let body = fs::read_to_string(dir.join("pr-body.md")).unwrap();
    assert!(body.starts_with("## PRD-001 — Fixture\n"), "{body}");

    // Pushed to the fixture's origin, then exactly one draft PR, then green CI.
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));
    assert_eq!(
        gh_calls(&fx, "pr create"),
        [format!(
            "pr create --draft --base main --title \"PRD-001: Fixture\" --body-file {}",
            dir.join("pr-body.md").display()
        )]
    );
    assert_eq!(
        gh_calls(&fx, "pr checks"),
        ["pr checks 1 --watch --interval 60"]
    );
    assert_eq!(
        gh_calls(&fx, "pr view"),
        ["pr view feat/fixture --json url,state"]
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
    let heading = block_heading(&fx, "BLOCK-1", "polish");
    assert_eq!(heading, format!("### BLOCK-1 — {q} · polish · open"));
    let log = fx.read(AGENT_LOG);
    assert!(
        log.contains("**Happened:** the polish commit is in and `docs ship 001` refused (exit "),
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
    assert!(subjects(&fx).contains(&"polish: review fixes".to_string()));
    assert!(!subjects(&fx).contains(&"docs: ship PRD-001".to_string()));
    assert_eq!(fm_value(&fx, PRD, "status"), "ready");
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
        "fix round 1 of three",
        "**Your AD numbers start at AD-8101**",
        "# Autonomy",
        "# Inner loop",
    ] {
        assert!(
            pack.contains(want),
            "{want:?} missing from the CI fix pack:\n{pack}"
        );
    }

    // Its commit is `fix: CI test`, pushed; the retro counts the cycle.
    assert_eq!(
        subjects(&fx)[7..],
        ["polish: review fixes", "docs: ship PRD-001", "fix: CI test"]
    );
    let fix_sha = sha_of(&fx, "fix: CI test");
    assert_eq!(
        files_of(&fx, &fix_sha),
        [RETRO.to_string(), "src/lib.txt".to_string()]
    );
    assert!(fx.read("src/lib.txt").contains("fixed by spawn 7\n"));
    assert_eq!(fm_value(&fx, RETRO, "ci_cycles"), "1");
    assert_eq!(fm_value(&fx, PRD, "status"), "shipped");
    assert_eq!(origin_head(&fx, "feat/fixture"), Some(head(&fx)));

    // Two `pr checks`, one run list, one log read, no rerun, one PR.
    assert_eq!(gh_calls(&fx, "pr checks").len(), 2, "{:?}", fx.gh_log());
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert_eq!(
        gh_calls(&fx, "run list"),
        [
            "run list --branch feat/fixture --limit 20 --json databaseId,name,workflowName,conclusion,headSha"
        ]
    );
    assert_eq!(gh_calls(&fx, "run view"), ["run view 1000 --log-failed"]);
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
    assert!(!subjects(&fx).iter().any(|s| s.starts_with("fix: CI")));
    assert_eq!(fm_value(&fx, RETRO, "ci_cycles"), "0");
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
    block_heading(&fx, "BLOCK-1", "ci");
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
    let log = fx.read(AGENT_LOG);
    assert!(
        log.contains("### BLOCK-1 — flaky check test red again after its rerun · ci · resolved"),
        "{log}"
    );
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
    block_heading(&fx, "BLOCK-1", "ci");

    assert_eq!(roles(&fx, &dir)[5..], ["polish", "fix", "fix", "fix"]);
    assert_eq!(
        subjects(&fx)[8..],
        [
            "docs: ship PRD-001",
            "fix: CI test",
            "fix: CI test",
            "fix: CI test"
        ]
    );
    assert_eq!(fm_value(&fx, RETRO, "ci_cycles"), "3");
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
    block_heading(&fx, "BLOCK-1", "ci");
    assert_eq!(prompts(&fx).len(), 6, "{:?}", prompts(&fx));
    assert!(gh_calls(&fx, "run rerun").is_empty());
    assert!(gh_calls(&fx, "run view").is_empty());
    assert!(!subjects(&fx).iter().any(|s| s.starts_with("fix: CI")));
    stop(&fx, &dir);
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
    block_heading(&fx, "BLOCK-1", "polish");
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
    let log = fx.read(AGENT_LOG);
    assert_eq!(log.matches("- **Dead polish worker (").count(), 2, "{log}");
    assert!(!subjects(&fx).contains(&"polish: review fixes".to_string()));
    assert!(gh_calls(&fx, "pr create").is_empty());

    let out = factory(&fx, &["answer", "BLOCK-1", "try once more"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(roles(&fx, &dir)[5..], ["polish", "polish", "polish"]);
    assert_eq!(prompts(&fx)[7], "/polish");
    assert_eq!(
        subjects(&fx)[7..],
        ["polish: review fixes", "docs: ship PRD-001"]
    );
    assert_eq!(gh_calls(&fx, "pr create").len(), 1);
    assert_eq!(events(&dir).pop().unwrap()["kind"], "run_finished");
}

#[test]
fn the_driver_never_merges_or_readies_a_pr() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src];
    let mut hits = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = fs::read_to_string(&p).unwrap();
                if text.contains("pr merge") || text.contains("pr ready") {
                    hits.push(p);
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the driver names `gh pr merge`/`pr ready`: {hits:?}"
    );
}
