//! Ideas as one file per idea: `idea add|list|drop`, `new --from-idea`,
//! `init`'s README and `check` on the folder (PRD-002 slice 05, FR-20).
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use factory::docs::{frontmatter, ideas};
use harness::Fixture;
use serde_json::Value;

const README: &str = include_str!("../assets/docs/ideas-README.md");

/// `factory docs idea add …` with `body` on stdin.
fn add(fx: &Fixture, args: &[&str], body: &str) -> Output {
    let mut child = fx
        .command(env!("CARGO_BIN_EXE_factory"))
        .args(["docs", "idea", "add"])
        .args(args)
        .args(["--body-file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn idea add");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    child.wait_with_output().expect("idea add")
}

/// The id `idea add` printed; panics on a non-zero exit.
fn add_ok(fx: &Fixture, args: &[&str], body: &str) -> String {
    let o = add(fx, args, body);
    assert!(o.status.success(), "idea add {args:?}: {}", stderr(&o));
    stdout(&o).trim().to_string()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The idea files, README excluded.
fn idea_files(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root.join("docs/ideas"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n != "README.md")
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn list_json(fx: &Fixture) -> (Vec<Value>, String) {
    let o = fx.docs_output(&["idea", "list", "--json"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let v: Value = serde_json::from_str(&stdout(&o)).expect("idea list --json");
    (v.as_array().expect("an array").clone(), stderr(&o))
}

#[test]
fn concurrent_adds_of_one_title_give_one_file_each() {
    let fx = Fixture::new("ideas-concurrent");
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| {
                for _ in 0..20 {
                    add_ok(&fx, &["--title", "Same"], "Body.\n");
                }
            });
        }
    });
    let files = idea_files(fx.path());
    assert_eq!(files.len(), 40, "{files:?}");
    for f in &files {
        let idea = ideas::read(&fx.path().join("docs/ideas").join(f)).expect(f);
        assert_eq!(idea.title, "Same");
        assert_eq!(idea.state, "open");
    }
    assert!(files.iter().any(|f| f.ends_with("-same-2.md")));
    let check = fx.docs_output(&["check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
}

#[test]
fn add_slugs_refuses_and_round_trips() {
    let fx = Fixture::new("ideas-add");
    let long = "A very long title that goes on and on well past fifty characters, really";
    assert!(long.len() >= 70);
    let id = add_ok(&fx, &["--title", long], "x\n");
    let slug = &id[11..];
    assert!(slug.len() <= 50 && !slug.ends_with('-'), "{slug}");
    assert!(fx.path().join(format!("docs/ideas/{id}.md")).is_file());

    for bad in [
        &["--title", "Later", "--state", "later"][..],
        &["--title", "Low", "--priority", "low"],
        &["--title", "Both \"double\" and 'single'"],
    ] {
        let before = idea_files(fx.path());
        let o = add(&fx, bad, "x\n");
        assert_eq!(o.status.code(), Some(2), "{bad:?}: {}", stdout(&o));
        assert_eq!(idea_files(fx.path()), before, "{bad:?} wrote a file");
    }

    let quoted = "Say \"hello\" to it";
    let id = add_ok(
        &fx,
        &[
            "--title",
            quoted,
            "--state",
            "owed",
            "--origin",
            "it's the owner",
        ],
        "y\n",
    );
    let (rows, _) = list_json(&fx);
    let row = rows
        .iter()
        .find(|r| r["id"] == id.as_str())
        .expect("listed");
    assert_eq!(row["title"], quoted);
    assert_eq!(row["origin"], "it's the owner");
    assert_eq!(row["state"], "owed");

    fs::remove_dir_all(fx.path().join("docs/ideas")).unwrap();
    let id = add_ok(&fx, &["--title", "Fresh"], "z\n");
    assert!(fx.path().join(format!("docs/ideas/{id}.md")).is_file());
}

#[test]
fn list_is_the_scanner_contract() {
    let fx = Fixture::new("ideas-list");
    let body = "Owed by the bank. They answer by mail? Not yet.\n\nSecond paragraph, with recon.\n";
    let owed = add_ok(
        &fx,
        &[
            "--title",
            "Bank answer",
            "--state",
            "owed",
            "--priority",
            "high",
        ],
        body,
    );
    let plain = add_ok(&fx, &["--title", "Plain one"], "Just a line\n");
    fs::write(
        fx.path().join("docs/ideas/2026-01-01-broken.md"),
        "no frontmatter here\n",
    )
    .unwrap();

    let (rows, err) = list_json(&fx);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert!(err.contains("docs/ideas/2026-01-01-broken.md"), "{err}");
    let keys = [
        "id",
        "path",
        "title",
        "state",
        "added",
        "origin",
        "retrigger",
        "priority",
        "first_sentence",
        "body_chars",
    ];
    for r in &rows {
        let obj = r.as_object().unwrap();
        assert_eq!(obj.len(), 10, "{r}");
        for k in keys {
            assert!(obj.contains_key(k), "{k} missing from {r}");
        }
        assert_eq!(
            r["path"],
            format!("docs/ideas/{}.md", r["id"].as_str().unwrap())
        );
    }
    let o = rows.iter().find(|r| r["id"] == owed.as_str()).unwrap();
    assert_eq!(o["first_sentence"], "Owed by the bank.");
    assert_eq!(o["body_chars"], body.chars().count());
    assert_eq!(o["priority"], "high");
    assert_eq!(o["origin"], Value::Null);
    let p = rows.iter().find(|r| r["id"] == plain.as_str()).unwrap();
    assert_eq!(p["first_sentence"], "Just a line");
    assert_eq!(p["priority"], Value::Null);

    let text = fx.docs_output(&["idea", "list"]);
    let text = stdout(&text);
    assert!(
        text.contains(&format!(
            "{owed}  owed  Bank answer  [high]\n    Owed by the bank."
        )),
        "{text}"
    );
    assert!(!text.contains("README"), "{text}");
}

#[test]
fn drop_commits_one_path_with_the_reason() {
    let fx = Fixture::new("ideas-drop");
    let id = add_ok(&fx, &["--title", "Doomed"], "Will go.\n");
    let edited = add_ok(&fx, &["--title", "Edited"], "Stays.\n");
    fx.git(&["add", "docs/ideas"]);
    fx.git(&["commit", "-q", "-m", "docs: two ideas"]);
    fs::write(fx.path().join("a.txt"), "staged\n").unwrap();
    fx.git(&["add", "a.txt"]);
    let index = fx.path().join("docs/index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    text.push_str("\nunstaged edit\n");
    fs::write(&index, text).unwrap();

    let o = fx.docs_output(&["idea", "drop", &id, "--because", "x marks it"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let show = fx.git(&["show", "--name-status", "--format=%B", "HEAD"]);
    assert!(show.contains(&format!("docs: drop idea {id}")), "{show}");
    assert!(show.contains("x marks it"), "{show}");
    let paths: Vec<&str> = show.lines().filter(|l| l.contains('\t')).collect();
    assert_eq!(paths, [format!("D\tdocs/ideas/{id}.md")], "{show}");
    assert_eq!(fx.git(&["diff", "--cached", "--name-only"]).trim(), "a.txt");
    assert_eq!(fx.git(&["diff", "--name-only"]).trim(), "docs/index.md");

    let nope = fx.docs_output(&["idea", "drop", "nope", "--because", "x"]);
    assert_eq!(nope.status.code(), Some(2));

    let p = fx.path().join(format!("docs/ideas/{edited}.md"));
    let mut t = fs::read_to_string(&p).unwrap();
    t.push_str("one more line\n");
    fs::write(&p, t).unwrap();
    let o = fx.docs_output(&["idea", "drop", &edited, "--because", "x"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(p.is_file());

    let fresh = add_ok(&fx, &["--title", "Fresh"], "New.\n");
    let o = fx.docs_output(&["idea", "drop", &format!("{fresh}.md"), "--because", "x"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("not committed yet"), "{}", stderr(&o));
    assert!(fx.path().join(format!("docs/ideas/{fresh}.md")).is_file());
}

#[test]
fn new_from_idea_carries_the_body_and_deletes_the_file() {
    let fx = Fixture::new("ideas-from");
    let body = "Why it matters. Recon below.\n\n- `src/x.rs:12` does it twice\n";
    let id = add_ok(&fx, &["--title", "From an idea"], body);
    fx.docs(&["new", "demo", "--title", "Demo", "--from-idea", &id]);
    let prd = fs::read_to_string(fx.path().join("docs/features/002-demo/prd.md")).unwrap();
    let fm = frontmatter::parse(&prd);
    let keys: Vec<&str> = fm
        .data
        .as_ref()
        .unwrap()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "number",
            "slug",
            "title",
            "type",
            "created",
            "author",
            "summary",
            "from_idea"
        ]
    );
    assert_eq!(fm.str("from_idea"), Some(id.as_str()));
    assert_eq!(fm.str("title"), Some("Demo"));
    assert!(
        prd.contains(&format!(
            "# PRD-002 — Demo\n\n## From idea {id}\n\n{body}\n## 1."
        )),
        "{prd}"
    );
    assert!(!fx.path().join(format!("docs/ideas/{id}.md")).exists());
    let check = fx.docs_output(&["check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));

    let id2 = add_ok(&fx, &["--title", "Untitled no more"], "b\n");
    fx.docs(&["new", "demo2", "--from-idea", &id2]);
    let prd = fs::read_to_string(fx.path().join("docs/features/003-demo2/prd.md")).unwrap();
    assert_eq!(
        frontmatter::parse(&prd).str("title"),
        Some("Untitled no more")
    );

    let reservation = fx.path().join(".git/docs-next-number");
    let before = fs::read_to_string(&reservation).unwrap();
    let o = fx.docs_output(&["new", "demo3", "--from-idea", "nope"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(!fx.path().join("docs/features/004-demo3").exists());
    assert_eq!(fs::read_to_string(&reservation).unwrap(), before);
}

#[test]
fn init_writes_the_readme_once() {
    let dir = tempfile::tempdir().unwrap();
    let docs = |args: &[&str]| -> Output {
        Command::new(env!("CARGO_BIN_EXE_factory"))
            .arg("docs")
            .args(args)
            .current_dir(dir.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("run factory docs")
    };
    let git = Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(git.success());
    assert!(docs(&["init"]).status.success());
    let readme = dir.path().join("docs/ideas/README.md");
    assert_eq!(fs::read_to_string(&readme).unwrap(), README);
    fs::write(&readme, "my own rules\n").unwrap();
    assert!(docs(&["init"]).status.success());
    assert_eq!(fs::read_to_string(&readme).unwrap(), "my own rules\n");
    let o = docs(&["idea", "list", "--json"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "[]");
}

#[test]
fn check_validates_every_idea_and_warns_on_the_v2_inbox() {
    let fx = Fixture::new("ideas-check");
    add_ok(&fx, &["--title", "Twice"], "a\n");
    add_ok(&fx, &["--title", "Twice"], "b\n");
    let clean = fx.docs_output(&["check"]);
    assert_eq!(clean.status.code(), Some(0), "{}", stderr(&clean));

    let later = fx.path().join("docs/ideas/2026-10-13-later.md");
    fs::write(
        &later,
        "---\ntitle: \"Later\"\nstate: later\nadded: 2026-10-13\n---\n\nx\n",
    )
    .unwrap();
    let o = fx.docs_output(&["check"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("E-IDEA-FIELD"), "{}", stderr(&o));
    fs::remove_file(&later).unwrap();

    let notes = fx.path().join("docs/ideas/notes.md");
    fs::write(&notes, "---\ntitle: x\n---\n").unwrap();
    let o = fx.docs_output(&["check"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("E-IDEA-NAME"), "{}", stderr(&o));
    fs::remove_file(&notes).unwrap();

    fs::write(
        fx.path().join("docs/ideas.md"),
        "# Ideas\n\n- **Old** — line\n",
    )
    .unwrap();
    let o = fx.docs_output(&["check"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stderr(&o).contains("W-IDEAS-V2"), "{}", stderr(&o));
}
