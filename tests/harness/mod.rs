//! The fixture every integration test builds: a throwaway git repo on the
//! feature-docs v2 framework with the vendored `docs.mjs`, a scripted gate and
//! the stub `claude` and `gh` first on PATH (PRD-001 FR-10, slice 01).
//!
//! Shared by `tests/*.rs` through `#[path = "harness/mod.rs"] mod harness;`.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// The fixture's one feature, as `docs new` numbers it.
pub const FEATURE: &str = "001";
pub const FEATURE_DIR: &str = "docs/features/001-fixture";

/// `tests/stubs/`: the stub `claude` and `gh`.
pub fn stubs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stubs")
}

/// `tests/fixture/`: the vendored docs tool and `gate.sh`.
pub fn fixture_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture")
}

/// One line of `scenario/gate.log`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateRun {
    pub epoch: u64,
    pub kind: String,
    pub exit: i32,
}

pub struct Fixture {
    base: TempDir,
    root: PathBuf,
    stub_dir: PathBuf,
    node: PathBuf,
    git: PathBuf,
    env: BTreeMap<String, String>,
}

impl Fixture {
    /// A fixture with the default index keys.
    pub fn new(name: &str) -> Fixture {
        Self::with_index(name, &[])
    }

    /// A fixture whose `docs/index.md` frontmatter also carries `extra` keys
    /// (`checkpoint_every`, for one).
    pub fn with_index(name: &str, extra: &[(&str, &str)]) -> Fixture {
        let fx = Self::bare(name);
        fx.build(extra);
        fx
    }

    /// The same repo on the v3 layout (PRD-002 FR-18), built by the binary:
    /// `factory docs init`, `new`, `slice`, then `set 001 status=ready`.
    pub fn v3(name: &str) -> Fixture {
        Self::v3_with(name, &[])
    }

    /// A v3 fixture whose `[docs]` table also carries `extra` keys.
    pub fn v3_with(name: &str, extra: &[(&str, &str)]) -> Fixture {
        let fx = Self::bare(name);
        fx.build_v3(extra);
        fx
    }

    /// The temp dirs and the env, before any repo is built.
    fn bare(name: &str) -> Fixture {
        let mut base = tempfile::Builder::new()
            .prefix(&format!("factory-{name}-"))
            .tempdir()
            .expect("tempdir");
        // `FACTORY_KEEP_FIXTURE=1` leaves the repo, the run dir and the stub's
        // call log on disk after the test: a checkpoint's evidence (CP-2) is the
        // `events.jsonl` and the pack a passing run leaves behind.
        if std::env::var_os("FACTORY_KEEP_FIXTURE").is_some() {
            base.disable_cleanup(true);
            eprintln!("fixture {name} kept at {}", base.path().display());
        }
        let root = base.path().join("repo");
        let stub_dir = base.path().join("stub");
        for d in [
            &root,
            &stub_dir,
            &base.path().join("data"),
            &base.path().join("config"),
        ] {
            fs::create_dir_all(d).expect("fixture dirs");
        }
        let gitconfig = base.path().join("gitconfig");
        fs::write(&gitconfig, "").expect("empty gitconfig");
        let node = find_in_path("node").expect("node on PATH");
        let git = find_in_path("git").expect("git on PATH");
        let env = build_env(base.path(), &stub_dir, &gitconfig, &node, &git);
        Fixture {
            base,
            root,
            stub_dir,
            node,
            git,
            env,
        }
    }

    fn build(&self, extra: &[(&str, &str)]) {
        // A bare origin next to the repo: preflight refuses a repo with no
        // `origin` remote (docs.mjs `preflight`), and later slices push to it.
        // The gate script exists before the index names it: docs.mjs resolves a
        // `./gate.sh` gate by checking the file is there.
        self.init_repo();
        copy_dir(&fixture_src().join(".docs"), &self.root.join(".docs"));

        self.docs(&["init"]);
        self.write_index_frontmatter(extra);
        self.docs(&["new", "fixture", "--title", "Fixture"]);
        self.docs(&["slice", FEATURE, "a"]);
        self.docs(&["slice", FEATURE, "b"]);
        self.docs(&["slice", FEATURE, "c", "--depends", "2"]);
        self.patch_prd();
        self.docs(&["render", "--write"]);

        fs::create_dir_all(self.root.join("src")).unwrap();
        fs::write(self.root.join("src/lib.txt"), "fixture code file\n").unwrap();
        fs::create_dir_all(self.root.join("scenario")).unwrap();
        // Scripted outcomes and the gate log change on every run; ignored so a
        // run never dirties the tree the driver is about to commit from.
        fs::write(self.root.join(".gitignore"), "/scenario/\n").unwrap();

        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "chore: fixture repo"]);
    }

    /// The origin, the git identity and the gate script, shared by both
    /// layouts.
    fn init_repo(&self) {
        let origin = self.base.path().join("origin.git");
        self.git_in(
            self.base.path(),
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                origin.to_str().unwrap(),
            ],
        );
        self.git(&["init", "-q", "-b", "main"]);
        self.git(&["config", "user.name", "Fixture"]);
        self.git(&["config", "user.email", "fixture@example.invalid"]);
        self.git(&["remote", "add", "origin", origin.to_str().unwrap()]);
        let gate = self.root.join("gate.sh");
        fs::copy(fixture_src().join("gate.sh"), &gate).expect("copy gate.sh");
        fs::set_permissions(&gate, fs::Permissions::from_mode(0o755)).expect("chmod gate.sh");
    }

    fn build_v3(&self, extra: &[(&str, &str)]) {
        self.init_repo();
        self.factory_docs(&["init"]);
        let mut toml = String::from(
            "[docs]\nframework = 3\ngate = \"./gate.sh\"\ninner = \"./gate.sh --inner\"\nlanes = 2\ngate_parallel = true\n",
        );
        for (k, v) in extra {
            toml.push_str(&format!("{k} = {v}\n"));
        }
        fs::write(self.root.join("docs/project.toml"), toml).unwrap();
        self.factory_docs(&["new", "fixture", "--title", "Fixture"]);
        self.factory_docs(&["slice", FEATURE, "a"]);
        self.factory_docs(&["slice", FEATURE, "b"]);
        self.factory_docs(&["slice", FEATURE, "c", "--depends", "2"]);
        self.patch_prd_v3();
        self.factory_docs(&["set", FEATURE, "status=ready"]);

        fs::create_dir_all(self.root.join("src")).unwrap();
        fs::write(self.root.join("src/lib.txt"), "fixture code file\n").unwrap();
        fs::create_dir_all(self.root.join("scenario")).unwrap();
        fs::write(self.root.join(".gitignore"), "/scenario/\n").unwrap();

        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "chore: fixture repo"]);
    }

    /// The v3 scaffold with its `summary` filled, the harness line and one
    /// `run` row per slice.
    fn patch_prd_v3(&self) {
        let p = self.root.join(FEATURE_DIR).join("prd.md");
        let text = fs::read_to_string(&p).expect("fixture prd.md");
        let mut out = Vec::new();
        let mut in_summary = false;
        for line in text.lines() {
            if in_summary && line.starts_with("  ") {
                continue;
            }
            in_summary = false;
            if line == "summary: |" {
                out.push(line.to_string());
                out.push("  Fixture PRD, approved for the harness run.".to_string());
                out.push("  Three slices; c waits for b.".to_string());
                in_summary = true;
            } else if line.starts_with("**Harness**:") {
                out.push("**Harness**: none — fixture, gate-only".to_string());
            } else if line.starts_with("| FR-1 |") {
                for (n, name) in [("01", "a"), ("02", "b"), ("03", "c")] {
                    out.push(format!(
                        "| FR-1 | run slice {n} ({name}) in the fixture → the board shows it done | test transcript | no |"
                    ));
                }
            } else {
                out.push(line.to_string());
            }
        }
        fs::write(&p, out.join("\n") + "\n").unwrap();
    }

    /// `factory docs <args>`; panics on a non-zero exit.
    pub fn factory_docs(&self, args: &[&str]) -> String {
        let out = self.factory_docs_output(args);
        assert!(
            out.status.success(),
            "factory docs {args:?} exited {:?}\nstdout:\n{}\nstderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("utf8")
    }

    /// `factory docs <args>` whatever the exit code.
    pub fn factory_docs_output(&self, args: &[&str]) -> Output {
        self.command(env!("CARGO_BIN_EXE_factory"))
            .arg("docs")
            .args(args)
            .output()
            .expect("run factory docs")
    }

    /// `factory docs <cmd> 001 --json` parsed, whatever the exit code.
    pub fn factory_docs_json(&self, cmd: &str) -> Value {
        let out = self.factory_docs_output(&[cmd, FEATURE, "--json"]);
        let stdout = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "factory docs {cmd} --json is not JSON ({e})\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
    }

    fn write_index_frontmatter(&self, extra: &[(&str, &str)]) {
        let p = self.root.join("docs/index.md");
        let text = fs::read_to_string(&p).expect("docs/index.md");
        let rest = text.strip_prefix("---\n").expect("index frontmatter");
        let end = rest.find("\n---\n").expect("index frontmatter end");
        let body = &rest[end + 5..];
        let mut fm = String::from(
            "---\nframework: 2\ngate: ./gate.sh\ninner: ./gate.sh --inner\nlanes: 2\ngate_parallel: true\n",
        );
        for (k, v) in extra {
            fm.push_str(&format!("{k}: {v}\n"));
        }
        fs::write(&p, format!("{fm}---\n{body}")).unwrap();
    }

    /// The scaffold is a draft with a placeholder QA row; the fixture is a PRD
    /// approved for delivery with one `run` row per slice.
    fn patch_prd(&self) {
        let p = self.root.join(FEATURE_DIR).join("prd.md");
        let text = fs::read_to_string(&p).expect("fixture prd.md");
        let mut out = Vec::new();
        let mut in_fm = true;
        for (i, line) in text.lines().enumerate() {
            if i > 0 && line == "---" {
                in_fm = false;
            }
            if in_fm && line == "status: draft" {
                out.push("status: ready".to_string());
            } else if line.starts_with("**Status**:") {
                out.push(
                    "**Status**: **ready** — fixture PRD, approved for the harness run".to_string(),
                );
            } else if line.starts_with("**Harness**:") {
                out.push("**Harness**: none — fixture, gate-only".to_string());
            } else if line.starts_with("| FR-1 |") {
                for (n, name) in [("01", "a"), ("02", "b"), ("03", "c")] {
                    out.push(format!(
                        "| FR-1 | run slice {n} ({name}) in the fixture → the board shows it done | test transcript | no |"
                    ));
                }
            } else {
                out.push(line.to_string());
            }
        }
        fs::write(&p, out.join("\n") + "\n").unwrap();
    }

    /// The repo root.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// `$FACTORY_STUB_DIR`: `calls/`, `plan.jsonl`, `gh.log`.
    pub fn stub_dir(&self) -> &Path {
        &self.stub_dir
    }

    /// The environment the driver and the stubs run with: PATH with the stubs
    /// first, the `FACTORY_*` dirs and an empty global git config.
    pub fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    /// A command prepared with the fixture's env, cwd at the repo root.
    pub fn command<S: AsRef<OsStr>>(&self, program: S) -> Command {
        let mut c = Command::new(program);
        c.current_dir(&self.root).envs(&self.env);
        c
    }

    /// `node .docs/bin/docs.mjs <args>`; panics on a non-zero exit so a shape
    /// change in the tool fails the fixture, not a test three steps later.
    pub fn docs(&self, args: &[&str]) -> String {
        let out = self
            .command(&self.node)
            .arg(".docs/bin/docs.mjs")
            .args(args)
            .output()
            .expect("run docs.mjs");
        assert!(
            out.status.success(),
            "docs {args:?} exited {:?}\nstdout:\n{}\nstderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("utf8")
    }

    /// `docs <cmd> 001 --json` parsed, whatever the exit code (preflight exits
    /// 1 or 2 when it refuses or asks, and the JSON says why).
    pub fn docs_json(&self, cmd: &str) -> Value {
        let out = self
            .command(&self.node)
            .args([".docs/bin/docs.mjs", cmd, FEATURE, "--json"])
            .output()
            .expect("run docs.mjs");
        let stdout = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "docs {cmd} --json is not JSON ({e})\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
    }

    pub fn git(&self, args: &[&str]) -> String {
        self.git_in(&self.root, args)
    }

    fn git_in(&self, dir: &Path, args: &[&str]) -> String {
        let out = Command::new(&self.git)
            .args(args)
            .current_dir(dir)
            .envs(&self.env)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} exited {:?}\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("utf8")
    }

    /// A file under the repo root.
    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    }

    /// The fixture's `slice-0N-*.md`.
    pub fn slice_path(&self, n: u8) -> PathBuf {
        let prefix = format!("slice-{n:02}-");
        fs::read_dir(self.root.join(FEATURE_DIR))
            .expect("feature dir")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.starts_with(&prefix) && f.ends_with(".md"))
            })
            .unwrap_or_else(|| panic!("no slice {n:02} in {FEATURE_DIR}"))
    }

    /// `status:` from a slice's frontmatter.
    pub fn slice_status(&self, n: u8) -> String {
        let text = fs::read_to_string(self.slice_path(n)).expect("slice file");
        text.lines()
            .skip(1)
            .take_while(|l| *l != "---")
            .find_map(|l| l.strip_prefix("status:"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| panic!("slice {n:02} has no status"))
    }

    /// Script the full gate: one outcome per line of `scenario/gate`.
    /// Make slice 02 depend on 01 so the plan is a chain, 01 → 02 → 03, and
    /// the two lanes of the index build one slice at a time. The default
    /// fixture leaves 01 and 02 independent (slice 04 builds them at once);
    /// the tests of the one-at-a-time loop want them in order.
    pub fn chain(&self) {
        let p = self.slice_path(2);
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("depends_on: []"), "{text}");
        fs::write(&p, text.replacen("depends_on: []", "depends_on: [1]", 1)).unwrap();
        self.docs(&["render", "--write"]);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "chore: chain the fixture slices"]);
    }

    pub fn script_gate(&self, outcomes: &[&str]) {
        self.script("gate", outcomes);
    }

    /// Script the inner loop: `scenario/inner`.
    pub fn script_inner(&self, outcomes: &[&str]) {
        self.script("inner", outcomes);
    }

    /// Script the stub `gh`'s `pr checks` (slice 09): `scenario/ci`, one
    /// line per call — `green`, `red:<check>[@<sha>]`, `flaky:<check>`.
    pub fn script_ci(&self, lines: &[&str]) {
        self.script("ci", lines);
    }

    /// Every recorded stub `gh` call, one argv line each, in order.
    pub fn gh_log(&self) -> Vec<String> {
        fs::read_to_string(self.stub_dir.join("gh.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn script(&self, kind: &str, outcomes: &[&str]) {
        fs::create_dir_all(self.root.join("scenario")).unwrap();
        fs::write(
            self.root.join("scenario").join(kind),
            outcomes.join("\n") + "\n",
        )
        .unwrap();
    }

    /// Run `./gate.sh` (or `./gate.sh --inner`) as the driver would.
    pub fn run_gate_output(&self, inner: bool) -> Output {
        let mut c = self.command(self.root.join("gate.sh"));
        if inner {
            c.arg("--inner");
        }
        c.output().expect("run gate.sh")
    }

    /// The gate's exit code.
    pub fn run_gate(&self, inner: bool) -> i32 {
        self.run_gate_output(inner).status.code().unwrap_or(-1)
    }

    /// `scenario/gate.log`, parsed; empty when no gate has run.
    pub fn gate_log(&self) -> Vec<GateRun> {
        let Ok(text) = fs::read_to_string(self.root.join("scenario/gate.log")) else {
            return Vec::new();
        };
        text.lines()
            .map(|l| {
                let mut it = l.split_whitespace();
                let epoch = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| panic!("gate.log line {l:?}"));
                let kind = it
                    .next()
                    .unwrap_or_else(|| panic!("gate.log line {l:?}"))
                    .to_string();
                let exit = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| panic!("gate.log line {l:?}"));
                GateRun { epoch, kind, exit }
            })
            .collect()
    }

    /// The stub claude's plan: one step per spawn, in order.
    pub fn write_plan(&self, steps: &[Value]) {
        let text: String = steps.iter().map(|s| format!("{s}\n")).collect();
        fs::write(self.stub_dir.join("plan.jsonl"), text).unwrap();
    }

    /// Every recorded stub claude call, in spawn order.
    pub fn calls(&self) -> Vec<Value> {
        let dir = self.stub_dir.join("calls");
        let mut files: Vec<PathBuf> = match fs::read_dir(&dir) {
            Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
            Err(_) => return Vec::new(),
        };
        files.sort_by_key(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
        });
        // The stub claims `N.json` empty (`wx`) and fills it a moment later:
        // a poll that lands in between sees a claim, not a call, and skips it.
        files
            .iter()
            .filter_map(|p| {
                let text = fs::read_to_string(p).unwrap();
                if text.trim().is_empty() {
                    return None;
                }
                Some(serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
            })
            .collect()
    }
}

fn build_env(
    base: &Path,
    stub_dir: &Path,
    gitconfig: &Path,
    node: &Path,
    git: &Path,
) -> BTreeMap<String, String> {
    let mut dirs: Vec<PathBuf> = vec![stubs_dir()];
    for tool in [
        node.to_path_buf(),
        git.to_path_buf(),
        find_in_path("sh").expect("sh on PATH"),
    ] {
        dirs.push(tool.parent().expect("tool dir").to_path_buf());
    }
    dirs.push(PathBuf::from("/usr/bin"));
    dirs.push(PathBuf::from("/bin"));
    let mut seen = Vec::new();
    for d in dirs {
        if !seen.contains(&d) {
            seen.push(d);
        }
    }
    let path = seen
        .iter()
        .map(|d| d.to_str().expect("utf8 path"))
        .collect::<Vec<_>>()
        .join(":");
    let s = |p: &Path| p.to_str().expect("utf8 path").to_string();
    BTreeMap::from([
        ("PATH".to_string(), path),
        ("FACTORY_STUB_DIR".to_string(), s(stub_dir)),
        ("FACTORY_DATA_DIR".to_string(), s(&base.join("data"))),
        ("FACTORY_CONFIG_DIR".to_string(), s(&base.join("config"))),
        ("GIT_CONFIG_GLOBAL".to_string(), s(gitconfig)),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        // Claude Code's config dir: where the stub writes a session's
        // subagent transcripts and the driver reads them (PRD-003 FR-21).
        ("CLAUDE_CONFIG_DIR".to_string(), s(&base.join("claude"))),
    ])
}

/// The first executable `name` on the test process's own PATH.
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_str()?
        .split(':')
        .map(|d| Path::new(d).join(name))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("mkdir {}: {e}", to.display()));
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display())) {
        let entry = entry.expect("dir entry");
        let dest = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), &dest)
                .unwrap_or_else(|e| panic!("copy {}: {e}", entry.path().display()));
        }
    }
}
