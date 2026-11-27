use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use clap::Subcommand;

use crate::docs::{Docs, project};
use crate::error::io;
use crate::{Result, lanes, paths, run};

#[derive(Subcommand, Debug, Clone)]
pub enum LanesVerb {
    /// Create, seed and set up every lane, one line each
    Up {
        /// Then run the inner loop in every lane at once; red if any lane is
        #[arg(long)]
        prove: bool,
    },
    /// Remove the lanes above the declared count
    Prune {
        /// Remove every lane of this repo
        #[arg(long)]
        all: bool,
    },
    /// Trim build caches and stale main-tree lane targets
    Clean {
        /// Report sizes only; no deletions
        #[arg(long)]
        dry: bool,
        /// Age passed to cargo-sweep for Cargo target dirs
        #[arg(long, default_value_t = 7)]
        days: u32,
    },
}

pub fn lanes(verb: &LanesVerb, out: &mut dyn Write) -> Result<i32> {
    let cwd = std::env::current_dir().map_err(io("current directory"))?;
    let main = lanes::main_tree(&cwd)?;
    let data = paths::data_dir();
    match verb {
        LanesVerb::Up { prove: false } => up(&data, &main, out),
        LanesVerb::Up { prove: true } => prove(&data, &main, out),
        LanesVerb::Prune { all } => prune(&data, &main, *all, out),
        LanesVerb::Clean { dry, days } => clean(&data, &main, *dry, *days, out),
    }
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(io("stdout"))
}

fn count(main: &Path) -> Result<u32> {
    Ok(project::load(main)?.map_or(1, |p| p.lanes))
}

pub(crate) fn refuse_if_open(
    data: &Path,
    main: &Path,
    verb: &str,
    out: &mut dyn Write,
) -> Result<bool> {
    let open = run::list_open(data, main)?;
    if open.is_empty() {
        return Ok(false);
    }
    say(
        out,
        &format!("  ✗ {verb} refused: this repo has an unfinished run, which holds its lanes"),
    )?;
    for s in &open {
        say(
            out,
            &format!("    run {}  feature {}  {}", s.id, s.feature, s.phase),
        )?;
    }
    Ok(true)
}

fn up_lanes(data: &Path, main: &Path, reset_to_head: bool, out: &mut dyn Write) -> Result<i32> {
    let Some(seed) = project::seed(main)? else {
        say(out, &format!("  ✗ {}", lanes::NO_SEED))?;
        return Ok(1);
    };
    let head = if reset_to_head {
        Some(lanes::head_rev(main)?)
    } else {
        None
    };
    let mut failed = false;
    for n in 1..=count(main)? {
        let report = match lanes::ensure(data, main, n, &seed) {
            Ok(r) => r,
            Err(e) => {
                say(out, &format!("lane-{n}  ✗ {e}"))?;
                failed = true;
                continue;
            }
        };
        if let Some(head) = &head
            && let Err(e) = lanes::reset(&report.dir, head)
        {
            say(out, &format!("lane-{n}  ✗ {e}"))?;
            failed = true;
            continue;
        }
        // The lane file is read from the main tree: the lane's checkout may predate an edit to it.
        let configured = project::load(main)
            .ok()
            .flatten()
            .and_then(|p| p.gate_memory_max);
        let cap = crate::memcap::resolve_limit(configured.as_deref());
        let (exit, detail) = match lanes::load(main, n)
            .and_then(|l| l.run_setup_capped(&report.dir, cap.as_deref()))
        {
            Ok(None) => ("-".to_string(), String::new()),
            Ok(Some(o)) if o.status.success() => ("0".to_string(), String::new()),
            Ok(Some(o)) => {
                failed = true;
                let code = o.status.code().map_or("killed".into(), |c| c.to_string());
                (code, String::from_utf8_lossy(&o.stderr).trim().to_string())
            }
            Err(e) => {
                failed = true;
                ("✗".to_string(), e.to_string())
            }
        };
        say(out, &format!("{}  setup {exit}", report.line()))?;
        for l in detail.lines() {
            say(out, &format!("    {l}"))?;
        }
    }
    Ok(i32::from(failed))
}

pub fn up(data: &Path, main: &Path, out: &mut dyn Write) -> Result<i32> {
    if refuse_if_open(data, main, "up", out)? {
        return Ok(1);
    }
    up_lanes(data, main, false, out)
}

pub fn prove(data: &Path, main: &Path, out: &mut dyn Write) -> Result<i32> {
    if refuse_if_open(data, main, "prove", out)? {
        return Ok(1);
    }
    let code = up_lanes(data, main, true, out)?;
    if code != 0 {
        return Ok(code);
    }
    let prove_shell = env::var("FACTORY_PROVE_SHELL").ok();
    if prove_shell.as_deref() != Some("off") {
        let shell_override = prove_shell
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "off");
        let allow_any = shell_override.is_some();
        let shell = shell_override
            .map(PathBuf::from)
            .or_else(|| env::var_os("SHELL").map(PathBuf::from));
        if let Some(shell) = shell {
            let lane_dir = lanes::dir(data, main, 1);
            let lane_env = lanes::load(main, 1)?.env_for(&lane_dir);
            for (k, want, got) in detect_shell_clobber(&shell, &lane_env, allow_any) {
                say(
                    out,
                    &format!(
                        "! shell {} -lic overwrites {k} ({want} → {got}) — guard it in your shell profile with export {k}=\"${{{k}:-{got}}}\"",
                        shell.display()
                    ),
                )?;
            }
        }
    }
    let inner = match Docs::open(main).and_then(|d| d.gate_inner()) {
        Ok(cmd) => cmd,
        Err(e) => {
            say(out, &format!("  ✗ {e}"))?;
            return Ok(1);
        }
    };
    let jobs = (1..=count(main)?)
        .map(|n| Ok((lanes::dir(data, main, n), lanes::load(main, n)?)))
        .collect::<Result<Vec<(PathBuf, lanes::Lane)>>>()?;
    // One thread per lane, so every child starts before any is waited on.
    let results = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(dir, lane)| {
                let inner = &inner;
                s.spawn(move || {
                    let start = Instant::now();
                    let res = Command::new("sh")
                        .arg("-c")
                        .arg(inner)
                        .current_dir(dir)
                        .envs(lane.env_for(dir))
                        .output();
                    (lane.n, res, start.elapsed().as_secs_f64())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a prove thread does not panic"))
            .collect::<Vec<_>>()
    });
    let mut red = Vec::new();
    for (n, res, secs) in results {
        let (exit, tail) = match res {
            Ok(o) if o.status.success() => ("0".to_string(), String::new()),
            Ok(o) => {
                let code = o.status.code().map_or("killed".into(), |c| c.to_string());
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                (code, text)
            }
            Err(e) => ("✗".to_string(), format!("could not start sh: {e}")),
        };
        say(out, &format!("lane-{n}  inner  exit {exit}  {secs:.1}s"))?;
        if exit != "0" {
            red.push(format!("lane-{n}"));
            let lines: Vec<&str> = tail.lines().collect();
            for l in &lines[lines.len().saturating_sub(TAIL)..] {
                say(out, &format!("    {l}"))?;
            }
        }
    }
    if red.is_empty() {
        return Ok(0);
    }
    say(out, &format!("✗ inner red in {}", red.join(", ")))?;
    Ok(1)
}

const TAIL: usize = 20;
const CLOBBER_TIMEOUT: Duration = Duration::from_secs(3);

fn posix_ident(k: &str) -> bool {
    let mut chars = k.chars();
    let Some(c) = chars.next() else {
        return false;
    };
    (c.is_ascii_alphabetic() || c == '_') && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn known_login_shell(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some("sh" | "bash" | "zsh" | "dash" | "ash" | "ksh")
    )
}

pub fn detect_shell_clobber(
    shell: &Path,
    env: &[(String, String)],
    allow_any_basename: bool,
) -> Vec<(String, String, String)> {
    let vars: Vec<(String, String)> = env
        .iter()
        .filter(|(k, _)| k.as_str() != lanes::ENV_LANE && posix_ident(k))
        .cloned()
        .collect();
    if vars.is_empty() || !shell.is_file() {
        return Vec::new();
    }
    if !allow_any_basename && shell.file_name().is_none_or(|n| !known_login_shell(n)) {
        return Vec::new();
    }
    let mut script = String::new();
    for (k, _) in &vars {
        script.push_str(&format!("printf '__FACTORY_ENV_{k}__=%s\\n' \"${k}\"; "));
    }
    let mut child = match Command::new(shell)
        .args(["-lic", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .envs(vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let stdout = child.stdout.take();
    let deadline = Instant::now() + CLOBBER_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
            Err(_) => return Vec::new(),
        }
    };
    if !status.success() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if let Some(mut pipe) = stdout {
        let _ = pipe.read_to_end(&mut buf);
    }
    let text = String::from_utf8_lossy(&buf);
    let mut got: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in text.lines().rev() {
        let Some(rest) = line.strip_prefix("__FACTORY_ENV_") else {
            continue;
        };
        let Some((k, val)) = rest.split_once("__=") else {
            continue;
        };
        got.entry(k.to_string()).or_insert_with(|| val.to_string());
    }
    let mut clobbered = Vec::new();
    for (k, want) in vars {
        if let Some(g) = got.get(&k)
            && g != &want
        {
            clobbered.push((k, want, g.clone()));
        }
    }
    clobbered
}

fn format_bytes(n: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let f = n as f64;
    if n < 1024 {
        format!("{n} B")
    } else if f < MIB {
        format!("{:.1} KiB", f / KIB)
    } else if f < GIB {
        format!("{:.1} MiB", f / MIB)
    } else {
        format!("{:.1} GiB", f / GIB)
    }
}

fn stale_label(main: &Path, p: &Path) -> String {
    match p.strip_prefix(main) {
        Ok(r) => r.display().to_string(),
        Err(_) => p.display().to_string(),
    }
}

fn lane_cache_bytes(main: &Path, lane_dir: &Path, n: u32) -> Result<u64> {
    Ok(lanes::cache_dirs(main, lane_dir, n)?
        .iter()
        .map(|(p, _)| lanes::dir_size(p))
        .sum())
}

pub fn clean(data: &Path, main: &Path, dry: bool, days: u32, out: &mut dyn Write) -> Result<i32> {
    if !dry && refuse_if_open(data, main, "clean", out)? {
        return Ok(1);
    }
    let on_disk = lanes::on_disk(data, main)?;
    let mut total = 0u64;
    for (n, lane_dir) in &on_disk {
        let size = lane_cache_bytes(main, lane_dir, *n)?;
        if dry {
            say(
                out,
                &format!("lane-{n}  build cache  {}", format_bytes(size)),
            )?;
            total += size;
        } else {
            let mut freed = 0u64;
            for (dir, is_cargo) in lanes::cache_dirs(main, lane_dir, *n)? {
                freed += lanes::trim_cache_dir(lane_dir, &dir, is_cargo, days, false)?;
            }
            say(out, &format!("lane-{n}  freed  {}", format_bytes(freed)))?;
            total += freed;
        }
    }
    for p in lanes::stale_main_lane_targets(main, data)? {
        let size = lanes::dir_size(&p);
        if dry {
            say(
                out,
                &format!(
                    "main  stale {}  {}",
                    stale_label(main, &p),
                    format_bytes(size)
                ),
            )?;
            total += size;
        } else if fs::remove_dir_all(&p).is_ok() {
            say(
                out,
                &format!(
                    "main  removed stale {}  {}",
                    stale_label(main, &p),
                    format_bytes(size)
                ),
            )?;
            total += size;
        }
    }
    if dry {
        say(out, &format!("total  reclaimable  {}", format_bytes(total)))?;
    } else {
        say(out, &format!("total  freed  {}", format_bytes(total)))?;
    }
    Ok(0)
}

pub fn prune(data: &Path, main: &Path, all: bool, out: &mut dyn Write) -> Result<i32> {
    if refuse_if_open(data, main, "prune", out)? {
        return Ok(1);
    }
    let keep = if all { 0 } else { count(main)? };
    let mut removed = 0;
    for (n, dir) in lanes::on_disk(data, main)? {
        if n > keep {
            lanes::remove_lane(main, &dir)?;
            say(out, &format!("lane-{n}  {}  removed", dir.display()))?;
            removed += 1;
        }
    }
    lanes::prune(main)?;
    if removed == 0 {
        say(out, &format!("no lane above {keep} to remove"))?;
    }
    Ok(0)
}

/// No setup here: the run does it at each lane's first spawn.
pub fn ensure_all(data: &Path, main: &Path, out: &mut dyn Write) -> Result<bool> {
    let seed = project::seed(main)?.unwrap_or_default();
    for n in 1..=count(main)? {
        match lanes::ensure(data, main, n, &seed) {
            Ok(r) => say(out, &format!("  {}", r.line()))?,
            Err(e) => {
                say(out, &format!("  ✗ lane-{n}: {e}"))?;
                return Ok(false);
            }
        }
    }
    if let Some(limit_gib) = project::clean_above_gb(main)?
        && run::list_open(data, main)?.is_empty()
    {
        let threshold = (limit_gib * 1024.0 * 1024.0 * 1024.0) as u64;
        for n in 1..=count(main)? {
            let lane_dir = lanes::dir(data, main, n);
            if !lane_dir.exists() {
                continue;
            }
            let before = lane_cache_bytes(main, &lane_dir, n)?;
            if before <= threshold {
                continue;
            }
            for (dir, is_cargo) in lanes::cache_dirs(main, &lane_dir, n)? {
                if let Err(e) = lanes::trim_cache_dir(&lane_dir, &dir, is_cargo, 7, false) {
                    say(
                        out,
                        &format!("  lane-{n}  warning: could not trim {}: {e}", dir.display()),
                    )?;
                }
            }
            let mut after = lane_cache_bytes(main, &lane_dir, n)?;
            if after > threshold {
                for (dir, is_cargo) in lanes::cache_dirs(main, &lane_dir, n)? {
                    if !dir.exists() {
                        continue;
                    }
                    match lanes::trim_cache_dir(&lane_dir, &dir, is_cargo, 7, true) {
                        Ok(_) => {}
                        Err(e) => say(
                            out,
                            &format!("  lane-{n}  warning: could not trim {}: {e}", dir.display()),
                        )?,
                    }
                }
                after = lane_cache_bytes(main, &lane_dir, n)?;
            }
            if after < before || after <= threshold {
                say(
                    out,
                    &format!(
                        "  lane-{n}  trimmed build cache ({} > {:.1} GiB)",
                        format_bytes(before),
                        limit_gib
                    ),
                )?;
            }
        }
    }
    Ok(true)
}
