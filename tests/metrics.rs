#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use factory::db::{Db, GateRow, ToolCallRow, WorkerRow};
use factory::quota;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

const KEYS: [&str; 23] = [
    "score",
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
    "input_tokens",
    "cache_write_tokens",
    "weighted_tokens",
    "cockpit_tokens",
    "workers",
    "lookup_chars",
    "ctx_share",
    "repeat_rate",
    "followup_reads",
    "pack_doc_read_chars",
];

const EXTRA_KEYS: [&str; 19] = [
    "worker_gate_runs",
    "worker_gate_red",
    "cockpit_peak_context",
    "cockpit_out_tokens",
    "cockpit_cache_read_tokens",
    "cockpit_start_context",
    "lookup_share_chars",
    "followup_reads_by_role",
    "weighted_by_role",
    "fix_commits",
    "ci_red_rounds",
    "ci_reruns",
    "findings",
    "decisions",
    "decide",
    "blocks",
    "knobs",
    "hypothesis",
    "pack_by_role",
];

const FINDINGS: &str = "high=1 medium=2 low=3 fixed=4 parked=2";

const THRESHOLDS: [&str; 3] = [
    "followup threshold 30%",
    "ctx_share threshold 10%",
    "repeat_rate vs 26.7%",
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

fn only_run_id(fx: &Fixture) -> String {
    let mut dirs: Vec<PathBuf> = fs::read_dir(data_dir(fx).join("runs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    dirs.sort();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs[0].file_name().unwrap().to_str().unwrap().to_string()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

fn cockpit_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture/cockpit-session.jsonl")
}

fn key(line: &str) -> &str {
    line.split_once(':').map(|(k, _)| k).unwrap_or(line)
}

fn value<'a>(lines: &'a [&str], k: &str) -> &'a str {
    lines
        .iter()
        .find(|l| key(l) == k)
        .map(|l| l.split_once(": ").map_or("", |(_, v)| v))
        .unwrap_or_else(|| panic!("no {k} in {lines:?}"))
}

/// `--foreground`: the run is over when this returns.
fn finished_run() -> (Fixture, String) {
    let fx = Fixture::with_index("metrics", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done", "inner": "red"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let out = factory(&fx, &["run", "001", "--budget", "70000000", "--foreground"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let id = only_run_id(&fx);
    assert_eq!(
        fx.calls().len(),
        5,
        "three builds, a checkpoint, the polish"
    );
    (fx, id)
}

#[test]
fn every_tool_call_is_a_row_with_its_kind_and_repeat_flags() {
    let (fx, id) = finished_run();
    let db = db(&fx);
    let workers: Vec<WorkerRow> = db.workers(&id).unwrap();
    let rows: Vec<ToolCallRow> = db.tool_calls(&id).unwrap();
    let builds: Vec<&WorkerRow> = workers.iter().filter(|w| w.role == "build").collect();
    assert_eq!(builds.len(), 3);

    // A build: Read prd, Read slice, Write code, Bash `docs log add`, Bash inner, Bash `docs set`.
    for b in &builds {
        let mine: Vec<&ToolCallRow> = rows.iter().filter(|r| r.worker_id == b.id).collect();
        assert_eq!(
            mine.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
            ["read", "read", "edit", "run", "run-gate", "run"],
            "worker {}",
            b.id
        );
        assert_eq!(
            mine.iter().map(|r| r.seq).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6]
        );
        for r in &mine {
            assert!(r.result_chars.is_some_and(|n| n > 0), "{r:?}");
            assert_eq!(r.target_hash.len(), 64);
            assert!(r.ts.is_some());
        }
        // The slice file is read once and never edited, and the two `docs` calls differ: no repeat.
        assert!(mine.iter().all(|r| !r.repeat_in_worker), "{mine:?}");
    }
    // The checkpoint worker's one call is its `docs log add`.
    let cp = workers.iter().find(|w| w.role == "checkpoint").unwrap();
    let cp_rows: Vec<&ToolCallRow> = rows.iter().filter(|r| r.worker_id == cp.id).collect();
    assert_eq!(cp_rows.len(), 1);
    assert_eq!(cp_rows[0].kind, "run");
    // Polish: Read prd, Write code, Bash inner, Write retro, Bash `docs set`, Write the PR body.
    let polish = workers.iter().find(|w| w.role == "polish").unwrap();
    let polish_rows: Vec<&ToolCallRow> = rows.iter().filter(|r| r.worker_id == polish.id).collect();
    assert_eq!(
        polish_rows
            .iter()
            .map(|r| r.kind.as_str())
            .collect::<Vec<_>>(),
        ["read", "edit", "run-gate", "edit", "run", "edit"]
    );
    assert_eq!(rows.len(), 3 * 6 + 1 + 6);

    // prd.md is the same file for every build; the slice files differ.
    let prd: Vec<&ToolCallRow> = builds
        .iter()
        .map(|b| {
            rows.iter()
                .find(|r| r.worker_id == b.id && r.seq == 1)
                .unwrap()
        })
        .collect();
    assert_eq!(
        prd.iter()
            .map(|r| (r.repeat_in_worker, r.repeat_in_run))
            .collect::<Vec<_>>(),
        [(false, false), (false, true), (false, true)]
    );
    let slice_reads: Vec<&ToolCallRow> = builds
        .iter()
        .map(|b| {
            rows.iter()
                .find(|r| r.worker_id == b.id && r.seq == 2)
                .unwrap()
        })
        .collect();
    assert!(
        slice_reads.iter().all(|r| !r.repeat_in_run),
        "{slice_reads:?}"
    );
    assert!(
        workers.iter().all(|w| w.start_context.is_some()),
        "{workers:?}"
    );

    // Every read here is of a file the pack held; with no ctx call there is no follow-up label.
    for r in &rows {
        assert_eq!(r.pack_doc, r.kind == "read", "{r:?}");
        assert_eq!(r.label_followup, None, "{r:?}");
        assert_eq!(r.label_expanded.is_some(), r.kind == "read", "{r:?}");
    }
    assert!(
        workers.iter().all(|w| !w.pack_files.is_empty()),
        "{workers:?}"
    );
    for call in fx.calls() {
        let argv: Vec<&str> = call["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let flag = |name: &str| argv[argv.iter().position(|a| *a == name).unwrap() + 1];
        assert_eq!(call["settings"], flag("--settings"), "{argv:?}");
        assert!(Path::new(flag("--settings")).is_file());
        assert_eq!(
            call["env"]["FACTORY_PACK"],
            flag("--append-system-prompt-file")
        );
    }
}

#[test]
fn a_worker_inner_run_is_a_gates_row_with_source_worker() {
    let (fx, id) = finished_run();
    let gates: Vec<GateRow> = db(&fx).gates(&id).unwrap();
    let worker: Vec<&GateRow> = gates.iter().filter(|g| g.source == "worker").collect();
    // Three builds' inner runs and the polish worker's; its run is kind `slice` too.
    assert_eq!(worker.len(), 4, "{gates:?}");
    for g in &worker {
        assert_eq!(g.kind, "slice");
        assert_eq!(g.cmd, "./gate.sh --inner");
        assert!(g.worker_id.is_some());
        assert!(!g.cached);
        assert_eq!(g.checkpoint, None);
        assert!(g.minutes >= 0.0);
    }
    assert_eq!(worker.iter().filter(|g| g.red).count(), 1);
    let driver: Vec<&GateRow> = gates.iter().filter(|g| g.source == "driver").collect();
    assert_eq!(driver.len(), 1);
    assert_eq!(driver[0].kind, "checkpoint");
    // The stub's inner run never executes gate.sh: the fixture gate log has only the driver's run.
    assert_eq!(fx.gate_log().len(), 1, "{:?}", fx.gate_log());
}

#[test]
fn report_prints_the_retro_keys_then_the_run_keys() {
    let (fx, id) = finished_run();
    let out = factory(&fx, &["report", &id]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    let keys: Vec<&str> = lines.iter().map(|l| key(l)).collect();
    assert_eq!(&keys[..23], &KEYS);
    let q = keys
        .iter()
        .position(|k| *k == "quality")
        .expect("a quality line");
    assert_eq!(&keys[23..q], &EXTRA_KEYS);
    assert_eq!(&keys[q + 1..], &THRESHOLDS);
    assert_eq!(lines[0], "score: ");
    assert_eq!(value(&lines, "slices"), "3");
    assert_eq!(value(&lines, "checkpoints"), "1");
    assert_eq!(value(&lines, "ci_cycles"), "0");
    assert_eq!(value(&lines, "gate_runs"), "1");
    assert_eq!(value(&lines, "gate_red"), "0");
    assert_eq!(
        value(&lines, "workers"),
        "5 (build 3, checkpoint 1, polish 1)"
    );
    assert_eq!(value(&lines, "cockpit_tokens"), "0 (no --cockpit)");
    assert_eq!(value(&lines, "worker_gate_runs"), "4");
    assert_eq!(value(&lines, "worker_gate_red"), "1");
    assert_eq!(value(&lines, "findings"), FINDINGS);
    assert_eq!(value(&lines, "fix_commits"), "0");
    let quality = value(&lines, "quality");
    assert!(
        quality.starts_with("gate_red 0 · worker_gate_red 1 · fix_commits 0 · ")
            && quality.contains(&format!(" · findings {FINDINGS} · decisions ")),
        "{quality}"
    );
    // The stub polish returns within a second.
    let polish_minutes: f64 = value(&lines, "polish_minutes").parse().unwrap();
    assert!((0.0..0.5).contains(&polish_minutes), "{polish_minutes}");

    let db = db(&fx);
    let workers = db.workers(&id).unwrap();
    let n = |v: Option<i64>| v.unwrap_or(0) as u64;
    let by_hand: f64 = workers
        .iter()
        .map(|w| quota::weigh(n(w.input), n(w.cache_write), n(w.cache_read), n(w.output)))
        .sum();
    assert_eq!(
        value(&lines, "weighted_tokens").parse::<u64>().unwrap(),
        by_hand.round() as u64
    );
    assert_eq!(
        value(&lines, "weighted_tokens").parse::<u64>().unwrap(),
        quota::weighted(&db, &id).unwrap()
    );
    let sum = |f: &dyn Fn(&WorkerRow) -> Option<i64>| workers.iter().map(|w| n(f(w))).sum::<u64>();
    assert_eq!(
        value(&lines, "out_tokens").parse::<u64>().unwrap(),
        sum(&|w| w.output)
    );
    assert_eq!(
        value(&lines, "cache_read_tokens").parse::<u64>().unwrap(),
        sum(&|w| w.cache_read)
    );
    assert_eq!(
        value(&lines, "input_tokens").parse::<u64>().unwrap(),
        sum(&|w| w.input)
    );
    assert_eq!(
        value(&lines, "cache_write_tokens").parse::<u64>().unwrap(),
        sum(&|w| w.cache_write)
    );
    let peak = workers.iter().filter_map(|w| w.peak_context).max().unwrap();
    assert_eq!(value(&lines, "peak_context").parse::<i64>().unwrap(), peak);
    let start = workers
        .iter()
        .filter_map(|w| w.start_context)
        .min()
        .unwrap();
    assert_eq!(
        value(&lines, "start_context").parse::<i64>().unwrap(),
        start
    );

    let rows = db.tool_calls(&id).unwrap();
    let chars = |pred: &dyn Fn(&ToolCallRow) -> bool| {
        rows.iter()
            .filter(|r| pred(r))
            .map(|r| r.result_chars.unwrap_or(0) as u64)
            .sum::<u64>()
    };
    let lookup = chars(&|r| r.kind == "read");
    let all = chars(&|_| true);
    assert_eq!(
        value(&lines, "lookup_chars").parse::<u64>().unwrap(),
        lookup
    );
    // `ctx_share` counts ctx calls among lookups: none in this run.
    let share: f64 = value(&lines, "lookup_share_chars").parse().unwrap();
    assert!((0.0..=1.0).contains(&share), "{share}");
    assert!(
        (share - lookup as f64 / all as f64).abs() < 0.001,
        "{share}"
    );
    assert_eq!(value(&lines, "ctx_share"), "0.000");
    assert_eq!(value(&lines, "repeat_rate"), "0.000");
    assert_eq!(value(&lines, "followup_reads"), "0 of 0 ctx calls (0%)");
    assert_eq!(value(&lines, "followup_reads_by_role"), "none");
    // Every read is of a held file, so the held chars are the lookup chars.
    assert_eq!(
        value(&lines, "pack_doc_read_chars").parse::<u64>().unwrap(),
        lookup
    );
    // No ctx call: the follow-up share is n/a, not ok.
    assert_eq!(value(&lines, "followup threshold 30%"), "n/a");
    assert_eq!(value(&lines, "ctx_share threshold 10%"), "under");
    assert_eq!(value(&lines, "repeat_rate vs 26.7%"), "lower");
    let minutes: f64 = value(&lines, "build_minutes").parse().unwrap();
    assert!(minutes >= 0.0);
}

#[test]
fn report_json_round_trips_and_cockpit_measures_the_transcript() {
    let (fx, id) = finished_run();
    let out = factory(&fx, &["report", &id, "--json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_str(&stdout(&out)).expect("one JSON object");
    assert_eq!(v["slices"], 3);
    assert_eq!(v["checkpoints"], 1);
    assert_eq!(v["workers"]["total"], 5);
    assert_eq!(v["workers"]["build"], 3);
    assert_eq!(v["workers"]["polish"], 1);
    assert!(v["score"].is_null());
    assert!(v["cockpit_tokens"].is_null());
    assert_eq!(v["worker_gate_red"], 1);
    assert_eq!(
        v["findings"],
        json!({"high": 1, "medium": 2, "low": 3, "fixed": 4, "parked": 2})
    );
    let status: Value = serde_json::from_str(&fx.docs(&["status", "001", "--json"])).unwrap();
    assert_eq!(
        v["decisions"], status["log"]["decisions"],
        "{}",
        status["log"]
    );
    assert_eq!(
        v["decide"].as_u64(),
        status["log"]["decide"].as_array().map(|a| a.len() as u64)
    );
    assert_eq!(v["blocks"], status["log"]["blocks"]);
    let text = stdout(&factory(&fx, &["report", &id]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        v["weighted_tokens"].as_u64().unwrap(),
        value(&lines, "weighted_tokens").parse::<u64>().unwrap()
    );
    for k in KEYS.iter().chain(EXTRA_KEYS.iter()) {
        assert!(v.get(k).is_some(), "{k} missing from {v}");
    }
    assert_eq!(v["followup_reads"]["ctx_calls"], 0);
    assert_eq!(v["thresholds"]["ctx_share"], "under");
    assert_eq!(v["thresholds"]["followup"], "n/a");
    assert!(v.get("per_worker").is_none(), "only with --per-worker");

    // Pinned against `measure.py --retro`; input 98 and cache write 32926 make the weighted 63828.
    let cockpit = cockpit_fixture();
    let cp = cockpit.to_str().unwrap();
    let out = factory(&fx, &["report", &id, "--cockpit", cp]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(value(&lines, "cockpit_tokens"), "63828");
    assert_eq!(value(&lines, "cockpit_peak_context"), "58351");
    assert_eq!(value(&lines, "cockpit_out_tokens"), "2524");
    assert_eq!(value(&lines, "cockpit_cache_read_tokens"), "181841");
    assert_eq!(value(&lines, "cockpit_start_context"), "45507");
    let out = factory(&fx, &["report", &id, "--json", "--cockpit", cp, cp]);
    let v: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["cockpit_tokens"], 63828 * 2);
    assert_eq!(v["cockpit_peak_context"], 58351);
    assert_eq!(v["cockpit_out_tokens"], 2524 * 2);
    assert_eq!(v["cockpit_cache_read_tokens"], 181841 * 2);

    let out = factory(&fx, &["report", "001-nope"]);
    assert_ne!(out.status.code(), Some(0));
    let out = factory(&fx, &["report", &id, "--cockpit", "/nonexistent.jsonl"]);
    assert_ne!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nonexistent"));
}

/// `ctx: true`: two `factory ctx` calls in the build, one followed by a read of the file it named.
#[test]
fn ctx_calls_are_labelled_and_the_report_reads_the_reversal_rule() {
    let fx = Fixture::with_index("metrics-ctx", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done", "ctx": true}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let out = factory(&fx, &["run", "001", "--budget", "70000000", "--foreground"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let id = only_run_id(&fx);
    let db = db(&fx);
    let workers = db.workers(&id).unwrap();
    let rows = db.tool_calls(&id).unwrap();
    let first = workers.iter().find(|w| w.slice == Some(1)).unwrap();
    let mine: Vec<&ToolCallRow> = rows.iter().filter(|r| r.worker_id == first.id).collect();
    assert_eq!(
        mine.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
        [
            "read",
            "read",
            "ctx-refs",
            "read",
            "ctx-outline",
            "edit",
            "run",
            "run-gate",
            "run"
        ]
    );
    let labels: Vec<(Option<bool>, Option<bool>, bool)> = mine
        .iter()
        .map(|r| (r.label_followup, r.label_expanded, r.pack_doc))
        .collect();
    assert_eq!(
        labels,
        [
            (None, Some(false), true),        // Read prd.md: held
            (None, Some(false), true),        // Read the slice: held
            (Some(true), Some(false), false), // ctx refs, then Read src/lib.txt
            (None, Some(false), false),       // Read src/lib.txt: not held
            (Some(false), None, false),       // ctx outline, then two Writes
            (None, None, false),
            (None, None, false),
            (None, None, false),
            (None, None, false),
        ]
    );

    let out = factory(&fx, &["report", &id, "--per-worker"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(value(&lines, "followup_reads"), "1 of 2 ctx calls (50%)");
    assert_eq!(value(&lines, "followup threshold 30%"), "over");
    // Only the builder called ctx, so the polish role is left out.
    assert_eq!(
        value(&lines, "followup_reads_by_role"),
        "build 1 of 2 (50%)"
    );
    // Lookups: six build reads, the extra read, two ctx calls, the polish read of prd.md: 2 of 10.
    assert_eq!(value(&lines, "ctx_share"), "0.200");
    assert_eq!(value(&lines, "ctx_share threshold 10%"), "ok");
    assert_eq!(value(&lines, "repeat_rate vs 26.7%"), "lower");
    let held: u64 = rows
        .iter()
        .filter(|r| r.pack_doc)
        .map(|r| r.result_chars.unwrap() as u64)
        .sum();
    assert!(held > 0);
    assert_eq!(
        value(&lines, "pack_doc_read_chars").parse::<u64>().unwrap(),
        held
    );
    let per: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| l.starts_with("worker "))
        .collect();
    assert_eq!(per.len(), workers.len(), "{text}");
    let sid: String = first
        .session_id
        .as_deref()
        .unwrap()
        .chars()
        .take(8)
        .collect();
    // `model_id` follows `subagents`, so the head is a prefix.
    let head = format!(
        "worker {} build slice 01 ({sid}): lookups 5, ctx_share 0.400, repeat_rate 0.000, own {} sub 0 subagents 0",
        first.id,
        quota::weigh_own(first).round() as u64
    );
    assert!(per[0].starts_with(&head), "{}", per[0]);
    assert!(per[1].contains(" build slice 02 "), "{}", per[1]);
    assert!(
        per[1].contains("lookups 2, ctx_share 0.000, repeat_rate 0.000, own "),
        "{}",
        per[1]
    );
    assert!(per[1].contains(" sub 0 subagents 0"), "{}", per[1]);
    assert!(text.find("repeat_rate vs 26.7%").unwrap() < text.find(per[0]).unwrap());

    let out = factory(&fx, &["report", &id, "--json", "--per-worker"]);
    let v: Value = serde_json::from_str(&stdout(&out)).expect("one JSON object");
    assert_eq!(v["followup_reads"]["count"], 1);
    assert_eq!(v["followup_reads"]["ctx_calls"], 2);
    assert_eq!(v["followup_reads"]["share"], 0.5);
    assert_eq!(v["followup_reads_by_role"]["build"]["count"], 1);
    assert_eq!(v["followup_reads_by_role"]["build"]["ctx_calls"], 2);
    assert!(v["followup_reads_by_role"]["polish"].is_null());
    assert_eq!(v["pack_doc_read_chars"], held);
    assert!(v["lookup_share_chars"].is_number());
    assert_eq!(v["thresholds"]["followup"], "over");
    assert_eq!(v["thresholds"]["repeat_rate"], "lower");
    assert_eq!(v["per_worker"][0]["ctx_share"], 0.4);
    assert_eq!(v["per_worker"][0]["ctx_calls"], 2);
    assert_eq!(
        v["per_worker"].as_array().map(Vec::len),
        Some(workers.len())
    );
}

fn subagent_run(name: &str, polish: Value) -> (Fixture, String) {
    let fx = Fixture::with_index(name, &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
        polish,
    ]);
    let out = factory(&fx, &["run", "001", "--budget", "70000000", "--foreground"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let id = only_run_id(&fx);
    (fx, id)
}

#[test]
fn a_polish_return_without_findings_is_nulls_and_one_event() {
    let (fx, id) = subagent_run(
        "metrics-findings-missing",
        json!({"outcome": "polish", "skip": ["findings"]}),
    );
    assert_eq!(db(&fx).run_row(&id).unwrap().unwrap().findings, None);
    let events =
        fs::read_to_string(data_dir(&fx).join("runs").join(&id).join("events.jsonl")).unwrap();
    assert_eq!(
        events.matches("\"polish_findings_missing\"").count(),
        1,
        "{events}"
    );
    let text = stdout(&factory(&fx, &["report", &id]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(value(&lines, "findings"), "—");
    assert!(
        value(&lines, "quality").contains(" · findings: — · "),
        "{text}"
    );
    let v: Value =
        serde_json::from_str(&stdout(&factory(&fx, &["report", &id, "--json"]))).unwrap();
    assert!(v["findings"].is_null(), "{v}");
}

fn cwd_of(fx: &Fixture, session: &str) -> PathBuf {
    let calls = fx.calls();
    let call = calls
        .iter()
        .find(|c| c["session_id"] == session)
        .unwrap_or_else(|| panic!("no call of session {session}"));
    PathBuf::from(call["cwd"].as_str().unwrap())
}

fn assert_lane(fx: &Fixture, cwd: &Path) {
    let lanes = data_dir(fx).canonicalize().unwrap().join("lanes");
    assert!(
        cwd.starts_with(&lanes) && cwd.file_name().is_some_and(|n| n == "lane-1"),
        "{} is not lane 1 under {}",
        cwd.display(),
        lanes.display()
    );
}

fn subagents_dir(fx: &Fixture, session: &str) -> PathBuf {
    let config = PathBuf::from(&fx.env()["CLAUDE_CONFIG_DIR"]);
    let t = factory::measure::transcript_path(&config, &cwd_of(fx, session), session);
    t.with_extension("").join("subagents")
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    rd.flatten()
        .flat_map(|e| {
            let p = e.path();
            let mut v = walk(&p);
            v.push(p);
            v
        })
        .collect()
}

/// The stub's own subagent tool ids (`polish-subagents.jsonl`).
const SUB_CALLS: [&str; 3] = [
    "toolu_01A2vUPNoiVFpu8NZt47c9Hw",
    "toolu_01XF4DoDBK9iqyiTtW1y3BoL",
    "toolu_018mpyWtq6JxVie45VoGUJyx",
];

#[test]
fn a_polish_with_subagents_counts_them_apart_from_its_own() {
    let (fx, id) = subagent_run(
        "metrics-subagents",
        json!({"outcome": "polish", "subagents": 2}),
    );
    let db = db(&fx);
    let workers = db.workers(&id).unwrap();
    let polish = workers.iter().find(|w| w.role == "polish").unwrap();
    let session = polish.session_id.as_deref().unwrap();
    let dir = subagents_dir(&fx, session);
    assert!(
        dir.is_dir(),
        "the stub wrote where the driver looks: {} not in {:?}",
        dir.display(),
        walk(Path::new(&fx.env()["CLAUDE_CONFIG_DIR"]))
    );
    // The stub's final usage per message id (tests/stubs/claude).
    assert_eq!(
        (
            polish.sub_input,
            polish.sub_cache_write,
            polish.sub_cache_read,
            polish.sub_output,
            polish.subagents
        ),
        (Some(16), Some(3600), Some(102_000), Some(1000), Some(2)),
        "{polish:?}"
    );
    // The cut's larger own message; its subagent lines run past 220k.
    assert_eq!(polish.peak_context, Some(89_526));
    for w in workers.iter().filter(|w| w.role != "polish") {
        assert_eq!(
            (
                w.sub_input,
                w.sub_cache_write,
                w.sub_cache_read,
                w.sub_output,
                w.subagents
            ),
            (Some(0), Some(0), Some(0), Some(0), Some(0)),
            "{w:?}"
        );
    }

    let stream = fs::read_to_string(
        data_dir(&fx)
            .join("runs")
            .join(&id)
            .join("workers")
            .join(format!("{session}.jsonl")),
    )
    .unwrap();
    let ids: Vec<String> = stream
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "assistant")
        .flat_map(|v| {
            v["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|b| b["type"] == "tool_use")
        .map(|b| b["id"].as_str().unwrap().to_string())
        .collect();
    let calls: Vec<ToolCallRow> = db
        .tool_calls(&id)
        .unwrap()
        .into_iter()
        .filter(|c| c.worker_id == polish.id)
        .collect();
    assert_eq!(calls.len(), ids.len());
    for (c, tid) in calls.iter().zip(&ids) {
        assert_eq!(
            c.in_subagent,
            SUB_CALLS.contains(&tid.as_str()),
            "{tid}: {c:?}"
        );
    }
    assert_eq!(calls.iter().filter(|c| c.in_subagent).count(), 3);
    let events =
        fs::read_to_string(data_dir(&fx).join("runs").join(&id).join("events.jsonl")).unwrap();
    assert!(!events.contains("subagents_unread"), "{events}");

    let out = factory(&fx, &["report", &id, "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let r: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let sub = quota::weigh(16, 3600, 102_000, 1000);
    let own: f64 = workers.iter().map(quota::weigh_own).sum();
    assert_eq!(r["weighted_tokens"], (own + sub).round() as u64);
    assert_eq!(r["weighted_by_role"]["polish"]["sub"], sub.round() as u64);
    assert_eq!(r["weighted_by_role"]["build"]["sub"], 0);
    let sum = |f: fn(&WorkerRow) -> Option<i64>| -> i64 {
        workers.iter().map(|w| f(w).unwrap_or(0)).sum()
    };
    assert_eq!(r["out_tokens"], sum(|w| w.output) + 1000);
    assert_eq!(r["cache_read_tokens"], sum(|w| w.cache_read) + 102_000);
}

#[cfg(unix)]
#[test]
fn an_unreadable_subagents_dir_is_nulls_and_one_event_never_a_failed_run() {
    use std::os::unix::fs::PermissionsExt;
    let (fx, id) = subagent_run(
        "metrics-subagents-unread",
        json!({"outcome": "polish", "subagents": 2, "subagents_unreadable": true}),
    );
    let db = db(&fx);
    let polish = db
        .workers(&id)
        .unwrap()
        .into_iter()
        .find(|w| w.role == "polish")
        .unwrap();
    let dir = subagents_dir(&fx, polish.session_id.as_deref().unwrap());
    // Root reads through any mode: nothing to test there.
    let root = fs::read_dir(&dir).is_ok();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    if root {
        return;
    }
    assert_eq!(db.run_row(&id).unwrap().unwrap().phase, "finished");
    assert_eq!(
        (
            polish.sub_input,
            polish.sub_cache_write,
            polish.sub_cache_read,
            polish.sub_output,
            polish.subagents
        ),
        (None, None, None, None, None),
        "{polish:?}"
    );
    let events =
        fs::read_to_string(data_dir(&fx).join("runs").join(&id).join("events.jsonl")).unwrap();
    assert_eq!(
        events.matches("\"subagents_unread\"").count(),
        1,
        "{events}"
    );
}

/// A tool call row without its id: what `--recount` must write back.
fn call_key(c: &ToolCallRow) -> String {
    format!(
        "{} {} {} {} {:?} {} {} {} {:?} {:?} {} {}",
        c.worker_id,
        c.seq,
        c.tool,
        c.kind,
        c.result_chars,
        c.target_hash,
        c.repeat_in_worker,
        c.repeat_in_run,
        c.label_followup,
        c.label_expanded,
        c.pack_doc,
        c.in_subagent
    )
}

fn polish_row(db: &Db, id: &str) -> WorkerRow {
    db.workers(id)
        .unwrap()
        .into_iter()
        .find(|w| w.role == "polish")
        .unwrap()
}

fn sub_cols(w: &WorkerRow) -> [Option<i64>; 5] {
    [
        w.sub_input,
        w.sub_cache_write,
        w.sub_cache_read,
        w.sub_output,
        w.subagents,
    ]
}

fn events_text(fx: &Fixture, id: &str) -> Vec<u8> {
    fs::read(data_dir(fx).join("runs").join(id).join("events.jsonl")).unwrap()
}

fn weighted_tokens(fx: &Fixture, id: &str) -> u64 {
    let out = factory(fx, &["report", id, "--json"]);
    let v: Value = serde_json::from_str(&stdout(&out)).unwrap();
    v["weighted_tokens"].as_u64().unwrap()
}

#[test]
fn recount_rewrites_the_tokens_and_tool_calls_from_the_streams() {
    let (fx, id) = subagent_run(
        "metrics-recount",
        json!({"outcome": "polish", "subagents": 2}),
    );
    let db = db(&fx);
    let before = polish_row(&db, &id);
    let calls_before: Vec<String> = db.tool_calls(&id).unwrap().iter().map(call_key).collect();
    let gates_before = db.gates(&id).unwrap();
    let weighted_before = weighted_tokens(&fx, &id);
    let events_before = events_text(&fx, &id);
    assert_eq!(before.subagents, Some(2));
    assert_lane(&fx, &cwd_of(&fx, before.session_id.as_deref().unwrap()));

    db.conn()
        .execute(
            "UPDATE workers SET output = 0, sub_input = NULL, sub_cache_write = NULL,
                                sub_cache_read = NULL, sub_output = NULL, subagents = NULL
             WHERE id = ?1",
            [before.id],
        )
        .unwrap();
    let deleted = db
        .conn()
        .execute(
            "DELETE FROM tool_calls WHERE worker_id = ?1 AND seq % 2 = 0",
            [before.id],
        )
        .unwrap();
    assert!(deleted > 0);
    assert_ne!(weighted_tokens(&fx, &id), weighted_before);

    let out = factory(&fx, &["report", &id, "--recount"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    let recount: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("recount: "))
        .collect();
    assert_eq!(recount.len(), 5, "{text}");
    let short: String = before
        .session_id
        .as_deref()
        .unwrap()
        .chars()
        .take(8)
        .collect();
    let sub = quota::weigh(16, 3600, 102_000, 1000).round() as u64;
    assert_eq!(
        recount[4],
        format!(
            "recount: polish {short} own {} sub {sub} subagents 2",
            quota::weigh_own(&before).round() as u64
        )
    );
    assert!(text.contains("\nweighted_tokens: "), "{text}");

    let after = polish_row(&db, &id);
    assert_eq!(after.output, before.output);
    assert_eq!(sub_cols(&after), sub_cols(&before));
    assert_eq!(after, before);
    let calls_after: Vec<String> = db.tool_calls(&id).unwrap().iter().map(call_key).collect();
    assert_eq!(calls_after, calls_before);
    assert_eq!(db.gates(&id).unwrap(), gates_before);
    assert_eq!(weighted_tokens(&fx, &id), weighted_before);
    assert_eq!(events_text(&fx, &id), events_before);

    let out = factory(&fx, &["report", &id, "--json", "--per-worker"]);
    let v: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let per = v["per_worker"].as_array().unwrap();
    let polish = per.iter().find(|p| p["role"] == "polish").unwrap();
    assert_eq!(
        polish["weighted_own"],
        quota::weigh_own(&after).round() as u64
    );
    assert_eq!(polish["weighted_sub"], sub);
    assert_eq!(polish["subagents"], 2);
    let build = per.iter().find(|p| p["role"] == "build").unwrap();
    assert_eq!(build["weighted_sub"], 0);
    assert_eq!(build["subagents"], 0);
    assert!(build["weighted_own"].as_u64().unwrap() > 0);
}

#[test]
fn recount_without_the_transcript_keeps_own_tokens_only() {
    let (fx, id) = subagent_run(
        "metrics-recount-lost",
        json!({"outcome": "polish", "subagents": 2}),
    );
    let db = db(&fx);
    let before = polish_row(&db, &id);
    let session = before.session_id.clone().unwrap();
    let transcript = subagents_dir(&fx, &session)
        .parent()
        .unwrap()
        .with_extension("jsonl");
    assert!(transcript.is_file(), "{}", transcript.display());
    fs::remove_file(&transcript).unwrap();
    let events_before = events_text(&fx, &id);

    let out = factory(&fx, &["report", &id, "--recount", "--per-worker"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    let line = text
        .lines()
        .find(|l| l.starts_with("recount: polish "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(
        line.ends_with(" sub — subagents —, transcript missing, own tokens only"),
        "{line}"
    );
    let after = polish_row(&db, &id);
    assert_eq!(sub_cols(&after), [None; 5]);
    assert_eq!(after.output, before.output);
    assert_eq!(events_text(&fx, &id), events_before);
    let per: Vec<&str> = text.lines().filter(|l| l.starts_with("worker ")).collect();
    assert_eq!(per.len(), 5, "{text}");
    for l in &per {
        assert!(
            l.contains(", own ") && l.contains(" sub ") && l.contains(" subagents "),
            "{l}"
        );
    }
    let polish = per.iter().find(|l| l.contains(" polish ")).unwrap();
    assert!(polish.contains(" sub — subagents —"), "{polish}");

    // A live run's budget reads these rows, so a running run is refused.
    let state = data_dir(&fx).join("runs").join(&id).join("state.json");
    let mut s: Value = serde_json::from_str(&fs::read_to_string(&state).unwrap()).unwrap();
    s["phase"] = json!("building");
    fs::write(&state, serde_json::to_string(&s).unwrap()).unwrap();
    let out = factory(&fx, &["report", &id, "--recount"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("is building"), "{err}");
    assert!(stdout(&out).is_empty());
}

/// The pack's header lines and the blank line after them are not in `pack_parts`.
#[test]
fn every_worker_row_records_what_its_pack_was_made_of() {
    let (fx, id) = finished_run();
    let workers = db(&fx).workers(&id).unwrap();
    let calls = fx.calls();
    let mut lists: Vec<(String, Vec<String>)> = Vec::new();
    for w in &workers {
        let parts: Vec<Value> = serde_json::from_str(
            w.pack_parts
                .as_deref()
                .unwrap_or_else(|| panic!("{} row has no pack_parts", w.role)),
        )
        .unwrap();
        let call = calls
            .iter()
            .find(|c| c["session_id"].as_str() == w.session_id.as_deref())
            .unwrap_or_else(|| panic!("no call for {:?}", w.session_id));
        let pack = call["pack"].as_str().unwrap();
        let (_, body) = pack.split_once("\n\n").unwrap();
        let sum: u64 = parts.iter().map(|p| p["chars"].as_u64().unwrap()).sum();
        assert_eq!(sum, body.chars().count() as u64, "{} pack", w.role);
        let names: Vec<String> = parts
            .iter()
            .map(|p| p["section"].as_str().unwrap().to_string())
            .collect();
        if !lists.iter().any(|(r, _)| *r == w.role) {
            lists.push((w.role.clone(), names));
        }
    }
    let roles: Vec<&str> = lists.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(roles, ["build", "checkpoint", "polish"]);
    assert_eq!(lists[0].1[0], "slice");
    assert_eq!(lists[1].1[0], "gate_transcript");
    assert_eq!(lists[2].1[0], "branch");
    assert!(lists[0].1 != lists[1].1 && lists[1].1 != lists[2].1 && lists[0].1 != lists[2].1);

    let r: Value =
        serde_json::from_str(&stdout(&factory(&fx, &["report", &id, "--json"]))).unwrap();
    let polish = workers.iter().find(|w| w.role == "polish").unwrap();
    let parts: Vec<Value> = serde_json::from_str(polish.pack_parts.as_deref().unwrap()).unwrap();
    let branch = parts.iter().find(|p| p["section"] == "branch").unwrap();
    assert_eq!(
        r["pack_by_role"]["polish"]["sections"]["branch"],
        branch["chars"]
    );
    let starts: Vec<i64> = workers
        .iter()
        .filter(|w| w.role == "build")
        .filter_map(|w| w.start_context)
        .collect();
    let mean = (starts.iter().sum::<i64>() as f64 / starts.len() as f64).round() as u64;
    assert_eq!(r["pack_by_role"]["build"]["start_context"], json!(mean));

    let text = stdout(&factory(&fx, &["report", &id]));
    let line = text
        .lines()
        .find(|l| l.starts_with("pack_by_role: "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(line.starts_with("pack_by_role: build slice "), "{line}");
    assert!(line.contains(" · checkpoint gate_transcript "), "{line}");
    assert!(line.contains(" · polish branch "), "{line}");
}

/// "Before" is read from a copy still at version 2, so the compare is not v3 against itself.
#[test]
fn a_version_2_store_gains_retros_and_reports_the_same() {
    let (fx, id) = finished_run();
    let data = data_dir(&fx);
    let path = data.join("factory.db");
    db(&fx)
        .conn()
        .execute_batch(
            "DROP TABLE retros;
             DELETE FROM schema_version;
             INSERT INTO schema_version (version, applied_at) VALUES (1, 'then'), (2, 'later');",
        )
        .unwrap();
    let raw_shape = || {
        let c = rusqlite::Connection::open(&path).unwrap();
        let retros: i64 = c
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'retros'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let version: i64 = c
            .query_row("SELECT max(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        (retros, version)
    };
    assert_eq!(raw_shape(), (0, 2));

    let copy = tempfile::tempdir().unwrap();
    let cp = std::process::Command::new("cp")
        .arg("-a")
        .arg(data.join("."))
        .arg(copy.path())
        .status()
        .unwrap();
    assert!(cp.success());
    let report = |dir: &Path, json: bool| {
        let mut args = vec!["report", id.as_str()];
        if json {
            args.push("--json");
        }
        let out = fx
            .command(BIN)
            .env("FACTORY_DATA_DIR", dir)
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    };
    let before = (report(copy.path(), false), report(copy.path(), true));
    assert_eq!(raw_shape(), (0, 2), "the original is untouched so far");

    let after = (report(&data, false), report(&data, true));
    let db = db(&fx);
    assert!(db.tables().unwrap().contains(&"retros".to_string()));
    assert_eq!(db.schema_version().unwrap(), 3);
    assert_eq!(
        String::from_utf8_lossy(&after.0),
        String::from_utf8_lossy(&before.0)
    );
    assert_eq!(after.1, before.1);
}

#[test]
fn recount_finds_a_lane_workers_transcript_by_its_init_cwd() {
    let (fx, id) = subagent_run(
        "metrics-recount-lane",
        json!({"outcome": "polish", "subagents": 2}),
    );
    let db = db(&fx);
    let before = polish_row(&db, &id);
    let session = before.session_id.clone().unwrap();
    let sub = subagents_dir(&fx, &session);
    let (from_dir, from) = (
        sub.parent().unwrap().to_path_buf(),
        sub.parent().unwrap().with_extension("jsonl"),
    );
    let lane = fx.path().with_file_name("metrics-recount-lane-2");
    let config = PathBuf::from(&fx.env()["CLAUDE_CONFIG_DIR"]);
    let to = factory::measure::transcript_path(&config, &lane, &session);
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::rename(&from, &to).unwrap();
    fs::rename(&from_dir, to.with_extension("")).unwrap();
    let stream = data_dir(&fx)
        .join("runs")
        .join(&id)
        .join("workers")
        .join(format!("{session}.jsonl"));
    let root = serde_json::to_string(&cwd_of(&fx, &session).to_string_lossy()).unwrap();
    let lane_json = serde_json::to_string(&lane.to_string_lossy()).unwrap();
    let text = fs::read_to_string(&stream).unwrap();
    assert!(text.contains(&format!("\"cwd\":{root}")), "{text}");
    fs::write(
        &stream,
        text.replace(&format!("\"cwd\":{root}"), &format!("\"cwd\":{lane_json}")),
    )
    .unwrap();

    let out = factory(&fx, &["report", &id, "--recount"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    let line = text
        .lines()
        .find(|l| l.starts_with("recount: polish "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(line.ends_with(" subagents 2"), "{line}");
    assert_eq!(sub_cols(&polish_row(&db, &id)), sub_cols(&before));
}
