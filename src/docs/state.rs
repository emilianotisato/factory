//! A feature's `state.toml` (PRD-002 FR-13): its status, its dates and one
//! `[slices.N]` table per slice. [`set`] is the one write path: under the
//! feature's `docs.lock`, read, apply, validate, write beside and rename.
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::io;
use crate::scheduler::write_atomic;
use crate::{Error, Result};

pub const FILE: &str = "state.toml";
pub const LOCK: &str = "docs.lock";

/// The feature vocabulary, v2's (`docs.mjs:19`).
pub const STATUSES: [&str; 7] = [
    "idea",
    "draft",
    "ready",
    "in-progress",
    "shipped",
    "archived",
    "retired",
];
/// The slice vocabulary (`docs.mjs:22`).
pub const SLICE_STATUSES: [&str; 4] = ["pending", "building", "blocked", "done"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub status: String,
    pub reviewed: Option<String>,
    pub shipped: Option<String>,
    pub slices: BTreeMap<u32, SliceState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SliceState {
    pub status: String,
    #[serde(default)]
    pub lane: Option<u32>,
    #[serde(default)]
    pub depends_on: Vec<u32>,
}

impl Default for SliceState {
    fn default() -> Self {
        SliceState {
            status: "pending".into(),
            lane: None,
            depends_on: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    status: String,
    reviewed: Option<String>,
    shipped: Option<String>,
    #[serde(default)]
    slices: BTreeMap<String, SliceState>,
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// `<dir>/state.toml`; `None` when the feature has none (a v2 folder).
pub fn read(dir: &Path) -> Result<Option<State>> {
    let p = path(dir);
    let text = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(format!("read {}", p.display()))(e)),
    };
    parse(&text)
        .map(Some)
        .map_err(|e| Error::Msg(format!("{}: {e}", p.display())))
}

pub fn parse(text: &str) -> std::result::Result<State, String> {
    let raw: Raw = toml::from_str(text).map_err(|e| e.message().to_string())?;
    let mut slices = BTreeMap::new();
    for (k, v) in raw.slices {
        let n = k
            .parse::<u32>()
            .map_err(|_| format!("[slices.{k}] is not a slice number"))?;
        slices.insert(n, v);
    }
    Ok(State {
        status: raw.status,
        reviewed: raw.reviewed,
        shipped: raw.shipped,
        slices,
    })
}

/// The file as [`set`] writes it: top keys, then one table per slice.
pub fn render(s: &State) -> String {
    let q = |v: &str| toml::Value::String(v.to_string()).to_string();
    let mut out = format!("status = {}\n", q(&s.status));
    if let Some(d) = &s.reviewed {
        out.push_str(&format!("reviewed = {}\n", q(d)));
    }
    if let Some(d) = &s.shipped {
        out.push_str(&format!("shipped = {}\n", q(d)));
    }
    for (n, sl) in &s.slices {
        out.push_str(&format!("\n[slices.{n}]\nstatus = {}\n", q(&sl.status)));
        if let Some(l) = sl.lane {
            out.push_str(&format!("lane = {l}\n"));
        }
        let deps: Vec<String> = sl.depends_on.iter().map(u32::to_string).collect();
        out.push_str(&format!("depends_on = [{}]\n", deps.join(", ")));
    }
    out
}

/// The feature's advisory lock, held until the `File` drops. `log add`
/// (slice 02) takes the same one.
pub fn lock(dir: &Path) -> Result<File> {
    let p = dir.join(LOCK);
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&p)
        .map_err(io(format!("open {}", p.display())))?;
    f.lock().map_err(io(format!("lock {}", p.display())))?;
    Ok(f)
}

/// Apply `key=value` pairs to `<dir>/state.toml` under the lock: all of them
/// or, on any refusal, none. Keys: `status`, `reviewed`, `shipped`,
/// `slices.N.status`, `slices.N.lane`, `slices.N.depends_on` (`1,2`). An
/// empty value removes `reviewed`, `shipped` or `slices.N.lane`. A slice
/// entry is created only for a slice that has its `slice-NN-*.md` file.
pub fn set(dir: &Path, pairs: &[(&str, &str)]) -> Result<State> {
    let _lock = lock(dir)?;
    // Read inside the lock: two lanes set their own slice at the same time.
    let mut s = read(dir)?.unwrap_or_default();
    let files = slice_files(dir)?;
    for (key, value) in pairs {
        apply(&mut s, &files, key, value.trim())?;
    }
    validate(&s)?;
    write_atomic(&path(dir), &render(&s))?;
    Ok(s)
}

/// `slice-NN-*.md` numbers in the folder.
pub fn slice_files(dir: &Path) -> Result<Vec<u32>> {
    let rd = fs::read_dir(dir).map_err(io(format!("read {}", dir.display())))?;
    let mut out: Vec<u32> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| slice_number(&e.file_name().to_string_lossy()))
        .collect();
    out.sort_unstable();
    Ok(out)
}

/// `^slice-(\d+)-(.+)\.md$` → the number.
pub fn slice_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("slice-")?.strip_suffix(".md")?;
    let (n, subject) = rest.split_once('-')?;
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && !subject.is_empty())
        .then(|| n.parse().ok())
        .flatten()
}

fn refuse<T>(msg: String) -> Result<T> {
    Err(Error::Msg(msg))
}

fn apply(s: &mut State, files: &[u32], key: &str, value: &str) -> Result<()> {
    let date = |v: &str| -> Result<Option<String>> {
        if v.is_empty() {
            Ok(None)
        } else if is_date(v) {
            Ok(Some(v.to_string()))
        } else {
            refuse(format!("{key}={v}: not a date (YYYY-MM-DD)"))
        }
    };
    match key {
        "status" => s.status = value.to_string(),
        "reviewed" => s.reviewed = date(value)?,
        "shipped" => s.shipped = date(value)?,
        _ => {
            let Some((n, field)) = key.strip_prefix("slices.").and_then(|r| r.split_once('.'))
            else {
                return refuse(unknown(key));
            };
            let Ok(n) = n.parse::<u32>() else {
                return refuse(unknown(key));
            };
            if !s.slices.contains_key(&n) && !files.contains(&n) {
                return refuse(format!("{key}: there is no slice {n}"));
            }
            let exists = |d: u32| s.slices.contains_key(&d) || files.contains(&d);
            let deps = if field == "depends_on" {
                let mut deps = Vec::new();
                for t in value.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                    match t.parse::<u32>() {
                        Ok(d) if d != n && exists(d) => deps.push(d),
                        Ok(d) if d == n => {
                            return refuse(format!("{key}: a slice cannot depend on itself"));
                        }
                        _ => return refuse(format!("{key}={value}: there is no slice {t}")),
                    }
                }
                Some(deps)
            } else {
                None
            };
            let slice = s.slices.entry(n).or_default();
            match field {
                "status" => slice.status = value.to_string(),
                "lane" if value.is_empty() => slice.lane = None,
                "lane" => match value.parse::<u32>() {
                    Ok(l) if l >= 1 => slice.lane = Some(l),
                    _ => return refuse(format!("{key}={value}: a lane is a number from 1")),
                },
                "depends_on" => slice.depends_on = deps.unwrap_or_default(),
                _ => return refuse(unknown(key)),
            }
        }
    }
    Ok(())
}

fn unknown(key: &str) -> String {
    format!(
        "unknown key `{key}`; known: status, reviewed, shipped, slices.N.status, slices.N.lane, slices.N.depends_on"
    )
}

fn validate(s: &State) -> Result<()> {
    if !STATUSES.contains(&s.status.as_str()) {
        return refuse(format!(
            "status \"{}\" is not one of {}",
            s.status,
            STATUSES.join(" | ")
        ));
    }
    for (n, sl) in &s.slices {
        if !SLICE_STATUSES.contains(&sl.status.as_str()) {
            return refuse(format!(
                "slices.{n}.status \"{}\" is not one of {}",
                sl.status,
                SLICE_STATUSES.join(" | ")
            ));
        }
    }
    Ok(())
}

/// `YYYY-MM-DD`, digits where digits go.
pub fn is_date(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(slices: &[&str]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in slices {
            fs::write(d.path().join(f), "# Slice\n").unwrap();
        }
        fs::write(path(d.path()), "status = \"draft\"\n").unwrap();
        d
    }

    fn text(d: &tempfile::TempDir) -> String {
        fs::read_to_string(path(d.path())).unwrap()
    }

    #[test]
    fn an_empty_lane_removes_the_key() {
        let d = feature(&["slice-01-a.md", "slice-02-b.md", "slice-03-c.md"]);
        let s = set(
            d.path(),
            &[("slices.3.status", "building"), ("slices.3.lane", "1")],
        )
        .unwrap();
        assert_eq!(s.slices[&3].lane, Some(1));
        let s = set(d.path(), &[("slices.3.lane", "")]).unwrap();
        assert_eq!(s.slices[&3].status, "building");
        assert_eq!(s.slices[&3].lane, None);
        assert_eq!(read(d.path()).unwrap().unwrap(), s);
        assert!(!text(&d).contains("lane"), "{}", text(&d));

        let s = set(
            d.path(),
            &[("reviewed", "2026-10-13"), ("slices.3.depends_on", "1, 2")],
        )
        .unwrap();
        assert_eq!(s.reviewed.as_deref(), Some("2026-10-13"));
        assert_eq!(s.slices[&3].depends_on, vec![1, 2]);
        let s = set(d.path(), &[("reviewed", "")]).unwrap();
        assert_eq!(s.reviewed, None);
    }

    #[test]
    fn refusals_name_the_vocabulary_and_leave_the_file_alone() {
        let d = feature(&["slice-01-a.md"]);
        let before = text(&d);
        let e = set(d.path(), &[("status", "later")])
            .unwrap_err()
            .to_string();
        for s in STATUSES {
            assert!(e.contains(s), "{e}");
        }
        assert_eq!(text(&d), before);

        let e = set(d.path(), &[("slices.9.status", "done")])
            .unwrap_err()
            .to_string();
        assert!(e.contains("no slice 9"), "{e}");
        let e = set(d.path(), &[("owner", "x")]).unwrap_err().to_string();
        assert!(e.contains("unknown key `owner`"), "{e}");
        let e = set(d.path(), &[("slices.1.status", "finished")])
            .unwrap_err()
            .to_string();
        assert!(e.contains("pending | building | blocked | done"), "{e}");
        let e = set(d.path(), &[("shipped", "13/10/2026")])
            .unwrap_err()
            .to_string();
        assert!(e.contains("YYYY-MM-DD"), "{e}");
        let e = set(d.path(), &[("slices.1.depends_on", "4")])
            .unwrap_err()
            .to_string();
        assert!(e.contains("no slice 4"), "{e}");
        // One bad pair refuses the whole call.
        assert!(set(d.path(), &[("status", "ready"), ("slices.1.lane", "0")]).is_err());
        assert_eq!(text(&d), before);
    }

    #[test]
    fn slice_names_match_the_v2_pattern() {
        assert_eq!(slice_number("slice-03-c.md"), Some(3));
        assert_eq!(slice_number("slice-10-two-words.md"), Some(10));
        assert_eq!(slice_number("slice-03.md"), None);
        assert_eq!(slice_number("slice-x-c.md"), None);
        assert_eq!(slice_number("prd.md"), None);
    }
}
