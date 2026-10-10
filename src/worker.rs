//! One worker: `claude -p` with a context pack (PRD §4 FR-2) — a build
//! worker per slice, a fix or checkpoint worker per checkpoint (FR-4). The stream is
//! tee'd line by line into `runs/<id>/workers/<session>.jsonl` and into the
//! parser, stderr goes to `<session>.stderr`, the pid lands in `state.json`
//! and a `workers` row is opened at spawn and closed at exit.
//!
//! A worker inherits the driver's session and process group (the daemon's
//! `setsid`), so one group signal reaches the driver and its worker together.
//! It does not inherit `CLAUDECODE` or any `CLAUDE_*` variable: a driver
//! started from inside a Claude Code session would otherwise hand its workers
//! the nested-session guard. It does get its lane (FR-3): `FACTORY_LANE=N`
//! and whatever `env` the lane file declares.
//!
//! Every worker also gets `--settings <file>` (slice 13, §4 FR-2 as
//! corrected 2026-10-09): the two permission deny rules and the PreToolUse
//! hook `<this binary> hook`, written per worker to
//! `runs/<id>/workers/<session>.settings.json` — a file the driver owns, so
//! nothing is installed under the owner's `~/.claude/` or the project's
//! `.claude/`. `FACTORY_PACK=<pack>` tells the hook which files the pack holds.
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;

use crate::config;
use crate::error::io;
use crate::events::EventKind;
use crate::pack::Pack;
use crate::run::{Run, WorkerRef};
use crate::stream::{Collector, Summary};
use crate::{Error, Result};

pub const DIR: &str = "workers";

/// The permission deny rules every worker carries (§4 FR-2): no merge, no
/// force push, whatever the project's own settings say.
pub const DENY_RULES: [&str; 3] = [
    "Bash(gh pr merge*)",
    "Bash(git push --force*)",
    "Bash(git push -f*)",
];

/// The hook's command line: `exe` (shell-quoted only when it must be) and
/// `hook`, so it ends in `factory hook` for an installed binary.
pub fn hook_command(exe: &Path) -> String {
    let s = exe.to_string_lossy();
    let plain = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+:@%,".contains(c));
    if plain && !s.is_empty() {
        format!("{s} hook")
    } else {
        format!("'{}' hook", s.replace('\'', "'\\''"))
    }
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
    tee: Option<JoinHandle<Summary>>,
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
    /// driver's environment, record it in the run.
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
        let wdir = dir(run.dir());
        fs::create_dir_all(&wdir).map_err(io(format!("mkdir {}", wdir.display())))?;
        let stream_path = stream_path(run.dir(), &session_id);
        let stderr_path = wdir.join(format!("{session_id}.stderr"));
        let stream_file =
            File::create(&stream_path).map_err(io(format!("create {}", stream_path.display())))?;
        let stderr_file =
            File::create(&stderr_path).map_err(io(format!("create {}", stderr_path.display())))?;
        let exe = std::env::current_exe().map_err(io("locate the factory binary"))?;
        let settings_file = settings_path(run.dir(), &session_id);
        fs::write(&settings_file, format!("{:#}\n", settings(&exe)))
            .map_err(io(format!("write {}", settings_file.display())))?;
        let pack_path = std::path::absolute(&pack.path)
            .map_err(io(format!("resolve {}", pack.path.display())))?;

        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg(&pack.prompt)
            .args(["--output-format", "stream-json", "--verbose"])
            .args(["--permission-mode", "bypassPermissions"])
            .arg("--append-system-prompt-file")
            .arg(&pack.path)
            .args(["--session-id", &session_id])
            .args(["--model", &cfg.model])
            .args(["--effort", &cfg.effort])
            .arg("--settings")
            .arg(&settings_file)
            .current_dir(&run.state().repo)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr_file));
        for (k, _) in std::env::vars_os() {
            let k = k.to_string_lossy();
            if k == "CLAUDECODE" || k.starts_with("CLAUDE_") {
                cmd.env_remove(&*k);
            }
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.env(crate::cli::hook::PACK_ENV, &pack_path);
        let mut child = cmd.spawn().map_err(|e| Error::Tool {
            cmd: format!("claude -p … --session-id {session_id}"),
            detail: format!("could not start claude: {e}"),
        })?;
        let pid = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Msg("worker stdout was not piped".into()))?;
        let tee = std::thread::spawn(move || tee(stdout, stream_file));

        let started_at = crate::clock::now_rfc3339();
        let row_id = run.db().insert_worker(
            run.id().as_str(),
            role,
            slice,
            job.checkpoint(),
            &session_id,
            &cfg.model,
            &cfg.effort,
            &started_at,
        )?;
        run.db().set_worker_pack_files(row_id, &pack.files)?;
        run.add_worker(WorkerRef {
            slice,
            checkpoint: job.checkpoint(),
            role: role.to_string(),
            session_id: session_id.clone(),
            pid,
            row_id,
            spawned_at: started_at,
            outcome: None,
        })?;
        run.emit(match job {
            Job::Build { slice } => EventKind::SliceSpawned {
                slice,
                session_id: session_id.clone(),
                pid,
            },
            Job::Fix {
                checkpoint,
                attempt,
            } => EventKind::FixSpawned {
                n: checkpoint,
                attempt,
                session_id: session_id.clone(),
                pid,
            },
            Job::Checkpoint { n, attempt } => EventKind::CheckpointSpawned {
                n,
                attempt,
                session_id: session_id.clone(),
                pid,
            },
            Job::Polish { attempt } => EventKind::PolishSpawned {
                attempt,
                session_id: session_id.clone(),
                pid,
            },
            Job::CiFix { round, attempt } => EventKind::CiFixSpawned {
                round,
                attempt,
                session_id: session_id.clone(),
                pid,
            },
        })?;
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
            tee: Some(tee),
        })
    }

    /// Block until the process exits and the stream is drained.
    pub fn wait(mut self) -> Result<Exit> {
        let status = self
            .child
            .wait()
            .map_err(io(format!("wait for worker {}", self.session_id)))?;
        Ok(self.exit(status))
    }

    /// Has the process exited? `Some` drains the stream and settles the
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

    fn exit(&mut self, status: std::process::ExitStatus) -> Exit {
        let code = status.code().or_else(|| status.signal().map(|s| 128 + s));
        let summary = match self.tee.take() {
            Some(t) => t.join().unwrap_or_else(|_| Summary::dead()),
            None => Summary::dead(),
        };
        Exit { code, summary }
    }
}

/// Copy every stdout line into the record file and the collector.
fn tee(stdout: std::process::ChildStdout, mut file: File) -> Summary {
    let mut collector = Collector::new();
    let reader = BufReader::new(stdout);
    for line in reader.split(b'\n') {
        let Ok(mut bytes) = line else { break };
        bytes.push(b'\n');
        let _ = file.write_all(&bytes);
        collector.push_line(&String::from_utf8_lossy(&bytes));
    }
    let _ = file.flush();
    collector.finish()
}

/// The stream a worker left on disk, for a worker this process did not wait
/// on (a resume after the driver died). No file → dead.
pub fn summary_from_disk(run_dir: &Path, session_id: &str) -> Summary {
    match fs::read_to_string(stream_path(run_dir, session_id)) {
        Ok(text) => crate::stream::parse(&text).1,
        Err(_) => Summary::dead(),
    }
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
}
