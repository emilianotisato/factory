#[path = "harness/mod.rs"]
mod harness;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Output;

use factory::db::{Db, RetroRow};
use factory::docs::ideas;
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
    fx.docs_output(args)
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
        fx.docs_output(&["status", feature, "--json"]),
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

/// `flagged` and `decide` compare as sets: v2 lists them in file order, v3 by id.
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

fn fm_number(fm: &str, key: &str) -> Option<f64> {
    fm.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}:")))
        .and_then(|v| v.trim().parse().ok())
}

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

fn migrate_and_undo(fx: &Fixture, features: &[&str]) -> BTreeMap<String, Vec<u8>> {
    let o = node_check(fx);
    assert!(o.status.success(), "node check before: {:?}", text(&o));
    let before: Vec<Value> = features.iter().map(|f| node_status(fx, f)).collect();

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

    let o = migrate(fx, false);
    let (out, err) = text(&o);
    assert_eq!(o.status.code(), Some(0), "migrate\n{out}\n{err}");
    assert!(out.contains("migrated: "), "{out}");
    let o = fx.docs_output(&["check"]);
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

    let shown = fx.docs(&["log", "show", "001"]);
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
        (
            "AD-10001",
            "\n**Review:** fyi — a reviewer reading the diff would miss the extra run\n",
        ),
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
        (4, 1, 1, 1, 2, 1)
    );
    let ads: Vec<&str> = entries
        .iter()
        .filter_map(|e| e["id"].as_str())
        .filter(|id| id.starts_with("AD-"))
        .collect();
    assert_eq!(ads, ["AD-1", "AD-101", "AD-201", "AD-501", "AD-10001"]);
    let cp = entries.iter().find(|e| e["id"] == "AD-10001").unwrap();
    assert_eq!(cp["phase"], "checkpoint");
    assert_eq!(cp["flag"], "fyi");
    let block = entries.iter().find(|e| e["id"] == "BLOCK-201").unwrap();
    assert_eq!(
        block["ts"], "2026-09-30T00:00:00Z",
        "a BLOCK takes its blame date"
    );
    let polish = entries.iter().find(|e| e["id"] == "AD-501").unwrap();
    assert_eq!(polish["phase"], "polish");

    row_matches(&row(&fx, 1), &old_retro);
    assert_eq!(
        fx.read(RETRO),
        old_retro
            .replace("[the log](agent-log.md#ad-201)", "the log")
            .replace("](../../ideas.md)", "](../../ideas/README.md)"),
        "retro.md"
    );
    assert_ne!(fx.read(RETRO), old_retro, "the fixture retro has the links");
    let project = fx.read("docs/project.toml");
    assert!(
        project.starts_with(
            "[run]\nbudget = 5\n\n[workers.polish]\nmodel = \"sonnet\"\n\n[docs]\nframework = 3\n"
        ),
        "{project}"
    );
    assert!(!fx.path().join(LOG).exists());
    let prd = fx.read("docs/features/001-fixture/prd.md");
    assert!(
        prd.contains(
            "\n**Split from / Related**: the inbox line in [`ideas.md`](../../ideas/README.md).\n"
        ),
        "{prd}"
    );
    assert!(
        !prd.contains("**Owns FR range**") && !prd.contains("**Status**:"),
        "{prd}"
    );
    assert!(tree.contains_key("docs/ideas/README.md"));
    assert!(!tree.contains_key("docs/ideas.md"));

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

fn source_body(src: &str, title: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with(&format!("- **{title}")))
        .unwrap_or_else(|| panic!("no bullet {title}"));
    let first = &lines[at][4..];
    let rest = &first[first.find("**").unwrap() + 2..];
    let rest = rest
        .strip_prefix(" — ")
        .or_else(|| rest.strip_prefix(' '))
        .unwrap_or(rest);
    let mut body = format!("{rest}\n");
    for l in &lines[at + 1..] {
        if l.trim().is_empty() || l.starts_with("- ") || l.starts_with('#') {
            break;
        }
        body.push_str(l);
        body.push('\n');
    }
    body
}

/// The undo and re-run of `docs/ideas/` are the fixture test's snapshot of all `docs/`.
#[test]
fn ideas_migrate() {
    let fx = Fixture::v2("migrate-ideas");
    let source = fx.git(&["show", "HEAD:docs/ideas.md"]);

    let o = migrate(&fx, true);
    let (plan, err) = text(&o);
    assert_eq!(o.status.code(), Some(0), "migrate --dry\n{plan}\n{err}");
    eprintln!("--- migrate --dry\n{plan}");
    let rows: Vec<&str> = plan.lines().collect();
    assert!(
        rows[0].starts_with("guess ") && rows[0].contains("\"Echo\""),
        "{plan}"
    );
    let row = |title: &str| {
        let q = format!("\"{title}\"");
        let hits: Vec<&&str> = rows.iter().filter(|r| r.contains(&q)).collect();
        assert_eq!(hits.len(), 1, "{title}:\n{plan}");
        hits[0].to_string()
    };
    for (title, start) in [
        ("Alpha", "idea 2026-09-01-alpha open \""),
        ("Bravo", "idea 2026-09-30-bravo open high \""),
        ("Charlie", "idea 2026-09-15-charlie owed \""),
        ("Echo", "guess 2026-09-22-echo open \""),
        ("Delta", "idea not migrated \""),
    ] {
        assert!(row(title).starts_with(start), "{}", row(title));
    }
    let drops: Vec<&&str> = rows
        .iter()
        .filter(|r| r.starts_with("drop docs/ideas.md:"))
        .collect();
    assert_eq!(
        drops.len(),
        7,
        "five rules and two placeholders: {drops:#?}"
    );
    assert_eq!(
        drops.iter().filter(|r| r.contains("_(")).count(),
        2,
        "{drops:#?}"
    );
    assert!(rows.contains(&"delete docs/ideas.md"), "{plan}");

    let o = migrate(&fx, false);
    assert_eq!(o.status.code(), Some(0), "migrate: {:?}", text(&o));
    let mut names: Vec<String> = fs::read_dir(fx.path().join("docs/ideas"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "2026-09-01-alpha.md",
            "2026-09-15-charlie.md",
            "2026-09-22-echo.md",
            "2026-09-30-bravo.md",
            "README.md"
        ]
    );
    assert!(!fx.path().join("docs/ideas.md").exists());
    let list = json(
        fx.docs_output(&["idea", "list", "--json"]),
        "idea list --json",
    );
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 4, "{list:#?}");
    let null = Value::Null;
    for (id, title, state, added, origin, retrigger, priority) in [
        (
            "2026-09-01-alpha",
            "Alpha",
            "open",
            "2026-09-01",
            None,
            None,
            None,
        ),
        (
            "2026-09-30-bravo",
            "Bravo",
            "open",
            "2026-09-30",
            None,
            Some("when PRD-002 ships."),
            Some("high"),
        ),
        (
            "2026-09-15-charlie",
            "Charlie",
            "owed",
            "2026-09-15",
            Some("PRD-001 polish"),
            None,
            None,
        ),
        (
            "2026-09-22-echo",
            "Echo",
            "open",
            "2026-09-22",
            Some("PRD-001 polish review"),
            None,
            None,
        ),
    ] {
        let r = list
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{id}"));
        let opt = |k: &str, v: Option<&str>| {
            let want = v.map_or(null.clone(), Value::from);
            assert_eq!(r[k], want, "{id} {k}");
        };
        assert_eq!(
            (&r["title"], &r["state"], &r["added"]),
            (
                &Value::from(title),
                &Value::from(state),
                &Value::from(added)
            ),
            "{id}"
        );
        opt("origin", origin);
        opt("retrigger", retrigger);
        opt("priority", priority);
        let idea = ideas::read(&fx.path().join(format!("docs/ideas/{id}.md"))).unwrap();
        assert_eq!(idea.body, source_body(&source, title), "{id} body");
    }
    assert!(
        source_body(&source, "Alpha").contains("\n  and its second line"),
        "the multi-line body"
    );
    let index = fx.read("docs/index.md");
    assert!(
        index.contains("[`ideas.md`](ideas/README.md)") && !index.contains("](ideas.md"),
        "the index link"
    );

    let o = fx.docs_output(&["check"]);
    let (out, err) = text(&o);
    eprintln!("--- factory docs check\n{out}{err}");
    assert_eq!(o.status.code(), Some(0), "check\n{out}\n{err}");
    assert!(!out.contains("W-") && !err.contains("W-"), "{out}{err}");
}
