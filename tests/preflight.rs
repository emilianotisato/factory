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
        line.contains("one Claude agent per 600-line pass"),
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
