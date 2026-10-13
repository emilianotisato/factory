//! `factory retro <run>` (PRD-003 FR-25): write the `## Numbers` section of
//! a stored run into its feature's `retro.md`, between the markers the
//! driver owns, after a `--recount` or a schema fix. The ship step writes
//! the same section; a second call on the same store changes nothing.
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::db::Db;
use crate::error::io;
use crate::run::Run;
use crate::{Error, Result, paths, retro};

#[derive(Args, Debug, Clone)]
pub struct RetroArgs {
    /// The run id (`001-20261009-1a2b`)
    pub run: String,
    /// Write into this `retro.md` instead of the run's feature folder
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
}

pub fn retro(args: &RetroArgs, out: &mut dyn Write) -> Result<i32> {
    let db = Db::open(&Run::db_path(&paths::data_dir()))?;
    let row = db
        .run_row(&args.run)?
        .ok_or_else(|| Error::Msg(format!("no run {} in the store", args.run)))?;
    let path = match &args.file {
        Some(p) => p.clone(),
        None => retro::feature_dir(Path::new(&row.repo), &row.feature)?.join("retro.md"),
    };
    if !path.is_file() {
        writeln!(
            out,
            "retro: {} does not exist — the ship step creates it",
            path.display()
        )
        .map_err(io("stdout"))?;
        return Ok(1);
    }
    let changed = retro::write(&path, &retro::numbers(&db, &args.run)?)?;
    writeln!(
        out,
        "retro: {} {}",
        path.display(),
        if changed { "updated" } else { "unchanged" }
    )
    .map_err(io("stdout"))?;
    Ok(0)
}
