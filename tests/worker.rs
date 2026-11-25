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
    // A relative data dir from a subdirectory: the worker, run in the repo, must still get absolute paths.
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
        // Nothing inherited, so the push-off entries start at 0.
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

/// The push URL's scheme is one git has no helper for, so a push fails before any connection.
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

/// `PATH` with the non-Claude worker stubs first; a fixture's own `PATH` holds no codex, agy or opencode.
fn with_worker_stubs(fx: &Fixture) -> String {
    format!(
        "{}:{}",
        harness::stubs_dir().join("workers").display(),
        fx.env()["PATH"]
    )
}

fn run_on(fx: &Fixture, workers: &[&str]) -> std::process::Output {
    let mut cmd = fx.command(BIN);
    cmd.args(["run", "001", "--foreground"])
        .env("PATH", with_worker_stubs(fx));
    for w in workers {
        cmd.args(["--worker", w]);
    }
    let out = cmd.output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect()
}

fn refused(o: &std::process::Output, what: &str) {
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(126), "{what}: {err}");
    assert!(
        err.contains(factory::cli::hook::NEVER_HINT),
        "{what}: {err}"
    );
}

fn passed(o: std::process::Output, what: &str) -> String {
    assert!(
        o.status.success(),
        "{what}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap()
}

#[test]
fn universal_path_shims_block_gh_and_git_push_including_pushurl_while_passing_normal_git() {
    for provider in ["claude", "codex"] {
        let fx = Fixture::with_index(
            &format!("worker-shims-{provider}"),
            &[("checkpoint_every", "10")],
        );
        fx.write_plan(&[json!({"outcome": "done"})]);
        run_on(&fx, &[provider]);
        let dir = only_run_dir(&fx);
        let bin = dir.join("bin");
        assert_eq!(mode(&bin), 0o700, "{provider}");
        assert_eq!(mode(&dir.join("gh-config")), 0o700, "{provider}");
        for shim in ["gh", "git"] {
            assert_eq!(mode(&bin.join(shim)), 0o700, "{provider}: {shim}");
        }

        let calls = fx.calls();
        assert!(calls.len() >= 4, "{calls:?}");
        let call = &calls[0];
        assert_eq!(call["provider"], provider);
        let path = call["path"].as_str().unwrap().to_string();
        for c in &calls {
            let first = c["path"].as_str().unwrap().split(':').next().unwrap();
            assert_eq!(real(Path::new(first)), real(&bin), "{provider}: {path}");
        }
        let mut env: Vec<(String, String)> = call["git_config"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect();
        env.push(("PATH".into(), path.clone()));
        env.push((
            "GH_CONFIG_DIR".into(),
            call["gh_config_dir"].as_str().unwrap().into(),
        ));
        let base = fx.path().parent().unwrap().to_path_buf();
        let repo = base.join(format!("shim-{provider}"));
        let bare = base.join(format!("shim-{provider}-pushurl.git"));
        fs::create_dir_all(&repo).unwrap();
        passed(
            fx.command("git")
                .args(["init", "-q", "--bare", "-b", "main"])
                .arg(&bare)
                .output()
                .unwrap(),
            "init bare",
        );
        let worker = |env: &[(String, String)], cwd: &Path, prog: &str, args: &[&str]| {
            fx.command(prog)
                .current_dir(cwd)
                .env_remove("GIT_CONFIG_COUNT")
                .envs(env.iter().map(|(k, v)| (k, v)))
                .args(args)
                .output()
                .unwrap()
        };
        let w = |prog: &str, args: &[&str]| worker(&env, &repo, prog, args);

        refused(&w("gh", &["pr", "list"]), "gh pr list");
        refused(&w("gh", &["api", "repos/x/y"]), "gh api");

        passed(w("git", &["init", "-q", "-b", "main"]), "init");
        fs::write(repo.join("a.txt"), "first\n").unwrap();
        assert!(passed(w("git", &["status", "--short"]), "status").contains("a.txt"));
        passed(w("git", &["add", "a.txt"]), "add");
        passed(
            w(
                "git",
                &[
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "commit",
                    "-q",
                    "-m",
                    "first",
                ],
            ),
            "commit",
        );
        fs::write(repo.join("a.txt"), "first\nsecond\n").unwrap();
        assert!(passed(w("git", &["diff"]), "diff").contains("+second"));
        let log = worker(
            &env,
            &base,
            "git",
            &["-C", repo.to_str().unwrap(), "log", "-1", "--format=%s"],
        );
        assert_eq!(passed(log, "log").trim(), "first");
        let missing = w("git", &["rev-parse", "--verify", "-q", "nope"]);
        assert_eq!(
            missing.status.code(),
            Some(1),
            "an exit code passes through"
        );

        passed(
            w(
                "git",
                &["remote", "add", "origin", "https://example.invalid/x.git"],
            ),
            "remote add",
        );
        passed(
            w(
                "git",
                &["config", "remote.origin.pushurl", bare.to_str().unwrap()],
            ),
            "pushurl",
        );
        let to = ["origin", "HEAD:refs/heads/main"];
        refused(&w("git", &["push"]), "bare push");
        refused(&w("git", &[&["push"][..], &to].concat()), "pushurl push");
        refused(
            &w(
                "git",
                &[
                    &["-c", "url.https://example.invalid/.pushInsteadOf=x", "push"][..],
                    &to,
                ]
                .concat(),
            ),
            "-c push",
        );
        refused(
            &worker(
                &env,
                &base,
                "git",
                &[&["-C", repo.to_str().unwrap(), "push"][..], &to].concat(),
            ),
            "-C push",
        );
        refused(
            &w("git", &["--no-pager", "--git-dir=.git", "push", "origin"]),
            "options then push",
        );
        refused(
            &w("git", &["send-pack", bare.to_str().unwrap(), "HEAD"]),
            "send-pack",
        );
        let refs = |what: &str| {
            passed(
                fx.command("git")
                    .args(["--git-dir", bare.to_str().unwrap(), "for-each-ref"])
                    .output()
                    .unwrap(),
                what,
            )
        };
        assert_eq!(refs("refs"), "", "{provider}: nothing reached the pushurl");

        // The hole the shim closes: push_off_env never rewrites an explicit local pushurl.
        let unshimmed: Vec<(String, String)> = env
            .iter()
            .map(|(k, v)| match k.as_str() {
                "PATH" => (k.clone(), v.split_once(':').unwrap().1.to_string()),
                _ => (k.clone(), v.clone()),
            })
            .collect();
        passed(
            worker(
                &unshimmed,
                &repo,
                "git",
                &[&["push", "-q"][..], &to].concat(),
            ),
            "push without the shim",
        );
        assert_ne!(refs("refs after"), "");
    }
}

/// Roles by the pack a call was handed.
fn role_of(call: &Value) -> &'static str {
    let pack = call["env"]["FACTORY_PACK"].as_str().unwrap();
    let name = Path::new(pack).file_name().unwrap().to_str().unwrap();
    [
        ("slice-", "build"),
        ("merge-", "merge"),
        ("fix-", "fix"),
        ("ci-fix-", "fix"),
        ("checkpoint-", "checkpoint"),
        ("polish-", "polish"),
    ]
    .iter()
    .find(|(p, _)| name.starts_with(p))
    .map(|(_, r)| *r)
    .unwrap_or_else(|| panic!("no role for {name}"))
}

#[test]
fn all_five_worker_providers_spawn_with_expected_flags_and_large_packs() {
    const BIG: usize = 150 * 1024;
    let read_line = |p: &Path| format!("Read and follow all instructions in {}.", p.display());
    for (provider, specs) in [
        ("claude", &["claude:sonnet:low"][..]),
        ("codex", &["codex:gpt-5.4:xhigh"]),
        ("cursor", &["cursor:grok-4.7-high"]),
        ("agy", &["agy:gemini-4-neon-high:max"]),
        (
            "opencode",
            &[
                "opencode:anthropic/claude-x",
                "polish=opencode:anthropic/claude-x:max",
            ],
        ),
    ] {
        let fx = Fixture::with_index(
            &format!("worker-provider-{provider}"),
            &[("checkpoint_every", "10")],
        );
        let slice = fx.slice_path(1);
        let mut text = fs::read_to_string(&slice).unwrap();
        for i in 0..(BIG / 60 + 10) {
            text.push_str(&format!(
                "Filler line {i:06} so the pack outgrows one argv string.\n"
            ));
        }
        fs::write(&slice, text).unwrap();
        fx.git(&["commit", "-qam", "a slice too big for argv"]);
        fx.write_plan(&[json!({"outcome": "done"})]);
        run_on(&fx, specs);

        let dir = only_run_dir(&fx);
        let wdir = dir.join("workers");
        let st = state(&dir);
        let workers = st["workers"].as_array().unwrap();
        let calls = fx.calls();
        assert_eq!(calls.len(), workers.len(), "{provider}");
        assert!(
            calls.len() >= 5,
            "three slices, a checkpoint, a polish: {calls:?}"
        );
        let mut big = 0;
        for w in workers {
            assert_eq!(w["provider"], provider, "{w}");
            let sid = w["session_id"].as_str().unwrap();
            let mut files = vec![format!("{sid}.jsonl"), format!("{sid}.stderr")];
            if provider == "claude" {
                files.push(format!("{sid}.settings.json"));
            } else {
                files.push(format!("{sid}.prompt.md"));
                assert!(!wdir.join(format!("{sid}.settings.json")).exists());
            }
            for f in &files {
                assert_eq!(mode(&wdir.join(f)), 0o600, "{provider}: {f}");
            }
            let stderr = fs::read_to_string(wdir.join(format!("{sid}.stderr"))).unwrap();
            assert!(
                !stderr.contains("Argument list too long") && !stderr.contains("E2BIG"),
                "{provider}: {stderr}"
            );
            assert_eq!(w["outcome"], "done", "{provider}: {w}");
        }
        for call in &calls {
            assert_eq!(call["provider"], provider);
            let role = role_of(call);
            let argv = strings(&call["provider_argv"]);
            assert_eq!(call["env"]["FACTORY_REPO"], call["cwd"]);
            assert_eq!(call["env"]["FACTORY_BOARD"], fx.path().to_str().unwrap());
            assert!(call["env"]["FACTORY_EVIDENCE"].is_string(), "{call}");
            let pack_len = call["pack"].as_str().unwrap().len();
            if role == "build"
                && call["pack"]
                    .as_str()
                    .unwrap()
                    .contains("Filler line 000000")
            {
                assert!(pack_len > BIG, "{provider}: {pack_len}");
                big += 1;
            }
            if provider == "claude" {
                assert_eq!(
                    flag(&call["argv"].as_array().unwrap()[..], "--model"),
                    "sonnet"
                );
                assert_eq!(
                    flag(&call["argv"].as_array().unwrap()[..], "--effort"),
                    "low"
                );
                harness::assert_role_scope(call);
                continue;
            }
            let prompt = PathBuf::from(call["prompt_file"].as_str().unwrap());
            assert_eq!(
                call["prompt_ok"], true,
                "{provider}: the prompt file carries the pack"
            );
            assert_eq!(real(prompt.parent().unwrap()), real(&wdir));
            let name = prompt.file_name().unwrap().to_str().unwrap();
            let sid = name.strip_suffix(".prompt.md").unwrap();
            assert!(workers.iter().any(|w| w["session_id"] == sid), "{name}");
            let body = fs::read_to_string(&prompt).unwrap();
            assert!(body.ends_with(&format!("\n---\n\n{}\n", call["prompt"].as_str().unwrap())));
            let s = |v: &[&str]| v.iter().map(|a| a.to_string()).collect::<Vec<_>>();
            let want = match provider {
                "codex" => s(&[
                    "exec",
                    "--json",
                    "-m",
                    "gpt-5.4",
                    "-c",
                    "model_reasoning_effort=xhigh",
                    "--dangerously-bypass-approvals-and-sandbox",
                    "-",
                ]),
                "cursor" => s(&[
                    "-p",
                    "--force",
                    "--trust",
                    "--output-format",
                    "stream-json",
                    "--stream-partial-output",
                    "--model",
                    "grok-4.7-high",
                    "--add-dir",
                    wdir.to_str().unwrap(),
                    &read_line(&prompt),
                ]),
                "agy" => {
                    let mut v = s(&[
                        "-p",
                        "--output-format",
                        "stream-json",
                        "--dangerously-skip-permissions",
                        "--model",
                        "gemini-4-neon-high",
                        "--effort",
                        "max",
                    ]);
                    if matches!(role, "build" | "fix" | "merge") {
                        v.push("--disable-slash-commands".into());
                    }
                    v.extend(s(&[
                        "--add-dir",
                        wdir.to_str().unwrap(),
                        &read_line(&prompt),
                    ]));
                    v
                }
                _ => s(&[
                    "run",
                    "--standalone",
                    "--format",
                    "json",
                    "-m",
                    if role == "polish" {
                        "anthropic/claude-x#max"
                    } else {
                        "anthropic/claude-x"
                    },
                    "-f",
                    prompt.to_str().unwrap(),
                    "Read and follow all instructions in the attached prompt file.",
                ]),
            };
            assert_eq!(argv, want, "{provider} {role}");
        }
        assert_eq!(big, 1, "{provider}: the big slice was built once");
    }
}

#[test]
fn mixed_provider_run_completes_slices_checkpoint_and_polish() {
    let fx = Fixture::with_index("worker-mixed", &[("checkpoint_every", "10")]);
    fx.write_plan(&[json!({"outcome": "done"})]);
    run_on(
        &fx,
        &[
            "build=codex",
            "checkpoint=agy",
            "polish=claude",
            "fix=claude",
        ],
    );
    let dir = only_run_dir(&fx);
    let st = state(&dir);
    assert_eq!(st["phase"], "finished", "{st}");
    let workers = st["workers"].as_array().unwrap();
    let mut seen = Vec::new();
    for w in workers {
        let want = match w["role"].as_str().unwrap() {
            "build" | "merge" => "codex",
            "checkpoint" => "agy",
            _ => "claude",
        };
        assert_eq!(w["provider"], want, "{w}");
        assert_eq!(w["outcome"], "done", "{w}");
        seen.push(w["role"].as_str().unwrap().to_string());
    }
    for role in ["build", "checkpoint", "polish"] {
        assert!(seen.iter().any(|r| r == role), "{role}: {seen:?}");
    }
    assert_eq!(seen.iter().filter(|r| *r == "build").count(), 3, "{seen:?}");
    let events = fs::read_to_string(dir.join("events.jsonl")).unwrap();
    assert!(!events.contains("\"subagents_unread\""), "{events}");
    assert_eq!(
        fx.state()
            .slices
            .values()
            .filter(|s| s.status == "done")
            .count(),
        3
    );
}
