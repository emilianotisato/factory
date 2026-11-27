#[path = "harness/mod.rs"]
mod harness;

use std::fs;

use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn write(fx: &Fixture, rel: &str, text: &str) {
    let p = fx.path().join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn commit(fx: &Fixture, msg: &str) {
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", msg]);
}

fn docs(fx: &Fixture, args: &[&str]) -> (Option<i32>, String, String) {
    let out = fx.command(BIN).arg("docs").args(args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ctx(fx: &Fixture, args: &[&str]) -> (Option<i32>, String) {
    let out = fx.command(BIN).arg("ctx").args(args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn leaks(fx: &Fixture, args: &[&str]) -> (Option<i32>, String) {
    let out = fx.command(BIN).arg("leaks").args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr);
    (out.status.code(), format!("{text}{err}"))
}

#[test]
fn docs_check_can_scope_to_one_feature() {
    let fx = Fixture::new("docs-check-scoped");
    let root = fx.path();
    let second = root.join("docs/features/002-second");
    scaffold_second_feature(&fx, &second);
    fs::write(second.join("state.toml"), "status = \n").unwrap();
    drop_summary(&second);
    write(&fx, "docs/ideas/bad name.md", "# bad\n");

    let (code, out, err) = docs(&fx, &["check", "001"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert_eq!(out.trim(), "docs check: clean");

    let (code, _, err) = docs(&fx, &["check"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("E-STATE-INVALID"), "{err}");
    assert!(err.contains("E-IDEA-NAME"), "{err}");
}

fn scaffold_second_feature(fx: &Fixture, second: &std::path::Path) {
    fs::create_dir_all(second).unwrap();
    fs::write(
        second.join("prd.md"),
        "---\nnumber: 2\nslug: second\ntitle: Second\ntype: feature\ncreated: 2026-01-01\nauthor: t\nsummary: |\n  x\n---\n",
    )
    .unwrap();
    fs::write(second.join("state.toml"), "status = \"ready\"\n").unwrap();
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "second feature"]);
}

fn drop_summary(dir: &std::path::Path) {
    let p = dir.join("prd.md");
    let text = fs::read_to_string(&p).unwrap();
    let mut out = Vec::new();
    let mut skipping = false;
    for l in text.lines() {
        if l == "summary: |" {
            skipping = true;
            continue;
        }
        if skipping && l.starts_with("  ") {
            continue;
        }
        skipping = false;
        out.push(l);
    }
    fs::write(p, out.join("\n") + "\n").unwrap();
}

#[test]
fn ctx_def_skips_generated_unless_vendor() {
    let fx = Fixture::new("ctx-def-generated");
    write(
        &fx,
        ".gitattributes",
        "src/hidden_gen.rs linguist-generated\n",
    );
    write(&fx, "src/live.rs", "pub fn scoped_def_sym() {}\n");
    write(&fx, "src/hidden_gen.rs", "pub fn scoped_def_sym() {}\n");
    commit(&fx, "sources");

    let (_, out) = ctx(&fx, &["def", "scoped_def_sym"]);
    assert!(out.contains("src/live.rs"), "{out}");
    assert!(!out.contains("hidden_gen.rs"), "{out}");

    let (_, out) = ctx(&fx, &["def", "--vendor", "scoped_def_sym"]);
    assert!(out.contains("hidden_gen.rs"), "{out}");
}

#[test]
fn leaks_ignores_local_attributes_file_not_committed_gitattributes() {
    let fx = Fixture::new("leaks-attributes-file");
    write(&fx, ".gitattributes", "src/gen.rs linguist-generated\n");
    write(&fx, "src/gen.rs", "// PRD-2: on main attrs only\n");
    commit(&fx, "main: attributes");

    let attrs = fx.path().join("local-attrs");
    fs::write(&attrs, "src/leak.rs linguist-generated\n").unwrap();
    fx.git(&["config", "core.attributesFile", attrs.to_str().unwrap()]);

    fx.git(&["checkout", "-q", "-b", "feat/leak"]);
    write(&fx, "src/leak.rs", "// PRD-1: must stay visible\n");
    commit(&fx, "branch leak");

    write(
        &fx,
        "src/gen.rs",
        "// PRD-2: on main attrs only\n// PRD-99: branch edit stays generated\n",
    );
    commit(&fx, "touch generated");

    let (code, out) = leaks(&fx, &["--base", "main"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(out.contains("src/leak.rs"), "{out}");
    assert!(out.contains("src/gen.rs (generated)"), "{out}");
    assert!(!out.contains("PRD-99"), "{out}");

    let (code, out) = leaks(&fx, &["--all"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(out.contains("src/leak.rs"), "{out}");
}
