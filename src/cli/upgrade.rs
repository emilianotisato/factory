use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use axoupdater::{AxoUpdater, ReleaseSource, ReleaseSourceType, UpdateRequest, Version};
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
    /// Include prereleases (rolling edge builds on main)
    #[arg(long)]
    pub edge: bool,
}

pub fn install_prefix_from_exe(exe: &Path) -> PathBuf {
    let parent = exe.parent().unwrap_or(Path::new("/"));
    if parent.file_name() == Some(OsStr::new("bin")) {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

fn receipt_config_dirs() -> Vec<PathBuf> {
    if std::env::var("AXOUPDATER_CONFIG_WORKING_DIR").is_ok() {
        return vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
    }
    if let Ok(path) = std::env::var("AXOUPDATER_CONFIG_PATH") {
        return vec![PathBuf::from(path)];
    }
    let mut dirs = Vec::new();
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let factory = PathBuf::from(xdg).join("factory");
        if factory.is_dir() {
            dirs.push(factory);
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".config").join("factory"));
    }
    dirs
}

fn receipt_version_on_disk() -> Option<String> {
    for dir in receipt_config_dirs() {
        let path = dir.join("factory-receipt.json");
        if !path.is_file() {
            continue;
        }
        let body = fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&body).ok()?;
        return json
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
    }
    None
}

fn receipt_version_matches_running() -> bool {
    receipt_version_on_disk().as_deref() == Some(env!("CARGO_PKG_VERSION"))
}

fn updater() -> Option<AxoUpdater> {
    let mut u = AxoUpdater::new_for("factory");
    u.load_receipt().ok()?;
    if !u.check_receipt_is_for_this_executable().unwrap_or(false) {
        return None;
    }
    if !receipt_version_matches_running() {
        return None;
    }
    Some(u)
}

pub fn from_release() -> bool {
    updater().is_some()
}

pub fn source_line(cfg: &Config) -> String {
    match &cfg.source {
        Some(s) => format!("cargo install --path {}", s.path.display()),
        None => INSTALLER.to_string(),
    }
}

/// A config that does not load reads as no `[source]`: doctor reports, never fails on the owner's file.
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

fn configure_consumer_updater(u: &mut AxoUpdater) -> Result<()> {
    u.set_release_source(ReleaseSource {
        release_type: ReleaseSourceType::GitHub,
        owner: "emilianotisato".into(),
        name: "factory".into(),
        app_name: "factory".into(),
    });
    let install = install_prefix_from_exe(&paths::self_exe().map_err(io("self exe"))?);
    let install = install
        .to_str()
        .ok_or_else(|| Error::Msg("install dir".into()))?;
    u.set_install_dir(install);
    Ok(())
}

fn check_or_run(
    u: &mut AxoUpdater,
    current: Version,
    args: &UpgradeArgs,
    out: &mut dyn Write,
) -> Result<()> {
    if args.check {
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
        return Ok(());
    }

    u.disable_installer_output();
    match u.run_sync().map_err(failed)? {
        Some(r) => {
            let old = r.old_version.unwrap_or(current);
            say(out, &format!("{old} → {}", r.new_version))?;
        }
        None => say(out, "already current")?,
    }
    Ok(())
}

pub fn upgrade(args: &UpgradeArgs, out: &mut dyn Write) -> Result<i32> {
    let cfg = config::load(&paths::config_dir())
        .map(|l| l.config)
        .unwrap_or_default();
    let matching = updater();

    if cfg.source.is_some() && matching.is_none() && !args.edge {
        say(
            out,
            &format!("installed from source — {}", source_line(&cfg)),
        )?;
        return Ok(0);
    }

    let current: Version = env!("CARGO_PKG_VERSION")
        .parse()
        .map_err(|e| Error::Msg(format!("version: {e}")))?;

    let mut u = match matching {
        Some(mut u) => {
            u.set_current_version(current.clone()).map_err(failed)?;
            u
        }
        None => {
            let mut u = AxoUpdater::new_for("factory");
            configure_consumer_updater(&mut u)?;
            u.set_current_version(current.clone()).map_err(failed)?;
            u
        }
    };

    u.configure_version_specifier(if args.edge {
        UpdateRequest::LatestMaybePrerelease
    } else {
        UpdateRequest::Latest
    });
    check_or_run(&mut u, current, args, out)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::install_prefix_from_exe;
    use std::path::Path;

    #[test]
    fn install_prefix_strips_cargo_bin() {
        let exe = Path::new("/home/user/.cargo/bin/factory");
        assert_eq!(install_prefix_from_exe(exe), Path::new("/home/user/.cargo"));
    }

    #[test]
    fn install_prefix_strips_usr_local_bin() {
        let exe = Path::new("/usr/local/bin/factory");
        assert_eq!(install_prefix_from_exe(exe), Path::new("/usr/local"));
    }

    #[test]
    fn install_prefix_keeps_flat_install_dir() {
        let exe = Path::new("/opt/factory/factory");
        assert_eq!(install_prefix_from_exe(exe), Path::new("/opt/factory"));
    }
}
