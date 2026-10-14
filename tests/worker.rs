//! What a worker process gets from the driver, seen from the stub `claude`:
//! no GitHub credential and no session `CLAUDE_*` variable, but the account
//! and the provider (S1, L17); absolute paths on its command line however
//! the data dir is spelled (Ln6); files only its owner reads (S-n2); a
//! stream that is a file of its own, so the worker finishes its record even
//! when the driver dies first (L9); and a git env that pushes nowhere
//! (AD-10001).
#[path = "harness/mod.rs"]
mod harness;

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use factory::run::pid_alive;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn only_run_dir(fx: &Fixture) -> PathBuf {
    let runs = PathBuf::from(&fx.env()["FACTORY_DATA_DIR"]).join("runs");
    let dirs: Vec<PathBuf> = fs::read_dir(&runs)
        .map(|rd| rd.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs[0].clone()
}

fn state(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn mode(p: &Path) -> u32 {
    fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn real(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn flag<'a>(argv: &'a [Value], name: &str) -> &'a str {
    let i = argv
        .iter()
        .position(|a| a == name)
        .unwrap_or_else(|| panic!("{name} missing in {argv:?}"));
    argv[i + 1].as_str().unwrap()
}

fn wait_until(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        assert!(
            Instant::now() < deadline,
            "timed out after {secs}s waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_worker_gets_no_github_and_no_session_but_keeps_the_account() {
    let fx = Fixture::with_index("worker-env", &[("checkpoint_every", "10")]);
    fx.write_plan(&[json!({"outcome": "done"})]);
    // The driver runs from a subdirectory with a data dir relative to it: a
    // relative path on the worker's command line would resolve against the
    // repo, where the worker runs, and miss (Ln6).
    let account = fx.path().parent().unwrap().join("claude-config");
    let out = fx
        .command(BIN)
        .args(["run", "001", "--foreground"])
        .current_dir(fx.path().join("docs"))
        .env("FACTORY_DATA_DIR", "../../data")
        .envs([
            ("GH_TOKEN", "gho_x"),
            ("GITHUB_TOKEN", "ghs_x"),
            ("GH_ENTERPRISE_TOKEN", "x"),
            ("GITHUB_ENTERPRISE_TOKEN", "x"),
            ("GH_CONFIG_DIR", "/nonexistent/gh"),
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_ENTRYPOINT", "cli"),
            ("CLAUDE_CODE_SSE_PORT", "1"),
            ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat-x"),
            ("CLAUDE_CODE_USE_BEDROCK", "1"),
            ("SSH_AUTH_SOCK", "/nonexistent/agent.sock"),
        ])
        .env("CLAUDE_CONFIG_DIR", &account)
        .env_remove("GIT_CONFIG_COUNT")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let dir = only_run_dir(&fx);
    assert!(dir.is_absolute(), "{dir:?}");
    let workers = dir.join("workers");
    assert_eq!(mode(&workers), 0o700);

    let calls = fx.calls();
    assert!(calls.len() >= 4, "three slices and a checkpoint: {calls:?}");
    for call in &calls {
        let keys: Vec<&str> = call["env_keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap())
            .collect();
        for gone in [
            "GH_TOKEN",
            "GITHUB_TOKEN",
            "GH_ENTERPRISE_TOKEN",
            "GITHUB_ENTERPRISE_TOKEN",
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_SSE_PORT",
        ] {
            assert!(!keys.contains(&gone), "{gone} reached the worker");
        }
        for kept in [
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_USE_BEDROCK",
            "SSH_AUTH_SOCK",
        ] {
            assert!(keys.contains(&kept), "{kept} was scrubbed");
        }
        // gh, were it ever run, finds no stored login.
        let gh = PathBuf::from(call["gh_config_dir"].as_str().unwrap());
        assert!(gh.is_absolute(), "{gh:?}");
        assert_eq!(real(&gh), real(&dir.join("gh-config")));
        assert_eq!(mode(&gh), 0o700);
        assert_eq!(fs::read_dir(&gh).unwrap().count(), 0);
        // git pushes nowhere (AD-10001); nothing inherited, so ours start at 0.
        let git = &call["git_config"];
        assert_eq!(git["GIT_CONFIG_COUNT"], "5", "{git}");
        assert_eq!(
            git["GIT_CONFIG_KEY_0"],
            "url.factory-no-push://.pushInsteadOf"
        );
        assert_eq!(git["GIT_CONFIG_VALUE_0"], "git@");

        let argv = call["argv"].as_array().unwrap();
        let pack = Path::new(flag(argv, "--append-system-prompt-file"));
        let settings = Path::new(flag(argv, "--settings"));
        assert!(pack.is_absolute() && pack.is_file(), "{pack:?}");
        assert!(settings.is_absolute() && settings.is_file(), "{settings:?}");
        assert!(real(pack).starts_with(real(&dir)), "{pack:?}");
        assert_eq!(call["env"]["FACTORY_PACK"], pack.to_str().unwrap());
        // PRD-005 FR-2: the repo is the worker's cwd, its lane for a build
        // worker; the board is always the main tree's.
        assert_eq!(call["env"]["FACTORY_REPO"], call["cwd"]);
        assert_eq!(call["env"]["FACTORY_BOARD"], fx.path().to_str().unwrap());

        let sid = flag(argv, "--session-id");
        for file in [
            format!("{sid}.jsonl"),
            format!("{sid}.stderr"),
            format!("{sid}.settings.json"),
        ] {
            assert_eq!(mode(&workers.join(&file)), 0o600, "{file}");
        }
    }
}

/// Run `git args` in `repo` with the fixture's git env and `extra` on top.
fn git_in(
    fx: &Fixture,
    repo: &Path,
    extra: &[(String, String)],
    args: &[&str],
) -> std::process::Output {
    fx.command("git")
        .current_dir(repo)
        .env_remove("GIT_CONFIG_COUNT")
        .envs(extra.iter().map(|(k, v)| (k, v)))
        .args(args)
        .output()
        .unwrap()
}

/// AD-10001: a worker's push URLs go to a transport git has no helper for.
/// The driver inherits one env-config entry of the owner's; the worker keeps
/// it and gets the five `pushInsteadOf` entries after it. Then, with the env
/// the stub saw, a push to an https, an http, a git:// or an scp-style ssh
/// origin fails at the helper lookup, before any connection, and fetch URLs
/// are untouched.
#[test]
fn a_worker_pushes_nowhere_through_the_git_push_off_env() {
    let fx = Fixture::with_index("worker-push-off", &[("checkpoint_every", "10")]);
    fx.write_plan(&[json!({"outcome": "done"})]);
    let out = fx
        .command(BIN)
        .args(["run", "001", "--foreground"])
        .envs([
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "factory.owner"),
            ("GIT_CONFIG_VALUE_0", "kept"),
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let calls = fx.calls();
    assert!(calls.len() >= 4, "three slices and a checkpoint: {calls:?}");
    let key = "url.factory-no-push://.pushInsteadOf";
    let want = json!({
        "GIT_CONFIG_COUNT": "6",
        "GIT_CONFIG_KEY_0": "factory.owner",
        "GIT_CONFIG_VALUE_0": "kept",
        "GIT_CONFIG_KEY_1": key,
        "GIT_CONFIG_VALUE_1": "git@",
        "GIT_CONFIG_KEY_2": key,
        "GIT_CONFIG_VALUE_2": "ssh://",
        "GIT_CONFIG_KEY_3": key,
        "GIT_CONFIG_VALUE_3": "https://",
        "GIT_CONFIG_KEY_4": key,
        "GIT_CONFIG_VALUE_4": "http://",
        "GIT_CONFIG_KEY_5": key,
        "GIT_CONFIG_VALUE_5": "git://",
    });
    for call in &calls {
        assert_eq!(call["git_config"], want, "call {}", call["n"]);
    }
    let env: Vec<(String, String)> = calls[0]["git_config"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect();

    let repo = fx.path().parent().unwrap().join("push-off");
    fs::create_dir_all(&repo).unwrap();
    let ok = |o: std::process::Output| {
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    };
    ok(git_in(&fx, &repo, &[], &["init", "-q", "-b", "main"]));
    ok(git_in(
        &fx,
        &repo,
        &[],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ],
    ));
    ok(git_in(
        &fx,
        &repo,
        &[],
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    ));
    assert_eq!(
        ok(git_in(&fx, &repo, &env, &["config", "factory.owner"])),
        "kept"
    );

    // ssh would leave this mark if git ever reached for it.
    let mark = repo.join("ssh-ran");
    let ssh = (
        "GIT_SSH_COMMAND".to_string(),
        format!("touch {} && false", mark.display()),
    );
    for (url, rewritten) in [
        (
            "https://example.invalid/x.git",
            "factory-no-push://example.invalid/x.git",
        ),
        (
            "git@example.invalid:x.git",
            "factory-no-push://example.invalid:x.git",
        ),
        (
            "http://example.invalid/x.git",
            "factory-no-push://example.invalid/x.git",
        ),
        (
            "git://example.invalid/x.git",
            "factory-no-push://example.invalid/x.git",
        ),
    ] {
        ok(git_in(
            &fx,
            &repo,
            &[],
            &["remote", "set-url", "origin", url],
        ));
        let push_url = ok(git_in(
            &fx,
            &repo,
            &env,
            &["remote", "get-url", "--push", "origin"],
        ));
        assert!(
            push_url.starts_with("factory-no-push://"),
            "{url}: {push_url}"
        );
        assert_eq!(push_url, rewritten);
        assert_eq!(
            ok(git_in(&fx, &repo, &env, &["remote", "get-url", "origin"])),
            url
        );
        let mut with_ssh = env.clone();
        with_ssh.push(ssh.clone());
        let push = git_in(
            &fx,
            &repo,
            &with_ssh,
            &["push", "origin", "HEAD:refs/heads/main"],
        );
        let err = String::from_utf8_lossy(&push.stderr);
        assert!(!push.status.success(), "{url}: {err}");
        assert!(err.contains("remote-factory-no-push"), "{url}: {err}");
        assert!(!err.contains("resolve") && !mark.exists(), "{url}: {err}");
        // Without the push-off env the same push would reach for the network.
        let push_url = ok(git_in(
            &fx,
            &repo,
            &[],
            &["remote", "get-url", "--push", "origin"],
        ));
        assert_eq!(push_url, url);
    }
}

#[test]
fn the_stream_outlives_the_driver() {
    let fx = Fixture::with_index("worker-orphan", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[json!({"outcome": "done", "delay_ms": 2500})]);
    let scenario = fx.path().join("scenario");
    fs::create_dir_all(&scenario).unwrap();
    let mut child = fx
        .command(BIN)
        .args(["run", "001", "--foreground"])
        .stdin(Stdio::null())
        .stdout(File::create(scenario.join("driver.out")).unwrap())
        .stderr(File::create(scenario.join("driver.err")).unwrap())
        .process_group(0)
        .spawn()
        .unwrap();
    let driver = child.id();
    wait_until("slice 01 under a worker", 20, || {
        fs::read_dir(PathBuf::from(&fx.env()["FACTORY_DATA_DIR"]).join("runs"))
            .is_ok_and(|mut rd| rd.next().is_some())
            && fx.calls().len() == 1
            && state(&only_run_dir(&fx))["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 1)
    });
    let dir = only_run_dir(&fx);
    let w = &state(&dir)["workers"][0];
    let worker = w["pid"].as_u64().unwrap() as u32;
    let stream = dir
        .join("workers")
        .join(format!("{}.jsonl", w["session_id"].as_str().unwrap()));

    // Only the driver dies; its worker is still sleeping before its output.
    // SAFETY: a plain kill(2) on the test's own child.
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    assert!(child.wait().unwrap().code().is_none());
    assert!(pid_alive(worker), "the worker outlives the driver");
    wait_until("the orphaned worker to finish", 20, || !pid_alive(worker));

    let text = fs::read_to_string(&stream).unwrap();
    let last: Value = serde_json::from_str(text.lines().last().expect("a stream")).unwrap();
    assert_eq!(last["type"], "result", "{text}");
    assert_eq!(last["is_error"], false, "{text}");
    // SAFETY: the group is the test's own; nothing in it should be left.
    unsafe { libc::kill(-(driver as i32), libc::SIGKILL) };
}
