//! A worker shares the driver's process group (the daemon's `setsid`), so one group signal reaches both.
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

/// A prefix rule cannot see `true && gh …` or `git -c k=v push`; `cli::hook::guard` catches those.
pub const DENY_RULES: [&str; 3] = ["Bash(gh)", "Bash(gh *)", "Bash(git push*)"];

pub const GH_TOKEN_VARS: [&str; 4] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
];

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

/// A driver started inside a Claude Code session would otherwise hand its workers the nested-session guard.
pub fn scrubbed(name: &str) -> bool {
    if name == "CLAUDECODE" {
        return true;
    }
    name.starts_with("CLAUDE_")
        && !KEEP_CLAUDE_VARS.contains(&name)
        && !name.starts_with("CLAUDE_CODE_USE_")
}

pub const REPO_ENV: &str = "FACTORY_REPO";

/// Always the run's main tree: a lane worker writes that board, not its lane's copy.
pub const BOARD_ENV: &str = "FACTORY_BOARD";

/// The run dir's evidence folder: proof a worker saves stays on this machine, out of every commit.
pub const EVIDENCE_ENV: &str = "FACTORY_EVIDENCE";

pub const GIT_CONFIG_COUNT: &str = "GIT_CONFIG_COUNT";

/// A transport git has no `git-remote-*` helper for, so a push fails before any connection opens.
pub const PUSH_OFF_BASE: &str = "factory-no-push://";

pub const PUSH_OFF_PREFIXES: [&str; 5] = ["git@", "ssh://", "https://", "http://", "git://"];

/// git ignores `pushInsteadOf` for a remote with an explicit `pushurl`; the deny rules and hook cover that one.
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

pub fn gh_config_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("gh-config")
}

pub fn hook_command(exe: &Path) -> String {
    format!("{} hook", paths::shell_word(&exe.to_string_lossy()))
}

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

pub fn settings_path(run_dir: &Path, session_id: &str) -> PathBuf {
    dir(run_dir).join(format!("{session_id}.settings.json"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Build { slice: u32 },
    Fix { checkpoint: u32, attempt: u32 },
    Checkpoint { n: u32, attempt: u32 },
    Polish { attempt: u32 },
    CiFix { round: u32, attempt: u32 },
    Merge { slice: u32, attempt: u32 },
}

impl Job {
    pub fn role(&self) -> &'static str {
        match self {
            Job::Build { .. } => "build",
            Job::Merge { .. } => "merge",
            Job::Fix { .. } | Job::CiFix { .. } => "fix",
            Job::Checkpoint { .. } => "checkpoint",
            Job::Polish { .. } => "polish",
        }
    }

    pub fn slice(&self) -> Option<u32> {
        match self {
            Job::Build { slice } | Job::Merge { slice, .. } => Some(*slice),
            _ => None,
        }
    }

    pub fn checkpoint(&self) -> Option<u32> {
        match self {
            Job::Build { .. } | Job::Polish { .. } | Job::CiFix { .. } | Job::Merge { .. } => None,
            Job::Fix { checkpoint, .. } => Some(*checkpoint),
            Job::Checkpoint { n, .. } => Some(*n),
        }
    }
}

pub struct Worker {
    pub job: Job,
    pub slice: Option<u32>,
    pub role: String,
    pub session_id: String,
    pub pid: u32,
    pub row_id: i64,
    pub stream_path: PathBuf,
    pub stderr_path: PathBuf,
    child: Child,
}

#[derive(Debug)]
pub struct Exit {
    pub code: Option<i32>,
    pub summary: Summary,
}

pub fn dir(run_dir: &Path) -> PathBuf {
    run_dir.join(DIR)
}

pub fn stream_path(run_dir: &Path, session_id: &str) -> PathBuf {
    dir(run_dir).join(format!("{session_id}.jsonl"))
}

impl Worker {
    /// Every path on the command line is absolute: the worker's cwd is not the driver's.
    pub fn spawn(
        run: &mut Run,
        job: Job,
        pack: &Pack,
        cfg: &config::Worker,
        env: &[(String, String)],
        cwd: &Path,
        lane: Option<u32>,
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
        // Canonical: the hook keys its guards to `FACTORY_REPO`, the transcript slug to the cwd the worker sees.
        let cwd = std::fs::canonicalize(cwd).map_err(io(format!("resolve {}", cwd.display())))?;

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
            .current_dir(&cwd)
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
        // After the lane's env too: a lane's count wins, its entries stay, ours go after them.
        let inherited = env
            .iter()
            .rev()
            .find(|(k, _)| k == GIT_CONFIG_COUNT)
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var(GIT_CONFIG_COUNT).ok());
        cmd.envs(push_off_env(inherited.as_deref()));
        cmd.env(crate::cli::hook::PACK_ENV, &pack_path);
        cmd.env(REPO_ENV, &cwd);
        cmd.env(BOARD_ENV, &repo);
        let evidence = run_dir.join("evidence");
        let _ = std::fs::create_dir_all(&evidence);
        cmd.env(EVIDENCE_ENV, evidence);
        let mut child = cmd.spawn().map_err(|e| Error::Tool {
            cmd: format!("claude -p … --session-id {session_id}"),
            detail: format!("could not start claude: {e}"),
        })?;
        let pid = child.id();
        let recorded = record(run, job, pack, cfg, &session_id, pid, lane);
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

    pub fn wait(mut self) -> Result<Exit> {
        let status = self
            .child
            .wait()
            .map_err(io(format!("wait for worker {}", self.session_id)))?;
        Ok(self.exit(status))
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }

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

    fn exit(&mut self, status: std::process::ExitStatus) -> Exit {
        let code = status.code().or_else(|| status.signal().map(|s| 128 + s));
        Exit {
            code,
            summary: summary_at(&self.stream_path),
        }
    }
}

fn record(
    run: &mut Run,
    job: Job,
    pack: &Pack,
    cfg: &config::Worker,
    session_id: &str,
    pid: u32,
    lane: Option<u32>,
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
    run.db().set_worker_pack_parts(row_id, &pack.parts)?;
    run.add_worker(WorkerRef {
        slice,
        checkpoint: job.checkpoint(),
        lane,
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
        Job::Merge { slice, attempt } => EventKind::MergeSpawned {
            slice,
            attempt,
            session_id,
            pid,
        },
    })?;
    Ok(row_id)
}

/// No worker may run that the run has no record of.
fn reaped_on_err<T>(child: &mut Child, recorded: Result<T>) -> Result<T> {
    if recorded.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    recorded
}

fn private_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(io(format!("create {}", path.display())))
}

fn summary_at(path: &Path) -> Summary {
    match fs::read_to_string(path) {
        Ok(text) => crate::stream::parse(&text).1,
        Err(_) => Summary::dead(),
    }
}

pub fn summary_from_disk(run_dir: &Path, session_id: &str) -> Summary {
    summary_at(&stream_path(run_dir, session_id))
}

/// `/dev/urandom` over a crate: Linux is the only target.
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
            let mut v = vec![(GIT_CONFIG_COUNT.to_string(), (from + 5).to_string())];
            for (i, p) in ["git@", "ssh://", "https://", "http://", "git://"]
                .iter()
                .enumerate()
            {
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
