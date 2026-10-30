//! A 2-second poll, not a watcher crate: the driver appends to `events.jsonl` a few times an hour.
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

pub const POLL: Duration = Duration::from_secs(2);
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
    // The driver saves `state.json` before the event that says why, so each is read again before it is believed.
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
            // Said again so a reattached cockpit knows where the run stands; never block on a run that is over.
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
            writeln!(out, "{}", cockpit::driver_dead_line(&feature)).map_err(io("stdout"))?;
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
