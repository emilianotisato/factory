//! `factory stop <run>` (PRD §3 FR-1, §9 FR-7): a request. The loop (slice
//! 03) checks `stop_requested` between spawns and finishes in-flight workers
//! before exiting with phase `finished`. With no live process there is no
//! loop to honour it, so `stop` sets `finished` itself and says so.
use std::io::Write;

use clap::Args;

use crate::error::io;
use crate::events::EventKind;
use crate::run::{Phase, Run, pid_alive};
use crate::{Result, paths};

#[derive(Args, Debug, Clone)]
pub struct StopArgs {
    /// The run id (`001-20261009-1a2b`)
    pub run: String,
}

pub fn stop(args: &StopArgs, out: &mut dyn Write) -> Result<i32> {
    let mut run = Run::open(&paths::data_dir(), &args.run)?;
    if run.state().phase.is_over() {
        writeln!(out, "run {} is already {}", run.id(), run.state().phase).map_err(io("stdout"))?;
        return Ok(0);
    }
    run.request_stop()?;
    run.emit(EventKind::StopRequested)?;
    let pid = run.state().pid;
    if pid_alive(pid) {
        writeln!(
            out,
            "stop requested for run {} (pid {pid}): the loop finishes its in-flight workers, then exits",
            run.id()
        )
        .map_err(io("stdout"))?;
    } else {
        run.set_phase(Phase::Finished)?;
        writeln!(
            out,
            "run {} had no live process (pid {pid}): marked finished",
            run.id()
        )
        .map_err(io("stdout"))?;
    }
    Ok(0)
}
