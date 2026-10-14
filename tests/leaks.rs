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

fn leaks(fx: &Fixture, args: &[&str]) -> (Option<i32>, String) {
    let out = fx.command(BIN).arg("leaks").args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr);
    (out.status.code(), format!("{text}{err}"))
}

/// The heading of a rule, its fix line and its hits, up to the blank line.
fn section<'a>(out: &'a str, rule: &str) -> &'a str {
    let start = out
        .find(&format!("\n{rule}: "))
        .map(|i| i + 1)
        .or_else(|| out.starts_with(&format!("{rule}: ")).then_some(0))
        .unwrap_or_else(|| panic!("no {rule} heading in\n{out}"));
    let body = &out[start..];
    let end = body.find("\n\n").unwrap_or(body.len());
    let block = &body[..end];
    let second = block.lines().nth(1).unwrap_or("");
    assert!(
        second.starts_with("  fix: "),
        "{rule} has no fix line:\n{block}"
    );
    block
}

#[test]
fn base_reads_what_the_branch_added_and_all_reads_the_tree() {
    let fx = Fixture::new("leaks-input");
    write(&fx, "src/a.rs", "pub fn a() {}\n\n");
    write(&fx, "src/before.rs", "// FR-1: on main before the branch\n");
    write(&fx, "tests/data/x.jsonl", "{\"a\": 1}\n");
    commit(&fx, "main: base");

    fx.git(&["checkout", "-q", "-b", "feat/x"]);
    write(&fx, "src/a.rs", "pub fn a() {}\n\n// FR-2: on the branch\n");
    fs::create_dir_all(fx.path().join("tests/fixture/prd-9")).unwrap();
    fx.git(&["mv", "tests/data/x.jsonl", "tests/fixture/prd-9/x.jsonl"]);
    commit(&fx, "branch work");

    fx.git(&["checkout", "-q", "main"]);
    write(&fx, "src/after.rs", "// FR-4: on main after the branch\n");
    commit(&fx, "main: moves on");
    fx.git(&["checkout", "-q", "feat/x"]);

    let (code, out) = leaks(&fx, &["--base", "main"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(
        section(&out, "id-comment").contains("  src/a.rs:3  // FR-2: on the branch"),
        "{out}"
    );
    assert!(
        section(&out, "id-name").contains("  tests/fixture/prd-9/x.jsonl"),
        "{out}"
    );
    assert!(!out.contains("FR-1") && !out.contains("FR-4"), "{out}");
    assert!(out.ends_with("2 fix hits\n"), "{out}");

    let (code, out) = leaks(&fx, &["--all"]);
    assert_eq!(code, Some(1), "{out}");
    let ids = section(&out, "id-comment");
    assert!(ids.contains("src/before.rs:1  // FR-1"), "{out}");
    assert!(ids.contains("src/a.rs:3  // FR-2"), "{out}");
    assert!(!out.contains("FR-4"), "{out}");
}

#[test]
fn the_verb_takes_exactly_one_of_base_and_all() {
    let fx = Fixture::new("leaks-flags");
    assert_eq!(leaks(&fx, &[]).0, Some(2));
    assert_eq!(leaks(&fx, &["--base", "main", "--all"]).0, Some(2));
    let (code, out) = leaks(&fx, &["--base", "no-such-ref"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("factory: git") && out.contains("no-such-ref"),
        "{out}"
    );
}

const WHY: &str = "// The lock stays held across both writes, or a reader sees half a record.\n";
const QUIRK: &str =
    "// git prints rename paths from the repo root, even when run in a subdirectory.\n";

#[test]
fn a_seeded_branch_lists_each_leak_under_its_rule_and_skips_the_rest() {
    let fx = Fixture::new("leaks-seeded");
    let project = fs::read_to_string(fx.path().join("docs/project.toml")).unwrap();
    write(
        &fx,
        "docs/project.toml",
        &format!("{project}\n[leaks]\nallow = [{{ path = \"src/ids.rs\" }}]\n"),
    );
    commit(&fx, "chore: allow the id table");

    fx.git(&["checkout", "-q", "-b", "feat/seeded"]);
    write(
        &fx,
        "src/guard.rs",
        &format!(
            "// FR-3: guard the lane before the merge\n// until slice 4 this runs twice\npub fn guard() {{}}\n{WHY}{QUIRK}"
        ),
    );
    write(
        &fx,
        "tests/guard.rs",
        "#[test]\nfn fr_12_guards_the_lane() {}\n",
    );
    write(&fx, "tests/fixture/prd-9/case.jsonl", "{\"a\": 1}\n");
    write(&fx, "src/ids.rs", "pub const FIRST: &str = \"BLOCK-1\";\n");
    write(&fx, "README.md", "FR-3 lands here, until slice 4.\n");
    write(&fx, "docs/snippet.rs", "// FR-3: in the docs\n");
    commit(&fx, "seeded leaks");

    let (code, out) = leaks(&fx, &["--base", "main"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(
        section(&out, "id-comment").contains("  src/guard.rs:1  // FR-3: guard the lane"),
        "{out}"
    );
    assert!(
        section(&out, "history").contains("  src/guard.rs:2  // until slice 4 this runs twice"),
        "{out}"
    );
    let names = section(&out, "id-name");
    assert!(
        names.contains("  tests/guard.rs:2  fn fr_12_guards_the_lane() {}"),
        "{out}"
    );
    assert!(
        names
            .lines()
            .any(|l| l == "  tests/fixture/prd-9/case.jsonl"),
        "{out}"
    );
    for absent in [
        "half a record",
        "subdirectory",
        "BLOCK-1",
        "README",
        "docs/",
    ] {
        assert!(!out.contains(absent), "{absent} in\n{out}");
    }
    assert!(out.contains("\ncomment share: "), "{out}");

    fx.git(&["checkout", "-q", "main"]);
    fx.git(&["checkout", "-q", "-b", "feat/clean"]);
    write(
        &fx,
        "src/guard.rs",
        &format!("pub fn guard() {{}}\n{WHY}{QUIRK}"),
    );
    commit(&fx, "only the why and the quirk");
    let (code, out) = leaks(&fx, &["--base", "main"]);
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.ends_with("comment share: 66.7% (2 of 3 added lines)\nclean\n"),
        "{out}"
    );
}

#[test]
fn doctor_and_preflight_print_the_leaks_line_and_never_block_on_it() {
    let fx = Fixture::new("leaks-doctor");
    let doctor =
        |fx: &Fixture| String::from_utf8_lossy(&fx.docs_output(&["doctor"]).stdout).into_owned();
    let preflight = |fx: &Fixture| {
        let o = fx.docs_output(&["preflight", "001"]);
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };

    let out = doctor(&fx);
    assert!(out.contains("\nleaks          clean\n"), "{out}");
    let drift = out.find("\ndrift ").expect("a drift line");
    assert!(out.find("\nleaks ").unwrap() > drift, "{out}");
    let (clean_code, out) = preflight(&fx);
    assert!(out.contains("  ✓ leaks clean\n"), "{out}");

    fx.git(&["checkout", "-q", "-b", "feat/fixture"]);
    write(
        &fx,
        "src/guard.rs",
        "// FR-3: guard the lane\npub fn guard() {}\n// kept for now\n",
    );
    commit(&fx, "two leaks");
    let line = "2 hits — run factory leaks --all and clean before the first run";
    let out = doctor(&fx);
    assert!(out.contains(&format!("\nleaks          {line}\n")), "{out}");
    let (code, out) = preflight(&fx);
    assert!(out.contains(&format!("  ? leaks {line}\n")), "{out}");
    assert_eq!(code, clean_code, "{out}");
}
