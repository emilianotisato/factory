//! `factory wait <run> [--timeout S] [--replay]` (PRD §9 FR-7): block until
//! the next event the owner must see, print it as one line and exit 0; at
//! the timeout print `still running: <status line>` and exit 3; a dead
//! driver exits 4 with the command that resumes it. A new cockpit reattaches
//! by calling this: everything after `runs/<id>/cockpit.cursor` is replayed,
//! and `--replay` ignores the cursor. A question still open or a park still
//! standing is printed again on every call, before anything new, even when
//! the cursor is past it: the cockpit that was shown it may have died before
//! the owner saw it (AD-40005). The tail is a 2-second poll of
//! `events.jsonl` (AD-1001): no watcher crate for a file the driver appends
//! to a few times an hour. An `events.jsonl` line that does not parse is
//! skipped, and one line on stderr says which (DEC-3).
use std::io::Write;
use std::time::{Duration, Instant};

use clap::Args;

use crate::error::io;
use crate::events::{self, Event};
use crate::{Result, cockpit, paths, run};

#[derive(Args, Debug, Clone)]
pub struct WaitArgs {
    /// The run id (`001-20261009-1a2b`)
    pub run: String,
    /// Give up after this many seconds: `still running: <status>`, exit 3
    #[arg(long, value_name = "S")]
    pub timeout: Option<u64>,
    /// Ignore the cursor: print every cockpit event from the first
    #[arg(long)]
    pub replay: bool,
}

/// How often the tail re-reads `events.jsonl`.
pub const POLL: Duration = Duration::from_secs(2);
/// How long `wait` lets the driver finish a write before it believes a dead
/// pid or an ended run with no last event, and how many times.
const SETTLE: Duration = Duration::from_millis(100);
const SETTLE_REREADS: u32 = 20;

pub fn wait(args: &WaitArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let data_dir = paths::data_dir();
    let Some(run) = cockpit::resolve(&data_dir, &args.run, out)? else {
        return Ok(1);
    };
    let dir = run.dir().to_path_buf();
    let feature = run.state().feature.clone();
    drop(run);
    let cursor = if args.replay {
        0
    } else {
        cockpit::read_cursor(&dir)
    };
    let deadline = args
        .timeout
        .map(|s| Instant::now() + Duration::from_secs(s));
    // The driver saves `state.json` before it appends the event that says
    // why, and exits after both: a state read just before a write can be
    // stale. A dead driver and an ended run without its last event are each
    // read again before they are believed (AD-1005).
    let mut rereads = 0u32;
    let mut noted = false;
    loop {
        let state = run::load_state(&dir)?;
        let (events, skipped) = events::scan(&dir)?;
        if !noted && let Some(note) = events::skip_note(&dir, &skipped) {
            writeln!(err, "{note}").map_err(io("stderr"))?;
            noted = true;
        }
        let owed = cockpit::owed(&state, &events, cursor);
        let fresh: Vec<&Event> = events
            .iter()
            .filter(|e| e.seq > cursor && cockpit::is_cockpit(&e.kind))
            .collect();
        if let Some(last) = fresh.last() {
            print(out, owed.iter().chain(&fresh), &feature)?;
            cockpit::write_cursor(&dir, last.seq)?;
            return Ok(0);
        }
        if state.phase.is_over() {
            // Already shown once: say it again so a reattached cockpit knows
            // where the run stands, and never block on a run that is over.
            let end = events.iter().rev().find(|e| cockpit::is_terminal(&e.kind));
            if end.is_none() && rereads < SETTLE_REREADS {
                rereads += 1;
                std::thread::sleep(SETTLE);
                continue;
            }
            let last = end
                .and_then(|e| cockpit::line(e, &feature))
                .unwrap_or_else(|| format!("{}: no event recorded", state.phase));
            writeln!(out, "{last}").map_err(io("stdout"))?;
            return Ok(0);
        }
        if cockpit::driver_dead(&state, &events) {
            if rereads == 0 {
                rereads += 1;
                std::thread::sleep(SETTLE);
                continue;
            }
            writeln!(out, "driver dead; run: factory run {feature}").map_err(io("stdout"))?;
            return Ok(4);
        }
        rereads = 0;
        if !owed.is_empty() {
            print(out, owed.iter(), &feature)?;
            return Ok(0);
        }
        let now = Instant::now();
        if let Some(d) = deadline
            && now >= d
        {
            writeln!(
                out,
                "still running: {}",
                cockpit::status_line(&state, &events)
            )
            .map_err(io("stdout"))?;
            return Ok(3);
        }
        let nap = deadline.map_or(POLL, |d| POLL.min(d.saturating_duration_since(now)));
        std::thread::sleep(nap);
    }
}

/// One line per cockpit event, in the order given.
fn print<'a>(
    out: &mut dyn Write,
    events: impl Iterator<Item = &'a &'a Event>,
    feature: &str,
) -> Result<()> {
    for e in events {
        if let Some(l) = cockpit::line(e, feature) {
            writeln!(out, "{l}").map_err(io("stdout"))?;
        }
    }
    Ok(())
}
