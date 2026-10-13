//! `factory upgrade` (PRD-004 FR-8): replaces a binary the release installer
//! put down with the latest GitHub release, through `axoupdater` and the
//! receipt the installer wrote. `--check` only prints. A binary with no
//! receipt, or a receipt for another install dir, came from source: it
//! prints the line that rebuilds it and exits 0.
//!
//! Doctor's `ahead` hint follows the same rule, through [`from_release`] and
//! [`source_line`].
use std::io::Write;

use axoupdater::{AxoUpdater, Version};
use clap::Args;

use crate::config::{self, Config};
use crate::error::io;
use crate::{Error, Result, paths};

pub const INSTALLER: &str = "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/emilianotisato/factory/releases/latest/download/factory-installer.sh | sh";

#[derive(Args, Debug, Clone)]
pub struct UpgradeArgs {
    /// Print the newer version without installing it
    #[arg(long)]
    pub check: bool,
}

/// The updater with this binary's receipt loaded, `None` when the binary was
/// not installed from a release.
fn updater() -> Option<AxoUpdater> {
    let mut u = AxoUpdater::new_for("factory");
    u.load_receipt().ok()?;
    if !u.check_receipt_is_for_this_executable().unwrap_or(false) {
        return None;
    }
    Some(u)
}

/// True when the release installer put this binary down.
pub fn from_release() -> bool {
    updater().is_some()
}

/// The command that rebuilds a from-source binary: `cargo install --path`
/// on `[source] path`, else the installer.
pub fn source_line(cfg: &Config) -> String {
    match &cfg.source {
        Some(s) => format!("cargo install --path {}", s.path.display()),
        None => INSTALLER.to_string(),
    }
}

/// What doctor's `ahead` line points at. A config that does not load reads
/// as no `[source]`: doctor reports, it does not fail on the owner's file.
pub fn ahead_hint() -> String {
    if from_release() {
        return "factory upgrade".into();
    }
    let cfg = config::load(&paths::config_dir())
        .map(|l| l.config)
        .unwrap_or_default();
    source_line(&cfg)
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(io("stdout"))
}

fn failed(e: axoupdater::AxoupdateError) -> Error {
    Error::Msg(e.to_string())
}

pub fn upgrade(args: &UpgradeArgs, out: &mut dyn Write) -> Result<i32> {
    let Some(mut u) = updater() else {
        let cfg = config::load(&paths::config_dir())?.config;
        say(
            out,
            &format!("installed from source — {}", source_line(&cfg)),
        )?;
        return Ok(0);
    };
    // The running binary is the truth, not the version the receipt recorded.
    let current: Version = env!("CARGO_PKG_VERSION")
        .parse()
        .map_err(|e| Error::Msg(format!("version: {e}")))?;
    u.set_current_version(current.clone()).map_err(failed)?;

    if args.check {
        // `query_new_version` has no sync variant; a runtime here keeps every
        // other command synchronous.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(io("tokio runtime"))?;
        let newer = rt
            .block_on(u.query_new_version())
            .map_err(failed)?
            .filter(|v| **v > current)
            .cloned();
        match newer {
            Some(v) => say(out, &format!("{current} → {v}"))?,
            None => say(out, "already current")?,
        }
        return Ok(0);
    }

    u.disable_installer_output();
    match u.run_sync().map_err(failed)? {
        Some(r) => {
            let old = r.old_version.unwrap_or(current);
            say(out, &format!("{old} → {}", r.new_version))?;
        }
        None => say(out, "already current")?,
    }
    Ok(0)
}
