//! A throwaway v3 repo built by the binary, a scripted gate, stub `claude` and `gh` first on PATH.
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

// The `bad` strings are the v2 log's vocabulary.
pub fn assert_v3_contract(pack: &str) {
    let contract = &pack[pack.find("\n# Contract\n").expect("a # Contract section")..];
    let bin = env!("CARGO_BIN_EXE_factory");
    assert!(
        contract.contains(&format!("`{bin} docs ")),
        "no `{bin} docs …` in the contract:\n{contract}"
    );
    for bad in [
        "{seed}",
        "{factory}",
        "agent-log.md",
        "log-slice-",
        "log-fix-",
        "log-ci-",
        "render --write",
    ] {
        assert!(!pack.contains(bad), "{bad:?} in the pack:\n{pack}");
    }
}

/// Checks a worker call's tools, MCP, slash commands, output style and memory against its role,
/// read from its pack's file name, and returns that role.
pub fn assert_role_scope(call: &Value) -> &'static str {
    let argv: Vec<&str> = call["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let flag = |name: &str| {
        let i = argv
            .iter()
            .position(|a| *a == name)
            .unwrap_or_else(|| panic!("{name} missing in {argv:?}"));
        argv[i + 1]
    };
    let pack = Path::new(flag("--append-system-prompt-file"));
    let name = pack.file_name().unwrap().to_str().unwrap();
    let role = [
        ("slice-", "build"),
        ("merge-", "merge"),
        ("fix-", "fix"),
        ("ci-fix-", "fix"),
        ("checkpoint-", "checkpoint"),
        ("polish-", "polish"),
    ]
    .iter()
    .find(|(prefix, _)| name.starts_with(prefix))
    .map(|(_, role)| *role)
    .unwrap_or_else(|| panic!("no role for pack {name}"));
    let narrowed = matches!(role, "build" | "fix" | "merge");
    for f in ["--strict-mcp-config", "--disable-slash-commands", "--tools"] {
        assert_eq!(argv.contains(&f), narrowed, "{role}: {f} in {argv:?}");
    }
    if narrowed {
        assert_eq!(flag("--tools"), "Bash,Read,Edit,Write", "{argv:?}");
    }
    let settings = check_settings(Path::new(flag("--settings")));
    assert_eq!(settings["outputStyle"], "default", "{role}: {settings}");
    let memory = settings.get("autoMemoryEnabled");
    if narrowed {
        assert_eq!(memory, Some(&Value::Bool(false)), "{role}: {settings}");
    } else {
        assert_eq!(memory, None, "{role}: {settings}");
    }
    role
}

/// The deny rules and the hook every worker's settings carry; returns the whole file.
pub fn check_settings(path: &Path) -> Value {
    let v: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let deny: Vec<&str> = v["permissions"]["deny"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_str().unwrap())
        .collect();
    assert_eq!(deny, ["Bash(gh)", "Bash(gh *)", "Bash(git push*)"]);
    let pre = &v["hooks"]["PreToolUse"];
    assert_eq!(pre.as_array().map(Vec::len), Some(1), "{v}");
    assert_eq!(pre[0]["matcher"], "Bash|Read");
    let hook = &pre[0]["hooks"][0];
    assert_eq!(hook["type"], "command");
    let cmd = hook["command"].as_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_factory");
    assert_eq!(cmd, format!("{} hook", factory::paths::shell_word(bin)));
    assert_eq!(hook["timeout"], 5);
    v
}

pub fn stubs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stubs")
}

pub fn fixture_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateRun {
    pub epoch: u64,
    pub kind: String,
    pub exit: i32,
    /// The directory the gate ran in, as `pwd -P` printed it.
    pub pwd: PathBuf,
    /// The HEAD it saw; `-` when HEAD is unborn.
    pub head: String,
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
    pub fn new(name: &str) -> Fixture {
        Self::with_index(name, &[])
    }

    /// `extra` values are raw TOML: a string is passed quoted.
    pub fn with_index(name: &str, extra: &[(&str, &str)]) -> Fixture {
        let fx = Self::bare(name);
        fx.build_v3(extra);
        fx
    }

    pub fn v2(name: &str) -> Fixture {
        Self::v2_with_docs(name, &fixture_src().join("v2/docs"))
    }

    /// Committed with a fixed date so every `git blame` in it reads 2026-09-30.
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

    pub fn data_dir(&self) -> PathBuf {
        self.base.path().join("data")
    }

    pub fn set_seed(&self, paths: &[&str]) {
        let items: Vec<String> = paths.iter().map(|p| format!("{p:?}")).collect();
        self.rewrite_lanes(Some(&format!("[lanes]\nseed = [{}]\n", items.join(", "))));
    }

    pub fn drop_lanes_table(&self) {
        self.rewrite_lanes(None);
    }

    fn rewrite_lanes(&self, table: Option<&str>) {
        let file = self.root.join("docs/project.toml");
        let text = fs::read_to_string(&file).unwrap();
        let mut out = String::new();
        let mut inside = false;
        for line in text.lines() {
            if line.trim_start().starts_with('[') {
                inside = line.trim() == "[lanes]";
            }
            if !inside {
                out.push_str(line);
                out.push('\n');
            }
        }
        let mut out = out.trim_end().to_string();
        out.push('\n');
        if let Some(t) = table {
            out.push('\n');
            out.push_str(t);
        }
        fs::write(&file, out).unwrap();
        self.git(&["add", "docs/project.toml"]);
        self.git(&["commit", "-q", "-m", "chore: lanes table"]);
    }

    fn bare(name: &str) -> Fixture {
        let mut base = tempfile::Builder::new()
            .prefix(&format!("factory-{name}-"))
            .tempdir()
            .expect("tempdir");
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
        self.docs(&["init"]);
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
        // Preflight refuses a repo with no `[lanes] seed`.
        toml.push_str("\n[lanes]\nseed = []\n");
        fs::write(self.root.join("docs/project.toml"), toml).unwrap();
        self.docs(&["new", "fixture", "--title", "Fixture"]);
        self.docs(&["slice", FEATURE, "a"]);
        self.docs(&["slice", FEATURE, "b"]);
        self.docs(&["slice", FEATURE, "c", "--depends", "2"]);
        self.patch_prd_v3();
        self.docs(&["set", FEATURE, "status=ready"]);

        fs::create_dir_all(self.root.join("src")).unwrap();
        fs::write(self.root.join("src/lib.txt"), "fixture code file\n").unwrap();
        fs::create_dir_all(self.root.join("scenario")).unwrap();
        fs::write(self.root.join(".gitignore"), "/scenario/\n").unwrap();

        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "chore: fixture repo"]);
    }

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

    /// Panics on a non-zero exit so a tool shape change fails the fixture, not a later test.
    pub fn docs(&self, args: &[&str]) -> String {
        let out = self.docs_output(args);
        assert!(
            out.status.success(),
            "factory docs {args:?} exited {:?}\nstdout:\n{}\nstderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("utf8")
    }

    pub fn docs_output(&self, args: &[&str]) -> Output {
        self.command(env!("CARGO_BIN_EXE_factory"))
            .arg("docs")
            .args(args)
            .output()
            .expect("run factory docs")
    }

    /// No exit check: preflight exits 1 or 2 when it refuses or asks, and the JSON says why.
    pub fn docs_json(&self, cmd: &str) -> Value {
        let out = self.docs_output(&[cmd, FEATURE, "--json"]);
        let stdout = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "factory docs {cmd} --json is not JSON ({e})\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn stub_dir(&self) -> &Path {
        &self.stub_dir
    }

    pub fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub fn command<S: AsRef<OsStr>>(&self, program: S) -> Command {
        let mut c = Command::new(program);
        c.current_dir(&self.root).envs(&self.env);
        c
    }

    pub fn log_entries(&self) -> Vec<Value> {
        let out = self.docs(&["log", "show", FEATURE, "--json"]);
        match serde_json::from_str::<Value>(&out).expect("log show --json") {
            Value::Array(a) => a,
            other => panic!("log show --json is not an array: {other}"),
        }
    }

    pub fn log_of(&self, kind: &str) -> Vec<Value> {
        self.log_entries()
            .into_iter()
            .filter(|e| e["kind"] == kind)
            .collect()
    }

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

    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    }

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

    pub fn slice_status(&self, n: u8) -> String {
        self.state()
            .slices
            .get(&u32::from(n))
            .map(|s| s.status.clone())
            .unwrap_or_else(|| panic!("slice {n:02} has no status in state.toml"))
    }

    pub fn slice_lane(&self, n: u8) -> Option<u32> {
        self.state().slices.get(&u32::from(n)).and_then(|s| s.lane)
    }

    pub fn set_slice_status(&self, n: u8, status: &str) {
        self.docs(&["set", FEATURE, &format!("slices.{n}.status={status}")]);
    }

    /// Chains 01 → 02 → 03 so the two lanes build one slice at a time.
    pub fn chain(&self) {
        assert!(self.state().slices[&2].depends_on.is_empty());
        self.docs(&["set", FEATURE, "slices.2.depends_on=1"]);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "chore: chain the fixture slices"]);
    }

    pub fn script_gate(&self, outcomes: &[&str]) {
        self.script("gate", outcomes);
    }

    pub fn script_inner(&self, outcomes: &[&str]) {
        self.script("inner", outcomes);
    }

    /// One line per `gh pr checks` poll: `green`, or rows `<kind>:<check>[@<sha>]`.
    pub fn script_ci(&self, lines: &[&str]) {
        self.script("ci", lines);
    }

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

    pub fn run_gate_output(&self, inner: bool) -> Output {
        let mut c = self.command(self.root.join("gate.sh"));
        if inner {
            c.arg("--inner");
        }
        c.output().expect("run gate.sh")
    }

    pub fn run_gate(&self, inner: bool) -> i32 {
        self.run_gate_output(inner).status.code().unwrap_or(-1)
    }

    pub fn gate_log(&self) -> Vec<GateRun> {
        let Ok(text) = fs::read_to_string(self.root.join("scenario/gate.log")) else {
            return Vec::new();
        };
        text.lines()
            .map(|l| {
                let parse = || {
                    let mut it = l.splitn(4, ' ');
                    let epoch = it.next()?.parse().ok()?;
                    let kind = it.next()?.to_string();
                    let exit = it.next()?.parse().ok()?;
                    // The pwd may hold a space; the head never does.
                    let (pwd, head) = it.next()?.rsplit_once(' ')?;
                    Some(GateRun {
                        epoch,
                        kind,
                        exit,
                        pwd: PathBuf::from(pwd),
                        head: head.to_string(),
                    })
                };
                parse().unwrap_or_else(|| panic!("gate.log line {l:?}"))
            })
            .collect()
    }

    /// One step per spawn, in order.
    pub fn write_plan(&self, steps: &[Value]) {
        let text: String = steps.iter().map(|s| format!("{s}\n")).collect();
        fs::write(self.stub_dir.join("plan.jsonl"), text).unwrap();
    }

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
        // The stub claims `N.json` with `wx` and fills it later: an empty file is a claim, not a call.
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
        // The stub `claude` writes the board through this binary.
        (
            "FACTORY_BIN".to_string(),
            env!("CARGO_BIN_EXE_factory").to_string(),
        ),
        ("FACTORY_DATA_DIR".to_string(), s(&base.join("data"))),
        ("FACTORY_CONFIG_DIR".to_string(), s(&base.join("config"))),
        // Never the owner's real install receipt: a test binary is always "from source".
        (
            "AXOUPDATER_CONFIG_PATH".to_string(),
            s(&base.join("axoupdater")),
        ),
        ("GIT_CONFIG_GLOBAL".to_string(), s(gitconfig)),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        // Where the stub writes a session's subagent transcripts and the driver reads them.
        ("CLAUDE_CONFIG_DIR".to_string(), s(&base.join("claude"))),
    ])
}

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
