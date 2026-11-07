//! `factory live <id> --json`: one run's live detail, read without writing anything.
use std::io::Write;
use std::path::{Component, Path};
use std::time::{Duration, Instant, SystemTime};

use clap::Args;

use crate::error::io;
use crate::live::{self, Ctx, Opts};
use crate::run::{Run, STATE_FILE};
use crate::{Result, events, paths};

#[derive(Args, Debug, Clone)]
pub struct LiveArgs {
    /// The run id (`001-20261009-1a2b`)
    pub run: String,
    /// Print one JSON object; required, there is no human mode
    #[arg(long, required = true)]
    pub json: bool,
    /// Only events after this seq reach the timeline; without it, the last 100
    #[arg(long)]
    pub since: Option<u64>,
    /// Wait up to this many seconds for something visible to change; a timeout exits 0
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u64).range(0..=25))]
    pub wait: u64,
    /// The most evidence files listed
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u64).range(0..=300))]
    pub evidence_limit: u64,
}

/// A full reread this often while waiting.
const REREAD: Duration = Duration::from_secs(1);

/// The run's own files are checked this often, so an appended event ends a wait at once.
const POLL: Duration = Duration::from_millis(100);

pub fn live(args: &LiveArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let dir = Run::runs_dir(&paths::data_dir_path()).join(&args.run);
    let mut parts = Path::new(&args.run).components();
    let one_name = matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none();
    if !one_name || !dir.join(STATE_FILE).is_file() {
        writeln!(err, "no run {}", args.run).map_err(io("stderr"))?;
        return Ok(1);
    }
    let opts = Opts {
        since: args.since,
        evidence_limit: args.evidence_limit as usize,
    };
    let ctx = Ctx::read(&dir)?;
    if let Some(note) = events::skip_note(&dir, &ctx.skipped) {
        writeln!(err, "{note}").map_err(io("stderr"))?;
    }
    let mut snap = live::snapshot(&ctx, &opts);
    let first = snap["rev"].clone();
    let deadline = Instant::now() + Duration::from_secs(args.wait);
    let mut seen = stamp(&dir);
    let mut read_at = Instant::now();
    while let Some(left) = deadline.checked_duration_since(Instant::now())
        && !left.is_zero()
    {
        std::thread::sleep(left.min(POLL));
        let now = stamp(&dir);
        if now == seen && read_at.elapsed() < REREAD {
            continue;
        }
        seen = now;
        read_at = Instant::now();
        snap = live::snapshot(&Ctx::read(&dir)?, &opts);
        if snap["rev"] != first {
            break;
        }
    }
    writeln!(out, "{}", live::render(&snap)).map_err(io("stdout"))?;
    Ok(0)
}

type Stamp = [Option<(u64, SystemTime)>; 2];

fn stamp(dir: &Path) -> Stamp {
    [events::FILE, STATE_FILE].map(|f| {
        let m = std::fs::metadata(dir.join(f)).ok()?;
        Some((m.len(), m.modified().ok()?))
    })
}
