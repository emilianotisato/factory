//! The fixture every integration test builds: a throwaway git repo on the
//! v3 docs layout, built by the binary (`factory docs init`, `new`, `slice`,
//! PRD-002 FR-18), a scripted gate and the stub `claude` and `gh` first on
//! PATH (PRD-001 FR-10, slice 01). `Fixture::v2` keeps a v2 repo for the
//! migration and refusal tests.
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

/// `tests/fixture/`: `gate.sh`, the v2 fixture and the stub `docs.mjs`.
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
    /// A fixture with the default `[docs]` keys.
    pub fn new(name: &str) -> Fixture {
        Self::with_index(name, &[])
    }

    /// A fixture whose `docs/project.toml` `[docs]` table also carries
    /// `extra` keys (`checkpoint_every`, for one); a key it already has is
    /// replaced. Values are TOML: a string is passed quoted.
    pub fn with_index(name: &str, extra: &[(&str, &str)]) -> Fixture {
        Self::v3_with(name, extra)
    }

    /// The v3 repo (PRD-002 FR-18), built by the binary: `factory docs
    /// init`, `new`, `slice`, then `set 001 status=ready`. The same as
    /// [`Fixture::new`].
    pub fn v3(name: &str) -> Fixture {
        Self::v3_with(name, &[])
    }

    /// A v3 fixture whose `[docs]` table also carries `extra` keys.
    pub fn v3_with(name: &str, extra: &[(&str, &str)]) -> Fixture {
        let fx = Self::bare(name);
        fx.build_v3(extra);
        fx
    }

    /// The checked-in v2 repo of `tests/fixture/v2/` (PRD-002 slice 07), for
    /// `migrate` and the v2 routing tests.
    pub fn v2(name: &str) -> Fixture {
        Self::v2_with_docs(name, &fixture_src().join("v2/docs"))
    }

    /// The v2 fixture's tool and gate with another `docs/` tree, committed
    /// once with fixed dates, so every `git blame` date in it is 2026-09-30.
    pub fn v2_with_docs(name: &str, docs: &Path) -> Fixture {
        let fx = Self::bare(name);
        fx.init_repo();
        copy_dir(&fixture_src().join("v2/.docs"), &fx.root.join(".docs"));
        copy_dir(docs, &fx.root.join("docs"));
        fx.git(&["add", "-A"]);
        let date = "2026-09-30T12:00:00+00:00";
        let out = fx
            .command(&fx.git)
            .args(["commit", "-q", "-m", "chore: v2 fixture repo"])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .expect("run git commit");
        assert!(
            out.status.success(),
            "git commit: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        fx
    }

    /// `$FACTORY_DATA_DIR`: where the metrics store lives.
    pub fn data_dir(&self) -> PathBuf {
        self.base.path().join("data")
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
        let mut keys: Vec<(&str, &str)> = vec![
            ("framework", "3"),
            ("gate", "\"./gate.sh\""),
            ("inner", "\"./gate.sh --inner\""),
            ("lanes", "2"),
            ("gate_parallel", "true"),
        ];
        for (k, v) in extra {
            match keys.iter_mut().find(|(have, _)| have == k) {
                Some(kv) => kv.1 = v,
                None => keys.push((k, v)),
            }
        }
        let mut toml = String::from("[docs]\n");
        for (k, v) in keys {
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

    /// `factory docs <args>`; panics on a non-zero exit so a shape change in
    /// the tool fails the fixture, not a test three steps later.
    pub fn docs(&self, args: &[&str]) -> String {
        self.factory_docs(args)
    }

    /// `factory docs <cmd> 001 --json` parsed, whatever the exit code
    /// (preflight exits 1 or 2 when it refuses or asks, and the JSON says why).
    pub fn docs_json(&self, cmd: &str) -> Value {
        self.factory_docs_json(cmd)
    }

    /// The fixture feature's `log.jsonl` entries, as `log show --json` reads
    /// them; empty when the log is.
    pub fn log_entries(&self) -> Vec<Value> {
        let out = self.factory_docs(&["log", "show", FEATURE, "--json"]);
        match serde_json::from_str::<Value>(&out).expect("log show --json") {
            Value::Array(a) => a,
            other => panic!("log show --json is not an array: {other}"),
        }
    }

    /// The log entries of one `kind` (`block`, `decision`, `note`, …).
    pub fn log_of(&self, kind: &str) -> Vec<Value> {
        self.log_entries()
            .into_iter()
            .filter(|e| e["kind"] == kind)
            .collect()
    }

    /// The fixture feature's `state.toml`, parsed by the library.
    pub fn state(&self) -> factory::docs::state::State {
        factory::docs::state::read(&self.root.join(FEATURE_DIR))
            .expect("read state.toml")
            .expect("state.toml exists")
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

    /// The slice's `status` in `state.toml`.
    pub fn slice_status(&self, n: u8) -> String {
        self.state()
            .slices
            .get(&u32::from(n))
            .map(|s| s.status.clone())
            .unwrap_or_else(|| panic!("slice {n:02} has no status in state.toml"))
    }

    /// The slice's `lane` in `state.toml`, `None` when it holds none.
    pub fn slice_lane(&self, n: u8) -> Option<u32> {
        self.state().slices.get(&u32::from(n)).and_then(|s| s.lane)
    }

    /// `factory docs set 001 slices.N.status=<status>`.
    pub fn set_slice_status(&self, n: u8, status: &str) {
        self.factory_docs(&["set", FEATURE, &format!("slices.{n}.status={status}")]);
    }

    /// Script the full gate: one outcome per line of `scenario/gate`.
    /// Make slice 02 depend on 01 so the plan is a chain, 01 → 02 → 03, and
    /// the two lanes of the index build one slice at a time. The default
    /// fixture leaves 01 and 02 independent (slice 04 builds them at once);
    /// the tests of the one-at-a-time loop want them in order.
    pub fn chain(&self) {
        assert!(self.state().slices[&2].depends_on.is_empty());
        self.factory_docs(&["set", FEATURE, "slices.2.depends_on=1"]);
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
        // The stub `claude` writes the board through the binary, as a real
        // worker does through its contract's resolved path.
        (
            "FACTORY_BIN".to_string(),
            env!("CARGO_BIN_EXE_factory").to_string(),
        ),
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
