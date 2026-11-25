use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use factory::cli::setup::{Env, SetupArgs, run_with};
use serde_json::Value;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

/// A config dir, a data dir and a `PATH` holding only stub provider CLIs that fail if run.
struct Dirs {
    tmp: TempDir,
}

impl Dirs {
    fn new(stubs: &[&str]) -> Dirs {
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        for name in stubs {
            let p = bin.join(name);
            fs::write(&p, "#!/bin/sh\necho ran >&2\nexit 1\n").unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::create_dir_all(tmp.path().join("config")).unwrap();
        Dirs { tmp }
    }

    fn config_dir(&self) -> PathBuf {
        self.tmp.path().join("config")
    }

    fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }

    fn config(&self) -> toml::Table {
        toml::from_str(&fs::read_to_string(self.config_file()).unwrap()).unwrap()
    }

    fn env(&self) -> Env {
        Env {
            config_dir: self.config_dir(),
            repo: None,
            path: self.tmp.path().join("bin").into_os_string(),
            is_tty: true,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(BIN);
        c.args(args)
            .current_dir(self.tmp.path())
            .env("PATH", self.tmp.path().join("bin"))
            .env("HOME", self.tmp.path())
            .env("FACTORY_CONFIG_DIR", self.config_dir())
            .env("FACTORY_DATA_DIR", self.tmp.path().join("data"))
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("FACTORY_REVIEW_PROVIDER")
            .env_remove("FACTORY_CI_MODE");
        c
    }

    fn factory(&self, args: &[&str]) -> Output {
        self.command(args).stdin(Stdio::null()).output().unwrap()
    }
}

fn wizard(dirs: &Dirs, script: &str) -> (factory::Result<i32>, String) {
    let mut input = Cursor::new(script.as_bytes().to_vec());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_with(
        &SetupArgs::default(),
        &mut input,
        &dirs.env(),
        &mut out,
        &mut err,
    );
    (code, String::from_utf8(out).unwrap())
}

fn get<'a>(t: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (last, tables) = path.split_last().unwrap();
    let mut t = t;
    for part in tables {
        t = t.get(*part)?.as_table()?;
    }
    t.get(*last)
}

fn str_at<'a>(t: &'a toml::Table, path: &[&str]) -> Option<&'a str> {
    get(t, path).and_then(|v| v.as_str())
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn setup_wizard_configures_all_sections_and_keeps_defaults() {
    let dirs = Dirs::new(&["claude", "codex"]);
    let seeded = "# my factory settings\nbudget_weighted_tokens = 1000\n\n[review]\n# legacy reviewer\nagent = \"cursor-agent\"\ntimeout_secs = 60\n";
    fs::write(dirs.config_file(), seeded).unwrap();

    let script = [
        "1", "n", // cockpit: claude, no mod install
        "2", "medium", "y", "polish", "haiku", "",
        "", // workers: sonnet · medium, polish model haiku
        "codex", "1", "high", // review: codex gpt-5.4 · high
        "1",    // ci: local
    ]
    .join("\n")
        + "\n";
    let (code, out) = wizard(&dirs, &script);
    assert_eq!(code.unwrap(), 0, "{out}");
    assert!(out.contains("On PATH: claude"), "{out}");
    assert!(out.contains("codex (on PATH)"), "{out}");
    assert!(
        out.contains(&format!("saved {}", dirs.config_file().display())),
        "{out}"
    );

    let first = fs::read_to_string(dirs.config_file()).unwrap();
    assert!(first.starts_with("# my factory settings\n"), "{first}");
    assert!(first.contains("# legacy reviewer"), "{first}");
    let t = dirs.config();
    assert_eq!(str_at(&t, &["cockpit", "provider"]), Some("claude"));
    assert_eq!(str_at(&t, &["workers", "model"]), Some("sonnet"));
    assert_eq!(str_at(&t, &["workers", "effort"]), Some("medium"));
    assert_eq!(str_at(&t, &["workers", "polish", "model"]), Some("haiku"));
    assert_eq!(get(&t, &["workers", "polish", "effort"]), None);
    assert_eq!(str_at(&t, &["review", "provider"]), Some("codex"));
    assert_eq!(str_at(&t, &["review", "model"]), Some("gpt-5.4"));
    assert_eq!(str_at(&t, &["review", "effort"]), Some("high"));
    assert_eq!(
        get(&t, &["review", "agent"]),
        None,
        "a cursor binary must not run the codex review"
    );
    assert_eq!(
        get(&t, &["review", "timeout_secs"]).unwrap().as_integer(),
        Some(60)
    );
    assert_eq!(str_at(&t, &["ci", "mode"]), Some("local"));
    assert_eq!(
        get(&t, &["budget_weighted_tokens"]).unwrap().as_integer(),
        Some(1000)
    );

    let modified = fs::metadata(dirs.config_file())
        .unwrap()
        .modified()
        .unwrap();
    let (code, out) = wizard(&dirs, &"\n".repeat(40));
    assert_eq!(code.unwrap(), 0, "{out}");
    assert!(
        out.contains("[sonnet]") && out.contains("[gpt-5.4]"),
        "{out}"
    );
    assert!(out.contains("nothing changed"), "{out}");
    assert_eq!(fs::read_to_string(dirs.config_file()).unwrap(), first);
    assert_eq!(
        fs::metadata(dirs.config_file())
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );

    let all_or_nothing = Dirs::new(&["claude"]);
    let inline = "ci = { mode = \"remote\" }\n";
    fs::write(all_or_nothing.config_file(), inline).unwrap();
    let (code, out) = wizard(&all_or_nothing, "2\n\n\n\n\n\n\n1\n");
    let e = code.expect_err("an inline [ci] table cannot be patched by line");
    assert!(e.to_string().contains("by hand"), "{e}\n{out}");
    assert_eq!(
        fs::read_to_string(all_or_nothing.config_file()).unwrap(),
        inline,
        "the cockpit and review answers were written without the ci one"
    );
}

#[test]
fn setup_non_interactive_flags_and_show_json() {
    let dirs = Dirs::new(&["codex"]);
    for args in [
        &[
            "setup",
            "review",
            "--provider",
            "codex",
            "--model",
            "gpt-5.4",
            "--effort",
            "high",
        ][..],
        &["setup", "ci", "--mode", "local"],
        &["setup", "cockpit", "--provider", "agy"],
        &[
            "setup", "workers", "--role", "polish", "--model", "sonnet", "--effort", "medium",
        ],
    ] {
        let o = dirs.factory(args);
        assert_eq!(o.status.code(), Some(0), "{args:?}: {}", text(&o.stderr));
        assert!(
            text(&o.stdout).contains("saved "),
            "{args:?}: {}",
            text(&o.stdout)
        );
    }
    let t = dirs.config();
    assert_eq!(str_at(&t, &["review", "provider"]), Some("codex"));
    assert_eq!(str_at(&t, &["review", "model"]), Some("gpt-5.4"));
    assert_eq!(str_at(&t, &["review", "effort"]), Some("high"));
    assert_eq!(str_at(&t, &["ci", "mode"]), Some("local"));
    assert_eq!(str_at(&t, &["cockpit", "provider"]), Some("agy"));
    assert_eq!(str_at(&t, &["workers", "polish", "model"]), Some("sonnet"));
    assert_eq!(str_at(&t, &["workers", "polish", "effort"]), Some("medium"));

    let before = fs::read_to_string(dirs.config_file()).unwrap();
    let modified = fs::metadata(dirs.config_file())
        .unwrap()
        .modified()
        .unwrap();
    let o = dirs.factory(&["setup", "--show", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["cockpit"]["provider"], "agy");
    assert_eq!(v["cockpit"]["source"], "user");
    assert_eq!(v["review"]["provider"], "codex");
    assert_eq!(v["review"]["model"], "gpt-5.4");
    assert_eq!(v["review"]["effort"], "high");
    assert_eq!(v["review"]["source"], "user");
    assert_eq!(v["ci"]["mode"], "local");
    assert_eq!(v["ci"]["source"], "user");
    assert_eq!(v["workers"]["polish"]["model"], "sonnet");
    assert_eq!(v["workers"]["polish"]["effort"], "medium");
    assert_eq!(v["workers"]["polish"]["model_source"], "user");
    assert_eq!(v["workers"]["build"]["model"], "opus");
    assert_eq!(v["workers"]["build"]["model_source"], "default");
    let codex = dirs.tmp.path().join("bin/codex");
    assert_eq!(v["detected"]["codex"], codex.to_str().unwrap());
    assert_eq!(v["detected"]["claude"], Value::Null);
    let o = dirs.factory(&["setup", "--show"]);
    assert!(
        text(&o.stdout).contains("review      codex gpt-5.4 · high (user)"),
        "{}",
        text(&o.stdout)
    );
    assert_eq!(fs::read_to_string(dirs.config_file()).unwrap(), before);
    assert_eq!(
        fs::metadata(dirs.config_file())
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );

    let o = dirs.factory(&[
        "setup",
        "review",
        "--provider",
        "cursor",
        "--effort",
        "high",
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("cursor takes no --effort"),
        "{}",
        text(&o.stderr)
    );
    assert_eq!(fs::read_to_string(dirs.config_file()).unwrap(), before);

    let o = dirs.factory(&["setup", "review", "--provider", "none"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let t = dirs.config();
    assert_eq!(str_at(&t, &["review", "provider"]), Some("none"));
    assert_eq!(get(&t, &["review", "model"]), None);
    assert_eq!(get(&t, &["review", "effort"]), None);
    let v: Value =
        serde_json::from_slice(&dirs.factory(&["setup", "--show", "--json"]).stdout).unwrap();
    assert_eq!(v["review"]["provider"], "none");
    assert_eq!(v["review"]["model"], Value::Null);
}

fn exits_within(dirs: &Dirs, args: &[&str], limit: Duration) -> Output {
    let mut child = dirs
        .command(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let held_open = child.stdin.take();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > limit {
            child.kill().unwrap();
            panic!("factory {args:?} blocked on a piped stdin");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(held_open);
    child.wait_with_output().unwrap()
}

#[test]
fn setup_non_tty_without_flags_exits_one() {
    let dirs = Dirs::new(&["claude", "codex"]);
    for args in [&["setup"][..], &["setup", "review"]] {
        let o = exits_within(&dirs, args, Duration::from_secs(10));
        assert_eq!(o.status.code(), Some(1), "{args:?}");
        let err = text(&o.stderr);
        for flag in ["--provider", "--mode", "--model", "--show"] {
            assert!(err.contains(flag), "{flag} missing from:\n{err}");
        }
        assert!(text(&o.stdout).is_empty(), "{}", text(&o.stdout));
    }
    assert!(!Path::new(&dirs.config_file()).exists());
}

#[test]
fn setup_refuses_what_it_cannot_save_and_saves_nothing_on_cut_input() {
    let dirs = Dirs::new(&["claude", "opencode"]);
    let o = dirs.factory(&["setup", "--show", "review", "--provider", "none"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stdout));
    let o = dirs.factory(&["setup", "review", "--provider", "opencode"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stdout));
    assert!(
        text(&o.stderr).contains("give --model"),
        "{}",
        text(&o.stderr)
    );
    let o = dirs.factory(&["setup", "review", "--provider", "codex", "--model=-x"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stdout));
    assert!(!dirs.config_file().exists());

    let (code, out) = wizard(&dirs, "");
    assert_eq!(code.unwrap(), 1, "{out}");
    assert!(out.contains("nothing saved"), "{out}");
    assert!(!dirs.config_file().exists());
}
