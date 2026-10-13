//! `factory docs migrate` on the checked-in v2 fixture (PRD-002 FR-16, slice
//! 07): the dry run writes nothing, the real one leaves a tree `check` passes
//! with the same board, the git undo restores the v2 tree, and a re-run lands
//! on the same bytes. Needs `node` on PATH for the v2 side, as CI has.
#[path = "harness/mod.rs"]
mod harness;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use factory::db::{Db, RetroRow};
use factory::run::Run;
use harness::Fixture;
use serde_json::Value;

const LOG: &str = "docs/features/001-fixture/agent-log.md";
const RETRO: &str = "docs/features/001-fixture/retro.md";

fn text(o: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn migrate(fx: &Fixture, dry: bool) -> Output {
    let args: &[&str] = if dry {
        &["migrate", "--dry"]
    } else {
        &["migrate"]
    };
    fx.factory_docs_output(args)
}

fn node_check(fx: &Fixture) -> Output {
    fx.command("node")
        .args([".docs/bin/docs.mjs", "check"])
        .output()
        .expect("run docs.mjs check")
}

fn json(o: Output, what: &str) -> Value {
    let (out, err) = text(&o);
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("{what} is not JSON ({e})\n{out}\n{err}"))
}

fn node_status(fx: &Fixture, feature: &str) -> Value {
    let o = fx
        .command("node")
        .args([".docs/bin/docs.mjs", "status", feature, "--json"])
        .output()
        .expect("run docs.mjs status");
    json(o, "node status")
}

fn v3_status(fx: &Fixture, feature: &str) -> Value {
    json(
        fx.factory_docs_output(&["status", feature, "--json"]),
        "factory docs status",
    )
}

fn sorted(v: &Value) -> Vec<i64> {
    let mut n: Vec<i64> = v
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();
    n.sort_unstable();
    n
}

/// What FR-16 says `status --json` keeps: the slices, the feature's status,
/// the lanes, what is ready and next, the checkpoints and the log counts.
/// `flagged` and `decide` are compared as sets: v2 lists them in file order,
/// v3 by id.
fn same_board(before: &Value, after: &Value) {
    let slices = |v: &Value| -> Vec<Value> {
        v["slices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "number": s["number"],
                    "status": s["status"],
                    "lane": s["lane"],
                    "depends_on": s["depends_on"],
                })
            })
            .collect()
    };
    assert_eq!(slices(before), slices(after), "slices");
    for k in [
        "status",
        "lanes",
        "freeLanes",
        "ready",
        "next",
        "checkpoints",
    ] {
        assert_eq!(before[k], after[k], "{k}");
    }
    assert_eq!(
        before["log"]["decisions"], after["log"]["decisions"],
        "log.decisions"
    );
    for k in ["flagged", "decide"] {
        assert_eq!(
            sorted(&before["log"][k]),
            sorted(&after["log"][k]),
            "log.{k}"
        );
    }
}

/// Every file under `docs/`, by path.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, root, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&root.join("docs"), root, &mut out);
    out
}

fn repo_key(fx: &Fixture) -> String {
    fx.path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn row(fx: &Fixture, feature: u32) -> RetroRow {
    let db = Db::open(&Run::db_path(&fx.data_dir())).unwrap();
    db.retro(&repo_key(fx), feature)
        .unwrap()
        .unwrap_or_else(|| panic!("no retros row for {feature}"))
}

/// A retro frontmatter number as the store holds it.
fn fm_number(fm: &str, key: &str) -> Option<f64> {
    fm.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}:")))
        .and_then(|v| v.trim().parse().ok())
}

/// The row against the frontmatter it came from, all thirteen numbers.
fn row_matches(row: &RetroRow, retro: &str) {
    let fm = &retro[..retro[4..].find("\n---").unwrap() + 4];
    let int = |k: &str| {
        fm_number(fm, k)
            .filter(|n| n.fract() == 0.0)
            .map(|n| n as i64)
    };
    assert_eq!(row.run_id, None);
    assert_eq!(row.score, int("score"), "score");
    assert_eq!(row.gate_runs, int("gate_runs"));
    assert_eq!(row.gate_red, int("gate_red"));
    assert_eq!(row.gate_minutes, fm_number(fm, "gate_minutes"));
    assert_eq!(row.ci_cycles, int("ci_cycles"));
    assert_eq!(row.slices, int("slices"));
    assert_eq!(row.checkpoints, int("checkpoints"));
    assert_eq!(row.build_minutes, fm_number(fm, "build_minutes"));
    assert_eq!(row.polish_minutes, fm_number(fm, "polish_minutes"));
    assert_eq!(row.out_tokens, int("out_tokens"));
    assert_eq!(row.cache_read_tokens, int("cache_read_tokens"));
    assert_eq!(row.start_context, int("start_context"));
    assert_eq!(row.peak_context, int("peak_context"));
}

/// The `### ` headings outside fences of a v2 log, each with its body's
/// first non-blank line.
fn headings(log: &str) -> Vec<(String, Option<String>)> {
    let lines: Vec<&str> = log.lines().collect();
    let mut fenced = false;
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if fenced || !l.starts_with("### ") {
            continue;
        }
        let first = lines[i + 1..]
            .iter()
            .find(|b| !b.trim().is_empty())
            .filter(|b| !b.starts_with('#') && b.trim() != "---")
            .map(|b| b.to_string());
        out.push((l.to_string(), first));
    }
    out
}

/// The whole of FR-16's acceptance on one repo, for each live feature in
/// `features`: steps 1-4 of the slice's task 7.
fn migrate_and_undo(fx: &Fixture, features: &[&str]) -> BTreeMap<String, Vec<u8>> {
    // 1. The v2 tree is sound.
    let o = node_check(fx);
    assert!(o.status.success(), "node check before: {:?}", text(&o));
    let before: Vec<Value> = features.iter().map(|f| node_status(fx, f)).collect();

    // 2. The dry run writes nothing, the store included.
    let o = migrate(fx, true);
    let (plan, err) = text(&o);
    assert_eq!(o.status.code(), Some(0), "migrate --dry\n{plan}\n{err}");
    assert_eq!(fx.git(&["status", "--porcelain"]), "", "--dry wrote");
    assert_eq!(
        fs::read_dir(fx.data_dir()).unwrap().count(),
        0,
        "--dry opened the store"
    );
    for want in [
        "docs/project.toml",
        "/state.toml",
        "/log.jsonl",
        "row retros",
    ] {
        assert!(plan.contains(want), "plan names {want}:\n{plan}");
    }
    assert!(
        plan.trim_end().ends_with("dry run: nothing written"),
        "{plan}"
    );
    eprintln!("--- migrate --dry\n{plan}");

    // 3. The real run: a tree `check` passes, with the same board.
    let o = migrate(fx, false);
    let (out, err) = text(&o);
    assert_eq!(o.status.code(), Some(0), "migrate\n{out}\n{err}");
    assert!(out.contains("migrated: "), "{out}");
    let o = fx.factory_docs_output(&["check"]);
    let (cout, cerr) = text(&o);
    eprintln!("--- factory docs check\n{cout}{cerr}");
    assert_eq!(o.status.code(), Some(0), "check\n{cout}\n{cerr}");
    assert!(!cerr.contains(" E-") && !cout.contains(" E-"), "{cerr}");
    for (f, b) in features.iter().zip(&before) {
        same_board(b, &v3_status(fx, f));
    }
    assert!(
        !fx.git(&["status", "--porcelain"]).contains("docs.lock"),
        "migrate left a docs.lock"
    );
    let migrated = snapshot(fx.path());

    // 4. The git undo restores v2; a second run lands on the same bytes.
    fx.git(&["checkout", "--", "docs"]);
    fx.git(&["clean", "-fdq", "docs"]);
    let o = node_check(fx);
    assert!(
        o.status.success(),
        "node check after the undo: {:?}",
        text(&o)
    );
    assert_eq!(
        fx.git(&["status", "--porcelain"]),
        "",
        "the undo left files"
    );
    let o = migrate(fx, false);
    assert_eq!(o.status.code(), Some(0), "second migrate: {:?}", text(&o));
    let again = snapshot(fx.path());
    assert_eq!(
        migrated.keys().collect::<Vec<_>>(),
        again.keys().collect::<Vec<_>>()
    );
    for (k, v) in &migrated {
        assert!(again[k] == *v, "{k} differs on the second run");
    }
    migrated
}

#[test]
fn the_v2_fixture_migrates_and_the_undo_restores_it() {
    let fx = Fixture::v2("migrate");
    let old_log = fx.git(&["show", &format!("HEAD:{LOG}")]);
    let old_retro = fx.git(&["show", &format!("HEAD:{RETRO}")]);

    let tree = migrate_and_undo(&fx, &["001"]);

    // The log: every v2 heading, its body and its review line.
    let shown = fx.factory_docs(&["log", "show", "001"]);
    for (h, first) in headings(&old_log) {
        assert!(shown.contains(&h), "log show lost {h}:\n{shown}");
        if let Some(f) = first {
            assert!(shown.contains(&f), "log show lost the body of {h}: {f}");
        }
    }
    for (id, review) in [
        ("AD-101", "**Review:** decide"),
        ("AD-201", "**Review:** fyi"),
        ("AD-501", "**Review:** decide"),
    ] {
        let at = shown.find(&format!("### {id} ")).unwrap();
        let next = shown[at + 4..]
            .find("\n### ")
            .map_or(shown.len(), |i| at + 4 + i);
        assert!(shown[at..next].contains(review), "{id} lost {review}");
    }
    let entries: Vec<Value> =
        String::from_utf8(tree["docs/features/001-fixture/log.jsonl"].clone())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
    let by = |k: &str| entries.iter().filter(|e| e["kind"] == k).count();
    assert_eq!(
        (
            by("decision"),
            by("ask"),
            by("block"),
            by("answer"),
            by("review"),
            by("checkpoint")
        ),
        (3, 1, 1, 1, 2, 1)
    );
    let block = entries.iter().find(|e| e["id"] == "BLOCK-201").unwrap();
    assert_eq!(
        block["ts"], "2026-09-30T00:00:00Z",
        "a BLOCK takes its blame date"
    );
    let polish = entries.iter().find(|e| e["id"] == "AD-501").unwrap();
    assert_eq!(polish["phase"], "polish");

    // The retro: a row equal to the frontmatter, the file untouched.
    row_matches(&row(&fx, 1), &old_retro);
    assert_eq!(fx.read(RETRO), old_retro, "retro.md changed");
    let project = fx.read("docs/project.toml");
    assert!(
        project.starts_with(
            "[run]\nbudget = 5\n\n[workers.polish]\nmodel = \"sonnet\"\n\n[docs]\nframework = 3\n"
        ),
        "{project}"
    );
    assert!(!fx.path().join(LOG).exists());

    // A re-run's row is the same in every column but `written_at`.
    let first = row(&fx, 1);
    fx.git(&["checkout", "--", "docs"]);
    fx.git(&["clean", "-fdq", "docs"]);
    assert_eq!(migrate(&fx, false).status.code(), Some(0));
    let second = row(&fx, 1);
    assert_eq!(
        RetroRow {
            written_at: String::new(),
            ..first
        },
        RetroRow {
            written_at: String::new(),
            ..second
        }
    );

    // 5. A migrated tree and a dirty one both refuse.
    let o = migrate(&fx, false);
    let (_, err) = text(&o);
    assert_eq!(o.status.code(), Some(2));
    assert!(err.contains("already migrated"), "{err}");
    fx.git(&["checkout", "--", "docs"]);
    fx.git(&["clean", "-fdq", "docs"]);
    fs::write(fx.path().join("docs/ideas.md"), "dirty\n").unwrap();
    let o = migrate(&fx, false);
    let (_, err) = text(&o);
    assert_eq!(o.status.code(), Some(2));
    assert!(err.contains("not clean"), "{err}");
    let project = fs::read_to_string(fx.path().join("docs/project.toml")).unwrap_or_default();
    assert!(!project.contains("[docs]"), "a refused run wrote {project}");
}

/// FR-16 on a copy of this repo's own `docs/` at HEAD: PRD-001's 145
/// decisions, 7 checkpoints, 1 block and 13 slices. Ignored in the inner
/// loop because the live docs change with every commit; it runs at the
/// slice's checkpoint and before the owner's real migration (§14).
#[test]
#[ignore]
fn prd_001_copy_migrates() {
    let src = tempfile::tempdir().unwrap();
    let archive = Command::new("git")
        .args(["archive", "--format=tar", "HEAD", "docs"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git archive");
    assert!(archive.status.success());
    let tar = src.path().join("docs.tar");
    fs::write(&tar, &archive.stdout).unwrap();
    let ok = Command::new("tar")
        .args(["-xf", tar.to_str().unwrap()])
        .current_dir(src.path())
        .status()
        .unwrap()
        .success();
    assert!(ok, "tar -x");
    let project = fs::read_to_string(src.path().join("docs/project.toml")).unwrap_or_default();
    if project.contains("[docs]") {
        eprintln!("skipped: docs already v3");
        return;
    }
    // Mid-run, HEAD holds the building slices' unmerged `log-*.md`, which
    // migrate refuses; merge them in the copy as the refusal tells the owner.
    let docs_mjs =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture/v2/.docs/bin/docs.mjs");
    for e in fs::read_dir(src.path().join("docs/features")).unwrap() {
        let dir = e.unwrap().path();
        let fragment = fs::read_dir(&dir).unwrap().any(|f| {
            let n = f.unwrap().file_name().to_string_lossy().into_owned();
            n.starts_with("log-") && n.ends_with(".md")
        });
        if fragment {
            let n = &dir.file_name().unwrap().to_string_lossy()[..3];
            let merged = Command::new("node")
                .arg(&docs_mjs)
                .args(["log", "merge", n])
                .current_dir(src.path())
                .output()
                .expect("node docs.mjs log merge");
            assert!(
                merged.status.success(),
                "log merge {n}: {}",
                String::from_utf8_lossy(&merged.stderr)
            );
        }
    }
    let fx = Fixture::v2_with_docs("migrate-001", &src.path().join("docs"));
    let mut live: Vec<String> = fs::read_dir(fx.path().join("docs/features"))
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().to_string_lossy().into_owned();
            n.get(..3)
                .filter(|d| d.bytes().all(|b| b.is_ascii_digit()))
                .map(str::to_string)
        })
        .collect();
    live.sort();
    let features: Vec<&str> = live.iter().map(String::as_str).collect();
    let tree = migrate_and_undo(&fx, &features);

    let s = v3_status(&fx, "001");
    assert_eq!(s["log"]["decisions"], 145);
    assert_eq!(s["checkpoints"].as_array().unwrap().len(), 7);
    let shown = fx.factory_docs(&["log", "show", "001"]);
    // The template's fenced `### BLOCK-1` example stays in a note: skip fences.
    let blocks: Vec<String> = headings(&shown)
        .into_iter()
        .map(|(h, _)| h)
        .filter(|h| h.starts_with("### BLOCK-"))
        .collect();
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert!(blocks[0].ends_with("resolved"), "{}", blocks[0]);
    let state = String::from_utf8(tree["docs/features/001-driver/state.toml"].clone()).unwrap();
    assert_eq!(state.matches("[slices.").count(), 13, "{state}");
    assert_eq!(state.matches("status = \"done\"").count(), 13, "{state}");
    for (n, dir) in [(1, "001-driver"), (3, "003-observatory")] {
        let retro =
            fs::read_to_string(src.path().join(format!("docs/features/{dir}/retro.md"))).unwrap();
        row_matches(&row(&fx, n), &retro);
    }
}
