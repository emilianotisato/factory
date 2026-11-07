//! `evidence`: the files checkpoint workers left behind.
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use super::harness::{FEATURE_DIR, Fixture};
use super::{live, write_run};

/// Writes `rel` under `dir` with its mtime `ago` in the past.
fn file(dir: &Path, rel: &str, ago: Duration) -> PathBuf {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, rel).unwrap();
    let f = fs::File::options().write(true).open(&path).unwrap();
    f.set_modified(SystemTime::now() - ago).unwrap();
    path
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn started(n: u32) -> Value {
    json!({"kind": "checkpoint_started", "n": n, "covers": [1]})
}

fn done(n: u32) -> Value {
    json!({"kind": "checkpoint_done", "n": n, "sha": null})
}

fn paths(cp: &Value) -> Vec<&str> {
    cp["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect()
}

fn all_paths(ev: &Value) -> Vec<&str> {
    ev["checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(paths)
        .collect()
}

#[test]
fn live_lists_evidence_newest_first() {
    let fx = Fixture::new("live-evidence-order");
    let dir = write_run(&fx, json!({}), &[started(1), done(1), started(2)]);
    let ev = dir.join("evidence");
    file(&ev, "cp-1/old.png", secs(600));
    file(&ev, "cp-2/b-tie.png", secs(60));
    file(&ev, "cp-2/a-tie.png", secs(60));
    file(&ev, "cp-2/newest_shot.txt", secs(30));
    file(&ev, "cp-9/other-run.png", secs(5));
    file(&ev, "stray.png", secs(5));

    let e = live(&fx, &[])["evidence"].clone();
    assert_eq!(e["root"], "evidence", "{e}");
    assert_eq!(e["skipped"], 0);
    assert_eq!(e["truncated"], false);
    let cps = e["checkpoints"].as_array().unwrap();
    let ns: Vec<u64> = cps.iter().map(|c| c["n"].as_u64().unwrap()).collect();
    assert_eq!(ns, [2, 1], "{e}");
    assert_eq!(cps[0]["state"], "writing");
    assert_eq!(cps[1]["state"], "done");
    assert_eq!(
        paths(&cps[0]),
        ["cp-2/newest_shot.txt", "cp-2/a-tie.png", "cp-2/b-tie.png"]
    );
    assert_eq!(paths(&cps[1]), ["cp-1/old.png"]);

    let f = &cps[0]["files"][0];
    assert_eq!(f["kind"], "text");
    assert_eq!(f["caption"], "Newest shot");
    let bytes = "cp-2/newest_shot.txt".len();
    assert_eq!(f["bytes"], bytes);
    let mtime = f["mtime"].as_str().unwrap();
    assert!(mtime.ends_with('Z') && mtime.contains('T'), "{mtime}");
    let etag = f["etag"].as_str().unwrap();
    let (b, ms) = etag.split_once('-').unwrap();
    assert_eq!(b, bytes.to_string());
    assert!(ms.parse::<u64>().unwrap() > 1_700_000_000_000, "{etag}");
    assert_eq!(cps[1]["files"][0]["kind"], "image");
}

#[test]
fn live_skips_a_symlink_in_evidence() {
    let fx = Fixture::new("live-evidence-symlink");
    let dir = write_run(&fx, json!({}), &[started(1)]);
    let cp = dir.join("evidence/cp-1");
    file(&cp, "real.png", secs(60));
    std::os::unix::fs::symlink("/etc/passwd", cp.join("passwd.txt")).unwrap();
    let fifo = CString::new(cp.join("pipe.txt").as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);

    let e = live(&fx, &[])["evidence"].clone();
    assert_eq!(all_paths(&e), ["cp-1/real.png"], "{e}");
    assert_eq!(e["skipped"], 2, "{e}");
}

#[test]
fn live_evidence_paths_stay_under_root() {
    let fx = Fixture::new("live-evidence-depth");
    let dir = write_run(&fx, json!({}), &[started(3)]);
    let ev = dir.join("evidence");
    file(&ev, "cp-3/top.png", secs(60));
    file(&ev, "cp-3/a/b/deep.png", secs(60));
    file(&ev, "cp-3/a/b/c/too-deep.png", secs(60));
    std::os::unix::fs::symlink("/etc", ev.join("cp-3/etc")).unwrap();

    let e = live(&fx, &[])["evidence"].clone();
    let listed = all_paths(&e);
    assert_eq!(listed, ["cp-3/a/b/deep.png", "cp-3/top.png"], "{e}");
    for p in listed {
        let path = Path::new(p);
        assert!(path.is_relative(), "{p}");
        assert!(
            path.components().all(|c| matches!(c, Component::Normal(_))),
            "{p}"
        );
    }
    assert_eq!(e["skipped"], 1, "the linked dir is not followed");
    let root = e["root"].as_str().unwrap();
    assert!(Path::new(root).is_relative(), "{root}");
}

#[test]
fn live_marks_a_file_written_this_second_unstable() {
    let fx = Fixture::new("live-evidence-stable");
    let dir = write_run(&fx, json!({}), &[started(1)]);
    let cp = dir.join("evidence/cp-1");
    fs::create_dir_all(&cp).unwrap();
    fs::write(cp.join("fresh.png"), "x").unwrap();
    file(&cp, "settled.png", secs(10));

    let e = live(&fx, &[])["evidence"].clone();
    let files = e["checkpoints"][0]["files"].as_array().unwrap();
    let by = |p: &str| files.iter().find(|f| f["path"] == p).unwrap()["stable"].clone();
    assert_eq!(by("cp-1/fresh.png"), false, "{e}");
    assert_eq!(by("cp-1/settled.png"), true, "{e}");
}

#[test]
fn live_evidence_respects_the_limit() {
    let fx = Fixture::new("live-evidence-limit");
    let dir = write_run(&fx, json!({}), &[started(1), done(1), started(2)]);
    let ev = dir.join("evidence");
    file(&ev, "cp-1/one.png", secs(500));
    file(&ev, "cp-1/two.png", secs(400));
    file(&ev, "cp-2/three.png", secs(300));
    file(&ev, "cp-2/four.png", secs(200));
    file(&ev, "cp-2/five.png", secs(100));

    let e = live(&fx, &["--evidence-limit", "2"])["evidence"].clone();
    assert_eq!(all_paths(&e), ["cp-2/five.png", "cp-2/four.png"], "{e}");
    assert_eq!(e["truncated"], true);

    let e = live(&fx, &[])["evidence"].clone();
    assert_eq!(all_paths(&e).len(), 5, "{e}");
    assert_eq!(e["truncated"], false);
}

#[test]
fn live_evidence_names_the_entries_citing_a_file_and_its_sidecars() {
    let fx = Fixture::new("live-evidence-cited");
    let dir = write_run(&fx, json!({}), &[started(1)]);
    let cp = dir.join("evidence/cp-1");
    file(&cp, "home.png", secs(60));
    file(&cp, "home.aria.txt", secs(50));
    file(&cp, "orders.png", secs(40));
    file(&cp, "lonely.aria.txt", secs(30));
    let entries = [
        json!({"id": "CP-1", "kind": "checkpoint", "ts": "2026-11-07T12:00:00Z", "title": "t",
               "body": "**Prove:** evidence: cp-1/home.png"}),
        json!({"kind": "note", "ts": "2026-11-07T12:00:00Z", "title": "n",
               "body": "also cp-1/home.png, but no id"}),
    ];
    let log: String = entries.iter().map(|e| format!("{e}\n")).collect();
    fs::write(fx.path().join(FEATURE_DIR).join("log.jsonl"), log).unwrap();

    let e = live(&fx, &[])["evidence"].clone();
    let files = e["checkpoints"][0]["files"].as_array().unwrap();
    assert_eq!(
        all_paths(&e),
        ["cp-1/lonely.aria.txt", "cp-1/orders.png", "cp-1/home.png"],
        "{e}"
    );
    let by = |p: &str| files.iter().find(|f| f["path"] == p).unwrap().clone();
    let home = by("cp-1/home.png");
    assert_eq!(home["citedBy"], json!(["CP-1"]));
    assert_eq!(home["sidecars"], json!(["cp-1/home.aria.txt"]));
    assert_eq!(by("cp-1/orders.png")["citedBy"], json!([]));
    assert_eq!(by("cp-1/orders.png")["sidecars"], json!([]));
    assert_eq!(by("cp-1/lonely.aria.txt")["caption"], "Lonely");
}
