//! The second-opinion line in `factory docs preflight` and `factory run`, against the stub `cursor-agent`.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn logged_out(fx: &Fixture) {
    fs::write(
        fx.stub_dir().join("cursor-agent.json"),
        json!({"list_models": "auth"}).to_string(),
    )
    .unwrap();
}

fn preflight(fx: &Fixture, args: &[&str]) -> (Option<i32>, String) {
    let o = fx.docs_output(&[&["preflight", "001"], args].concat());
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    eprintln!(
        "$ factory docs preflight 001 {}\n{text}exit={:?}",
        args.join(" "),
        o.status.code()
    );
    (o.status.code(), text)
}

fn json_lines(text: &str, key: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(text).expect(text);
    v[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_string())
        .collect()
}

fn second_opinion(lines: &[String]) -> Vec<&String> {
    lines
        .iter()
        .filter(|l| l.starts_with("second opinion"))
        .collect()
}

#[test]
fn preflight_names_the_second_opinion_ready_or_how_to_log_in_and_keeps_its_exit_code() {
    let fx = Fixture::new("preflight-second-opinion");

    let (ready_code, out) = preflight(&fx, &[]);
    let ok = "second opinion: cursor-agent with grok-4.7-high ready";
    assert!(out.contains(&format!("  ✓ {ok}\n")), "{out}");
    let (_, out) = preflight(&fx, &["--json"]);
    assert_eq!(second_opinion(&json_lines(&out, "ok")), [ok], "{out}");
    assert!(
        second_opinion(&json_lines(&out, "judge")).is_empty(),
        "{out}"
    );

    logged_out(&fx);
    let (out_code, out) = preflight(&fx, &[]);
    let line = out
        .lines()
        .find(|l| l.starts_with("  ? second opinion"))
        .unwrap_or_else(|| panic!("no judge line\n{out}"));
    assert!(line.contains("Authentication required"), "{line}");
    assert!(line.contains("`cursor-agent login`"), "{line}");
    assert!(
        line.contains("polish falls back to a fresh local subagent per 600-line pass"),
        "{line}"
    );
    assert!(!out.contains("  ✓ second opinion"), "{out}");
    let (json_code, json) = preflight(&fx, &["--json"]);
    assert_eq!(
        second_opinion(&json_lines(&json, "judge")),
        [&line["  ? ".len()..]],
        "{json}"
    );
    assert_eq!(out_code, ready_code);
    assert_eq!(json_code, ready_code);
}

#[test]
fn the_pass_size_in_the_line_comes_from_the_config() {
    let fx = Fixture::new("preflight-pass-lines");
    logged_out(&fx);
    let cfg = PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]);
    fs::write(cfg.join("config.toml"), "[review]\npass_lines = 250\n").unwrap();
    let (_, out) = preflight(&fx, &[]);
    assert!(out.contains("per 250-line pass"), "{out}");
}

#[test]
fn preflight_leaves_no_probe_log_behind() {
    let fx = Fixture::new("preflight-no-probe-log");
    preflight(&fx, &[]);
    logged_out(&fx);
    preflight(&fx, &[]);
    let reviews = PathBuf::from(&fx.env()["FACTORY_DATA_DIR"]).join("reviews");
    let left: Vec<PathBuf> = fs::read_dir(&reviews)
        .map(|d| d.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
}

fn events(fx: &Fixture) -> Vec<Value> {
    let runs = PathBuf::from(&fx.env()["FACTORY_DATA_DIR"]).join("runs");
    let dirs: Vec<PathBuf> = fs::read_dir(runs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    fs::read_to_string(dirs[0].join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect(l))
        .collect()
}

#[test]
fn a_logged_out_run_starts_and_notes_the_second_opinion_line() {
    let fx = Fixture::new("run-second-opinion");
    logged_out(&fx);
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let out: Output = fx
        .command(BIN)
        .args(["run", "001", "--foreground"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("  · second opinion not ready: "), "{text}");
    let noted: Vec<String> = events(&fx)
        .iter()
        .filter(|e| e["kind"] == "judge_noted")
        .filter_map(|e| e["text"].as_str().map(str::to_string))
        .filter(|t| t.starts_with("second opinion"))
        .collect();
    assert_eq!(noted.len(), 1, "{noted:?}");
    assert!(noted[0].contains("Authentication required"), "{noted:?}");
    assert!(noted[0].contains("`cursor-agent login`"), "{noted:?}");
}

fn strip_slices(fx: &Fixture) {
    let dir = fx.path().join(harness::FEATURE_DIR);
    for n in 1..=3 {
        fs::remove_file(fx.slice_path(n)).unwrap();
    }
    let state = fs::read_to_string(dir.join("state.toml")).unwrap();
    let mut kept = String::new();
    let mut in_slice = false;
    for line in state.lines() {
        if line.starts_with('[') {
            in_slice = line.starts_with("[slices.");
        }
        if !in_slice {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    fs::write(dir.join("state.toml"), kept).unwrap();
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: unslice the fixture"]);
}

#[test]
fn an_unsliced_feature_is_refused_before_a_run_starts() {
    let fx = Fixture::new("preflight-unsliced");
    let (_, out) = preflight(&fx, &[]);
    assert!(!out.contains("not sliced"), "{out}");

    strip_slices(&fx);
    assert!(fx.state().slices.is_empty());
    let (code, out) = preflight(&fx, &[]);
    assert_eq!(code, Some(1), "{out}");
    let refusal = format!("  ✗ not sliced — /slice {}\n", harness::FEATURE);
    assert!(out.contains(&refusal), "{out}");
}

#[test]
fn a_shipped_feature_with_a_log_resumes_without_slices() {
    let fx = Fixture::new("preflight-unsliced-shipped");
    strip_slices(&fx);
    fx.docs(&["set", "001", "status=shipped"]);
    fx.docs(&["log", "add", "001", "--kind", "note", "--title", "built"]);
    let (_, out) = preflight(&fx, &[]);
    assert!(out.contains("resumes at the PR"), "{out}");
    assert!(!out.contains("not sliced"), "{out}");
}

const CI_ASK: &str = "ci mode not set in config.toml";

/// The owner never chose a CI mode: no `FACTORY_CI_MODE`, no `[ci]` in config.toml.
fn unset(fx: &Fixture, args: &[&str]) -> (Option<i32>, String) {
    let o = fx
        .command(BIN)
        .args(args)
        .env_remove("FACTORY_CI_MODE")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    eprintln!(
        "$ factory {}\n{text}{}exit={:?}",
        args.join(" "),
        String::from_utf8_lossy(&o.stderr),
        o.status.code()
    );
    (o.status.code(), text)
}

fn config_toml(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]).join("config.toml")
}

fn run_dir(fx: &Fixture) -> PathBuf {
    let runs = PathBuf::from(&fx.env()["FACTORY_DATA_DIR"]).join("runs");
    let dirs: Vec<PathBuf> = fs::read_dir(runs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs[0].clone()
}

fn run_state(fx: &Fixture) -> Value {
    serde_json::from_str(&fs::read_to_string(run_dir(fx).join("state.json")).unwrap()).unwrap()
}

fn knobs_ci(fx: &Fixture) -> Value {
    let dir = run_dir(fx);
    let id = dir.file_name().unwrap().to_str().unwrap();
    let db =
        factory::db::Db::open(&PathBuf::from(&fx.env()["FACTORY_DATA_DIR"]).join("factory.db"))
            .unwrap();
    let row = db.run_row(id).unwrap().unwrap();
    serde_json::from_str::<Value>(row.knobs.as_deref().unwrap()).unwrap()["ci"].clone()
}

/// Starts a run that parks on the CI-mode ask and returns the ask's id.
fn parked_on_ci_ask(fx: &Fixture) -> String {
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let (code, out) = unset(fx, &["run", "001"]);
    assert_eq!(code, Some(0), "{out}");
    assert_eq!(run_state(fx)["phase"], "waiting-owner", "{out}");
    let asks: Vec<(String, String)> = events(fx)
        .iter()
        .filter(|e| e["kind"] == "ask_opened")
        .map(|e| {
            (
                e["id"].as_str().unwrap().to_string(),
                e["text"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(asks.len(), 1, "{asks:?}");
    let (id, text) = &asks[0];
    assert!(text.starts_with(CI_ASK), "{asks:?}");
    for choice in ["`local`", "`remote`", "`proceed`"] {
        assert!(text.contains(choice), "{choice}: {asks:?}");
    }
    id.clone()
}

fn answered(fx: &Fixture) -> usize {
    events(fx)
        .iter()
        .filter(|e| e["kind"] == "ask_answered")
        .count()
}

#[test]
fn preflight_asks_when_ci_mode_is_unset_and_exits_two() {
    let fx = Fixture::new("preflight-ci-unset");
    let (code, out) = unset(&fx, &["docs", "preflight", "001"]);
    assert_eq!(code, Some(2), "{out}");
    let line = out
        .lines()
        .find(|l| l.starts_with(&format!("  ! {CI_ASK}")))
        .unwrap_or_else(|| panic!("no ci ask\n{out}"));
    for choice in ["`local`", "`remote`", "`proceed`"] {
        assert!(line.contains(choice), "{choice}: {line}");
    }
    let (code, out) = unset(&fx, &["docs", "preflight", "001", "--json"]);
    assert_eq!(code, Some(2), "{out}");
    assert!(json_lines(&out, "ask")[0].starts_with(CI_ASK), "{out}");

    fs::write(fx.path().join("stray.txt"), "dirty").unwrap();
    let (code, out) = unset(&fx, &["docs", "preflight", "001"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(out.contains(&format!("  ! {CI_ASK}")), "{out}");
    fs::remove_file(fx.path().join("stray.txt")).unwrap();

    let fresh = Fixture::new("run-ci-unset");
    let _ = parked_on_ci_ask(&fresh);
    assert!(run_state(&fresh)["ci_mode"].is_null());
    assert!(!config_toml(&fresh).exists());

    let flagged = Fixture::new("run-ci-flag-bypasses-ask");
    let (code, out) = unset(&flagged, &["run", "001", "--ci", "local", "--foreground"]);
    assert_eq!(code, Some(0), "{out}");
    assert!(!out.contains(CI_ASK), "{out}");
    assert!(
        events(&flagged).iter().all(|e| e["kind"] != "ask_opened"),
        "{out}"
    );
    assert_eq!(run_state(&flagged)["ci_mode"], "local");
    assert!(!config_toml(&flagged).exists());
}

#[test]
fn answer_local_writes_config_and_clears_future_preflight() {
    let fx = Fixture::new("answer-ci-local");
    let ask = parked_on_ci_ask(&fx);
    let (code, out) = unset(&fx, &["answer", &ask, "local"]);
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("every ask answered — the run proceeds"),
        "{out}"
    );
    let saved = fs::read_to_string(config_toml(&fx)).unwrap();
    assert!(saved.contains("[ci]\nmode = \"local\"\n"), "{saved}");
    assert_eq!(run_state(&fx)["ci_mode"], "local");
    assert_eq!(knobs_ci(&fx), json!({"value": "local", "source": "user"}));

    // A second repo on the same machine: the driver woken above is busy with this one.
    let other = Fixture::new("answer-ci-local-next-repo");
    let o = other
        .command(BIN)
        .args(["docs", "preflight", "001"])
        .env_remove("FACTORY_CI_MODE")
        .env("FACTORY_CONFIG_DIR", &fx.env()["FACTORY_CONFIG_DIR"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "{text}");
    assert!(text.contains("  ✓ ci mode: local (user)\n"), "{text}");
    assert!(!text.contains(CI_ASK), "{text}");
}

#[test]
fn answer_proceed_keeps_config_unset_and_runs_remote() {
    let fx = Fixture::new("answer-ci-proceed");
    let ask = parked_on_ci_ask(&fx);
    let (code, out) = unset(&fx, &["answer", &ask, "proceed"]);
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("every ask answered — the run proceeds"),
        "{out}"
    );
    assert!(out.contains("driver pid "), "{out}");
    assert!(!config_toml(&fx).exists());
    assert_eq!(run_state(&fx)["ci_mode"], "remote");
    assert_eq!(
        knobs_ci(&fx),
        json!({"value": "remote", "source": "default"})
    );
    assert!(run_dir(&fx).join(format!("answers/{ask}.json")).is_file());
}

#[test]
fn answer_invalid_ci_mode_refuses_and_keeps_ask_open() {
    let fx = Fixture::new("answer-ci-bogus");
    let ask = parked_on_ci_ask(&fx);
    let log = fx.path().join(harness::FEATURE_DIR).join("log.jsonl");
    let log_before = fs::read_to_string(&log).unwrap_or_default();
    let (code, out) = unset(&fx, &["answer", &ask, "bogus"]);
    assert_eq!(code, Some(1), "{out}");
    for choice in ["`local`", "`remote`", "`proceed`"] {
        assert!(out.contains(choice), "{choice}: {out}");
    }
    assert!(!config_toml(&fx).exists());
    assert_eq!(fs::read_to_string(&log).unwrap_or_default(), log_before);
    assert_eq!(answered(&fx), 0);
    assert!(!run_dir(&fx).join(format!("answers/{ask}.json")).exists());
    assert_eq!(run_state(&fx)["phase"], "waiting-owner");
    assert!(run_state(&fx)["ci_mode"].is_null());

    let (code, out) = unset(&fx, &["answer", &ask, "Remote"]);
    assert_eq!(code, Some(0), "{out}");
    assert_eq!(answered(&fx), 1);
    assert_eq!(run_state(&fx)["ci_mode"], "remote");
    let saved = fs::read_to_string(config_toml(&fx)).unwrap();
    assert!(saved.contains("[ci]\nmode = \"remote\"\n"), "{saved}");
}

#[test]
fn preflight_names_an_unreadable_config_and_a_bad_env_ci_mode_instead_of_asking() {
    let fx = Fixture::new("preflight-ci-unreadable");
    fs::write(config_toml(&fx), "[ci]\nmode = \"cloud\"\n").unwrap();
    let (code, out) = unset(&fx, &["docs", "preflight", "001"]);
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("ci mode not read:"), "{out}");
    assert!(!out.contains(CI_ASK), "{out}");

    fs::remove_file(config_toml(&fx)).unwrap();
    let o = fx
        .command(BIN)
        .args(["docs", "preflight", "001"])
        .env("FACTORY_CI_MODE", "cloud")
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(2), "{out}");
    assert!(out.contains(&format!("  ! {CI_ASK}")), "{out}");
    assert!(
        out.contains("FACTORY_CI_MODE=\"cloud\" is neither `local` nor `remote`"),
        "{out}"
    );
}

const REVIEW_ASK: &str = "adversarial reviewer not set in config.toml";

/// No reviewer chosen: no `FACTORY_REVIEW_PROVIDER`, and whatever `config_dir` holds.
fn no_reviewer(fx: &Fixture, args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String) {
    let mut c = fx.command(BIN);
    c.args(args).env_remove("FACTORY_REVIEW_PROVIDER");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    eprintln!(
        "$ factory {}\n{text}{}exit={:?}",
        args.join(" "),
        String::from_utf8_lossy(&o.stderr),
        o.status.code()
    );
    (o.status.code(), text)
}

/// Starts a run that parks on the reviewer ask and returns the ask's id.
fn parked_on_review_ask(fx: &Fixture) -> String {
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let (code, out) = no_reviewer(fx, &["run", "001"], &[]);
    assert_eq!(code, Some(0), "{out}");
    assert_eq!(run_state(fx)["phase"], "waiting-owner", "{out}");
    let asks: Vec<(String, String)> = events(fx)
        .iter()
        .filter(|e| e["kind"] == "ask_opened")
        .map(|e| {
            (
                e["id"].as_str().unwrap().to_string(),
                e["text"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert!(asks[0].1.starts_with(REVIEW_ASK), "{asks:?}");
    asks[0].0.clone()
}

#[test]
fn preflight_asks_when_review_unset_and_answer_persists_or_proceeds() {
    let other = Fixture::new("preflight-review-unset");
    let (code, out) = no_reviewer(&other, &["docs", "preflight", "001"], &[]);
    assert_eq!(code, Some(2), "{out}");
    let line = out
        .lines()
        .find(|l| l.starts_with(&format!("  ! {REVIEW_ASK}")))
        .unwrap_or_else(|| panic!("no reviewer ask\n{out}"));
    for choice in ["`claude`", "`cursor`", "`none`", "`proceed`"] {
        assert!(line.contains(choice), "{choice}: {line}");
    }
    assert!(!out.contains("  ✓ second opinion"), "{out}");
    let (code, out) = no_reviewer(
        &other,
        &["docs", "preflight", "001"],
        &[("FACTORY_REVIEW_PROVIDER", "bogus")],
    );
    assert_eq!(code, Some(2), "{out}");
    assert!(out.contains(&format!("  ! {REVIEW_ASK}")), "{out}");
    assert!(
        out.contains("  ? FACTORY_REVIEW_PROVIDER=\"bogus\" names no reviewer"),
        "{out}"
    );

    let fx = Fixture::new("answer-review-codex");
    let ask = parked_on_review_ask(&fx);
    assert!(ask.starts_with("ASK-"), "{ask}");
    let log = fx.path().join(harness::FEATURE_DIR).join("log.jsonl");
    let log_before = fs::read_to_string(&log).unwrap_or_default();
    let (code, out) = no_reviewer(&fx, &["answer", &ask, "bogus"], &[]);
    assert_eq!(code, Some(1), "{out}");
    for choice in ["`codex`", "`opencode`", "`none`", "`proceed`"] {
        assert!(out.contains(choice), "{choice}: {out}");
    }
    assert!(!config_toml(&fx).exists());
    assert_eq!(fs::read_to_string(&log).unwrap_or_default(), log_before);
    assert_eq!(answered(&fx), 0);
    assert!(!run_dir(&fx).join(format!("answers/{ask}.json")).exists());
    assert_eq!(run_state(&fx)["phase"], "waiting-owner");

    let (code, out) = no_reviewer(&fx, &["answer", &ask, "codex:gpt-5.4:high"], &[]);
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("every ask answered — the run proceeds"),
        "{out}"
    );
    let saved = fs::read_to_string(config_toml(&fx)).unwrap();
    let review: toml::Table = toml::from_str::<toml::Table>(&saved).unwrap()["review"]
        .as_table()
        .unwrap()
        .clone();
    assert_eq!(review["provider"].as_str(), Some("codex"), "{saved}");
    assert_eq!(review["model"].as_str(), Some("gpt-5.4"), "{saved}");
    assert_eq!(review["effort"].as_str(), Some("high"), "{saved}");
    assert_eq!(answered(&fx), 1);
    assert!(run_state(&fx)["review_override"].is_null());

    let codex = other.stub_dir().join("codex-bin");
    fs::create_dir_all(&codex).unwrap();
    fs::write(codex.join("codex"), "#!/bin/sh\necho 'codex-cli 1.0'\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(codex.join("codex"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:{}", codex.display(), other.env()["PATH"]);
    let (code, out) = no_reviewer(
        &other,
        &["docs", "preflight", "001"],
        &[
            ("FACTORY_CONFIG_DIR", &fx.env()["FACTORY_CONFIG_DIR"]),
            ("PATH", &path),
        ],
    );
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("  ✓ second opinion: codex with gpt-5.4 ready\n"),
        "{out}"
    );
    assert!(!out.contains(REVIEW_ASK), "{out}");

    let none = Fixture::new("answer-review-none");
    let ask = parked_on_review_ask(&none);
    let (code, out) = no_reviewer(&none, &["answer", &ask, "None"], &[]);
    assert_eq!(code, Some(0), "{out}");
    let saved = fs::read_to_string(config_toml(&none)).unwrap();
    assert!(saved.contains("[review]\nprovider = \"none\"\n"), "{saved}");
    assert_eq!(run_state(&none)["review_override"], "none");
    let (code, out) = no_reviewer(
        &other,
        &["docs", "preflight", "001"],
        &[("FACTORY_CONFIG_DIR", &none.env()["FACTORY_CONFIG_DIR"])],
    );
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("  ✓ second opinion: disabled (user)\n"),
        "{out}"
    );

    let proceed = Fixture::new("answer-review-proceed");
    let ask = parked_on_review_ask(&proceed);
    let (code, out) = no_reviewer(&proceed, &["answer", &ask, "proceed"], &[]);
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("every ask answered — the run proceeds"),
        "{out}"
    );
    assert!(!config_toml(&proceed).exists());
    assert_eq!(run_state(&proceed)["review_override"], "none");
    assert!(
        run_dir(&proceed)
            .join(format!("answers/{ask}.json"))
            .is_file()
    );
}

const WORKER_ASK: &str = "worker provider not set in config.toml";

/// No worker provider chosen: no `FACTORY_WORKER_PROVIDER`, and whatever `config_dir` holds.
fn no_worker(fx: &Fixture, args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String) {
    let mut c = fx.command(BIN);
    c.args(args).env_remove("FACTORY_WORKER_PROVIDER");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    eprintln!(
        "$ factory {}\n{text}{}exit={:?}",
        args.join(" "),
        String::from_utf8_lossy(&o.stderr),
        o.status.code()
    );
    (o.status.code(), text)
}

/// Starts a run that parks on the worker ask and returns the ask's id.
fn parked_on_worker_ask(fx: &Fixture) -> String {
    let (code, out) = no_worker(fx, &["run", "001"], &[]);
    assert_eq!(code, Some(0), "{out}");
    assert_eq!(run_state(fx)["phase"], "waiting-owner", "{out}");
    assert!(run_state(fx)["worker_config"].is_null());
    let asks: Vec<(String, String)> = events(fx)
        .iter()
        .filter(|e| e["kind"] == "ask_opened")
        .map(|e| {
            (
                e["id"].as_str().unwrap().to_string(),
                e["text"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert!(asks[0].1.starts_with(WORKER_ASK), "{asks:?}");
    asks[0].0.clone()
}

fn stub_bin(fx: &Fixture, name: &str) -> String {
    let dir = fx.stub_dir().join("extra-bin");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), "#!/bin/sh\nexit 0\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    format!("{}:{}", dir.display(), fx.env()["PATH"])
}

#[test]
fn preflight_asks_when_worker_provider_unset_and_refuses_missing_binary() {
    let other = Fixture::new("preflight-worker-unset");
    let (code, out) = no_worker(&other, &["docs", "preflight", "001"], &[]);
    assert_eq!(code, Some(2), "{out}");
    let line = out
        .lines()
        .find(|l| l.starts_with(&format!("  ! {WORKER_ASK}")))
        .unwrap_or_else(|| panic!("no worker ask\n{out}"));
    for choice in [
        "`claude`",
        "`cursor`",
        "`proceed` runs every role on claude",
    ] {
        assert!(line.contains(choice), "{choice}: {line}");
    }
    assert!(!out.contains("  ✓ workers:"), "{out}");
    let (code, out) = no_worker(
        &other,
        &["docs", "preflight", "001"],
        &[("FACTORY_WORKER_PROVIDER", "gemini")],
    );
    assert_eq!(code, Some(2), "{out}");
    assert!(
        out.contains("  ? FACTORY_WORKER_PROVIDER=\"gemini\" names no worker provider"),
        "{out}"
    );
    let (code, out) = preflight(&other, &[]);
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("  ✓ workers: claude opus · high (every role)\n"),
        "{out}"
    );

    let fx = Fixture::new("answer-worker-codex");
    let ask = parked_on_worker_ask(&fx);
    let log = fx.path().join(harness::FEATURE_DIR).join("log.jsonl");
    let log_before = fs::read_to_string(&log).unwrap_or_default();
    for bad in [
        "bogus",
        "build=codex",
        "opencode",
        "cursor:grok-4.7-high:high",
    ] {
        let (code, out) = no_worker(&fx, &["answer", &ask, bad], &[]);
        assert_eq!(code, Some(1), "{bad}: {out}");
        assert!(out.contains("`proceed`"), "{bad}: {out}");
        assert!(!config_toml(&fx).exists(), "{bad}");
        assert_eq!(fs::read_to_string(&log).unwrap_or_default(), log_before);
        assert_eq!(answered(&fx), 0);
        assert_eq!(run_state(&fx)["phase"], "waiting-owner");
        assert!(run_state(&fx)["worker_config"].is_null());
    }
    let (code, out) = no_worker(&fx, &["answer", &ask, "codex:gpt-5.4:xhigh"], &[]);
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("every ask answered — the run proceeds"),
        "{out}"
    );
    let saved = fs::read_to_string(config_toml(&fx)).unwrap();
    assert!(
        saved
            .contains("[workers]\nprovider = \"codex\"\nmodel = \"gpt-5.4\"\neffort = \"xhigh\"\n"),
        "{saved}"
    );
    let codex = json!({"provider": "codex", "model": "gpt-5.4", "effort": "xhigh"});
    assert_eq!(
        run_state(&fx)["worker_config"],
        json!({"build": codex, "fix": codex, "checkpoint": codex, "polish": codex})
    );
    assert_eq!(answered(&fx), 1);

    // The saved provider answers the next repo's preflight, which refuses it while codex is not on PATH.
    let saved_dir = [(
        "FACTORY_CONFIG_DIR",
        fx.env()["FACTORY_CONFIG_DIR"].as_str(),
    )];
    let (code, out) = no_worker(&other, &["docs", "preflight", "001"], &saved_dir);
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains(
            "  ✗ worker provider codex (codex) not found on PATH — install it or run `factory setup workers`\n"
        ),
        "{out}"
    );
    assert!(!out.contains(WORKER_ASK), "{out}");
    let (code, out) = no_worker(&other, &["run", "001"], &saved_dir);
    assert_eq!(code, Some(1), "{out}");
    assert!(out.contains("  ✗ worker provider codex (codex)"), "{out}");
    assert!(!other.data_dir().join("runs").exists(), "{out}");
    let path = stub_bin(&other, "codex");
    let (code, out) = no_worker(
        &other,
        &["docs", "preflight", "001"],
        &[saved_dir[0], ("PATH", &path)],
    );
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("  ✓ workers: codex gpt-5.4 · xhigh (every role)\n"),
        "{out}"
    );

    let opencode = Fixture::new("preflight-worker-opencode-no-model");
    fs::write(
        config_toml(&opencode),
        "[workers]\nprovider = \"claude\"\n\n[workers.fix]\nprovider = \"opencode\"\n",
    )
    .unwrap();
    let path = stub_bin(&opencode, "opencode");
    let (code, out) = no_worker(&opencode, &["docs", "preflight", "001"], &[("PATH", &path)]);
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("  ✗ fix worker runs on opencode with no model"),
        "{out}"
    );

    let proceed = Fixture::new("answer-worker-proceed");
    let ask = parked_on_worker_ask(&proceed);
    let (code, out) = no_worker(&proceed, &["answer", &ask, "Proceed"], &[]);
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("for this run only"), "{out}");
    assert!(out.contains("driver pid "), "{out}");
    assert!(!config_toml(&proceed).exists());
    let claude = json!({"provider": "claude", "model": "opus", "effort": "high"});
    assert_eq!(
        run_state(&proceed)["worker_config"],
        json!({"build": claude, "fix": claude, "checkpoint": claude, "polish": claude})
    );
    assert!(
        run_dir(&proceed)
            .join(format!("answers/{ask}.json"))
            .is_file()
    );
}
