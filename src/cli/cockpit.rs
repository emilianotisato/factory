use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use clap::Subcommand;
use serde_json::{Value, json};

use crate::error::{io, json};
use crate::mods::COCKPIT_FILES;
use crate::{Error, Result, paths};

pub const MARKETPLACE: &str = "factory";
pub const PLUGIN: &str = "factory-cockpit@factory";
const INSTALL: [&str; 5] = ["plugin", "install", PLUGIN, "--scope", "user"];

#[derive(Subcommand, Debug, Clone)]
pub enum CockpitVerb {
    /// Write the cockpit mod under the data dir and install it in Claude Code; re-run it to upgrade
    Install,
}

pub fn cockpit(verb: &CockpitVerb, out: &mut dyn Write) -> Result<i32> {
    let CockpitVerb::Install = verb;
    install(&paths::data_dir().join("mods"), out)
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(io("stdout"))
}

/// `dir` becomes a folder marketplace: Claude Code reads it in place, so a rewrite reaches a session on `/reload-plugins`.
pub fn install(dir: &Path, out: &mut dyn Write) -> Result<i32> {
    write_marketplace(dir)?;
    say(
        out,
        &format!("wrote {}", paths::shell_word(&dir.to_string_lossy())),
    )?;
    if register(dir, out)? {
        say(out, "run /reload-plugins in open sessions")?;
        return Ok(0);
    }
    say(out, "claude is not on PATH; run these by hand:")?;
    say(
        out,
        &format!(
            "  claude plugin marketplace add {}",
            paths::shell_word(&dir.to_string_lossy())
        ),
    )?;
    say(out, &format!("  claude {}", INSTALL.join(" ")))?;
    say(out, "then run /reload-plugins in open sessions")?;
    Ok(1)
}

fn write_marketplace(dir: &Path) -> Result<()> {
    let manifest = json!({
        "name": MARKETPLACE,
        "owner": { "name": "factory" },
        "plugins": [{ "name": "factory-cockpit", "source": "./cockpit" }],
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(json("marketplace.json"))?;
    write(&dir.join(".claude-plugin/marketplace.json"), &(text + "\n"))?;
    let mod_dir = dir.join("cockpit");
    match fs::remove_dir_all(&mod_dir) {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            return Err(io(mod_dir.display().to_string())(e));
        }
        _ => {}
    }
    for (rel, text) in COCKPIT_FILES {
        write(&mod_dir.join(rel), text)?;
    }
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io(parent.display().to_string()))?;
    }
    fs::write(path, text).map_err(io(path.display().to_string()))
}

/// `false` when `claude` is not on PATH.
fn register(dir: &Path, out: &mut dyn Write) -> Result<bool> {
    let list = ["plugin", "marketplace", "list", "--json"];
    let Some(markets) = claude(&list)? else {
        return Ok(false);
    };
    match find(&markets, &list, |m| m["name"] == MARKETPLACE)? {
        Some(m) if m["source"] == "directory" && same_dir(&m["path"], dir) => {
            say(out, &format!("marketplace {MARKETPLACE}: already added"))?;
        }
        Some(m) => {
            let at = m["path"].as_str().or(m["repo"].as_str()).unwrap_or("?");
            return Err(Error::Msg(format!(
                "marketplace {MARKETPLACE} is already added from {at}, not {}; \
                 run: claude plugin marketplace remove {MARKETPLACE}, then factory cockpit install",
                dir.display()
            )));
        }
        None => {
            let add = [
                OsStr::new("plugin"),
                OsStr::new("marketplace"),
                OsStr::new("add"),
                dir.as_os_str(),
            ];
            if claude(&add)?.is_none() {
                return Ok(false);
            }
            say(out, &format!("marketplace {MARKETPLACE}: added"))?;
        }
    }
    let list = ["plugin", "list", "--json"];
    let Some(plugins) = claude(&list)? else {
        return Ok(false);
    };
    match find(&plugins, &list, |p| {
        p["id"] == PLUGIN && p["scope"] == "user"
    })? {
        Some(p) if p["enabled"] == false => say(
            out,
            &format!("{PLUGIN}: installed but disabled; run: claude plugin enable {PLUGIN}"),
        )?,
        Some(_) => say(out, &format!("{PLUGIN}: already installed"))?,
        None => {
            if claude(&INSTALL)?.is_none() {
                return Ok(false);
            }
            say(out, &format!("{PLUGIN}: installed at user scope"))?;
        }
    }
    Ok(true)
}

fn same_dir(path: &Value, dir: &Path) -> bool {
    let Some(path) = path.as_str() else {
        return false;
    };
    match (Path::new(path).canonicalize(), dir.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => Path::new(path) == dir,
    }
}

/// `None` when `claude` is not on PATH.
fn claude<S: AsRef<OsStr>>(args: &[S]) -> Result<Option<String>> {
    let shown = format!(
        "claude {}",
        args.iter()
            .map(|a| a.as_ref().to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let o = match Command::new("claude")
        .args(args)
        .stdin(Stdio::null())
        .output()
    {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(shown)(e)),
        Ok(o) => o,
    };
    if !o.status.success() {
        let stderr = String::from_utf8_lossy(&o.stderr);
        let why = stderr.lines().next().unwrap_or("").trim();
        return Err(Error::Msg(format!("{shown}: {}: {why}", o.status)));
    }
    Ok(Some(String::from_utf8_lossy(&o.stdout).into_owned()))
}

fn find(text: &str, args: &[&str], want: impl Fn(&Value) -> bool) -> Result<Option<Value>> {
    let items: Vec<Value> =
        serde_json::from_str(text).map_err(json(format!("claude {}", args.join(" "))))?;
    Ok(items.into_iter().find(want))
}
