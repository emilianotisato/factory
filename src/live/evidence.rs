//! `evidence`: the files checkpoint workers left under the run's evidence dir.
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use super::Ctx;
use crate::clock;
use crate::docs::log;
use crate::events::EventKind;

/// The evidence dir, relative to the run dir; the only root ever printed.
pub const ROOT: &str = "evidence";

/// The deepest a listed file sits below the root, counting its own name.
const DEPTH: usize = 4;

/// A file younger than this may still be written.
const STABLE_MS: u64 = 2000;

const SIDECAR: &str = ".aria.txt";

struct File {
    path: String,
    dir: String,
    name: String,
    bytes: u64,
    mtime_ms: u64,
}

/// `checkpoints` runs newest first, and each one's `files` newest first.
/// Only `lstat` is used: no link is followed and no file is read but the feature's log.
pub fn evidence(ctx: &Ctx, limit: usize) -> Value {
    let root = ctx.run_dir.join(ROOT);
    let ours = checkpoints(ctx);
    let mut skipped = 0;
    let real = match fs::symlink_metadata(&root) {
        Ok(m) if m.is_dir() => true,
        Ok(_) => {
            skipped += 1;
            false
        }
        Err(_) => false,
    };
    let mut found: Vec<(u32, Vec<File>)> = Vec::new();
    for (&n, _) in ours.iter().rev().filter(|_| real) {
        let rel = format!("cp-{n}");
        let Ok(meta) = fs::symlink_metadata(root.join(&rel)) else {
            continue;
        };
        if !meta.is_dir() {
            skipped += 1;
            continue;
        }
        let mut files = Vec::new();
        walk(&root, &rel, 1, &mut files, &mut skipped);
        files.sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms).then(a.path.cmp(&b.path)));
        found.push((n, files));
    }
    let bodies = bodies(ctx);
    let now_ms = ctx.now.saturating_mul(1000);
    let mut left = limit;
    let mut truncated = false;
    let mut out = Vec::new();
    for (n, files) in found {
        let rows = rows(files);
        if rows.len() > left {
            truncated = true;
        }
        if left == 0 && !rows.is_empty() {
            break;
        }
        let take = rows.len().min(left);
        left -= take;
        let files: Vec<Value> = rows
            .into_iter()
            .take(take)
            .map(|(f, sidecars)| row(&f, &sidecars, &bodies, now_ms))
            .collect();
        let state = if ours[&n] { "done" } else { "writing" };
        out.push(json!({ "n": n, "state": state, "files": files }));
    }
    json!({
        "root": ROOT,
        "checkpoints": out,
        "skipped": skipped,
        "truncated": truncated,
    })
}

/// This run's checkpoint numbers, each with whether a `checkpoint_done` closed it.
fn checkpoints(ctx: &Ctx) -> BTreeMap<u32, bool> {
    let mut out = BTreeMap::new();
    for e in &ctx.events {
        match &e.kind {
            EventKind::CheckpointStarted { n, .. } | EventKind::CheckpointResumed { n, .. } => {
                out.entry(*n).or_insert(false);
            }
            EventKind::CheckpointDone { n, .. } => {
                out.insert(*n, true);
            }
            _ => {}
        }
    }
    out
}

/// `rel` is a directory `depth` components below `root`; a symlink, FIFO or device counts in `skipped`.
fn walk(root: &Path, rel: &str, depth: usize, files: &mut Vec<File>, skipped: &mut usize) {
    let Ok(entries) = fs::read_dir(root.join(rel)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{rel}/{name}");
        let Ok(meta) = fs::symlink_metadata(root.join(&path)) else {
            continue;
        };
        if meta.is_dir() {
            if depth + 1 < DEPTH {
                walk(root, &path, depth + 1, files, skipped);
            }
        } else if !meta.is_file() {
            *skipped += 1;
        } else if depth < DEPTH {
            let mtime_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64);
            files.push(File {
                path,
                dir: rel.to_string(),
                name,
                bytes: meta.len(),
                mtime_ms,
            });
        }
    }
}

/// Attaches each `x.aria.txt` to the first row stemmed `x` in its dir; one without a row is its own.
fn rows(files: Vec<File>) -> Vec<(File, Vec<String>)> {
    let (sidecars, plain): (Vec<File>, Vec<File>) =
        files.into_iter().partition(|f| f.name.ends_with(SIDECAR));
    let mut rows: Vec<(File, Vec<String>)> = plain.into_iter().map(|f| (f, Vec::new())).collect();
    let mut orphans = Vec::new();
    for s in sidecars {
        let stem = &s.name[..s.name.len() - SIDECAR.len()];
        match rows
            .iter_mut()
            .find(|(f, _)| f.dir == s.dir && stem_of(&f.name) == stem)
        {
            Some((_, attached)) => attached.push(s.path),
            None => orphans.push(s),
        }
    }
    if !orphans.is_empty() {
        rows.extend(orphans.into_iter().map(|f| (f, Vec::new())));
        rows.sort_by(|(a, _), (b, _)| b.mtime_ms.cmp(&a.mtime_ms).then(a.path.cmp(&b.path)));
    }
    rows
}

fn row(f: &File, sidecars: &[String], bodies: &[(String, String)], now_ms: u64) -> Value {
    let cited_by: Vec<&str> = bodies
        .iter()
        .filter(|(_, body)| body.contains(&f.path))
        .map(|(id, _)| id.as_str())
        .collect();
    json!({
        "path": f.path,
        "kind": kind(&f.name),
        "bytes": f.bytes,
        "mtime": clock::rfc3339(f.mtime_ms / 1000),
        "stable": now_ms.saturating_sub(f.mtime_ms) > STABLE_MS,
        "etag": format!("{}-{}", f.bytes, f.mtime_ms),
        "caption": caption(&f.name),
        "citedBy": cited_by,
        "sidecars": sidecars,
    })
}

/// The feature log's entries that carry an id, as `(id, body)`.
fn bodies(ctx: &Ctx) -> Vec<(String, String)> {
    ctx.feature_dir
        .as_deref()
        .and_then(|dir| log::entries(dir).ok())
        .map(|(entries, _)| {
            entries
                .into_iter()
                .filter_map(|e| Some((e.id?, e.body)))
                .collect()
        })
        .unwrap_or_default()
}

fn stem_of(name: &str) -> &str {
    let name = name.strip_suffix(SIDECAR).unwrap_or(name);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

fn kind(name: &str) -> &'static str {
    let ext = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_ascii_lowercase(),
        _ => return "other",
    };
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => "image",
        "txt" | "log" | "md" | "json" | "jsonl" | "csv" => "text",
        "pdf" | "html" => "document",
        "zip" | "tar" | "gz" | "tgz" => "archive",
        _ => "other",
    }
}

/// `case-statuses_before` reads `Case statuses before`.
fn caption(name: &str) -> String {
    let words: Vec<&str> = stem_of(name)
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .collect();
    let text = words.join(" ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_give_kind_stem_and_caption() {
        assert_eq!(kind("a.PNG"), "image");
        assert_eq!(kind("run.jsonl"), "text");
        assert_eq!(kind("page.html"), "document");
        assert_eq!(kind("dump.tgz"), "archive");
        assert_eq!(kind("Makefile"), "other");
        assert_eq!(kind(".png"), "other");
        assert_eq!(stem_of("home.aria.txt"), "home");
        assert_eq!(stem_of("home.png"), "home");
        assert_eq!(stem_of("a.tar.gz"), "a.tar");
        assert_eq!(caption("case-statuses_before.png"), "Case statuses before");
        assert_eq!(caption("home.aria.txt"), "Home");
        assert_eq!(caption("--.png"), "");
    }
}
