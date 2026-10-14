#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use factory::db::Db;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const HYPOTHESIS: &str = "sonnet polish costs less";

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN).args(args).output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn data_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn only_run_id(fx: &Fixture) -> String {
    let dirs: Vec<PathBuf> = fs::read_dir(data_dir(fx).join("runs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs[0].file_name().unwrap().to_str().unwrap().to_string()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

fn project_fixture(name: &str) -> Fixture {
    let fx = Fixture::with_index(name, &[("checkpoint_every", "10"), ("lanes", "1")]);
    fx.chain();
    let file = fx.path().join("docs/project.toml");
    let docs = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        format!(
            "{}\n\n[workers.polish]\nmodel = \"sonnet\"\n",
            docs.trim_end()
        ),
    )
    .unwrap();
    fx.git(&["add", "docs/project.toml"]);
    fx.git(&["commit", "-q", "-m", "chore: project knobs"]);
    fx
}

#[test]
fn a_project_file_sets_the_polish_model_and_the_row_says_so() {
    let fx = project_fixture("knobs-project");
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let out = factory(
        &fx,
        &["run", "001", "--foreground", "--hypothesis", HYPOTHESIS],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    assert!(
        !text(&out.stderr).contains("claude --version"),
        "{}",
        text(&out.stderr)
    );
    let id = only_run_id(&fx);

    let calls = fx.calls();
    assert_eq!(calls.len(), 5, "three builds, a checkpoint, the polish");
    let is_polish = |c: &Value| c["pack"].as_str().unwrap().contains("\n# Branch\n");
    assert_eq!(calls.iter().filter(|c| is_polish(c)).count(), 1);
    for c in &calls {
        let want = if is_polish(c) { "sonnet" } else { "opus" };
        assert_eq!(c["model"], want, "call {}", c["n"]);
    }

    let db = db(&fx);
    let row = db.run_row(&id).unwrap().unwrap();
    let knobs: Value = serde_json::from_str(row.knobs.as_deref().unwrap()).unwrap();
    assert_eq!(knobs["workers"]["polish"]["model"], "sonnet");
    assert_eq!(knobs["workers"]["polish"]["source"], "project");
    assert_eq!(knobs["workers"]["build"]["model"], "opus");
    assert_eq!(knobs["workers"]["build"]["source"], "default");
    assert_eq!(knobs["workers"]["polish"]["effort_source"], "default");
    assert!(knobs["workers"]["build"].get("effort_source").is_none());
    assert_eq!(
        knobs["budget"],
        json!({"value": 30_000_000, "source": "default"})
    );
    assert_eq!(knobs["lanes"], 1);
    assert!(!knobs["factory_version"].as_str().unwrap().is_empty());
    assert_eq!(knobs["claude_version"], "2.1.296 (Claude Code)");
    assert_eq!(row.hypothesis.as_deref(), Some(HYPOTHESIS));

    // What answered: the stub's `init.model` after the `--model` override.
    let workers = db.workers(&id).unwrap();
    assert_eq!(workers.len(), 5);
    for w in &workers {
        let want = if w.role == "polish" { "sonnet" } else { "opus" };
        assert_eq!(
            w.model_id.as_deref(),
            Some(want),
            "{} {:?}",
            w.role,
            w.slice
        );
        assert_eq!(w.model.as_deref(), Some(want));
    }

    let out = factory(&fx, &["report", &id, "--per-worker"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let report = text(&out.stdout);
    let line = |k: &str| {
        report
            .lines()
            .find(|l| l.starts_with(&format!("{k}: ")))
            .unwrap_or_else(|| panic!("no {k} in {report}"))
            .to_string()
    };
    let knobs_line = line("knobs");
    assert!(
        knobs_line.contains("build opus/high default"),
        "{knobs_line}"
    );
    assert!(
        knobs_line.contains("polish sonnet/high project/default"),
        "{knobs_line}"
    );
    assert!(
        knobs_line.ends_with(&format!(
            "lanes 1 · factory {} · claude 2.1.296",
            env!("CARGO_PKG_VERSION")
        )),
        "{knobs_line}"
    );
    assert_eq!(line("hypothesis"), format!("hypothesis: {HYPOTHESIS}"));
    assert!(
        report
            .lines()
            .any(|l| l.contains(" polish ") && l.ends_with("model_id sonnet")),
        "{report}"
    );

    let out = factory(&fx, &["report", &id, "--json"]);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["hypothesis"], HYPOTHESIS);
    assert_eq!(v["knobs"], knobs);
}

#[test]
fn a_hypothesis_is_written_once_at_creation() {
    // Unchained, the run parks on a preflight ask, so a second `run` resumes it.
    let fx = Fixture::with_index("knobs-hypothesis", &[("lanes", "1")]);
    let out = factory(
        &fx,
        &[
            "run",
            "001",
            "--budget",
            "5000000",
            "--hypothesis",
            HYPOTHESIS,
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    let id = only_run_id(&fx);
    let db = db(&fx);
    let row = db.run_row(&id).unwrap().unwrap();
    assert_eq!(row.phase, "waiting-owner");
    assert_eq!(row.hypothesis.as_deref(), Some(HYPOTHESIS));
    let knobs: Value = serde_json::from_str(row.knobs.as_deref().unwrap()).unwrap();
    assert_eq!(
        knobs["budget"],
        json!({"value": 5_000_000, "source": "flag"})
    );

    let out = factory(&fx, &["run", "001", "--hypothesis", "other"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    let err = text(&out.stderr);
    assert_eq!(
        err.matches(&format!("hypothesis ignored: run {id} already exists"))
            .count(),
        1,
        "{err}"
    );
    let row = db.run_row(&id).unwrap().unwrap();
    assert_eq!(row.hypothesis.as_deref(), Some(HYPOTHESIS));
    assert_eq!(only_run_id(&fx), id);
}
