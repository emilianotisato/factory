//! `factory ctx def|refs|outline` (PRD §14 FR-12, slices 11 and 12): the
//! textual engine and its tree-sitter confirmation on the fixture files
//! under `tests/fixture/ctx/`, one directory per stack plus `refs/`, whose
//! symbol `planted` has 300 textual hits in 40 files (two of them inside a
//! comment and a string, which the tree drops) and one more in a test
//! file. Each command's full output is pinned in `tests/snapshots/ctx/`;
//! `FACTORY_BLESS=1 cargo test --test ctx` rewrites the snapshots after a
//! deliberate change.
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

const SNAPSHOTS: [(&str, &[&str]); 20] = [
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
    (
        "def-handle-kind-method",
        &["def", "handle", "--kind", "method"],
    ),
    (
        "def-handle-kind-class",
        &["def", "handle", "--kind", "class"],
    ),
    ("def-new-kind-fn", &["def", "new", "--kind", "fn"]),
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
    ("outline-php", &["outline", "php"]),
    ("outline-php-depth-1", &["outline", "php", "--depth", "1"]),
    ("outline-go", &["outline", "go/client.go"]),
    ("outline-rs", &["outline", "rs/config.rs"]),
    ("outline-vue", &["outline", "vue/UserCard.vue"]),
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
        ["php/UserService.php:19  method  public function handle(array $input): User"]
    );

    let out = ctx(&["def", "NewClient"]);
    assert_eq!(
        hits(&out),
        ["go/client.go:12  fn  func NewClient(base string) *Client {"]
    );

    let out = ctx(&["def", "Config"]);
    assert_eq!(hits(&out), ["rs/config.rs:4  class  pub struct Config {"]);

    let out = ctx(&["def", "useThing"]);
    assert_eq!(
        hits(&out),
        ["vue/UserCard.vue:10  fn  const useThing = (prefix: string) => {"]
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

/// Slice 12 state: of the two textual hits only the class is printed; the
/// docblock line that reads like a definition is counted, not shown.
#[test]
fn def_prints_only_the_definition_node() {
    let out = ctx(&["def", "OtherService"]);
    assert_eq!(
        hits(&out),
        ["php/OtherService.php:7  class  class OtherService implements Notifier"]
    );
    assert!(
        out.ends_with(
            "1 definition (searched 48 files); 1 textual match dropped: inside comments or strings\n"
        ),
        "{out}"
    );
}

/// Slice 12 state (PRD §14 acceptance: "drops the docblock mention").
#[test]
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
        trailer.starts_with("288 more hits in 40 files (--max, --per-file to widen)"),
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
        out.ends_with(
            "298 hits in 40 files (searched 48 files); 2 dropped (comments/strings); 1 in test files (--tests)\n"
        ),
        "{out}"
    );
    let out = ctx(&[&all[..], &["--tests"]].concat());
    assert!(out.contains(test_hit), "{out}");
    assert!(
        out.ends_with("299 hits in 41 files (searched 48 files); 2 dropped (comments/strings)\n"),
        "{out}"
    );
}

/// Slice 12 state: the first hit printed is the first call, not the
/// comment or the string above it.
#[test]
fn refs_starts_at_the_first_real_hit() {
    let out = ctx(&["refs", "planted"]);
    assert!(
        out.starts_with("refs/caller01.ts:5  ref  total += planted(0)\n"),
        "{out}"
    );
}

/// Slice 12 state (slice 12 task 3).
#[test]
fn refs_drops_the_comment_and_string_hits() {
    let out = ctx(&["refs", "planted"]);
    assert!(
        !out.contains("refs/caller01.ts:1 ") && !out.contains("refs/caller01.ts:2 "),
        "{out}"
    );
    assert!(out.contains("2 dropped (comments/strings)"), "{out}");
}

/// Slice 12 task 2: `--kind` filters; `fn` takes the `impl` method, which
/// is printed as a `method`.
#[test]
fn def_kind_filters_and_fn_takes_methods() {
    let out = ctx(&["def", "handle", "--kind", "method"]);
    assert_eq!(
        hits(&out),
        ["php/UserService.php:19  method  public function handle(array $input): User"]
    );
    let out = ctx(&["def", "handle", "--kind", "class"]);
    assert_eq!(
        out,
        "no definition of handle found (searched 48 files); 1 definition of another kind (--kind)\n"
    );
    let out = ctx(&["def", "new", "--kind", "fn"]);
    assert_eq!(
        hits(&out),
        ["rs/config.rs:10  method  pub fn new(root: PathBuf) -> Self {"]
    );
}

/// Slice 12 task 2: the body is the node's exact range: the class ends at
/// its closing brace, the struct is whole, a long body is not cut and the
/// last line stops where the node does.
#[test]
fn def_body_is_the_node_range() {
    let out = ctx(&["def", "UserService"]);
    assert!(
        out.ends_with(
            "            return $user;\n        }\n    }\n\n1 definition (searched 48 files)\n"
        ),
        "{out}"
    );
    let out = ctx(&["def", "Config"]);
    assert_eq!(
        out,
        "rs/config.rs:4  class  pub struct Config {\n        pub root: PathBuf,\n        pub verbose: bool,\n    }\n\n1 definition (searched 48 files)\n"
    );

    let tmp = tempfile::tempdir().unwrap();
    let mut rs = String::from("fn long() {\n");
    for i in 0..40 {
        rs.push_str(&format!("    let x{i} = {i};\n"));
    }
    rs.push_str("} fn after() {}\n");
    fs::write(tmp.path().join("long.rs"), &rs).unwrap();
    let (_, out, _) = ctx_in(tmp.path(), &["def", "long"]);
    assert_eq!(out.lines().count(), 44, "{out}");
    assert!(
        out.ends_with("    let x39 = 39;\n    }\n\n1 definition (searched 1 file)\n"),
        "{out}"
    );
}

/// CP-6 (AD-9003): a definition whose node opens with PHP attributes or a
/// TypeScript decorator is located at its name's line, as `outline` puts
/// it, with that line as the signature; the body is the node's whole
/// range, so the attributes are still printed, in place.
#[test]
fn def_locates_an_attributed_definition_at_its_name() {
    let tmp = tempfile::tempdir().unwrap();
    let php = "<?php\n\n#[Fillable(['name'])]\nclass Company extends Model\n{\n    #[Override]\n    public function save(): bool\n    {\n        return true;\n    }\n}\n";
    fs::write(tmp.path().join("Company.php"), php).unwrap();
    let ts =
        "@Component({ name: 'x' })\nclass Widget {\n  render(): string {\n    return ''\n  }\n}\n";
    fs::write(tmp.path().join("widget.ts"), ts).unwrap();

    let (code, out, _) = ctx_in(tmp.path(), &["def", "Company"]);
    assert_eq!(
        (code, out.as_str()),
        (
            0,
            "Company.php:4  class  class Company extends Model\n    #[Fillable(['name'])]\n    class Company extends Model\n    {\n        #[Override]\n        public function save(): bool\n        {\n            return true;\n        }\n    }\n\n1 definition (searched 2 files)\n"
        )
    );
    let (_, out, _) = ctx_in(tmp.path(), &["def", "save", "--no-body"]);
    assert_eq!(
        out,
        "Company.php:7  method  public function save(): bool\n\n1 definition (searched 2 files)\n"
    );
    let (_, out, _) = ctx_in(tmp.path(), &["def", "Widget"]);
    assert!(
        out.starts_with(
            "widget.ts:2  class  class Widget {\n    @Component({ name: 'x' })\n    class Widget {\n"
        ),
        "{out}"
    );
    let (_, out, _) = ctx_in(tmp.path(), &["outline", "Company.php", "widget.ts"]);
    assert!(
        out.contains("\n4  class  class Company extends Model\n  7  method  "),
        "{out}"
    );
    assert!(out.contains("\n2  class  class Widget {\n"), "{out}");
}

/// Slice 12 task 4: classes with their methods indented, the trait and the
/// interface; no `use` lines, properties or constants; `--depth 1` keeps
/// the top level.
#[test]
fn outline_prints_definitions_and_nothing_else() {
    let out = ctx(&["outline", "php"]);
    let heads: Vec<&str> = out.lines().filter(|l| l.starts_with("== ")).collect();
    assert_eq!(
        heads,
        [
            "== php/HasTimestamps.php ==",
            "== php/Notifier.php ==",
            "== php/OtherService.php ==",
            "== php/UserService.php ==",
        ]
    );
    assert!(
        out.contains("\n11  class  final class UserService\n  15  method  "),
        "{out}"
    );
    assert!(out.contains("\n5  class  trait HasTimestamps\n"), "{out}");
    assert!(out.contains("\n7  class  interface Notifier\n"), "{out}");
    assert!(!out.contains("use ") && !out.contains("namespace"), "{out}");
    let out = ctx(&["outline", "php", "--depth", "1"]);
    assert!(!out.contains("method"), "{out}");
    assert!(
        out.ends_with("\n6 nested definitions hidden (--depth)\n"),
        "{out}"
    );

    let tmp = tempfile::tempdir().unwrap();
    let php = "<?php\nclass A\n{\n    use T;\n    const X = 1;\n    private int $n = 0;\n    public function b(): int\n    {\n        return $this->n;\n    }\n}\n";
    fs::write(tmp.path().join("A.php"), php).unwrap();
    let (code, out, _) = ctx_in(tmp.path(), &["outline", "A.php"]);
    assert_eq!(
        (code, out.as_str()),
        (
            0,
            "== A.php ==\n2  class  class A\n  7  method  public function b(): int\n"
        )
    );
}

/// C1 (AD-40001): every definition shape `outline` lists is found by `def`
/// at the same line, whatever modifiers lead it or wherever its parameters
/// end: default exports, a member whose parameters span lines, a getter, a
/// generic method, PHP readonly, abstract and final in any order, Rust
/// `const fn` and `extern "C" fn`, and a Go `type ( … )` group, whose
/// struct field of the same name is not a definition.
#[test]
fn def_finds_every_shape_outline_lists() {
    let tmp = tempfile::tempdir().unwrap();
    let files = [
        (
            "page.ts",
            "export default function Page() {\n  return 1\n}\n",
        ),
        (
            "loader.ts",
            "export default async function Loader() {\n  return 2\n}\n",
        ),
        (
            "cart.ts",
            "export class Cart {\n  async handle(\n    req: Request,\n  ): Promise<void> {\n    return\n  }\n  get total(): number {\n    return 0\n  }\n  map<T>(x: T): T {\n    return x\n  }\n}\n",
        ),
        (
            "Dto.php",
            "<?php\n\nfinal readonly class Dto\n{\n}\n\nreadonly class Dto2\n{\n}\n\nabstract class Base\n{\n    abstract protected function rules(): array;\n\n    final public function handle(): void\n    {\n    }\n\n    public final function run(): void\n    {\n    }\n}\n",
        ),
        (
            "ffi.rs",
            "pub struct Thing;\n\nimpl Thing {\n    pub const fn new() -> Self {\n        Thing\n    }\n}\n\nconst fn helper() -> u8 {\n    1\n}\n\npub extern \"C\" fn ffi() {}\n",
        ),
        (
            "group.go",
            "package shapes\n\ntype (\n\tGrouped struct {\n\t\tOther int\n\t}\n\tOther int\n)\n",
        ),
    ];
    for (name, src) in files {
        fs::write(tmp.path().join(name), src).unwrap();
    }
    type Where = (&'static str, u64, &'static str);
    let shapes: [(&str, &[Where]); 13] = [
        ("Page", &[("page.ts", 1, "fn")]),
        ("Loader", &[("loader.ts", 1, "fn")]),
        (
            "handle",
            &[("Dto.php", 15, "method"), ("cart.ts", 2, "method")],
        ),
        ("total", &[("cart.ts", 7, "method")]),
        ("map", &[("cart.ts", 10, "method")]),
        ("Dto", &[("Dto.php", 3, "class")]),
        ("Dto2", &[("Dto.php", 7, "class")]),
        ("rules", &[("Dto.php", 13, "method")]),
        ("run", &[("Dto.php", 19, "method")]),
        ("new", &[("ffi.rs", 4, "method")]),
        ("helper", &[("ffi.rs", 9, "fn")]),
        ("ffi", &[("ffi.rs", 13, "fn")]),
        ("Grouped", &[("group.go", 4, "class")]),
    ];
    let names: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    let (code, outline, _) = ctx_in(tmp.path(), &[&["outline"], &names[..]].concat());
    assert_eq!(code, 0, "{outline}");
    for (name, want) in shapes {
        let (code, out, err) = ctx_in(tmp.path(), &["def", name, "--no-body"]);
        assert_eq!((code, err.as_str()), (0, ""), "{name}");
        let got: Vec<String> = hits(&out)
            .iter()
            .map(|l| l.split("  ").take(2).collect::<Vec<_>>().join("  "))
            .collect();
        let expected: Vec<String> = want
            .iter()
            .map(|(f, l, k)| format!("{f}:{l}  {k}"))
            .collect();
        assert_eq!(got, expected, "def {name}:\n{out}");
        assert!(!out.contains("not parsed cleanly"), "{out}");
        for (file, line, kind) in want {
            let section = outline
                .split(&format!("== {file} ==\n"))
                .nth(1)
                .unwrap()
                .split("== ")
                .next()
                .unwrap();
            assert!(
                section
                    .lines()
                    .any(|l| l.trim_start().starts_with(&format!("{line}  {kind}  "))),
                "outline of {file} lacks {line} {kind}:\n{outline}"
            );
        }
    }
    let (_, out, _) = ctx_in(tmp.path(), &["def", "Other", "--no-body"]);
    assert_eq!(
        out,
        "group.go:7  type  Other int\n\n1 definition (searched 6 files); 1 textual match dropped: no definition node\n"
    );
}

/// C4 (AD-40002): a Vue file with `<script>` and `<script setup>` is
/// tagged block by block, at its real line numbers, by `outline`, `def` and
/// the comment check of `refs`.
#[test]
fn every_vue_script_block_is_read() {
    let tmp = tempfile::tempdir().unwrap();
    let vue = "<script lang=\"ts\">\nexport function first() {\n  return 1\n}\n</script>\n\n<template>\n  <p>{{ second() }}</p>\n</template>\n\n<script setup lang=\"ts\">\n// second is set up below\nfunction second() {\n  return first()\n}\n</script>\n";
    fs::write(tmp.path().join("Two.vue"), vue).unwrap();
    let (code, out, _) = ctx_in(tmp.path(), &["outline", "Two.vue"]);
    assert_eq!(
        (code, out.as_str()),
        (
            0,
            "== Two.vue ==\n2  fn  export function first() {\n13  fn  function second() {\n"
        )
    );
    let (_, out, _) = ctx_in(tmp.path(), &["def", "second", "--no-body"]);
    assert_eq!(
        out,
        "Two.vue:13  fn  function second() {\n\n1 definition (searched 1 file)\n"
    );
    let (_, out, _) = ctx_in(tmp.path(), &["refs", "second"]);
    assert!(out.starts_with("Two.vue:8  ref  "), "{out}");
    assert!(out.contains("\nTwo.vue:13  ref  "), "{out}");
    assert!(out.contains("1 dropped (comments/strings)"), "{out}");
}

/// Cn6: a name that starts with `$` is found whole-word, as PHP writes a
/// variable; a longer name holding it is not.
#[test]
fn refs_finds_a_dollar_name() {
    let tmp = tempfile::tempdir().unwrap();
    let php = "<?php\nfunction a($user) {\n    return $user->name . $username;\n}\n";
    fs::write(tmp.path().join("a.php"), php).unwrap();
    let (code, out, _) = ctx_in(tmp.path(), &["refs", "$user"]);
    assert_eq!(code, 0);
    assert_eq!(
        hits(&out),
        [
            "a.php:2  ref  function a($user) {",
            "a.php:3  ref  return $user->name . $username;"
        ]
    );
    assert!(
        out.ends_with("2 hits in 1 file (searched 1 file)\n"),
        "{out}"
    );
}

/// Slice 12 tasks 4 and 5: Go and Rust per language; the Vue `<script>`
/// block at its real line numbers; an empty file and a template-only Vue
/// file print their header alone.
#[test]
fn outline_covers_go_rust_vue_and_empty_files() {
    let out = ctx(&["outline", "go/client.go"]);
    let kinds: Vec<&str> = out
        .lines()
        .skip(1)
        .map(|l| l.split("  ").nth(1).unwrap())
        .collect();
    assert_eq!(kinds, ["class", "fn", "method"], "{out}");
    let out = ctx(&["outline", "rs/config.rs"]);
    assert!(out.contains("\n4  class  pub struct Config {\n"), "{out}");
    assert!(out.contains("\n  10  method  pub fn new("), "{out}");
    assert!(out.contains("\n  17  method  pub fn loud("), "{out}");
    assert!(out.contains("\n23  class  pub trait Load {\n"), "{out}");
    assert!(!out.contains("make_fn"), "{out}");
    let out = ctx(&["outline", "vue/UserCard.vue"]);
    assert_eq!(
        out,
        "== vue/UserCard.vue ==\n6  fn  function greet(name: string): string {\n10  fn  const useThing = (prefix: string) => {\n"
    );

    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("empty.rs"), "").unwrap();
    fs::write(
        tmp.path().join("Only.vue"),
        "<template>\n  <p>hi</p>\n</template>\n",
    )
    .unwrap();
    for (file, want) in [
        ("empty.rs", "== empty.rs ==\n"),
        ("Only.vue", "== Only.vue ==\n"),
    ] {
        let (code, out, err) = ctx_in(tmp.path(), &["outline", file]);
        assert_eq!((code, out.as_str(), err.as_str()), (0, want, ""));
    }
    let (code, out, _) = ctx_in(tmp.path(), &["def", "hi"]);
    assert_eq!(
        (code, out.as_str()),
        (0, "no definition of hi found (searched 2 files)\n")
    );
    let (code, _, err) = ctx_in(tmp.path(), &["outline", "empty.rs", "nowhere.rs"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("factory ctx: cannot read nowhere.rs: "),
        "{err}"
    );
}

/// Slice 12 task 2: a file with a syntax error keeps slice 11's textual
/// answer, with a trailer note, so a broken file never hides a definition.
#[test]
fn a_file_that_does_not_parse_keeps_its_textual_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let php = "<?php\nclass Broken\n{\n    public function run(\n}\n";
    fs::write(tmp.path().join("Broken.php"), php).unwrap();
    let (code, out, _) = ctx_in(tmp.path(), &["def", "Broken"]);
    assert_eq!(code, 0);
    assert!(
        out.starts_with("Broken.php:2  class  class Broken\n"),
        "{out}"
    );
    assert!(
        out.ends_with(
            "1 definition (searched 1 file); 1 file not parsed cleanly: textual matches kept\n"
        ),
        "{out}"
    );
    let (_, out, _) = ctx_in(tmp.path(), &["def", "run", "--kind", "method", "--no-body"]);
    assert!(
        out.starts_with("Broken.php:4  fn  public function run(\n"),
        "{out}"
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
    assert_eq!(code, 0, "--kind class is a valid kind");
    assert!(out.contains("rs/config.rs:4  class"), "{out}");
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
/// Cn5 (AD-40004): `build/`, `dist/` and `storage/` are skipped only at the
/// root; `vendor/` and `node_modules/` at any depth.
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
        "build/Gen.php",
        "storage/framework/Cache.php",
        "php/vendor/Lib.php",
        "php/node_modules/x/index.js",
    ];
    let nested = ["php/build/Gen.php", "app/storage/Disk.php", "web/dist/a.ts"];
    for p in planted.iter().chain(&nested) {
        fs::create_dir_all(root.join(p).parent().unwrap()).unwrap();
        fs::write(root.join(p), "<?php class Planted {}\n").unwrap();
    }
    expected.extend(nested.iter().map(PathBuf::from));
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
        "no definition of Blob found (searched 51 files); skipped 1 binary file, 1 file over 2 MB\n"
    );
}
