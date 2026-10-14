//! The `[docs]` and `[lanes]` tables of `docs/project.toml`, which the driver's `[run]` and `[workers.*]` tables share.
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::PROJECT_FILE;
use crate::error::io;
use crate::scheduler::write_atomic;
use crate::{Error, Result};

pub const KEYS: [&str; 8] = [
    "framework",
    "gate",
    "inner",
    "lanes",
    "gate_parallel",
    "checkpoint_every",
    "prd_max_kb",
    "slice_max_kb",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub framework: u32,
    pub gate: Option<String>,
    pub inner: Option<String>,
    pub lanes: u32,
    /// Tri-state: absent means nobody measured it, which is not `false`.
    pub gate_parallel: Option<bool>,
    pub checkpoint_every: u32,
    pub prd_max_kb: f64,
    pub slice_max_kb: f64,
}

pub fn path(root: &Path) -> PathBuf {
    root.join(PROJECT_FILE)
}

pub fn load(root: &Path) -> Result<Option<Project>> {
    let p = path(root);
    let text = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(format!("read {}", p.display()))(e)),
    };
    let table: toml::Table = text.parse().map_err(|source| Error::Toml {
        what: format!("read {}", p.display()),
        source,
    })?;
    let Some(toml::Value::Table(docs)) = table.get("docs") else {
        return Ok(None);
    };
    let int = |k: &str| docs.get(k).and_then(toml::Value::as_integer);
    let float = |k: &str, d: f64| match docs.get(k) {
        Some(toml::Value::Integer(n)) => *n as f64,
        Some(toml::Value::Float(f)) => *f,
        _ => d,
    };
    let string = |k: &str| {
        docs.get(k)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    let floor1 = |k: &str, d: i64| int(k).unwrap_or(d).max(1) as u32;
    Ok(Some(Project {
        framework: int("framework").unwrap_or(0).max(0) as u32,
        gate: string("gate"),
        inner: string("inner"),
        lanes: floor1("lanes", 1),
        gate_parallel: match docs.get("gate_parallel") {
            Some(toml::Value::Boolean(b)) => Some(*b),
            Some(toml::Value::String(s)) => Some(s == "true"),
            _ => None,
        },
        checkpoint_every: floor1("checkpoint_every", 2),
        prd_max_kb: float("prd_max_kb", 120.0),
        slice_max_kb: float("slice_max_kb", 30.0),
    }))
}

/// `seed = []` is a declaration, so `Some` of nothing; an absent key is `None`.
pub fn seed(root: &Path) -> Result<Option<Vec<String>>> {
    let p = path(root);
    let text = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(format!("read {}", p.display()))(e)),
    };
    let table: toml::Table = text.parse().map_err(|source| Error::Toml {
        what: format!("read {}", p.display()),
        source,
    })?;
    let Some(toml::Value::Array(items)) = table.get("lanes").and_then(|l| l.get("seed")) else {
        return Ok(None);
    };
    Ok(Some(
        items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
    ))
}

pub fn is_v3(root: &Path) -> bool {
    matches!(load(root), Ok(Some(p)) if p.framework == 3)
}

/// Edits the text, so every other table, comment and blank line stays byte for byte.
pub fn set(root: &Path, key: &str, value: Option<&toml::Value>) -> Result<()> {
    if !KEYS.contains(&key) {
        return Err(Error::Msg(format!(
            "unknown [docs] key `{key}`; known: {}",
            KEYS.join(", ")
        )));
    }
    let p = path(root);
    let text = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io(format!("read {}", p.display()))(e)),
    };
    let line = value.map(|v| format!("{key} = {v}"));
    write_atomic(&p, &edit(&text, key, line.as_deref()))
}

fn edit(text: &str, key: &str, line: Option<&str>) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let is_header = |l: &str| l.trim_start().starts_with('[');
    let Some(start) = lines.iter().position(|l| l.trim() == "[docs]") else {
        let Some(line) = line else {
            return text.to_string();
        };
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.trim().is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("[docs]\n{line}\n"));
        return out;
    };
    let end = lines[start + 1..]
        .iter()
        .position(|l| is_header(l))
        .map_or(lines.len(), |i| start + 1 + i);
    let found = (start + 1..end).find(|&i| {
        lines[i]
            .split_once('=')
            .is_some_and(|(k, _)| k.trim() == key)
    });
    match (found, line) {
        (Some(i), Some(l)) => lines[i] = l.to_string(),
        (Some(i), None) => {
            lines.remove(i);
        }
        (None, Some(l)) => {
            let last = (start..end)
                .rev()
                .find(|&i| !lines[i].trim().is_empty())
                .unwrap_or(start);
            lines.insert(last + 1, l.to_string());
        }
        (None, None) => {}
    }
    let mut out = lines.join("\n");
    if text.ends_with('\n') || text.is_empty() {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    fn repo(text: &str) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("docs")).unwrap();
        fs::write(path(d.path()), text).unwrap();
        d
    }

    #[test]
    fn a_bare_docs_table_loads_the_v2_defaults() {
        let d = repo("[docs]\nframework = 3\n");
        let p = load(d.path()).unwrap().unwrap();
        assert_eq!(p.framework, 3);
        assert_eq!((p.lanes, p.checkpoint_every), (1, 2));
        assert_eq!((p.prd_max_kb, p.slice_max_kb), (120.0, 30.0));
        assert_eq!((p.gate, p.inner, p.gate_parallel), (None, None, None));
        assert!(is_v3(d.path()));

        let d = repo("[docs]\nframework = 3\ngate_parallel = false\nlanes = 0\n");
        let p = load(d.path()).unwrap().unwrap();
        assert_eq!(p.gate_parallel, Some(false));
        assert_eq!(p.lanes, 1, "floored at 1");

        let d = repo("[run]\nbudget = 5\n");
        assert_eq!(load(d.path()).unwrap(), None);
        assert!(!is_v3(d.path()));
    }

    #[test]
    fn lanes_seed_tells_absent_from_empty() {
        let d = repo("[docs]\nframework = 3\n");
        assert_eq!(seed(d.path()).unwrap(), None, "no table");
        let d = repo("[docs]\nframework = 3\n[lanes]\nother = 1\n");
        assert_eq!(seed(d.path()).unwrap(), None, "no key");
        let d = repo("[docs]\nframework = 3\n[lanes]\nseed = []\n");
        assert_eq!(seed(d.path()).unwrap(), Some(vec![]));
        let d = repo("[docs]\nframework = 3\n[lanes]\nseed = [\"target\"]\n");
        assert_eq!(seed(d.path()).unwrap(), Some(vec!["target".to_string()]));
        let none = tempfile::tempdir().unwrap();
        assert_eq!(seed(none.path()).unwrap(), None, "no file");
    }

    const SHARED: &str = "# shared by the driver and the docs tool\n[docs]\nframework = 3\ngate = \"./gate.sh\"\n\n[run]\nbudget = 5\n\n[workers.polish]\nmodel = \"sonnet\"   # cheap\n";

    #[test]
    fn the_driver_tables_and_docs_share_the_file_without_warnings() {
        let d = repo(SHARED);
        let cfg = tempfile::tempdir().unwrap();
        let l = config::load_with_project(cfg.path(), d.path()).unwrap();
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert_eq!(l.config.budget_weighted_tokens, 5);
        assert_eq!(l.config.workers.polish.model, "sonnet");
        let p = load(d.path()).unwrap().unwrap();
        assert_eq!(p.gate.as_deref(), Some("./gate.sh"));
    }

    #[test]
    fn writing_a_key_leaves_the_other_tables_byte_identical() {
        let d = repo(SHARED);
        set(d.path(), "lanes", Some(&toml::Value::Integer(3))).unwrap();
        let text = fs::read_to_string(path(d.path())).unwrap();
        let tail = |t: &str| t[t.find("[run]").unwrap()..].to_string();
        assert_eq!(tail(&text), tail(SHARED));
        assert!(text.starts_with("# shared by the driver and the docs tool\n[docs]\n"));
        assert_eq!(load(d.path()).unwrap().unwrap().lanes, 3);

        set(
            d.path(),
            "gate",
            Some(&toml::Value::String("make verify".into())),
        )
        .unwrap();
        set(d.path(), "lanes", None).unwrap();
        let text = fs::read_to_string(path(d.path())).unwrap();
        assert_eq!(tail(&text), tail(SHARED));
        let p = load(d.path()).unwrap().unwrap();
        assert_eq!((p.gate.as_deref(), p.lanes), (Some("make verify"), 1));

        assert!(set(d.path(), "owner", None).is_err());
    }

    #[test]
    fn a_missing_docs_table_is_appended() {
        let d = repo("[run]\nbudget = 5\n");
        set(d.path(), "framework", Some(&toml::Value::Integer(3))).unwrap();
        let text = fs::read_to_string(path(d.path())).unwrap();
        assert_eq!(text, "[run]\nbudget = 5\n\n[docs]\nframework = 3\n");
    }
}
