#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::process::Output;

use factory::docs::{Preflight, Status};
use harness::{FEATURE_DIR, Fixture};
use serde_json::Value;

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn exit(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn commit(fx: &Fixture) {
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "test: change"]);
}

fn edit_project(fx: &Fixture, from: &str, to: &str) {
    let p = fx.path().join("docs/project.toml");
    let text = fs::read_to_string(&p).unwrap();
    assert!(text.contains(from), "{text}");
    let to = if to.is_empty() {
        String::new()
    } else {
        format!("{to}\n")
    };
    fs::write(&p, text.replace(&format!("{from}\n"), &to)).unwrap();
    commit(fx);
}

fn preflight(fx: &Fixture, feature: &str) -> (Preflight, i32) {
    let o = fx.docs_output(&["preflight", feature, "--json"]);
    let pf: Preflight = serde_json::from_str(&stdout(&o))
        .unwrap_or_else(|e| panic!("{e}\nstdout:\n{}\nstderr:\n{}", stdout(&o), stderr(&o)));
    (pf, exit(&o))
}

// ── v2 equivalence ───────────────────────────────────────────────────────────

#[test]
fn the_v3_board_and_preflight_match_the_v2_fixture() {
    let fx = Fixture::new("v3-equivalence");

    let s: Status = serde_json::from_value(fx.docs_json("status")).unwrap();
    assert_eq!((s.number, s.slug.as_str()), (1, "fixture"));
    assert_eq!(s.slices.len(), 3);
    assert_eq!(s.ready.len(), 2);
    assert_eq!(s.spawnable, 2);
    assert_eq!(s.lanes, 2);
    assert_eq!(s.free_lanes, vec![1, 2]);
    assert_eq!(s.slices[2].depends_on, vec![2]);
    assert_eq!(s.slices[0].status.as_deref(), Some("pending"));
    assert_eq!(s.next.as_ref().map(|n| n.number), Some(1));
    assert_eq!(s.gate.declared.as_deref(), Some("./gate.sh"));
    assert_eq!(s.gate.status, "ok");
    assert!(
        s.gate.candidates.is_null(),
        "the board computes no candidates"
    );
    assert!(!s.checkpoint_due && !s.all_done && s.sliced && s.qa_plan);
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.log.decisions, 0);

    let (p, code) = preflight(&fx, "001");
    assert_eq!(code, 0, "{p:?}");
    assert!(p.pass && p.refuse.is_empty() && p.ask.is_empty(), "{p:?}");
    assert!(!p.judge.is_empty(), "{p:?}");
    assert_eq!(p.branch.current.as_deref(), Some("main"));
    assert_eq!(p.branch.default.as_deref(), Some("main"));
    assert_eq!(p.branch.feature.as_deref(), Some("feat/fixture"));
    assert_eq!(p.branch.alt.as_deref(), Some("feat/001-fixture"));
    assert!(!p.branch.feature_exists);
    assert_eq!(p.feature.as_ref().map(|f| f.number), Some(1));

    assert_eq!(fx.docs(&["gate"]).trim(), "./gate.sh");
    assert_eq!(fx.docs(&["gate", "--inner"]).trim(), "./gate.sh --inner");

    fs::write(fx.path().join("dirty.txt"), "x\n").unwrap();
    let (p, code) = preflight(&fx, "001");
    assert_eq!(code, 1);
    assert!(!p.pass);
    assert!(
        p.refuse
            .iter()
            .any(|r| r.contains("working tree not clean")),
        "{:?}",
        p.refuse
    );
}

// ── gate ─────────────────────────────────────────────────────────────────────

#[test]
fn gate_prints_the_bare_command_and_set_keeps_the_other_tables() {
    let fx = Fixture::new("v3-gate");
    let o = fx.docs_output(&["gate"]);
    assert_eq!((exit(&o), stdout(&o).as_str()), (0, "./gate.sh\n"));
    assert_eq!(stderr(&o), "");
    assert_eq!(fx.docs(&["gate", "--inner"]), "./gate.sh --inner\n");
    let o = fx.docs_output(&["gate", "--candidates"]);
    assert_eq!(exit(&o), 0, "{}", stderr(&o));
    assert!(stdout(&o).contains("candidates:"), "{}", stdout(&o));

    let p = fx.path().join("docs/project.toml");
    let tables = "\n[run]\nbudget = 5\n\n[workers.polish]\nmodel = \"opus\"\n";
    let before = fs::read_to_string(&p).unwrap() + tables;
    fs::write(&p, &before).unwrap();
    let said = fx.docs(&["gate", "set", "make verify"]);
    assert!(said.contains("make verify"), "{said}");
    let after = fs::read_to_string(&p).unwrap();
    assert!(after.ends_with(tables), "{after}");
    assert!(after.contains("gate = \"make verify\""), "{after}");
    let o = fx.docs_output(&["gate"]);
    assert_eq!((exit(&o), stdout(&o).as_str()), (0, "make verify\n"));
    assert!(stderr(&o).contains("! no Makefile"), "{}", stderr(&o));

    fx.docs(&["gate", "set", "--inner", "make quick"]);
    assert!(
        fs::read_to_string(&p)
            .unwrap()
            .contains("inner = \"make quick\"")
    );

    edit_project(&fx, "gate = \"make verify\"", "");
    let o = fx.docs_output(&["gate"]);
    assert_eq!(exit(&o), 1);
    assert!(stdout(&o).contains("candidates:"), "{}", stdout(&o));
}

#[test]
fn gate_inner_falls_back_to_the_gate_with_the_warning_on_stderr() {
    let fx = Fixture::new("v3-gate-inner");
    edit_project(&fx, "inner = \"./gate.sh --inner\"", "");
    let o = fx.docs_output(&["gate", "--inner"]);
    assert_eq!((exit(&o), stdout(&o).as_str()), (0, "./gate.sh\n"));
    assert!(
        stderr(&o).contains("no inner loop declared"),
        "{}",
        stderr(&o)
    );
}

// ── preflight ────────────────────────────────────────────────────────────────

#[test]
fn preflight_judges_a_building_slices_dirty_tree() {
    let fx = Fixture::new("v3-pf-building");
    fx.docs(&["set", "001", "slices.1.status=building", "slices.1.lane=1"]);
    fs::write(fx.path().join("dirty.txt"), "x\n").unwrap();
    let (p, code) = preflight(&fx, "001");
    assert_eq!(code, 0, "{p:?}");
    assert!(
        p.judge.iter().any(|j| j.contains("slice 01")),
        "{:?}",
        p.judge
    );
}

#[test]
fn preflight_takes_the_lock_without_dirtying_the_tree() {
    let fx = Fixture::new("v3-pf-lock");
    let lock = factory::docs::state::lock(&fx.path().join(FEATURE_DIR)).unwrap();
    let (p, code) = preflight(&fx, "001");
    drop(lock);
    assert_eq!(code, 0, "{p:?}");
    assert!(p.ok.iter().any(|o| o == "working tree clean"), "{:?}", p.ok);
}

#[test]
fn preflight_reads_gate_parallel_and_lanes_from_the_project() {
    let fx = Fixture::new("v3-pf-project");
    edit_project(&fx, "gate_parallel = true", "gate_parallel = false");
    let (p, code) = preflight(&fx, "001");
    assert_eq!((code, p.ask.len()), (2, 1), "{p:?}");
    assert!(p.ask[0].contains("gate_parallel"), "{:?}", p.ask);

    edit_project(&fx, "gate_parallel = false", "");
    let (p, code) = preflight(&fx, "001");
    assert_eq!(code, 0, "{p:?}");
    assert!(
        p.judge.iter().any(|j| j.contains("gate_parallel")),
        "{:?}",
        p.judge
    );

    edit_project(&fx, "lanes = 2", "lanes = 1");
    let (p, code) = preflight(&fx, "001");
    assert_eq!(code, 2, "{p:?}");
    assert!(p.ask.iter().any(|a| a.contains("lanes = 1")), "{:?}", p.ask);
}

#[test]
fn preflight_refuses_reviewed_open_blocks_and_unknown_features() {
    let fx = Fixture::new("v3-pf-refuse");
    fx.docs(&["set", "001", "reviewed=2026-10-13"]);
    commit(&fx);
    let (p, code) = preflight(&fx, "001");
    assert_eq!(code, 1);
    assert!(
        p.refuse.iter().any(|r| r.contains("reviewed 2026-10-13")),
        "{:?}",
        p.refuse
    );

    fx.docs(&["set", "001", "reviewed="]);
    fx.docs(&["log", "add", "001", "--kind", "block", "--title", "stuck"]);
    commit(&fx);
    let (p, code) = preflight(&fx, "001");
    assert_eq!(code, 1);
    assert_eq!(p.refuse.len(), 1, "{:?}", p.refuse);
    assert!(p.refuse[0].contains("BLOCK-1"), "{:?}", p.refuse);

    let o = fx.docs_output(&["preflight", "999", "--json"]);
    assert_eq!(exit(&o), 1);
    let v: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert!(v["feature"].is_null(), "{v}");

    let text = fx.docs_output(&["preflight", "001"]);
    assert!(
        stdout(&text).contains("✗ open blocker(s): BLOCK-1"),
        "{}",
        stdout(&text)
    );
}

// ── ship and archive ─────────────────────────────────────────────────────────

#[test]
fn ship_refuses_open_slices_and_is_a_no_op_the_second_time() {
    let fx = Fixture::new("v3-ship");
    let state = format!("{FEATURE_DIR}/state.toml");
    let before = fx.read(&state);
    let o = fx.docs_output(&["ship", "001"]);
    assert_eq!(exit(&o), 2);
    assert!(stderr(&o).contains("01, 02, 03"), "{}", stderr(&o));
    assert_eq!(fx.read(&state), before);

    fx.docs(&[
        "set",
        "001",
        "slices.1.status=done",
        "slices.2.status=done",
        "slices.3.status=done",
    ]);
    let said = fx.docs(&["ship", "001"]);
    assert!(
        said.lines()
            .any(|l| l.starts_with("⚠") && l.contains("reviewed")),
        "{said}"
    );
    let shipped = fx.read(&state);
    assert!(shipped.contains("status = \"shipped\""), "{shipped}");
    assert!(
        shipped.lines().any(|l| l.starts_with("shipped = \"20")),
        "{shipped}"
    );
    let staged = fx.git(&["diff", "--cached", "--name-only"]);
    assert!(staged.contains(&state), "{staged}");

    let said = fx.docs(&["ship", "001"]);
    assert_eq!(said, "PRD-001 is already shipped\n");
    assert_eq!(fx.read(&state), shipped);
}

#[test]
fn ship_names_a_state_toml_that_does_not_parse() {
    let fx = Fixture::new("v3-ship-broken");
    let state = fx.path().join(FEATURE_DIR).join("state.toml");
    fs::write(&state, "status = \n").unwrap();
    let o = fx.docs_output(&["ship", "001"]);
    assert_ne!(exit(&o), 0);
    assert!(stderr(&o).contains("state.toml"), "{}", stderr(&o));
    assert!(!stderr(&o).contains("not done"), "{}", stderr(&o));
}

#[test]
fn archive_moves_the_folder_and_leaves_the_index_alone() {
    let fx = Fixture::new("v3-archive");
    let o = fx.docs_output(&["archive", "001"]);
    assert_eq!(exit(&o), 2, "{}", stderr(&o));

    fx.docs(&[
        "set",
        "001",
        "slices.1.status=done",
        "slices.2.status=done",
        "slices.3.status=done",
    ]);
    let index = fx.read("docs/index.md");
    fx.docs(&["archive", "001"]);
    assert!(!fx.path().join(FEATURE_DIR).exists());
    let moved = fx.read("docs/archive/001-fixture/state.toml");
    assert!(moved.contains("status = \"archived\""), "{moved}");
    let staged = fx.git(&["diff", "--cached", "--name-only"]);
    assert!(
        staged.contains("docs/archive/001-fixture/state.toml"),
        "{staged}"
    );
    assert_eq!(fx.read("docs/index.md"), index);

    let board: Value = serde_json::from_str(&fx.docs(&["status", "--json"])).unwrap();
    assert_eq!(board["live"].as_array().unwrap().len(), 0, "{board}");
    assert_eq!(board["archived"].as_array().unwrap().len(), 1, "{board}");
    let o = fx.docs_output(&["preflight", "001", "--json"]);
    assert_eq!(exit(&o), 1);
    assert!(stdout(&o).contains("archived"), "{}", stdout(&o));
}

// ── lanes ────────────────────────────────────────────────────────────────────

#[test]
fn lanes_set_writes_missing_files_only() {
    let fx = Fixture::new("v3-lanes");
    let p = fx.path().join("docs/project.toml");
    let tables = "\n[workers.polish]\nmodel = \"opus\"\n";
    fs::write(&p, fs::read_to_string(&p).unwrap() + tables).unwrap();

    fx.docs(&[
        "lanes",
        "set",
        "3",
        "--env",
        "DB=app_{n}",
        "--setup",
        "make db LANE={n}",
    ]);
    for n in 1..=3 {
        let text = fs::read_to_string(factory::lanes::file(fx.path(), n)).unwrap();
        // A lane is its own worktree: the lane file never describes a shared tree.
        assert!(!text.contains("does not isolate"), "{text}");
    }
    let lane = factory::lanes::load(fx.path(), 2).unwrap();
    assert_eq!(lane.env, vec![("DB".to_string(), "app_2".to_string())]);
    assert_eq!(lane.setup.as_deref(), Some("make db LANE=2"));
    let project = factory::docs::project::load(fx.path()).unwrap().unwrap();
    assert_eq!(project.lanes, 3);
    assert!(fs::read_to_string(&p).unwrap().ends_with(tables));

    let one = factory::lanes::file(fx.path(), 1);
    let edited = fs::read_to_string(&one).unwrap() + "\nOnly lane 1 owns the cache dir.\n";
    fs::write(&one, &edited).unwrap();
    let said = fx.docs(&["lanes", "set", "3", "--env", "DB=app_{n}"]);
    assert!(said.contains("3 existing lane file(s)"), "{said}");
    assert_eq!(fs::read_to_string(&one).unwrap(), edited);

    let listed = fx.docs(&["lanes"]);
    assert!(listed.contains("lane-1, lane-2, lane-3"), "{listed}");
    assert!(listed.contains("DB=app_2"), "{listed}");

    let o = fx.docs_output(&["lanes", "set", "2"]);
    assert_eq!(exit(&o), 2);
    assert!(stderr(&o).contains("--env is required"), "{}", stderr(&o));
}

// ── renumber ─────────────────────────────────────────────────────────────────

#[test]
fn renumber_rewrites_live_docs_and_leaves_the_archive_alone() {
    let fx = Fixture::new("v3-renumber");
    let other = "docs/features/002-other/prd.md";
    fs::create_dir_all(fx.path().join("docs/features/002-other")).unwrap();
    fs::write(
        fx.path().join(other),
        "---\nnumber: 002\nslug: other\n---\n# PRD-002\n\nSee [the fixture](../001-fixture/prd.md) and PRD-001.\n",
    )
    .unwrap();
    let frozen = "docs/archive/000-old/prd.md";
    fs::create_dir_all(fx.path().join("docs/archive/000-old")).unwrap();
    fs::write(fx.path().join(frozen), "# PRD-000\n\nBefore PRD-001.\n").unwrap();
    commit(&fx);
    let prd_before = fx.read(&format!("{FEATURE_DIR}/prd.md"));

    let said = fx.docs(&["renumber", "001", "005"]);
    assert!(said.contains(&format!("left alone: {frozen}")), "{said}");
    assert!(!fx.path().join(FEATURE_DIR).exists());
    let prd = fx.read("docs/features/005-fixture/prd.md");
    let fm = |t: &str| -> Vec<String> {
        t.split("\n---")
            .next()
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    };
    let (old_fm, new_fm) = (fm(&prd_before), fm(&prd));
    assert_eq!(old_fm.len(), new_fm.len());
    for (a, b) in old_fm.iter().zip(&new_fm) {
        if a.starts_with("number:") {
            assert_eq!(b, "number: 005");
        } else {
            assert_eq!(a, b);
        }
    }
    let linked = fx.read(other);
    assert!(
        linked.contains("../005-fixture/prd.md") && linked.contains("PRD-005"),
        "{linked}"
    );
    assert_eq!(fx.read(frozen), "# PRD-000\n\nBefore PRD-001.\n");

    let s: Status = serde_json::from_str(&fx.docs(&["status", "005", "--json"])).unwrap();
    assert_eq!(s.slices.len(), 3);

    let o = fx.docs_output(&["renumber", "005", "002"]);
    assert_eq!(exit(&o), 2);
    assert!(
        stderr(&o).contains("PRD-002 already exists"),
        "{}",
        stderr(&o)
    );
}
