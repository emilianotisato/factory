//! Regression corpus for additive import merges and grep stdout redirects.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use factory::cli::hook::hook_with;
use factory::testguard::check;
use serde_json::{Value, json};

fn git(root: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap()
            .success(),
        "git {:?} in {}",
        args,
        root.display()
    );
}

fn import_repo() -> (tempfile::TempDir, String) {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "T"]);
    git(root, &["config", "user.email", "t@example.invalid"]);
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::a;\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/sample.test.ts"),
        "import { a } from 'x';\n\ntest('x', () => {});\n",
    )
    .unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "base"]);
    let base = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    let base = String::from_utf8_lossy(&base.stdout).trim().to_string();
    (d, base)
}

fn commit_all(root: &Path, msg: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", msg]);
}

fn reset_to_base(root: &Path, base: &str) {
    git(root, &["reset", "--hard", base]);
    git(root, &["clean", "-fd"]);
}

fn assert_clean(root: &Path, base: &str, label: &str) {
    let v = check(root, base, &[]).unwrap();
    assert!(v.is_empty(), "{label}: expected no violations, got {v:?}");
}

fn assert_violation(root: &Path, base: &str, label: &str) {
    let v = check(root, base, &[]).unwrap();
    assert!(!v.is_empty(), "{label}: expected a violation, got {v:?}");
}

const RS_TAIL: &str = "#[test]\nfn existing_test() {\n    assert!(true);\n}\n";

fn commit_path(root: &Path, path: &str, msg: &str) {
    git(root, &["add", path]);
    git(root, &["commit", "-q", "-m", msg]);
}

#[test]
fn testguard_allows_additive_imports_and_denies_removals_and_in_test_edits() {
    {
        let (d, base) = import_repo();
        let root = d.path();
        fs::write(
            root.join("tests/sample.rs"),
            &("use foo::{a, b};\n\n".to_string() + RS_TAIL),
        )
        .unwrap();
        commit_path(root, "tests/sample.rs", "merge");
        assert_clean(root, &base, "rust merge");
    }
    {
        let (d, _base) = import_repo();
        let root = d.path();
        fs::write(
            root.join("tests/sample.rs"),
            &("use a::b;\n\n".to_string() + RS_TAIL),
        )
        .unwrap();
        commit_path(root, "tests/sample.rs", "before");
        let mid = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        let mid = String::from_utf8_lossy(&mid.stdout).trim().to_string();
        fs::write(
            root.join("tests/sample.rs"),
            &("use a::{b, c};\n\n".to_string() + RS_TAIL),
        )
        .unwrap();
        commit_path(root, "tests/sample.rs", "merge");
        assert_clean(root, &mid, "rust nested merge");
    }
    {
        let (d, base) = import_repo();
        let root = d.path();
        fs::write(
            root.join("tests/sample.test.ts"),
            "import { a, b } from 'x';\n\ntest('x', () => {});\n",
        )
        .unwrap();
        commit_path(root, "tests/sample.test.ts", "merge");
        assert_clean(root, &base, "js merge");
    }
    {
        let (d, _base) = import_repo();
        let root = d.path();
        fs::write(
            root.join("tests/sample.test.ts"),
            "import type { A } from 'x';\n\ntest('x', () => {});\n",
        )
        .unwrap();
        commit_path(root, "tests/sample.test.ts", "before");
        let mid = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        let mid = String::from_utf8_lossy(&mid.stdout).trim().to_string();
        fs::write(
            root.join("tests/sample.test.ts"),
            "import type { A, B } from 'x';\n\ntest('x', () => {});\n",
        )
        .unwrap();
        commit_path(root, "tests/sample.test.ts", "merge");
        assert_clean(root, &mid, "js type merge");
    }

    let (d, base) = import_repo();
    let root = d.path();
    fs::write(
        root.join("tests/sample.rs"),
        "\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "delete use");
    assert_violation(root, &base, "delete use");

    reset_to_base(root, &base);
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::{a, b};\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "add b");
    let subset_mid = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    let subset_mid = String::from_utf8_lossy(&subset_mid.stdout)
        .trim()
        .to_string();
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::a;\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "subset remove");
    assert_violation(root, &subset_mid, "subset remove");

    reset_to_base(root, &base);
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::a as z;\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "alias");
    assert_violation(root, &base, "rust alias");

    reset_to_base(root, &base);
    fs::write(
        root.join("tests/sample.test.ts"),
        "import { a as z } from 'x';\n\ntest('x', () => {});\n",
    )
    .unwrap();
    commit_all(root, "js alias");
    assert_violation(root, &base, "js alias");

    reset_to_base(root, &base);
    fs::write(
        root.join("tests/sample.test.ts"),
        "import { a } from 'y';\n\ntest('x', () => {});\n",
    )
    .unwrap();
    commit_all(root, "source");
    assert_violation(root, &base, "js source");

    reset_to_base(root, &base);
    fs::write(
        root.join("tests/sample.test.ts"),
        "import type { A } from 'x';\n\ntest('x', () => {});\n",
    )
    .unwrap();
    commit_all(root, "type only");
    assert_violation(root, &base, "import to type");

    reset_to_base(root, &base);
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::a;\n\n#[test]\nfn existing_test() {\n    use bar::x;\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "in-test base");
    let in_test_mid = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    let in_test_mid = String::from_utf8_lossy(&in_test_mid.stdout)
        .trim()
        .to_string();
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::a;\n\n#[test]\nfn existing_test() {\n    use bar::{x, y};\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "in-test merge");
    assert_violation(root, &in_test_mid, "in-test use");

    reset_to_base(root, &base);
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::a;\nconst X: i32 = 1;\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "const base");
    fs::write(
        root.join("tests/sample.rs"),
        "use foo::{a, b};\nconst X: i32 = 2;\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    commit_all(root, "const plus import");
    assert_violation(root, &base, "const plus import");
}

fn grep_repo() -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.name", "T"]);
    git(&root, &["config", "user.email", "t@example.invalid"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("src/lib.rs"), "fn foo() {}\n").unwrap();
    fs::write(root.join("sub/lib.rs"), "fn foo() {}\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "init"]);
    (d, root)
}

fn hook_allows(cmd: &str, repo: &Path, cwd: &Path) {
    let input = json!({"tool_name": "Bash", "tool_input": {"command": cmd}, "cwd": cwd});
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = hook_with(
        &mut input.to_string().as_bytes(),
        None,
        Some(repo),
        &mut out,
        &mut err,
    )
    .unwrap();
    assert_eq!(code, 0, "{cmd}: stderr={}", String::from_utf8_lossy(&err));
    assert!(out.is_empty(), "{cmd}: {:?}", String::from_utf8(out));
}

fn hook_denies(cmd: &str, repo: &Path, cwd: &Path) {
    let input = json!({"tool_name": "Bash", "tool_input": {"command": cmd}, "cwd": cwd});
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = hook_with(
        &mut input.to_string().as_bytes(),
        None,
        Some(repo),
        &mut out,
        &mut err,
    )
    .unwrap();
    assert_eq!(code, 0, "{cmd}");
    assert!(!out.is_empty(), "{cmd} should be denied");
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v["hookSpecificOutput"]["permissionDecision"].as_str(),
        Some("deny"),
        "{cmd}"
    );
}

#[test]
fn grep_hook_allows_output_redirects_and_keeps_standalone_denied() {
    let (_d, root) = grep_repo();
    for cmd in [
        "grep -rn foo src > /tmp/out",
        "grep -rn foo src 1>> /tmp/out",
        "grep -rn foo src &> /tmp/out",
        "grep -rn foo src >& /tmp/out",
        "grep -rn foo src > /tmp/out && wc -l /tmp/out",
    ] {
        hook_allows(cmd, &root, &root);
    }
    for cmd in [
        "grep -rn foo src",
        "grep -rn foo src 2>/dev/null",
        "grep -rn foo src >&2",
        "rg foo src",
        "cd sub && grep -r foo .",
        "grep -rn foo src;",
    ] {
        hook_denies(cmd, &root, &root);
    }
}
