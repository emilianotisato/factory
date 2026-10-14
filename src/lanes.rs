//! Lanes (PRD-001 §5 FR-3, PRD-005 FR-1). `[docs] lanes` in
//! `docs/project.toml` says how many there are and the board which are free;
//! a lane file, `docs/reference/lanes/lane-N.md`, says what a worker on lane
//! N gets: an `env` list of `KEY=value` and a `setup:` command the driver
//! runs once per lane per run before the first spawn on it. A repo with no
//! lane files has lanes that differ in nothing but their number; every
//! worker still sees `FACTORY_LANE=N`.
//!
//! Lane N is also a place: a detached git worktree at
//! `<data dir>/lanes/<repo key>/lane-N`, into which the ignored state
//! `[lanes] seed` names is cloned once (a plain copy when the clone fails).
//! This module owns where a lane lives, creating and seeding it, resetting
//! it to a commit, running its setup there, and removing it (FR-10).
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use crate::error::io;
use crate::metrics::sha256_hex;
use crate::{Error, Result, paths};

pub const DIR: &str = "docs/reference/lanes";
pub const ENV_LANE: &str = "FACTORY_LANE";
/// What preflight and `factory lanes up` say to a repo with no `[lanes] seed`.
pub const NO_SEED: &str = "no lanes declared — /lanes picks the seed list and writes the lane files (an interview, never unattended)";

/// One lane as its file declares it; the bare lane when there is no file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lane {
    pub n: u32,
    pub env: Vec<(String, String)>,
    pub setup: Option<String>,
}

/// `docs/reference/lanes/lane-N.md`.
pub fn file(repo: &Path, n: u32) -> PathBuf {
    repo.join(DIR).join(format!("lane-{n}.md"))
}

/// The lane file parsed, or the bare lane when the repo has none for `n`.
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

/// The frontmatter of a lane file: `env: [K=v, K2=v2]` and `setup: <cmd>`,
/// the shape `docs lanes` writes (`laneTemplate`). Anything else is ignored.
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

/// `value` without one pair of matching quotes around it: a command that
/// ends in a quoted word keeps its closing quote.
fn unquote(value: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(q).and_then(|v| v.strip_suffix(q)) {
            return inner;
        }
    }
    value
}

impl Lane {
    /// What a worker on this lane gets on top of the driver's environment:
    /// `FACTORY_LANE=N` first, then the file's `env` in order.
    pub fn env(&self) -> Vec<(String, String)> {
        let mut out = vec![(ENV_LANE.to_string(), self.n.to_string())];
        out.extend(self.env.iter().cloned());
        out
    }

    /// Run `setup` in `dir` with the lane's env; nothing declared, nothing
    /// run. A failing setup is a tool error: the lane is not usable.
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

    /// `setup` run in `dir` with the lane's env, whatever its exit; `None`
    /// when the lane declares none.
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

/// The main tree of the repo `from` is in: the parent of the absolute git
/// common dir, canonical. The same from the main tree and from any lane,
/// where `--show-toplevel` would answer the lane itself.
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

/// The repo key: the main tree's directory name, `-`, and the first 8 hex
/// chars of the SHA-256 of its path. Two repos named `app` never share one.
pub fn key(main: &Path) -> String {
    let name = main
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".into());
    let hash = sha256_hex(main.as_os_str().as_encoded_bytes());
    format!("{name}-{}", &hash[..8])
}

/// `<data>/lanes/<repo key>/lane-<n>`; `main` is the canonical main tree.
pub fn dir(data: &Path, main: &Path, n: u32) -> PathBuf {
    data.join("lanes").join(key(main)).join(format!("lane-{n}"))
}

/// How one seed path reached a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seeded {
    Clone,
    Copy,
    /// The lane already had it: its own build state is kept.
    Kept,
    /// Not in the main tree, so nothing to seed.
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

/// What `ensure` did to one lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub n: u32,
    pub dir: PathBuf,
    pub created: bool,
    pub seeds: Vec<(String, Seeded)>,
}

impl Report {
    /// `lane-N  <path>  big clone · .env copy`.
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

/// Lane `n` of `main`, made and seeded: `git worktree add --detach` at
/// `dir(data, main, n)` from the main tree's HEAD when it is not already a
/// worktree of this repo, then each `seed` path the lane lacks. An existing
/// lane is kept as it is.
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
    let seeds = seed
        .iter()
        .map(|p| Ok((p.clone(), seed_one(main, &lane, p)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(Report {
        n,
        dir: lane,
        created,
        seeds,
    })
}

/// The worktree paths `git worktree list --porcelain` names, canonical where
/// they exist.
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

/// One seed path into the lane: skipped when the main tree lacks it, kept
/// when the lane has it, else cloned, else copied.
fn seed_one(main: &Path, lane: &Path, rel: &str) -> Result<Seeded> {
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
    let to = lane.join(relative);
    if to.symlink_metadata().is_ok() {
        return Ok(Seeded::Kept);
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(io(format!("mkdir {}", parent.display())))?;
    }
    if cp(CLONE, &from, &to) {
        return Ok(Seeded::Clone);
    }
    // A partial clone left behind would read as `kept` next time.
    remove(&to)?;
    if cp(COPY, &from, &to) {
        return Ok(Seeded::Copy);
    }
    remove(&to)?;
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

/// The lane at `rev`: a forced detached checkout, any cherry-pick or merge
/// state dropped, and untracked files that are not ignored removed. Ignored
/// seeded state stays (`clean -fd`, never `-x`). Nothing is saved first.
pub fn reset(lane: &Path, rev: &str) -> Result<()> {
    git(lane, &["checkout", "-q", "--force", "--detach", rev])?;
    // `reset` drops CHERRY_PICK_HEAD and MERGE_HEAD; HEAD is detached by
    // now, so no branch moves.
    git(lane, &["reset", "-q", "--hard", "HEAD"])?;
    git(lane, &["clean", "-q", "-fd"])?;
    Ok(())
}

/// The `lane-N` dirs under `main`'s lane dir, in number order, whether git
/// still lists them or not.
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

/// The lane at `lane` gone: `git worktree remove --force` from the main
/// tree, then the dir itself when git left it (a dir git no longer lists).
pub fn remove_lane(main: &Path, lane: &Path) -> Result<()> {
    let at = lane.to_string_lossy();
    // A dir git does not list fails here; the removal below covers it.
    let _ = git(main, &["worktree", "remove", "--force", &at]);
    remove(lane)
}

/// `git worktree prune` in the main tree: drops the rows of worktrees whose
/// dir is gone.
pub fn prune(main: &Path) -> Result<()> {
    git(main, &["worktree", "prune"]).map(|_| ())
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

    /// A repo at `<tmp>/<name>` with `f.txt`, `g.txt` and an ignore file
    /// that keeps `big/`, `.env` and `missing` out of git.
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

    /// Whether `cp` can clone a file inside `dir`: what a seed label is.
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

    #[test]
    fn reset_drops_edits_staging_untracked_files_and_a_cherry_pick_but_not_ignored_state() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let main = repo(tmp.path(), "app");
        fs::write(main.join(".env"), "SECRET=1\n").unwrap();
        // A commit off to the side that will conflict in the lane.
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
