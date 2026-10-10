//! `factory ctx def|refs|outline` (PRD §14 FR-12, slice 11): the textual
//! engine on the fixture files under `tests/fixture/ctx/`, one directory per
//! stack plus `refs/`, whose symbol `planted` has 300 hits in 40 files (two
//! of them inside a comment and a string, which slice 12 drops) and one
//! more in a test file. Each command's full output is pinned in
//! `tests/snapshots/ctx/`; `FACTORY_BLESS=1 cargo test --test ctx` rewrites
//! the snapshots after a deliberate change.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use factory::ctx::walk::{Lang, WalkOpts, walk};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture/ctx")
}

/// Runs `factory ctx <args>` in `dir`: (exit code, stdout, stderr).
fn ctx_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .arg("ctx")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn ctx(args: &[&str]) -> String {
    let (code, out, err) = ctx_in(&fixture(), args);
    assert_eq!(code, 0, "{args:?}: {err}");
    out
}

/// The `path:line` hit lines of an output.
fn hits(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|l| !l.starts_with(' ') && l.contains(":") && l.contains("  "))
        .collect()
}

const SNAPSHOTS: [(&str, &[&str]); 12] = [
    ("def-UserService", &["def", "UserService"]),
    (
        "def-UserService-no-body",
        &["def", "UserService", "--no-body"],
    ),
    ("def-handle", &["def", "handle"]),
    ("def-OtherService", &["def", "OtherService"]),
    ("def-NewClient", &["def", "NewClient"]),
    ("def-Config", &["def", "Config"]),
    ("def-made_by_macro", &["def", "made_by_macro"]),
    ("def-useThing", &["def", "useThing"]),
    ("refs-planted", &["refs", "planted"]),
    (
        "refs-planted-max-20-per-file-1",
        &["refs", "planted", "--max", "20", "--per-file", "1"],
    ),
    (
        "refs-planted-in-sub",
        &["refs", "planted", "--in", "refs/sub"],
    ),
    (
        "refs-planted-in-tests-with-tests",
        &["refs", "planted", "--in", "refs/tests", "--tests"],
    ),
];

#[test]
fn each_command_prints_its_snapshot_byte_for_byte_twice() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/ctx");
    let bless = std::env::var_os("FACTORY_BLESS").is_some();
    let mut wrong = Vec::new();
    for (name, args) in SNAPSHOTS {
        let first = ctx(args);
        assert_eq!(first, ctx(args), "{name}: two runs differ");
        let path = dir.join(format!("{name}.txt"));
        if bless {
            fs::create_dir_all(&dir).unwrap();
            fs::write(&path, &first).unwrap();
        } else if fs::read_to_string(&path).ok().as_deref() != Some(first.as_str()) {
            wrong.push(format!("--- {name} ({args:?}) printed:\n{first}"));
        }
    }
    assert!(wrong.is_empty(), "snapshots differ:\n{}", wrong.join("\n"));
}

#[test]
fn def_prints_location_kind_signature_and_body_on_every_stack() {
    let out = ctx(&["def", "UserService"]);
    assert!(out.starts_with("php/UserService.php:11  class  final class UserService\n    {\n"));
    assert!(
        out.ends_with("    }\n\n1 definition (searched 48 files)\n"),
        "{out}"
    );

    let out = ctx(&["def", "UserService", "--no-body"]);
    assert_eq!(
        out,
        "php/UserService.php:11  class  final class UserService\n\n1 definition (searched 48 files)\n"
    );

    let out = ctx(&["def", "handle"]);
    assert_eq!(
        hits(&out),
        ["php/UserService.php:19  function  public function handle(array $input): User"]
    );

    let out = ctx(&["def", "NewClient"]);
    assert_eq!(
        hits(&out),
        ["go/client.go:12  func  func NewClient(base string) *Client {"]
    );

    let out = ctx(&["def", "Config"]);
    assert_eq!(hits(&out), ["rs/config.rs:4  struct  pub struct Config {"]);

    let out = ctx(&["def", "useThing"]);
    assert_eq!(
        hits(&out),
        ["vue/UserCard.vue:10  const  const useThing = (prefix: string) => {"]
    );
}

#[test]
fn def_of_a_macro_made_function_finds_nothing_and_exits_0() {
    let (code, out, err) = ctx_in(&fixture(), &["def", "made_by_macro"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "no definition of made_by_macro found (searched 48 files)\n"
    );
}

/// Slice 11 state: the textual engine still prints the docblock line that
/// reads like a definition.
#[test]
fn def_still_prints_the_docblock_mention() {
    let out = ctx(&["def", "OtherService"]);
    assert_eq!(
        hits(&out),
        [
            "php/OtherService.php:7  class  class OtherService implements Notifier",
            "php/UserService.php:9  class  class OtherService, which this one wraps.",
        ]
    );
}

/// Slice 12 state (PRD §14 acceptance: "drops the docblock mention").
#[test]
#[ignore = "slice 12: tree-sitter drops hits inside comments"]
fn def_drops_the_docblock_mention() {
    let out = ctx(&["def", "OtherService"]);
    assert!(!out.contains("UserService.php"), "{out}");
}

#[test]
fn refs_caps_at_10_with_a_trailer() {
    let out = ctx(&["refs", "planted"]);
    assert_eq!(hits(&out).len(), 10, "{out}");
    let trailer = out.lines().last().unwrap();
    assert!(
        trailer.starts_with("290 more hits in 40 files (--max, --per-file to widen)"),
        "{trailer}"
    );

    let out = ctx(&["refs", "planted", "--max", "20", "--per-file", "1"]);
    let lines = hits(&out);
    let files: BTreeSet<&str> = lines.iter().map(|l| l.split(':').next().unwrap()).collect();
    assert_eq!((lines.len(), files.len()), (20, 20), "{out}");

    let out = ctx(&["refs", "planted", "--in", "refs/sub", "--max", "100"]);
    let lines = hits(&out);
    assert!(
        lines.len() > 3 && lines.iter().all(|l| l.starts_with("refs/sub/")),
        "{out}"
    );
}

#[test]
fn a_hit_in_a_test_file_shows_only_with_tests() {
    let test_hit = "refs/tests/planted.test.ts:1  ref  import { planted } from '../caller01'";
    let all = ["refs", "planted", "--max", "1000", "--per-file", "1000"];
    let out = ctx(&all);
    assert!(!out.contains(test_hit));
    assert!(
        out.ends_with("300 hits in 40 files (searched 48 files); 1 in test files (--tests)\n"),
        "{out}"
    );
    let out = ctx(&[&all[..], &["--tests"]].concat());
    assert!(out.contains(test_hit), "{out}");
    assert!(
        out.ends_with("301 hits in 41 files (searched 48 files)\n"),
        "{out}"
    );
}

/// Slice 11 state: the comment and string hits are printed.
#[test]
fn refs_still_prints_the_comment_and_string_hits() {
    let out = ctx(&["refs", "planted"]);
    assert!(
        out.starts_with("refs/caller01.ts:1  ref  // planted is the shared helper"),
        "{out}"
    );
    assert!(
        out.contains("refs/caller01.ts:2  ref  const label = 'planted';"),
        "{out}"
    );
}

/// Slice 12 state (slice 12 task 3).
#[test]
#[ignore = "slice 12: tree-sitter drops hits inside comments and strings"]
fn refs_drops_the_comment_and_string_hits() {
    let out = ctx(&["refs", "planted"]);
    assert!(
        !out.contains("refs/caller01.ts:1 ") && !out.contains("refs/caller01.ts:2 "),
        "{out}"
    );
    assert!(out.contains("2 dropped (comments/strings)"), "{out}");
}

#[test]
fn outline_says_it_needs_tree_sitter_and_exits_2() {
    let (code, out, err) = ctx_in(&fixture(), &["outline", "php/UserService.php"]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (2, "", "outline needs tree-sitter (slice 12)\n")
    );
}

#[test]
fn a_bad_flag_or_an_unreadable_root_exits_2() {
    let (code, _, err) = ctx_in(&fixture(), &["refs", "planted", "--in", "nowhere"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("factory ctx: cannot read nowhere: "),
        "{err}"
    );
    let (code, _, _) = ctx_in(&fixture(), &["refs", "planted", "--max", "ten"]);
    assert_eq!(code, 2);
    let (code, _, _) = ctx_in(&fixture(), &["def", "Config", "--kind", "struct"]);
    assert_eq!(code, 2);
    let (code, out, _) = ctx_in(&fixture(), &["def", "Config", "--kind", "class"]);
    assert_eq!(code, 0, "--kind is accepted until slice 12 applies it");
    assert!(out.contains("rs/config.rs:4  struct"), "{out}");
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &target);
        } else {
            fs::copy(e.path(), &target).unwrap();
        }
    }
}

fn source_files(dir: &Path, root: &Path, into: &mut BTreeSet<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            source_files(&p, root, into);
        } else if Lang::from_path(&p).is_some() {
            into.insert(p.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

/// Task 1: the walker skips vendor directories and gitignored files unless
/// `--vendor`; the trailer counts what it skipped for size or content.
#[test]
fn the_walk_skips_vendor_and_gitignored_paths_unless_vendor() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    copy_tree(&fixture(), root);
    let mut expected = BTreeSet::new();
    source_files(root, root, &mut expected);
    assert_eq!(expected.len(), 48);

    let git = Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(git.success());
    let planted = [
        "vendor/laravel/Kernel.php",
        "node_modules/vue/index.js",
        "php/build/Gen.php",
    ];
    for p in planted {
        fs::create_dir_all(root.join(p).parent().unwrap()).unwrap();
        fs::write(root.join(p), "<?php class Planted {}\n").unwrap();
    }
    fs::write(root.join(".gitignore"), "ignored.php\n").unwrap();
    fs::write(root.join("php/ignored.php"), "<?php class Ignored {}\n").unwrap();

    let names = |vendor: bool| -> BTreeSet<PathBuf> {
        walk(root, &WalkOpts { vendor })
            .files
            .into_iter()
            .map(|(p, _)| p.strip_prefix(root).unwrap().to_path_buf())
            .collect()
    };
    assert_eq!(names(false), expected);

    let mut with_vendor = expected.clone();
    with_vendor.extend(planted.iter().map(PathBuf::from));
    with_vendor.insert(PathBuf::from("php/ignored.php"));
    assert_eq!(names(true), with_vendor);

    fs::write(root.join("php/Blob.php"), b"<?php\0class Blob {}\n").unwrap();
    fs::write(root.join("rs/huge.rs"), vec![b'\n'; 2 * 1024 * 1024 + 1]).unwrap();
    let (code, out, _) = ctx_in(root, &["def", "Blob"]);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "no definition of Blob found (searched 48 files); skipped 1 binary file, 1 file over 2 MB\n"
    );
}
