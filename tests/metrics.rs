//! FR-8 (PRD §10) through the binary: after a fixture run every tool call is
//! a `tool_calls` row with its kind, every worker inner-loop run a `gates`
//! row with source `worker`, and `factory report <run>` prints the retro
//! keys then the run keys, with `--json` and `--cockpit`.
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

const KEYS: [&str; 21] = [
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

/// `key` of a `key: value` line.
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

/// Three chained slices, the second with a red inner run, one closing
/// checkpoint; the driver runs in the foreground so the run is over on return.
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

    // Each build: Read prd, Read slice, Write code, Write fragment, Bash inner, Edit slice.
    for b in &builds {
        let mine: Vec<&ToolCallRow> = rows.iter().filter(|r| r.worker_id == b.id).collect();
        assert_eq!(
            mine.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
            ["read", "read", "edit", "edit", "run-gate", "edit"],
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
        // Read slice then Edit slice: the same file, so the Edit repeats.
        assert!(
            !mine[1].repeat_in_worker && mine[5].repeat_in_worker,
            "{mine:?}"
        );
    }
    // The checkpoint worker writes agent-log.md: one row of its own.
    let cp = workers.iter().find(|w| w.role == "checkpoint").unwrap();
    let cp_rows: Vec<&ToolCallRow> = rows.iter().filter(|r| r.worker_id == cp.id).collect();
    assert_eq!(cp_rows.len(), 1);
    assert_eq!(cp_rows[0].kind, "edit");
    // The polish worker (slice 09): Read prd, Write code, Bash inner, Write
    // retro, Edit prd, Write the PR body.
    let polish = workers.iter().find(|w| w.role == "polish").unwrap();
    let polish_rows: Vec<&ToolCallRow> = rows.iter().filter(|r| r.worker_id == polish.id).collect();
    assert_eq!(
        polish_rows
            .iter()
            .map(|r| r.kind.as_str())
            .collect::<Vec<_>>(),
        ["read", "edit", "run-gate", "edit", "edit", "edit"]
    );
    assert_eq!(rows.len(), 3 * 6 + 1 + 6);

    // Every build reads prd.md first: the second and third see it from the
    // run, none from its own worker. The slice files differ per build.
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
    // `start_context` landed on every finished worker (the report's min).
    assert!(
        workers.iter().all(|w| w.start_context.is_some()),
        "{workers:?}"
    );
}

#[test]
fn a_worker_inner_run_is_a_gates_row_with_source_worker() {
    let (fx, id) = finished_run();
    let gates: Vec<GateRow> = db(&fx).gates(&id).unwrap();
    let worker: Vec<&GateRow> = gates.iter().filter(|g| g.source == "worker").collect();
    // Three builds' inner runs and the polish worker's (slice 09); slice 08
    // labels every worker's run `slice`, the polish one included.
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
    // The driver's checkpoint gate is still its own row, untouched.
    let driver: Vec<&GateRow> = gates.iter().filter(|g| g.source == "driver").collect();
    assert_eq!(driver.len(), 1);
    assert_eq!(driver[0].kind, "checkpoint");
    // The stub's inner run never executed: the fixture gate log has only the
    // driver's run.
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
    assert_eq!(&keys[..21], &KEYS);
    assert_eq!(
        &keys[21..],
        [
            "worker_gate_runs",
            "worker_gate_red",
            "cockpit_peak_context"
        ]
    );
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
    // The stub polish worker returns within a second; the figure is its
    // duration, two decimals.
    let polish_minutes: f64 = value(&lines, "polish_minutes").parse().unwrap();
    assert!((0.0..0.5).contains(&polish_minutes), "{polish_minutes}");

    // Slice 07's formula over the workers table.
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

    // The lookup set is the two Reads per build; the rest is edits and the
    // inner run.
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
    let share: f64 = value(&lines, "ctx_share").parse().unwrap();
    assert!((0.0..=1.0).contains(&share), "{share}");
    assert!(
        (share - lookup as f64 / all as f64).abs() < 0.001,
        "{share}"
    );
    assert_eq!(value(&lines, "repeat_rate"), "0.000");
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
    let text = stdout(&factory(&fx, &["report", &id]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        v["weighted_tokens"].as_u64().unwrap(),
        value(&lines, "weighted_tokens").parse::<u64>().unwrap()
    );
    for k in KEYS.iter().chain(
        [
            "worker_gate_runs",
            "worker_gate_red",
            "cockpit_peak_context",
        ]
        .iter(),
    ) {
        assert!(v.get(k).is_some(), "{k} missing from {v}");
    }

    // The fixture transcript, pinned against `measure.py --retro`
    // (src/measure.rs): out 2524, cache read 181841, peak 58351; with input
    // 98 and cache write 32926 the weighted figure is 63828. Two paths add.
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
    let out = factory(&fx, &["report", &id, "--json", "--cockpit", cp, cp]);
    let v: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["cockpit_tokens"], 63828 * 2);
    assert_eq!(v["cockpit_peak_context"], 58351);

    // An unknown run or a missing transcript is an error, not a zero report.
    let out = factory(&fx, &["report", "001-nope"]);
    assert_ne!(out.status.code(), Some(0));
    let out = factory(&fx, &["report", &id, "--cockpit", "/nonexistent.jsonl"]);
    assert_ne!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nonexistent"));
}
