//! The sweep never edits `docs/archive/`, which is frozen; it names what it left alone.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::board::{self, Kind};
use crate::error::io;
use crate::{Error, Result};

fn git(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn walk_md(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk_md(&p, into);
        } else if p.extension().is_some_and(|e| e == "md") {
            into.push(p);
        }
    }
}

fn set_number(text: &str, n: &str) -> String {
    let Some(rest) = text.strip_prefix("---\n") else {
        return text.to_string();
    };
    let Some(end) = rest.find("\n---") else {
        return text.to_string();
    };
    let block: Vec<String> = rest[..end]
        .split('\n')
        .map(|line| match line.strip_prefix("number:") {
            Some(v) => {
                let v = v.trim();
                let q = if v.starts_with('"') {
                    "\""
                } else if v.starts_with('\'') {
                    "'"
                } else {
                    ""
                };
                format!("number: {q}{n}{q}")
            }
            None => line.to_string(),
        })
        .collect();
    format!("---\n{}{}", block.join("\n"), &rest[end..])
}

pub fn run(root: &Path, from: &str, to: &str, out: &mut dyn Write) -> Result<()> {
    let b = board::scan(root)?;
    let f = b.resolve(from)?;
    if f.kind == Kind::Archive {
        return Err(Error::Msg(format!(
            "PRD-{:03} is archived; the archive is frozen",
            f.number
        )));
    }
    let n: u32 = to
        .parse()
        .map_err(|_| Error::Msg("usage: factory docs renumber <old> <new>".into()))?;
    if b.all().any(|x| x.number == n) {
        return Err(Error::Msg(format!("PRD-{n:03} already exists")));
    }
    let old_dir = f.dir_name.clone();
    let new_dir = format!("{n:03}-{}", f.slug);
    let (old_ref, new_ref) = (format!("PRD-{:03}", f.number), format!("PRD-{n:03}"));
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let dest = f.dir.with_file_name(&new_dir);
    if !git(root, &["mv", &rel(&f.dir), &rel(&dest)]) {
        fs::rename(&f.dir, &dest).map_err(io(format!("move {}", f.dir.display())))?;
    }
    let prd = dest.join("prd.md");
    if let Ok(text) = fs::read_to_string(&prd) {
        let next = set_number(&text, &format!("{n:03}"));
        if next != text {
            fs::write(&prd, next).map_err(io(format!("write {}", prd.display())))?;
        }
    }

    let docs = root.join("docs");
    let archive = docs.join("archive");
    let mut files = Vec::new();
    walk_md(&docs, &mut files);
    let mut touched: Vec<String> = Vec::new();
    let mut left: Vec<String> = Vec::new();
    for file in files {
        let Ok(t) = fs::read_to_string(&file) else {
            continue;
        };
        if file.starts_with(&archive) {
            if t.contains(&old_dir) || t.contains(&old_ref) {
                left.push(rel(&file));
            }
            continue;
        }
        let next = t.replace(&old_dir, &new_dir).replace(&old_ref, &new_ref);
        if next != t {
            fs::write(&file, next).map_err(io(format!("write {}", file.display())))?;
            touched.push(rel(&file));
        }
    }
    // Stage only what moved and changed: a sibling's unstaged work in docs/ is not this command's.
    let mut add = vec!["add".to_string(), "--".into(), rel(&dest)];
    add.extend(touched.iter().cloned());
    git(root, &add.iter().map(String::as_str).collect::<Vec<_>>());

    let mut text = format!(
        "renumbered {old_dir} → {new_dir} ({} file(s) updated)\n",
        touched.len()
    );
    for l in &left {
        text.push_str(&format!("left alone: {l}\n"));
    }
    text.push_str(&format!(
        "{old_ref} is now retired and must never be reused — add it to the retired numbers in docs/index.md.\n"
    ));
    out.write_all(text.as_bytes()).map_err(io("write stdout"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_number_line_changes() {
        let t = "---\nnumber: 001\nslug: x\nsummary: |\n  number: 001 stays\n---\n# PRD-001\n";
        assert_eq!(
            set_number(t, "005"),
            "---\nnumber: 005\nslug: x\nsummary: |\n  number: 001 stays\n---\n# PRD-001\n"
        );
        assert_eq!(
            set_number("---\nnumber: \"001\"\n---\n", "007"),
            "---\nnumber: \"007\"\n---\n"
        );
        assert_eq!(set_number("no frontmatter", "005"), "no frontmatter");
    }
}
