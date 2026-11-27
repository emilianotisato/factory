#[path = "harness/mod.rs"]
mod harness;

#[path = "common/passage.rs"]
mod passage;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::evidence::rotate_stale_evidence;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const FEATURE_DIR: &str = "docs/features/001-fixture";
fn prd_code(n: u32) -> String {
    format!("{}-{n:03}", "PRD")
}

fn checkpoint_prompt(n: u32) -> String {
    format!(
        "Run checkpoint {n} of {} as the appended system prompt says. Return the STATUS block.",
        prd_code(1)
    )
}

fn row_key(n: u32) -> String {
    format!("FR-{n}")
}

fn cp_dir(n: u32) -> String {
    format!("cp-{n}")
}

fn sent_back_dir(n: u32, k: u32) -> String {
    format!("cp-{n}.sent-back-{k}")
}

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
    v[0].clone()
}

fn state(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn driver_log(dir: &Path) -> String {
    fs::read_to_string(dir.join("driver.log")).unwrap_or_default()
}

fn wait_phase(_fx: &Fixture, dir: &Path, phase: &str, secs: u64) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while state(dir)["phase"] != phase {
        if Instant::now() >= deadline {
            panic!("timed out waiting for phase {phase}\n{}", driver_log(dir));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn start(fx: &Fixture) -> PathBuf {
    let out = factory(fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    only_run_dir(fx)
}

fn done() -> Value {
    json!({"outcome": "done"})
}

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
        .unwrap();
    let path = PathBuf::from(argv[i + 1]);
    (
        path.file_name().unwrap().to_str().unwrap().to_string(),
        fs::read_to_string(&path).unwrap(),
    )
}

fn qa_plan(fx: &Fixture, table: &[&str], named: &str) {
    let p = fx.path().join(FEATURE_DIR).join("prd.md");
    let text = fs::read_to_string(&p).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.starts_with("| FR |")).unwrap();
    let end = start
        + lines[start..]
            .iter()
            .take_while(|l| l.starts_with('|'))
            .count();
    let mut out: Vec<&str> = lines[..start].to_vec();
    out.extend(table);
    out.extend(&lines[end..]);
    fs::write(&p, out.join("\n") + "\n").unwrap();
    let first_row = row_key(1);
    let prove_needle = format!("the {first_row} row.");
    for n in 1..=3 {
        let s = fx.slice_path(n);
        let text = fs::read_to_string(&s).unwrap();
        assert!(text.contains(&prove_needle), "{}", s.display());
        fs::write(&s, text.replace(&prove_needle, named)).unwrap();
    }
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: the QA plan"]);
}

fn rows_table(rows: &[(u32, &str)]) -> Vec<String> {
    let mut t = vec![
        "| FR | Proof on the real surface | Evidence | Human? |".into(),
        "|---|---|---|---|".into(),
    ];
    for (i, (row_n, human)) in rows.iter().enumerate() {
        let fr = row_key(*row_n);
        t.push(format!(
            "| {fr} | run step {i} in the fixture | transcript | {human} |"
        ));
    }
    t
}

#[test]
fn checkpoint_send_back_tells_respawn_why_and_rotates_stale_evidence() {
    let fx = Fixture::new("send-back-pack");
    fx.chain();
    let row_a = row_key(1);
    let row_b = row_key(2);
    let cp_one = cp_dir(1);
    let table = rows_table(&[(1, "no"), (2, "no")]);
    qa_plan(
        &fx,
        &table.iter().map(String::as_str).collect::<Vec<_>>(),
        &format!("the {row_a} and {row_b} rows."),
    );
    let mut ev_second = serde_json::Map::new();
    ev_second.insert(row_b.clone(), json!("absent"));
    let mut ev_both = serde_json::Map::new();
    ev_both.insert(row_a.clone(), json!("absent"));
    fx.write_plan(&[
        done(),
        done(),
        json!({"outcome": "checkpoint", "covers": [1], "evidence": Value::Object(ev_second)}),
        json!({"outcome": "checkpoint", "covers": [1, 2], "evidence": Value::Object(ev_both)}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "waiting-owner", 90);

    let evidence = dir.join("evidence");
    assert!(
        evidence
            .join(format!("{}/{}", sent_back_dir(1, 1), row_a))
            .join("proof.txt")
            .is_file()
    );
    assert!(!evidence.join(format!("{cp_one}/{row_a}")).exists());

    let calls: Vec<Value> = fx.calls().to_vec();
    let cp_calls: Vec<_> = calls
        .iter()
        .filter(|c| c["prompt"] == checkpoint_prompt(1))
        .collect();
    assert_eq!(cp_calls.len(), 2, "{calls:?}");
    let (_, pack) = pack_of(cp_calls[1]);
    assert!(pack.contains("# Sent back"), "{pack}");
    assert!(pack.contains("Missing slices: 02"), "{pack}");
    assert!(
        pack.contains("re-drive and save fresh evidence for EVERY row of checkpoint 1"),
        "{pack}"
    );

    let text = driver_log(&dir);
    let no_ev = format!("no evidence for {row_a}");
    assert!(
        text.contains(&format!("left {no_ev}")) || text.contains(&no_ev),
        "{text}"
    );
    assert_eq!(fx.log_of("block").len(), 1);

    fs::create_dir_all(evidence.join(format!("{cp_one}/{row_a}"))).unwrap();
    fs::write(
        evidence.join(format!("{cp_one}/{row_a}/proof.txt")),
        "fresh",
    )
    .unwrap();
    let second = rotate_stale_evidence(&evidence, 1).unwrap().unwrap();
    assert_eq!(second, evidence.join(sent_back_dir(1, 2)));

    let skills = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    passage::assert_unique_passage(
        "prd",
        &fs::read_to_string(skills.join("prd/SKILL.md")).unwrap(),
        "external platform behavior",
    );
    passage::assert_unique_passage(
        "slice",
        &fs::read_to_string(skills.join("slice/SKILL.md")).unwrap(),
        "verify external CLI flags against the live binary",
    );
    passage::assert_unique_passage(
        "polish",
        &fs::read_to_string(skills.join("polish/SKILL.md")).unwrap(),
        "never invoke third-party CLIs or lazy shims not under review",
    );
}
