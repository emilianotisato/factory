//! A lane is both what a worker gets (env, setup) and where it runs: a detached worktree under the data dir.
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use crate::error::io;
use crate::metrics::sha256_hex;
use crate::{Error, Result, paths};

pub const DIR: &str = "docs/reference/lanes";
pub const ENV_LANE: &str = "FACTORY_LANE";
pub const NO_SEED: &str = "no lanes declared — /lanes picks the seed list and writes the lane files (an interview, never unattended)";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lane {
    pub n: u32,
    pub env: Vec<(String, String)>,
    pub setup: Option<String>,
}

pub fn file(repo: &Path, n: u32) -> PathBuf {
    repo.join(DIR).join(format!("lane-{n}.md"))
}

pub fn load(repo: &Path, n: u32) -> Result<Lane> {
    let path = file(repo, n);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(parse(n, &text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Lane {
            n,
            ..Lane::default()
        }),
        Err(e) => Err(io(format!("read {}", path.display()))(e)),
    }
}

/// The frontmatter shape `docs lanes` writes; the two must stay in step.
pub fn parse(n: u32, text: &str) -> Lane {
    let mut lane = Lane {
        n,
        ..Lane::default()
    };
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return lane;
    };
    let Some(end) = rest.find("\n---") else {
        return lane;
    };
    for line in rest[..end].lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "env" => {
                let inner = value.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let item = unquote(item.trim());
                    if let Some((k, v)) = item.split_once('=')
                        && !k.trim().is_empty()
                    {
                        lane.env.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
            }
            "setup" => {
                let cmd = unquote(value).trim();
                if !cmd.is_empty() {
                    lane.setup = Some(cmd.to_string());
                }
            }
            _ => {}
        }
    }
    lane
}

/// Only a matching pair: a command that ends in a quoted word keeps its closing quote.
fn unquote(value: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(q).and_then(|v| v.strip_suffix(q)) {
            return inner;
        }
    }
    value
}

impl Lane {
    pub fn env(&self) -> Vec<(String, String)> {
        let mut out = vec![(ENV_LANE.to_string(), self.n.to_string())];
        out.extend(self.env.iter().cloned());
        out
    }

    pub fn setup(&self, dir: &Path) -> Result<bool> {
        let Some(out) = self.run_setup(dir)? else {
            return Ok(false);
        };
        if !out.status.success() {
            return Err(Error::Tool {
                cmd: self.setup_name(),
                detail: format!(
                    "exit {}: {}",
                    out.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            });
        }
        Ok(true)
    }

    pub fn run_setup(&self, dir: &Path) -> Result<Option<Output>> {
        let Some(cmd) = &self.setup else {
            return Ok(None);
        };
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(dir)
            .envs(self.env())
            .output()
            .map(Some)
            .map_err(|e| Error::Tool {
                cmd: self.setup_name(),
                detail: format!("could not start sh: {e}"),
            })
    }

    fn setup_name(&self) -> String {
        let cmd = self.setup.as_deref().unwrap_or("");
        format!("lane {} setup: {cmd}", self.n)
    }
}

/// Via the common dir: `--show-toplevel` from a lane would answer the lane itself.
pub fn same_repo(a: &Path, b: &Path) -> bool {
    fn key(p: &Path) -> Option<PathBuf> {
        main_tree(p).ok().or_else(|| p.canonicalize().ok())
    }
    matches!((key(a), key(b)), (Some(x), Some(y)) if x == y)
}

pub fn main_tree(from: &Path) -> Result<PathBuf> {
    let common = git(
        from,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common = PathBuf::from(common.trim());
    let main = common
        .parent()
        .ok_or_else(|| Error::Msg(format!("git common dir {} has no parent", common.display())))?;
    main.canonicalize()
        .map_err(io(format!("canonicalize {}", main.display())))
}

/// The hash keeps two repos named `app` apart.
pub fn key(main: &Path) -> String {
    let name = main
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".into());
    let hash = sha256_hex(main.as_os_str().as_encoded_bytes());
    format!("{name}-{}", &hash[..8])
}

pub fn dir(data: &Path, main: &Path, n: u32) -> PathBuf {
    data.join("lanes").join(key(main)).join(format!("lane-{n}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seeded {
    Clone,
    Copy,
    Kept,
    Skipped,
}

impl Seeded {
    pub fn label(self) -> &'static str {
        match self {
            Seeded::Clone => "clone",
            Seeded::Copy => "copy",
            Seeded::Kept => "kept",
            Seeded::Skipped => "skipped (not in the main tree)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub n: u32,
    pub dir: PathBuf,
    pub created: bool,
    pub seeds: Vec<(String, Seeded)>,
}

impl Report {
    pub fn line(&self) -> String {
        let seeds = if self.seeds.is_empty() {
            "no seed".to_string()
        } else {
            let each: Vec<String> = self
                .seeds
                .iter()
                .map(|(p, s)| format!("{p} {}", s.label()))
                .collect();
            each.join(" · ")
        };
        format!("lane-{}  {}  {seeds}", self.n, self.dir.display())
    }
}

pub fn ensure(data: &Path, main: &Path, n: u32, seed: &[String]) -> Result<Report> {
    let want = dir(data, main, n);
    let parent = want.parent().expect("a lane dir has a parent");
    paths::private_dir_all(parent).map_err(io(format!("mkdir {}", parent.display())))?;
    let parent = parent
        .canonicalize()
        .map_err(io(format!("canonicalize {}", parent.display())))?;
    let lane = parent.join(format!("lane-{n}"));
    let registered = worktrees(main)?.contains(&lane);
    let created = !(registered && lane.is_dir());
    if created {
        let at = lane.to_string_lossy();
        // A worktree git still lists but whose dir is gone needs `-f`.
        let force = if registered { "-f" } else { "--quiet" };
        git(main, &["worktree", "add", force, "--detach", &at, "HEAD"])?;
    }
    sweep_seed_temps(&parent, n)?;
    let seeds = seed
        .iter()
        .map(|p| Ok((p.clone(), seed_one(main, &lane, n, p)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(Report {
        n,
        dir: lane,
        created,
        seeds,
    })
}

fn worktrees(main: &Path) -> Result<Vec<PathBuf>> {
    let list = git(main, &["worktree", "list", "--porcelain"])?;
    Ok(list
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(|p| {
            let p = PathBuf::from(p);
            p.canonicalize().unwrap_or(p)
        })
        .collect())
}

fn sweep_seed_temps(parent: &Path, n: u32) -> Result<()> {
    let prefix = format!(".seed-tmp-lane-{n}-");
    let Ok(entries) = fs::read_dir(parent) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".tmp") {
            remove(&entry.path())?;
        }
    }
    Ok(())
}

fn seed_one(main: &Path, lane: &Path, n: u32, rel: &str) -> Result<Seeded> {
    let inside = Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    // `big/.` is `big`: a `.` left in would make the copy's target odd.
    let relative: PathBuf = Path::new(rel)
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect();
    let relative = relative.as_path();
    if relative.as_os_str().is_empty() || !inside {
        return Err(Error::Msg(format!(
            "[lanes] seed `{rel}` is not a repo-relative path"
        )));
    }
    let from = main.join(relative);
    if from.symlink_metadata().is_err() {
        return Ok(Seeded::Skipped);
    }
    let lane_parent = lane.parent().expect("a lane dir has a parent");
    let to = lane.join(relative);
    if to.symlink_metadata().is_ok() {
        return Ok(Seeded::Kept);
    }
    let tmp = lane_parent.join(format!(".seed-tmp-lane-{n}-{}.tmp", std::process::id()));
    remove(&tmp)?;
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(io(format!("mkdir {}", parent.display())))?;
    }
    if cp(CLONE, &from, &tmp) {
        if fs::rename(&tmp, &to).is_err() {
            remove(&tmp)?;
            return Err(Error::Msg(format!(
                "seed `{rel}`: rename {} to {}",
                tmp.display(),
                to.display()
            )));
        }
        return Ok(Seeded::Clone);
    }
    remove(&tmp)?;
    if cp(COPY, &from, &tmp) {
        if fs::rename(&tmp, &to).is_err() {
            remove(&tmp)?;
            return Err(Error::Msg(format!(
                "seed `{rel}`: rename {} to {}",
                tmp.display(),
                to.display()
            )));
        }
        return Ok(Seeded::Copy);
    }
    remove(&tmp)?;
    Err(Error::Msg(format!(
        "seed `{rel}`: could not copy {} to {}",
        from.display(),
        to.display()
    )))
}

#[cfg(target_os = "macos")]
const CLONE: &[&str] = &["-c", "-R", "-p"];
#[cfg(not(target_os = "macos"))]
const CLONE: &[&str] = &["--reflink=always", "-a"];
#[cfg(target_os = "macos")]
const COPY: &[&str] = &["-R", "-p"];
#[cfg(not(target_os = "macos"))]
const COPY: &[&str] = &["-a"];

fn cp(flags: &[&str], from: &Path, to: &Path) -> bool {
    Command::new("cp")
        .args(flags)
        .arg(from)
        .arg(to)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn remove(p: &Path) -> Result<()> {
    let Ok(meta) = p.symlink_metadata() else {
        return Ok(());
    };
    if meta.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
    .map_err(io(format!("remove {}", p.display())))
}

pub fn head_rev(main: &Path) -> Result<String> {
    Ok(git(main, &["rev-parse", "HEAD"])?.trim().to_string())
}

/// `clean -fd`, never `-x`: ignored seeded state stays.
pub fn reset(lane: &Path, rev: &str) -> Result<()> {
    git(lane, &["checkout", "-q", "--force", "--detach", rev])?;
    // `reset` drops CHERRY_PICK_HEAD and MERGE_HEAD; HEAD is detached, so no branch moves.
    git(lane, &["reset", "-q", "--hard", "HEAD"])?;
    git(lane, &["clean", "-q", "-fd"])?;
    Ok(())
}

pub fn on_disk(data: &Path, main: &Path) -> Result<Vec<(u32, PathBuf)>> {
    let root = data.join("lanes").join(key(main));
    let entries = match fs::read_dir(&root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", root.display()))(e)),
    };
    let mut lanes: Vec<(u32, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let n = name.strip_prefix("lane-")?.parse::<u32>().ok()?;
            Some((n, e.path()))
        })
        .collect();
    lanes.sort_unstable();
    Ok(lanes)
}

pub fn remove_lane(main: &Path, lane: &Path) -> Result<()> {
    let at = lane.to_string_lossy();
    // A dir git does not list fails here; the removal below covers it.
    let _ = git(main, &["worktree", "remove", "--force", &at]);
    remove(lane)
}

pub fn prune(main: &Path) -> Result<()> {
    git(main, &["worktree", "prune"]).map(|_| ())
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let meta = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if let Ok(rd) = fs::read_dir(&p) {
                for e in rd.flatten() {
                    stack.push(e.path());
                }
            }
        } else if meta.is_file() {
            total += meta.len();
        }
    }
    total
}

fn canonical_existing(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    path.canonicalize().ok()
}

fn push_canonical(out: &mut HashSet<PathBuf>, path: &Path) {
    if let Some(c) = canonical_existing(path) {
        out.insert(c);
    }
}

pub fn declared_cargo_targets(main: &Path, data: &Path) -> Result<HashSet<PathBuf>> {
    let lanes_ref = main.join(DIR);
    let mut out = HashSet::new();
    let entries = match fs::read_dir(&lanes_ref) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(io(format!("read {}", lanes_ref.display()))(e)),
    };
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(digits) = name
            .strip_prefix("lane-")
            .and_then(|s| s.strip_suffix(".md"))
        else {
            continue;
        };
        let Ok(n) = digits.parse::<u32>() else {
            continue;
        };
        let lane = load(main, n)?;
        let Some(raw) = lane
            .env
            .iter()
            .find_map(|(k, v)| (k == "CARGO_TARGET_DIR").then_some(v))
        else {
            continue;
        };
        let p = PathBuf::from(raw);
        if p.is_absolute() {
            push_canonical(&mut out, &p);
            continue;
        }
        let lane_dir = dir(data, main, n);
        push_canonical(&mut out, &lane_dir.join(&p));
        push_canonical(&mut out, &main.join(&p));
    }
    Ok(out)
}

pub fn cache_dirs(main: &Path, lane_dir: &Path, n: u32) -> Result<Vec<(PathBuf, bool)>> {
    let lane = load(main, n)?;
    let mut raw: Vec<(PathBuf, bool)> = Vec::new();
    if let Some(target) = lane
        .env
        .iter()
        .find_map(|(k, v)| (k == "CARGO_TARGET_DIR").then(|| PathBuf::from(v)))
    {
        let p = if target.is_absolute() {
            target
        } else {
            lane_dir.join(target)
        };
        raw.push((p, true));
    }
    raw.push((lane_dir.join("target"), true));
    for rel in ["node_modules/.cache", ".next/cache", "vendor/.cache"] {
        raw.push((lane_dir.join(rel), false));
    }
    let mut items: Vec<(PathBuf, bool)> = raw
        .into_iter()
        .filter_map(|(p, is_cargo)| canonical_existing(&p).map(|c| (c, is_cargo)))
        .collect();
    items.sort_by_key(|(p, _)| (p.components().count(), p.as_os_str().len()));
    let mut kept: Vec<(PathBuf, bool)> = Vec::new();
    for (path, is_cargo) in items {
        if kept.iter().any(|(k, _)| path.starts_with(k)) {
            continue;
        }
        kept.push((path, is_cargo));
    }
    Ok(kept)
}

pub fn stale_main_lane_targets(main: &Path, data: &Path) -> Result<Vec<PathBuf>> {
    let target = main.join("target");
    let live = declared_cargo_targets(main, data)?;
    let entries = match fs::read_dir(&target) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", target.display()))(e)),
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        let Some(rest) = name.strip_prefix("lane-") else {
            continue;
        };
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Some(can) = canonical_existing(&p) else {
            continue;
        };
        if live.contains(&can) {
            continue;
        }
        out.push(p);
    }
    out.sort();
    Ok(out)
}

fn cargo_target_env(lane_dir: &Path, dir: &Path) -> OsString {
    match dir.strip_prefix(lane_dir) {
        Ok(p) => p.as_os_str().to_os_string(),
        Err(_) => dir.as_os_str().to_os_string(),
    }
}

fn find_cargo_sweep() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join("cargo-sweep");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn trim_cache_dir(
    lane_dir: &Path,
    dir: &Path,
    is_cargo: bool,
    days: u32,
    force_remove: bool,
) -> Result<u64> {
    if !dir.exists() {
        return Ok(0);
    }
    let before = dir_size(dir);
    if is_cargo && !force_remove {
        let sweep = std::env::var("FACTORY_CARGO_SWEEP").ok();
        let skip = sweep.as_deref().is_some_and(|s| s == "off");
        let run = if skip {
            None
        } else if let Some(ref p) = sweep.filter(|s| !s.is_empty()) {
            Some(PathBuf::from(p))
        } else {
            find_cargo_sweep()
        };
        if let Some(exe) = run {
            let ok = Command::new(&exe)
                .args(["sweep", "--time", &days.to_string()])
                .current_dir(lane_dir)
                .env("CARGO_TARGET_DIR", cargo_target_env(lane_dir, dir))
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                let after = dir_size(dir);
                return Ok(before.saturating_sub(after));
            }
        }
    }
    fs::remove_dir_all(dir).map_err(io(format!("remove {}", dir.display())))?;
    Ok(before)
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let cmd = format!("git {}", args.join(" "));
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| Error::Tool {
            cmd: cmd.clone(),
            detail: format!("could not start git: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Tool {
            cmd,
            detail: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crlf_lane_file_keeps_its_env_and_setup() {
        let lane = parse(
            2,
            "---\r\nlane: 2\r\nenv: [PORT=8002]\r\nsetup: make db\r\n---\r\n\r\nProse.\r\n",
        );
        assert_eq!(lane.env, vec![("PORT".to_string(), "8002".to_string())]);
        assert_eq!(lane.setup.as_deref(), Some("make db"));
    }

    #[test]
    fn only_a_matching_pair_of_quotes_comes_off() {
        let lane = parse(
            2,
            "---\nlane: 2\nenv: [\"A=1\", 'B=\"x\"']\nsetup: echo hi >> \"$(pwd)\"/log\n---\n",
        );
        assert_eq!(
            lane.env,
            vec![
                ("A".to_string(), "1".to_string()),
                ("B".to_string(), "\"x\"".to_string())
            ]
        );
        assert_eq!(lane.setup.as_deref(), Some("echo hi >> \"$(pwd)\"/log"));
        let quoted = parse(2, "---\nlane: 2\nsetup: \"make 'db'\"\n---\n");
        assert_eq!(quoted.setup.as_deref(), Some("make 'db'"));
    }

    #[test]
    fn parse_reads_env_and_setup_and_tolerates_the_rest() {
        let text = "---\nlane: 2\nenv: [DB_NAME=app_lane_2, PORT=8002]\nsetup: php artisan migrate --database=lane_2\n---\n\nProse.\n";
        let lane = parse(2, text);
        assert_eq!(
            lane.env,
            vec![
                ("DB_NAME".to_string(), "app_lane_2".to_string()),
                ("PORT".to_string(), "8002".to_string())
            ]
        );
        assert_eq!(
            lane.setup.as_deref(),
            Some("php artisan migrate --database=lane_2")
        );
        assert_eq!(lane.env()[0], ("FACTORY_LANE".to_string(), "2".to_string()));
        assert_eq!(lane.env().len(), 3);

        let bare = parse(1, "---\nlane: 1\nenv: []\nsetup: \n---\n");
        assert_eq!(
            bare,
            Lane {
                n: 1,
                ..Lane::default()
            }
        );
        assert_eq!(
            bare.env(),
            vec![("FACTORY_LANE".to_string(), "1".to_string())]
        );
        assert_eq!(
            parse(3, "no frontmatter"),
            Lane {
                n: 3,
                ..Lane::default()
            }
        );
    }

    #[test]
    fn load_without_a_file_is_the_bare_lane_and_setup_runs_with_the_env() {
        let dir = tempfile::tempdir().unwrap();
        let lane = load(dir.path(), 1).unwrap();
        assert_eq!(
            lane,
            Lane {
                n: 1,
                ..Lane::default()
            }
        );
        assert!(!lane.setup(dir.path()).unwrap());

        let lanes = dir.path().join(DIR);
        fs::create_dir_all(&lanes).unwrap();
        fs::write(
            lanes.join("lane-2.md"),
            "---\nlane: 2\nenv: [MARK=two]\nsetup: echo \"$FACTORY_LANE $MARK\" > setup.out\n---\n",
        )
        .unwrap();
        let lane = load(dir.path(), 2).unwrap();
        assert!(lane.setup(dir.path()).unwrap());
        assert_eq!(
            fs::read_to_string(dir.path().join("setup.out")).unwrap(),
            "2 two\n"
        );

        fs::write(
            lanes.join("lane-3.md"),
            "---\nlane: 3\nenv: []\nsetup: exit 3\n---\n",
        )
        .unwrap();
        let err = load(dir.path(), 3).unwrap().setup(dir.path()).unwrap_err();
        assert!(err.to_string().contains("lane 3 setup"), "{err}");
    }

    fn repo(tmp: &Path, name: &str) -> PathBuf {
        let r = tmp.join(name);
        fs::create_dir_all(&r).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "T"],
            vec!["config", "user.email", "t@example.invalid"],
            vec!["config", "core.hooksPath", "/nonexistent-hooks"],
        ] {
            git(&r, &args).unwrap();
        }
        fs::write(r.join(".gitignore"), "/big/\n/.env\n/missing\n").unwrap();
        fs::write(r.join("f.txt"), "f\n").unwrap();
        fs::write(r.join("g.txt"), "g\n").unwrap();
        git(&r, &["add", "-A"]).unwrap();
        git(&r, &["commit", "-q", "-m", "init"]).unwrap();
        r.canonicalize().unwrap()
    }

    fn clones(dir: &Path) -> bool {
        let probe = dir.join("probe");
        fs::write(&probe, "p").unwrap();
        cp(CLONE, &probe, &dir.join("probe-clone"))
    }

    fn worktree_blocks(main: &Path) -> Vec<String> {
        git(main, &["worktree", "list", "--porcelain"])
            .unwrap()
            .split("\n\n")
            .filter(|b| !b.trim().is_empty())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_lane_dir_is_the_same_from_the_main_tree_and_a_lane_and_differs_per_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let main = repo(tmp.path(), "app");
        let linked = tmp.path().join("linked");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                linked.to_str().unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(main_tree(&main).unwrap(), main);
        assert_eq!(main_tree(&linked).unwrap(), main);
        assert_eq!(
            dir(&data, &main_tree(&linked).unwrap(), 2),
            dir(&data, &main_tree(&main).unwrap(), 2)
        );
        let lane = dir(&data, &main, 2);
        assert!(lane.starts_with(data.join("lanes")), "{}", lane.display());
        assert_eq!(lane.file_name().unwrap(), "lane-2");
        assert!(key(&main).starts_with("app-") && key(&main).len() == "app-".len() + 8);

        fs::create_dir_all(tmp.path().join("other")).unwrap();
        let twin = repo(&tmp.path().join("other"), "app");
        assert_ne!(key(&twin), key(&main));
        assert!(key(&twin).starts_with("app-"));
    }

    #[test]
    fn ensure_makes_one_detached_lane_seeds_once_and_keeps_what_the_lane_grew() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let main = repo(tmp.path(), "app");
        fs::create_dir_all(main.join("big/nested")).unwrap();
        fs::write(main.join("big/one.bin"), "one").unwrap();
        fs::write(main.join("big/nested/two.bin"), "two").unwrap();
        fs::write(main.join(".env"), "SECRET=1\n").unwrap();
        let seed: Vec<String> = ["big", ".env", "missing"].map(String::from).to_vec();
        let want = if clones(tmp.path()) {
            Seeded::Clone
        } else {
            Seeded::Copy
        };

        let first = ensure(&data, &main, 1, &seed).unwrap();
        assert!(first.created);
        assert_eq!(
            first.seeds,
            vec![
                ("big".to_string(), want),
                (".env".to_string(), want),
                ("missing".to_string(), Seeded::Skipped)
            ]
        );
        let lane = &first.dir;
        for f in ["big/one.bin", "big/nested/two.bin", ".env", "f.txt"] {
            assert_eq!(
                fs::read(lane.join(f)).unwrap(),
                fs::read(main.join(f)).unwrap(),
                "{f}"
            );
        }
        assert!(first.line().contains("big ") && first.line().contains(" · .env "));
        assert!(
            first
                .line()
                .contains("missing skipped (not in the main tree)")
        );

        fs::write(lane.join("big/lane-own.bin"), "mine").unwrap();
        let second = ensure(&data, &main, 1, &seed).unwrap();
        assert!(!second.created);
        assert_eq!(&second.dir, lane);
        assert_eq!(second.seeds[0].1, Seeded::Kept);
        assert_eq!(second.seeds[1].1, Seeded::Kept);
        assert_eq!(fs::read(lane.join("big/lane-own.bin")).unwrap(), b"mine");

        let lanes: Vec<String> = worktree_blocks(&main)
            .into_iter()
            .filter(|b| b.contains("/lanes/"))
            .collect();
        assert_eq!(lanes.len(), 1, "{lanes:?}");
        assert!(lanes[0].lines().any(|l| l == "detached"), "{}", lanes[0]);

        let err = ensure(&data, &main, 1, &["../out".to_string()]).unwrap_err();
        assert!(err.to_string().contains("../out"), "{err}");

        // `big/.` names `big`, which the lane has by now.
        let dot = ensure(&data, &main, 1, &["big/.".to_string()]).unwrap();
        assert_eq!(dot.seeds[0].1, Seeded::Kept);
        let fresh = ensure(&data, &main, 2, &["big/.".to_string()]).unwrap();
        assert_eq!(fresh.seeds[0].1, want);
        assert_eq!(
            fs::read(fresh.dir.join("big/nested/two.bin")).unwrap(),
            b"two"
        );
    }

    fn seed_temps(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(".seed-tmp-"))
            })
            .collect()
    }

    #[test]
    fn seed_copy_uses_atomic_temp_sibling_and_cleans_up_on_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let main = repo(tmp.path(), "app");
        fs::create_dir_all(main.join("big")).unwrap();
        fs::write(main.join("big/one.bin"), "one").unwrap();
        let parent = data.join("lanes").join(key(&main));
        fs::create_dir_all(&parent).unwrap();
        fs::write(parent.join(".seed-tmp-lane-1-99999.tmp"), "stale").unwrap();

        let report = ensure(&data, &main, 1, &["big".to_string()]).unwrap();
        let lane_parent = report.dir.parent().unwrap();
        assert!(!parent.join(".seed-tmp-lane-1-99999.tmp").exists());
        assert!(seed_temps(&report.dir).is_empty());
        assert!(seed_temps(lane_parent).is_empty());
        assert_eq!(fs::read(report.dir.join("big/one.bin")).unwrap(), b"one");

        let fail_data = tmp.path().join("fail-data");
        let fail_main = repo(tmp.path(), "fail-app");
        fs::create_dir_all(fail_main.join("blocked")).unwrap();
        fs::write(fail_main.join("blocked/leaf.txt"), "leaf").unwrap();
        let fail_lane = ensure(&fail_data, &fail_main, 1, &[]).unwrap().dir;
        fs::write(fail_lane.join("blocked"), "not-a-dir").unwrap();
        let err = ensure(&fail_data, &fail_main, 1, &["blocked/leaf.txt".to_string()]).unwrap_err();
        assert!(err.to_string().contains("blocked"), "{err}");
        assert!(!fail_lane.join("blocked/leaf.txt").exists());
        let fail_parent = fail_lane.parent().unwrap();
        assert!(seed_temps(&fail_lane).is_empty());
        assert!(seed_temps(fail_parent).is_empty());
        let again =
            ensure(&fail_data, &fail_main, 1, &["blocked/leaf.txt".to_string()]).unwrap_err();
        assert!(again.to_string().contains("blocked"), "{again}");
        assert!(!fail_lane.join("blocked/leaf.txt").exists());
    }

    #[test]
    fn reset_drops_edits_staging_untracked_files_and_a_cherry_pick_but_not_ignored_state() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let main = repo(tmp.path(), "app");
        fs::write(main.join(".env"), "SECRET=1\n").unwrap();
        git(&main, &["checkout", "-q", "-b", "side"]).unwrap();
        fs::write(main.join("f.txt"), "side\n").unwrap();
        git(&main, &["commit", "-q", "-am", "side"]).unwrap();
        git(&main, &["checkout", "-q", "main"]).unwrap();
        let head = git(&main, &["rev-parse", "HEAD"]).unwrap();

        let lane = ensure(&data, &main, 1, &[".env".to_string()]).unwrap().dir;
        fs::write(lane.join("f.txt"), "lane\n").unwrap();
        git(&lane, &["commit", "-q", "-am", "lane"]).unwrap();
        assert!(git(&lane, &["cherry-pick", "side"]).is_err());
        assert!(git(&lane, &["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"]).is_ok());
        fs::write(lane.join("g.txt"), "edited\n").unwrap();
        fs::write(lane.join("staged.txt"), "s\n").unwrap();
        git(&lane, &["add", "staged.txt"]).unwrap();
        fs::write(lane.join("loose.txt"), "l\n").unwrap();

        reset(&lane, head.trim()).unwrap();
        assert_eq!(git(&lane, &["rev-parse", "HEAD"]).unwrap(), head);
        assert!(
            git(&lane, &["symbolic-ref", "-q", "HEAD"]).is_err(),
            "detached"
        );
        assert_eq!(git(&lane, &["status", "--porcelain"]).unwrap(), "");
        assert!(git(&lane, &["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"]).is_err());
        assert_eq!(fs::read_to_string(lane.join(".env")).unwrap(), "SECRET=1\n");
        assert_eq!(fs::read_to_string(lane.join("f.txt")).unwrap(), "f\n");
    }
}
