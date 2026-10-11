//! One worker: `claude -p` with a context pack (PRD §4 FR-2) — a build
//! worker per slice, a fix or checkpoint worker per checkpoint (FR-4). The
//! worker's stdout is the record file `runs/<id>/workers/<session>.jsonl`
//! itself, so the stream outlives the driver; the driver parses the file
//! when the worker exits. Stderr goes to `<session>.stderr`, the pid lands
//! in `state.json` and a `workers` row is opened at spawn and closed at exit.
//!
//! A worker inherits the driver's session and process group (the daemon's
//! `setsid`), so one group signal reaches the driver and its worker together.
//! It does not inherit `CLAUDECODE` or the session's `CLAUDE_*` variables: a
//! driver started from inside a Claude Code session would otherwise hand its
//! workers the nested-session guard. The variables that pick the account,
//! the config dir and the provider ([`KEEP_CLAUDE_VARS`]) pass through. It
//! does get its lane (FR-3): `FACTORY_LANE=N` and whatever `env` the lane
//! file declares.
//!
//! A worker never pushes and never talks to GitHub; the driver does both.
//! So no GitHub token reaches it ([`GH_TOKEN_VARS`]), `GH_CONFIG_DIR` points
//! at an empty directory under the run, and its `--settings <file>` (slice
//! 13, §4 FR-2 as corrected 2026-10-09) carries [`DENY_RULES`] and the
//! PreToolUse hook `<this binary> hook`, which refuses any `gh` and any
//! `git push` wherever it sits in a command line. The file is written per
//! worker to `runs/<id>/workers/<session>.settings.json` — a file the driver
//! owns, so nothing is installed under the owner's `~/.claude/` or the
//! project's `.claude/`. `FACTORY_PACK=<pack>` tells the hook which files
//! the pack holds and `FACTORY_REPO=<repo>` what they are relative to.
//! On top of those, a git push-off env ([`push_off_env`], AD-10001) rewrites
//! every ssh and https push URL to a transport git has no helper for, so a
//! push that slips past the rules and the hook still goes nowhere.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::config;
use crate::error::io;
use crate::events::EventKind;
use crate::pack::Pack;
use crate::paths;
use crate::run::{Run, WorkerRef};
use crate::stream::Summary;
use crate::{Error, Result};

pub const DIR: &str = "workers";

/// The permission deny rules every worker carries (§4 FR-2): no `gh` at
/// all and no `git push` of any kind, whatever the project's own settings
/// say. A prefix rule cannot see `true && gh …` or `git -c k=v push`; the
/// hook (`cli::hook::guard`) catches those.
pub const DENY_RULES: [&str; 3] = ["Bash(gh)", "Bash(gh *)", "Bash(git push*)"];

/// The GitHub tokens scrubbed from every worker's environment.
pub const GH_TOKEN_VARS: [&str; 4] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
];

/// The `CLAUDE_*` variables a worker keeps: the account, the config dir,
/// the provider and its client certificate. Every other `CLAUDE_*` belongs
/// to the session the driver may have been started from.
pub const KEEP_CLAUDE_VARS: [&str; 9] = [
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_API_KEY_HELPER_TTL_MS",
    "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
    "CLAUDE_CODE_SKIP_VERTEX_AUTH",
    "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
    "CLAUDE_CODE_CLIENT_CERT",
    "CLAUDE_CODE_CLIENT_KEY",
    "CLAUDE_CODE_CLIENT_KEY_PASSPHRASE",
];

/// Does a worker lose `name`? `CLAUDECODE` and the session's `CLAUDE_*`,
/// but not [`KEEP_CLAUDE_VARS`] nor `CLAUDE_CODE_USE_*` (Bedrock, Vertex,
/// Foundry).
pub fn scrubbed(name: &str) -> bool {
    if name == "CLAUDECODE" {
        return true;
    }
    name.starts_with("CLAUDE_")
        && !KEEP_CLAUDE_VARS.contains(&name)
        && !name.starts_with("CLAUDE_CODE_USE_")
}

/// The worker env var naming the repo the pack's paths are relative to.
pub const REPO_ENV: &str = "FACTORY_REPO";

/// git's count of `GIT_CONFIG_KEY_<i>`/`GIT_CONFIG_VALUE_<i>` pairs.
pub const GIT_CONFIG_COUNT: &str = "GIT_CONFIG_COUNT";

/// Where a worker's push URLs are sent (AD-10001): a transport no git has a
/// `git-remote-*` helper for, so a push fails before any connection opens.
pub const PUSH_OFF_BASE: &str = "factory-no-push://";

/// The URL prefixes rewritten to [`PUSH_OFF_BASE`] on push: scp-style ssh
/// (`git@host:path`), `ssh://` and `https://`.
pub const PUSH_OFF_PREFIXES: [&str; 3] = ["git@", "ssh://", "https://"];

/// The git push-off env (AD-10001, DEC-2): one
/// `url.factory-no-push://.pushInsteadOf` entry per [`PUSH_OFF_PREFIXES`],
/// set through git's `GIT_CONFIG_KEY_<i>`/`GIT_CONFIG_VALUE_<i>` env config.
/// `inherited` is the `GIT_CONFIG_COUNT` the worker would get otherwise: its
/// `n` entries are the owner's and stay, ours go at `n`, `n+1`, `n+2` and the
/// count becomes `n+3`. A count git itself would reject as bogus counts as 0.
/// Only push URLs change; fetch, pull and `get-url` keep the remote's URL.
///
/// Known gap: git ignores `pushInsteadOf` for a remote with an explicit
/// `pushurl` (`remote.<name>.pushurl`), which it pushes to unrewritten. The
/// other layers still cover that remote: the deny rules and the hook refuse
/// any `git push`, and no GitHub token reaches the worker.
pub fn push_off_env(inherited: Option<&str>) -> Vec<(String, String)> {
    let n: usize = inherited.and_then(|c| c.parse().ok()).unwrap_or(0);
    let mut env = vec![(
        GIT_CONFIG_COUNT.to_string(),
        (n + PUSH_OFF_PREFIXES.len()).to_string(),
    )];
    for (i, prefix) in PUSH_OFF_PREFIXES.iter().enumerate() {
        env.push((
            format!("GIT_CONFIG_KEY_{}", n + i),
            format!("url.{PUSH_OFF_BASE}.pushInsteadOf"),
        ));
        env.push((format!("GIT_CONFIG_VALUE_{}", n + i), (*prefix).to_string()));
    }
    env
}

/// The empty `GH_CONFIG_DIR` of a run's workers: `runs/<id>/gh-config`.
pub fn gh_config_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("gh-config")
}

/// The hook's command line: `exe` (shell-quoted only when it must be) and
/// `hook`, so it ends in `factory hook` for an installed binary.
pub fn hook_command(exe: &Path) -> String {
    format!("{} hook", paths::shell_word(&exe.to_string_lossy()))
}

/// The `--settings` document: [`DENY_RULES`] and the PreToolUse hook on
/// `Bash` and `Read` (slice 13 task 2).
pub fn settings(exe: &Path) -> serde_json::Value {
    serde_json::json!({
        "permissions": { "deny": DENY_RULES },
        "hooks": {
            "PreToolUse": [{
                "matcher": "Bash|Read",
                "hooks": [{ "type": "command", "command": hook_command(exe), "timeout": 5 }]
            }]
        }
    })
}

/// `runs/<id>/workers/<session>.settings.json`.
pub fn settings_path(run_dir: &Path, session_id: &str) -> PathBuf {
    dir(run_dir).join(format!("{session_id}.settings.json"))
}

/// What a worker is spawned for: a slice (FR-2), or a checkpoint's red gate
/// or green transcript (FR-4). The role is the `workers.role` column and
/// the config key under `[workers]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Build {
        slice: u32,
    },
    Fix {
        checkpoint: u32,
        attempt: u32,
    },
    Checkpoint {
        n: u32,
        attempt: u32,
    },
    /// The closeout's polish worker (FR-6); `attempt` counts its spawns.
    Polish {
        attempt: u32,
    },
    /// A CI fix worker (FR-6): role `fix` with no checkpoint.
    CiFix {
        round: u32,
        attempt: u32,
    },
}

impl Job {
    pub fn role(&self) -> &'static str {
        match self {
            Job::Build { .. } => "build",
            Job::Fix { .. } | Job::CiFix { .. } => "fix",
            Job::Checkpoint { .. } => "checkpoint",
            Job::Polish { .. } => "polish",
        }
    }

    pub fn slice(&self) -> Option<u32> {
        match self {
            Job::Build { slice } => Some(*slice),
            _ => None,
        }
    }

    pub fn checkpoint(&self) -> Option<u32> {
        match self {
            Job::Build { .. } | Job::Polish { .. } | Job::CiFix { .. } => None,
            Job::Fix { checkpoint, .. } => Some(*checkpoint),
            Job::Checkpoint { n, .. } => Some(*n),
        }
    }
}

/// A running worker; `wait` or a `try_wait` that returns `Some` turns it
/// into an [`Exit`].
pub struct Worker {
    pub job: Job,
    /// The slice of a build worker; `None` for a fix or checkpoint worker.
    pub slice: Option<u32>,
    pub role: String,
    pub session_id: String,
    pub pid: u32,
    pub row_id: i64,
    pub stream_path: PathBuf,
    pub stderr_path: PathBuf,
    child: Child,
}

/// How a worker ended: the process exit and what its stream amounted to.
#[derive(Debug)]
pub struct Exit {
    /// The exit code, or `128 + signal` when a signal ended it.
    pub code: Option<i32>,
    pub summary: Summary,
}

/// `runs/<id>/workers`.
pub fn dir(run_dir: &Path) -> PathBuf {
    run_dir.join(DIR)
}

/// `runs/<id>/workers/<session>.jsonl`.
pub fn stream_path(run_dir: &Path, session_id: &str) -> PathBuf {
    dir(run_dir).join(format!("{session_id}.jsonl"))
}

impl Worker {
    /// Spawn `claude -p` for `job` with FR-2's flags and `env` on top of the
    /// driver's environment, record it in the run. Every path on its command
    /// line is absolute, so a relative data dir resolves against the
    /// driver's cwd and not the repo the worker runs in.
    pub fn spawn(
        run: &mut Run,
        job: Job,
        pack: &Pack,
        cfg: &config::Worker,
        env: &[(String, String)],
    ) -> Result<Worker> {
        let role = job.role();
        let slice = job.slice();
        let session_id = uuid_v4()?;
        let run_dir = std::path::absolute(run.dir())
            .map_err(io(format!("resolve {}", run.dir().display())))?;
        let wdir = dir(&run_dir);
        paths::private_dir_all(&wdir).map_err(io(format!("mkdir {}", wdir.display())))?;
        let gh_config = gh_config_dir(&run_dir);
        paths::private_dir_all(&gh_config).map_err(io(format!("mkdir {}", gh_config.display())))?;
        let stream_path = stream_path(&run_dir, &session_id);
        let stderr_path = wdir.join(format!("{session_id}.stderr"));
        let stream_file = private_file(&stream_path)?;
        let stderr_file = private_file(&stderr_path)?;
        let exe = paths::self_exe().map_err(io("locate the factory binary"))?;
        let settings_file = settings_path(&run_dir, &session_id);
        private_file(&settings_file)?
            .write_all(format!("{:#}\n", settings(&exe)).as_bytes())
            .map_err(io(format!("write {}", settings_file.display())))?;
        let pack_path = std::path::absolute(&pack.path)
            .map_err(io(format!("resolve {}", pack.path.display())))?;
        let repo = run.state().repo.clone();

        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg(&pack.prompt)
            .args(["--output-format", "stream-json", "--verbose"])
            .args(["--permission-mode", "bypassPermissions"])
            .arg("--append-system-prompt-file")
            .arg(&pack_path)
            .args(["--session-id", &session_id])
            .args(["--model", &cfg.model])
            .args(["--effort", &cfg.effort])
            .arg("--settings")
            .arg(&settings_file)
            .current_dir(&repo)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stream_file))
            .stderr(Stdio::from(stderr_file));
        for (k, _) in std::env::vars_os() {
            let k = k.to_string_lossy();
            if scrubbed(&k) {
                cmd.env_remove(&*k);
            }
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        // After the lane's env: no lane hands a worker a GitHub token.
        for k in GH_TOKEN_VARS {
            cmd.env_remove(k);
        }
        cmd.env("GH_CONFIG_DIR", &gh_config);
        // After the lane's env too: the count a lane sets wins over the
        // driver's, and either way its entries stay and ours go after them.
        let inherited = env
            .iter()
            .rev()
            .find(|(k, _)| k == GIT_CONFIG_COUNT)
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var(GIT_CONFIG_COUNT).ok());
        cmd.envs(push_off_env(inherited.as_deref()));
        cmd.env(crate::cli::hook::PACK_ENV, &pack_path);
        cmd.env(REPO_ENV, &repo);
        let mut child = cmd.spawn().map_err(|e| Error::Tool {
            cmd: format!("claude -p … --session-id {session_id}"),
            detail: format!("could not start claude: {e}"),
        })?;
        let pid = child.id();
        let recorded = record(run, job, pack, cfg, &session_id, pid);
        let row_id = reaped_on_err(&mut child, recorded)?;
        Ok(Worker {
            job,
            slice,
            role: role.to_string(),
            session_id,
            pid,
            row_id,
            stream_path,
            stderr_path,
            child,
        })
    }

    /// Block until the process exits, then read its stream.
    pub fn wait(mut self) -> Result<Exit> {
        let status = self
            .child
            .wait()
            .map_err(io(format!("wait for worker {}", self.session_id)))?;
        Ok(self.exit(status))
    }

    /// SIGKILL the process; [`Worker::wait`] then reaps it. Best effort: a
    /// process that already exited is not an error.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }

    /// Has the process exited? `Some` reads the stream and settles the
    /// worker; `None` leaves it running. The scheduler polls its table with
    /// this between board reads.
    pub fn try_wait(&mut self) -> Result<Option<Exit>> {
        match self
            .child
            .try_wait()
            .map_err(io(format!("poll worker {}", self.session_id)))?
        {
            Some(status) => Ok(Some(self.exit(status))),
            None => Ok(None),
        }
    }

    /// The process is gone: its code and the stream file it wrote. A
    /// process it left behind holding the file open does not stall this.
    fn exit(&mut self, status: std::process::ExitStatus) -> Exit {
        let code = status.code().or_else(|| status.signal().map(|s| 128 + s));
        Exit {
            code,
            summary: summary_at(&self.stream_path),
        }
    }
}

/// Open the worker's row, its pack files, its `state.json` entry and its
/// spawn event; the row id.
fn record(
    run: &mut Run,
    job: Job,
    pack: &Pack,
    cfg: &config::Worker,
    session_id: &str,
    pid: u32,
) -> Result<i64> {
    let role = job.role();
    let slice = job.slice();
    let started_at = crate::clock::now_rfc3339();
    let row_id = run.db().insert_worker(
        run.id().as_str(),
        role,
        slice,
        job.checkpoint(),
        session_id,
        &cfg.model,
        &cfg.effort,
        &started_at,
    )?;
    run.db().set_worker_pack_files(row_id, &pack.files)?;
    run.add_worker(WorkerRef {
        slice,
        checkpoint: job.checkpoint(),
        role: role.to_string(),
        session_id: session_id.to_string(),
        pid,
        row_id,
        spawned_at: started_at,
        outcome: None,
    })?;
    let session_id = session_id.to_string();
    run.emit(match job {
        Job::Build { slice } => EventKind::SliceSpawned {
            slice,
            session_id,
            pid,
        },
        Job::Fix {
            checkpoint,
            attempt,
        } => EventKind::FixSpawned {
            n: checkpoint,
            attempt,
            session_id,
            pid,
        },
        Job::Checkpoint { n, attempt } => EventKind::CheckpointSpawned {
            n,
            attempt,
            session_id,
            pid,
        },
        Job::Polish { attempt } => EventKind::PolishSpawned {
            attempt,
            session_id,
            pid,
        },
        Job::CiFix { round, attempt } => EventKind::CiFixSpawned {
            round,
            attempt,
            session_id,
            pid,
        },
    })?;
    Ok(row_id)
}

/// `recorded` as is, but when it is an error the child is killed and
/// reaped first: no worker may run that the run has no record of.
fn reaped_on_err<T>(child: &mut Child, recorded: Result<T>) -> Result<T> {
    if recorded.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    recorded
}

/// Create (or truncate) `path` readable by the owner only.
fn private_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(io(format!("create {}", path.display())))
}

/// The summary of the stream at `path`. No file → dead.
fn summary_at(path: &Path) -> Summary {
    match fs::read_to_string(path) {
        Ok(text) => crate::stream::parse(&text).1,
        Err(_) => Summary::dead(),
    }
}

/// The stream a worker left on disk, for a worker this process did not wait
/// on (a resume after the driver died). No file → dead.
pub fn summary_from_disk(run_dir: &Path, session_id: &str) -> Summary {
    summary_at(&stream_path(run_dir, session_id))
}

/// A random v4 UUID from `/dev/urandom` (Linux is the target, §19).
pub fn uuid_v4() -> Result<String> {
    let mut bytes = [0u8; 16];
    let mut f = OpenOptions::new()
        .read(true)
        .open("/dev/urandom")
        .map_err(io("open /dev/urandom"))?;
    std::io::Read::read_exact(&mut f, &mut bytes).map_err(io("read /dev/urandom"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let h = hex.join("");
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    ))
}

/// `xxxxxxxx-xxxx-4xxx-[89ab]xxx-xxxxxxxxxxxx`.
pub fn is_uuid_v4(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    for (i, c) in b.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if *c != b'-' {
                    return false;
                }
            }
            14 => {
                if *c != b'4' {
                    return false;
                }
            }
            19 => {
                if !matches!(c, b'8' | b'9' | b'a' | b'b') {
                    return false;
                }
            }
            _ => {
                if !c.is_ascii_hexdigit() || c.is_ascii_uppercase() {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_v4_and_distinct() {
        let a = uuid_v4().unwrap();
        let b = uuid_v4().unwrap();
        assert!(is_uuid_v4(&a), "{a}");
        assert!(is_uuid_v4(&b), "{b}");
        assert_ne!(a, b);
        assert!(!is_uuid_v4("03e5f0d5-2e60-18d6-9f03-cfbc491cab0a"));
        assert!(!is_uuid_v4("not-a-uuid"));
    }

    #[test]
    fn a_spawn_that_cannot_be_recorded_leaves_no_process() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let failed: Result<i64> = Err(Error::Msg("database is locked".into()));
        assert!(reaped_on_err(&mut child, failed).is_err());
        let status = child.try_wait().unwrap().expect("killed and reaped");
        assert_eq!(status.signal(), Some(libc::SIGKILL));

        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        assert_eq!(reaped_on_err(&mut child, Ok(7)).unwrap(), 7);
        assert!(
            child.try_wait().unwrap().is_none(),
            "a recorded worker runs on"
        );
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn the_push_off_env_goes_after_the_inherited_git_config() {
        let key = format!("url.{PUSH_OFF_BASE}.pushInsteadOf");
        let ours = |from: usize| -> Vec<(String, String)> {
            let mut v = vec![(GIT_CONFIG_COUNT.to_string(), (from + 3).to_string())];
            for (i, p) in ["git@", "ssh://", "https://"].iter().enumerate() {
                v.push((format!("GIT_CONFIG_KEY_{}", from + i), key.clone()));
                v.push((format!("GIT_CONFIG_VALUE_{}", from + i), p.to_string()));
            }
            v
        };
        assert_eq!(push_off_env(None), ours(0));
        assert_eq!(push_off_env(Some("0")), ours(0));
        assert_eq!(
            push_off_env(Some("")),
            ours(0),
            "git reads an empty count as 0"
        );
        assert_eq!(push_off_env(Some("2")), ours(2));
        assert_eq!(push_off_env(Some("bogus")), ours(0));
    }

    #[test]
    fn the_session_loses_its_claude_vars_but_not_the_account() {
        for gone in [
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_SSE_PORT",
            "CLAUDE_PROJECT_DIR",
        ] {
            assert!(scrubbed(gone), "{gone}");
        }
        for kept in KEEP_CLAUDE_VARS.iter().copied().chain([
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "PATH",
            "SSH_AUTH_SOCK",
        ]) {
            assert!(!scrubbed(kept), "{kept}");
        }
    }
}
