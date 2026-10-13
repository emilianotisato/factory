//! `factory docs lanes [set N] [--env] [--setup]` (PRD-002 FR-15), ported
//! from `docs.mjs:1461-1544`. The count lives in `[docs] lanes`; the files
//! are the driver's own (`crate::lanes`), read and placed with its helpers.
use std::fs;
use std::io::Write;
use std::path::Path;

use super::{frontmatter, project};
use crate::error::io;
use crate::{Error, Result};

/// `laneTemplate` (`docs.mjs:1487`): the frontmatter the driver injects and
/// the prose an agent needs that no schema holds.
fn template(n: u32, env: &[String], setup: &str) -> String {
    format!(
        "---
lane: {n}
env: [{}]
setup: {setup}
---

This lane is yours for the whole slice. Nothing here is shared with a sibling lane.

Run `setup` above before the first test run; it is written to be safe to run twice.

**What this lane does not isolate** — name every shared path a sibling could be writing
at the same time (a build output, a manifest, a fixture directory), and what to do
instead of touching it. Delete this paragraph only when the answer is genuinely nothing.
",
        env.join(", ")
    )
}

/// `lane-N.md` files on disk, in number order.
fn on_disk(root: &Path) -> Vec<u32> {
    let mut v: Vec<u32> = fs::read_dir(root.join(crate::lanes::DIR))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_prefix("lane-")?
                .strip_suffix(".md")?
                .parse::<u32>()
                .ok()
        })
        .collect();
    v.sort_unstable();
    v
}

pub fn run(
    root: &Path,
    args: &[String],
    env: Option<&str>,
    setup: Option<&str>,
    out: &mut dyn Write,
) -> Result<()> {
    let Some(p) = project::load(root)? else {
        return Err(Error::Msg(
            "not on the framework: no [docs] in docs/project.toml".into(),
        ));
    };
    let mut text = String::new();
    match args.first().map(String::as_str) {
        Some("set") => {
            let n = args
                .get(1)
                .and_then(|a| a.parse::<u32>().ok())
                .filter(|n| *n >= 1)
                .ok_or_else(|| {
                    Error::Msg(
                        "usage: factory docs lanes set <N> --env \"K=V,K=V\" [--setup \"<cmd>\"]"
                            .into(),
                    )
                })?;
            let env: Vec<&str> = env
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|e| !e.is_empty())
                .collect();
            if env.is_empty() {
                return Err(Error::Msg("--env is required: what makes this lane private, as K=V pairs with {n} for the lane number\n  e.g. --env \"DB_DATABASE=app_lane_{n},REDIS_DB={n}\"".into()));
            }
            let dir = root.join(crate::lanes::DIR);
            fs::create_dir_all(&dir).map_err(io(format!("create {}", dir.display())))?;
            let mut written = Vec::new();
            for i in 1..=n {
                let f = crate::lanes::file(root, i);
                // Never overwrite: the prose half is hand-written and is the
                // point of the file.
                if f.exists() {
                    continue;
                }
                let sub = |s: &str| s.replace("{n}", &i.to_string());
                let body = template(
                    i,
                    &env.iter().map(|e| sub(e)).collect::<Vec<_>>(),
                    &sub(setup.unwrap_or("")),
                );
                fs::write(&f, body).map_err(io(format!("write {}", f.display())))?;
                written.push(format!("{}/lane-{i}.md", crate::lanes::DIR));
            }
            project::set(root, "lanes", Some(&toml::Value::Integer(i64::from(n))))?;
            text.push_str(&format!("lanes: {n}\n"));
            for f in &written {
                text.push_str(&format!("  + {f}\n"));
            }
            let kept = n as usize - written.len();
            if kept > 0 {
                text.push_str(&format!(
                    "  · {kept} existing lane file(s) left as they are\n"
                ));
            }
            text.push_str("\nNow fill each file's prose: what the lane does NOT isolate, and what an agent must not touch while a sibling runs.\n");
        }
        Some(other) => {
            return Err(Error::Msg(format!(
                "unknown lanes verb `{other}`; usage: factory docs lanes [set <N> --env \"K=V\" [--setup \"<cmd>\"]]"
            )));
        }
        None => {
            let disk = on_disk(root);
            text.push_str(&format!(
                "declared      lanes: {}{}\n",
                p.lanes,
                if p.lanes < 2 { " (no parallelism)" } else { "" }
            ));
            let names: Vec<String> = disk.iter().map(|n| format!("lane-{n}")).collect();
            text.push_str(&format!(
                "on disk       {}\n",
                if names.is_empty() {
                    "none".to_string()
                } else {
                    names.join(", ")
                }
            ));
            for n in &disk {
                let raw = fs::read_to_string(crate::lanes::file(root, *n)).unwrap_or_default();
                let lane = crate::lanes::parse(*n, &raw);
                let env: Vec<String> = lane.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
                let prose = !frontmatter::parse(&raw).body.trim().is_empty();
                text.push_str(&format!(
                    "  lane-{n}      {}{}\n",
                    if env.is_empty() {
                        "! no env — this lane isolates nothing".to_string()
                    } else {
                        env.join(" ")
                    },
                    if prose { "" } else { "  ! no prose" }
                ));
            }
            // `lanes = 1` is the default and means no parallelism: it wants
            // no lane file at all.
            let missing: Vec<String> = if p.lanes >= 2 {
                (1..=p.lanes)
                    .filter(|i| !disk.contains(i))
                    .map(|i| format!("lane-{i}"))
                    .collect()
            } else {
                Vec::new()
            };
            if !missing.is_empty() {
                text.push_str(&format!(
                    "\n! lanes: {} declared but {} missing — `factory docs lanes set {} --env \"...\"`\n",
                    p.lanes,
                    missing.join(", "),
                    p.lanes
                ));
            } else if p.lanes < 2 {
                text.push_str("\ndeclare lanes with `factory docs lanes set <N> --env \"DB_DATABASE=app_lane_{n}\"` — {n} is the lane number\n");
            }
        }
    }
    out.write_all(text.as_bytes()).map_err(io("write stdout"))
}
