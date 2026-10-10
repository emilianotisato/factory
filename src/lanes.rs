//! Lanes (PRD §5 FR-3). The board says how many there are (`lanes:` in
//! `docs/index.md`) and which are free; a lane file,
//! `docs/reference/lanes/lane-N.md`, says what a worker on lane N gets: an
//! `env` list of `KEY=value` and a `setup:` command the driver runs once per
//! lane per run before the first spawn on it. A repo with no lane files has
//! lanes that differ in nothing but their number; every worker still sees
//! `FACTORY_LANE=N`.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::io;
use crate::{Error, Result};

pub const DIR: &str = "docs/reference/lanes";
pub const ENV_LANE: &str = "FACTORY_LANE";

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
                    let item = item.trim().trim_matches(|c| c == '"' || c == '\'');
                    if let Some((k, v)) = item.split_once('=')
                        && !k.trim().is_empty()
                    {
                        lane.env.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
            }
            "setup" => {
                let cmd = value.trim_matches(|c| c == '"' || c == '\'').trim();
                if !cmd.is_empty() {
                    lane.setup = Some(cmd.to_string());
                }
            }
            _ => {}
        }
    }
    lane
}

impl Lane {
    /// What a worker on this lane gets on top of the driver's environment:
    /// `FACTORY_LANE=N` first, then the file's `env` in order.
    pub fn env(&self) -> Vec<(String, String)> {
        let mut out = vec![(ENV_LANE.to_string(), self.n.to_string())];
        out.extend(self.env.iter().cloned());
        out
    }

    /// Run `setup` in `repo` with the lane's env; nothing declared, nothing
    /// run. A failing setup is a tool error: the lane is not usable.
    pub fn setup(&self, repo: &Path) -> Result<bool> {
        let Some(cmd) = &self.setup else {
            return Ok(false);
        };
        let name = format!("lane {} setup: {cmd}", self.n);
        let out = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(repo)
            .envs(self.env())
            .output()
            .map_err(|e| Error::Tool {
                cmd: name.clone(),
                detail: format!("could not start sh: {e}"),
            })?;
        if !out.status.success() {
            return Err(Error::Tool {
                cmd: name,
                detail: format!(
                    "exit {}: {}",
                    out.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            });
        }
        Ok(true)
    }
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
}
