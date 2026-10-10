//! `factory stop <run>` (PRD §3 FR-1, §9 FR-7): a request. The loop checks
//! `stop_requested` between spawns, lets the in-flight workers finish and
//! commits their done slices, then ends the run with `run_stopped`, phase
//! `finished` and `stopped: true` (`scheduler::mark_stopped`). This command
//! says how many workers are still in flight; asked again it says how many
//! remain. With no live process there is no loop to honour it, so `stop`
//! marks the run stopped itself. There is no kill mode: a kill is `kill` on
//! the process group, and `factory run NNN` resumes what it left.
use std::io::Write;

use clap::Args;

use crate::error::io;
use crate::events::EventKind;
use crate::run::{Run, pid_alive};
use crate::{Result, cockpit, paths, scheduler};

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
    let again = run.state().stop_requested;
    run.request_stop()?;
    run.emit(EventKind::StopRequested)?;
    let pid = run.state().pid;
    if pid_alive(pid) {
        writeln!(
            out,
            "stop {}requested for run {} (pid {pid}): {}",
            if again { "already " } else { "" },
            run.id(),
            in_flight_text(cockpit::in_flight(run.state()))
        )
        .map_err(io("stdout"))?;
    } else {
        let slices = scheduler::mark_stopped(&mut run)?;
        writeln!(
            out,
            "run {} had no live process (pid {pid}): marked finished ({slices} slice(s) done; factory run {} resumes)",
            run.id(),
            run.state().feature
        )
        .map_err(io("stdout"))?;
    }
    Ok(0)
}

fn in_flight_text(n: usize) -> String {
    match n {
        0 => "no workers in flight; the run finishes on its next tick".to_string(),
        1 => "1 worker in flight; the run finishes when it exits".to_string(),
        n => format!("{n} workers in flight; the run finishes when they exit"),
    }
}
