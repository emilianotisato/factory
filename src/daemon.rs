//! The detached driver (PRD §3 FR-1: "`factory run NNN` is a detached
//! process … session death is not run death"). `factory run` re-executes
//! itself as `factory daemon <id>` in a new session (`setsid`), stdio on
//! `runs/<id>/driver.log`, and returns once the child has adopted the run.
//! Re-exec rather than `fork()`: the parent holds a SQLite connection and a
//! thread-capable runtime, neither of which survives a bare fork safely.
use std::fs::OpenOptions;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::io;
use crate::run::{self, Run};
use crate::{Error, Result};

pub const LOG: &str = "driver.log";

/// `runs/<id>/driver.log`.
pub fn log_path(run_dir: &Path) -> PathBuf {
    run_dir.join(LOG)
}

/// Start the daemon for `run` and wait until it owns the run (`state.json`
/// carries its pid). Returns that pid.
pub fn spawn(run: &Run) -> Result<u32> {
    let exe = std::env::current_exe().map_err(io("locate the factory binary"))?;
    let log = log_path(run.dir());
    let out = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&log)
        .map_err(io(format!("open {}", log.display())))?;
    let err = out
        .try_clone()
        .map_err(io(format!("dup {}", log.display())))?;
    let mut cmd = Command::new(exe);
    cmd.arg("daemon")
        .arg(run.id().as_str())
        .current_dir(&run.state().repo)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    // SAFETY: `setsid` is async-signal-safe and touches no memory of this
    // process; it only moves the child into a session of its own.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().map_err(|e| Error::Tool {
        cmd: format!("factory daemon {}", run.id()),
        detail: format!("could not start the driver: {e}"),
    })?;
    let pid = child.id();

    // Adoption is the hand-over: until `state.json` names the daemon's pid, a
    // second `factory run` would not see this run as live.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(s) = run::load_state(run.dir())
            && s.pid == pid
        {
            return Ok(pid);
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(Error::Tool {
                cmd: format!("factory daemon {}", run.id()),
                detail: format!(
                    "the driver exited ({status}) before taking the run over — see {}",
                    log.display()
                ),
            });
        }
        if Instant::now() > deadline {
            return Err(Error::Tool {
                cmd: format!("factory daemon {}", run.id()),
                detail: format!(
                    "the driver (pid {pid}) did not take the run over within 5 s — see {}",
                    log.display()
                ),
            });
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
