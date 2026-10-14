//! Prints the rule report and exits 1 on any `fix` hit, 2 when the scan could not run; the share is a figure, never a hit.
use std::io::Write;
use std::path::Path;

use clap::Args;

use crate::error::io;
use crate::leaks::{self, Input};
use crate::{Result, paths};

#[derive(Args, Debug, Clone)]
#[group(required = true, multiple = false)]
pub struct LeaksArgs {
    /// Read the lines and paths added since this ref (`<ref>...HEAD`)
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    /// Read the whole tree at HEAD
    #[arg(long)]
    pub all: bool,
}

pub fn leaks(args: &LeaksArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let input = match &args.base {
        Some(base) => Input::Base(base),
        None => Input::All,
    };
    let report = match paths::repo_root(Path::new(".")).and_then(|root| leaks::scan(&root, input)) {
        Ok(r) => r,
        Err(e) => {
            writeln!(err, "factory leaks: did not run — {e}").map_err(io("stderr"))?;
            return Ok(2);
        }
    };
    out.write_all(report.render().as_bytes())
        .map_err(io("stdout"))?;
    Ok(i32::from(report.fix_hits() > 0))
}
