//! Pre-existing test diffs against a run-start snapshot of authorized paths from the PRD.
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use regex::Regex;
use std::sync::LazyLock;

use crate::Result;
use crate::answers;
use crate::docs::frontmatter::{self, Value};
use crate::error::io;
use crate::events::EventKind;
use crate::run::Run;
use crate::scheduler;

pub const BLOCK_PREFIX: &str = "unauthorized test change:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub path: String,
    pub test_name: Option<String>,
    pub deleted: bool,
}

pub fn validate_entry(raw: &str) -> std::result::Result<(), String> {
    let item = raw.trim();
    if item.is_empty() {
        return Err("must not be empty".into());
    }
    if item.starts_with('/') {
        return Err("must not be absolute".into());
    }
    if item.split('/').any(|p| p == "..") {
        return Err("must not traverse with `..`".into());
    }
    if item.contains('*') || item.contains('?') || item.contains('[') {
        return Err("must not contain glob characters".into());
    }
    if let Some((path, name)) = item.split_once("::")
        && (path.trim().is_empty() || name.trim().is_empty())
    {
        return Err("`::` must separate a non-empty path and test name".into());
    }
    Ok(())
}

pub fn is_test_path(path: &str) -> bool {
    if path.starts_with("tests/")
        || path.starts_with("test/")
        || path.starts_with("__tests__/")
        || path.starts_with("spec/")
        || path.contains("/tests/")
        || path.contains("/__tests__/")
    {
        return true;
    }
    let Some((stem, ext)) = path.rsplit_once('.') else {
        return false;
    };
    if ext.is_empty() {
        return false;
    }
    let file = stem.rsplit('/').next().unwrap_or(stem);
    file.ends_with("_test")
        || file.ends_with(".test")
        || file.ends_with(".spec")
        || file.ends_with("Test")
        || file.ends_with("Tests")
}

pub fn prd_authorized_tests(prd_path: &Path) -> Result<Vec<String>> {
    let text =
        std::fs::read_to_string(prd_path).map_err(io(format!("read {}", prd_path.display())))?;
    let fm = frontmatter::parse(&text);
    Ok(list_of_strings(fm.get("authorized_tests")))
}

fn list_of_strings(v: Option<&Value>) -> Vec<String> {
    let Some(Value::List(items)) = v else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

pub fn check(repo: &Path, base_ref: &str, authorized: &[String]) -> Result<Vec<Violation>> {
    let range = format!("{base_ref}...HEAD");
    let diff = scheduler::git_ok(
        repo,
        &[
            "-c",
            "core.quotepath=off",
            "diff",
            "-U0",
            "--diff-filter=MDT",
            "--no-renames",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            &range,
        ],
    )?;
    Ok(parse_diff(repo, base_ref, authorized, &diff))
}

fn parse_diff(repo: &Path, base_ref: &str, authorized: &[String], diff: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for chunk in split_diff_files(diff) {
        let Some(path) = path_from_chunk(&chunk) else {
            continue;
        };
        if !is_test_path(&path) {
            continue;
        }
        if chunk.contains("deleted file mode") || chunk.contains("+++ /dev/null") {
            if authorized.iter().any(|a| a == &path) {
                continue;
            }
            push_violation(
                &mut out,
                &mut seen,
                Violation {
                    path: path.clone(),
                    test_name: None,
                    deleted: true,
                },
            );
            continue;
        }
        for hunk in hunks_in_chunk(&chunk) {
            if hunk.removed == 0 {
                continue;
            }
            if authorized.iter().any(|a| a == &path) {
                continue;
            }
            let enclosing = enclosing_test_name(repo, base_ref, &path, hunk.old_start, &hunk.ctx);
            if let Some(name) = &enclosing {
                let keyed = format!("{path}::{name}");
                if authorized.iter().any(|a| a == &keyed) {
                    continue;
                }
            }
            if enclosing.is_none() && is_additive_top_level_import(&path, &hunk) {
                continue;
            }
            push_violation(
                &mut out,
                &mut seen,
                Violation {
                    path: path.clone(),
                    test_name: enclosing,
                    deleted: false,
                },
            );
        }
    }
    out
}

fn push_violation(
    out: &mut Vec<Violation>,
    seen: &mut HashSet<(String, Option<String>, bool)>,
    v: Violation,
) {
    let key = (v.path.clone(), v.test_name.clone(), v.deleted);
    if seen.insert(key) {
        out.push(v);
    }
}

struct Hunk {
    old_start: usize,
    removed: usize,
    ctx: String,
    removed_lines: Vec<String>,
    added_lines: Vec<String>,
}

type RustImport = (String, String, String);
type JsImport = (bool, String, String, String);

fn is_additive_top_level_import(path: &str, hunk: &Hunk) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => {
            let mut items = BTreeSet::new();
            for line in &hunk.removed_lines {
                if line.trim().is_empty() {
                    continue;
                }
                let Some(parsed) = rust_use_items(line) else {
                    return false;
                };
                items.extend(parsed);
            }
            if items.is_empty() {
                return false;
            }
            for line in &hunk.added_lines {
                if line.trim().is_empty() {
                    continue;
                }
                if rust_use_items(line).is_none() {
                    return false;
                }
            }
            let added = rust_use_from_lines(&hunk.added_lines);
            items.is_subset(&added)
        }
        "ts" | "tsx" | "js" | "jsx" | "mjs" => {
            let mut items = BTreeSet::new();
            for line in &hunk.removed_lines {
                if line.trim().is_empty() {
                    continue;
                }
                let Some(parsed) = js_import_items(line) else {
                    return false;
                };
                items.extend(parsed);
            }
            if items.is_empty() {
                return false;
            }
            let added = js_import_from_lines(&hunk.added_lines);
            if !hunk
                .added_lines
                .iter()
                .all(|line| line.trim().is_empty() || js_import_items(line).is_some())
            {
                return false;
            }
            items.is_subset(&added)
        }
        _ => false,
    }
}

fn rust_use_from_lines(lines: &[String]) -> BTreeSet<RustImport> {
    let mut out = BTreeSet::new();
    for line in lines {
        if let Some(items) = rust_use_items(line) {
            out.extend(items);
        }
    }
    out
}

fn js_import_from_lines(lines: &[String]) -> BTreeSet<JsImport> {
    let mut out = BTreeSet::new();
    for line in lines {
        if let Some(items) = js_import_items(line) {
            out.extend(items);
        }
    }
    out
}

fn rust_use_items(line: &str) -> Option<Vec<RustImport>> {
    let line = line.trim();
    let (vis, rest) = strip_rust_use_vis(line)?;
    let rest = rest.strip_prefix("use")?.trim_start();
    let rest = rest.strip_suffix(';')?.trim();
    if rest.is_empty() {
        return None;
    }
    let items = expand_rust_use(vis, rest);
    if items.is_empty() { None } else { Some(items) }
}

fn strip_rust_use_vis(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.starts_with("pub(") {
        let close = line.find(')')?;
        let vis = &line[..=close];
        Some((vis, line[close + 1..].trim_start()))
    } else if line.starts_with("pub ") {
        Some(("pub", line[3..].trim_start()))
    } else {
        Some(("", line))
    }
}

fn expand_rust_use(vis: &str, rest: &str) -> Vec<RustImport> {
    let vis = vis.to_string();
    if let Some((prefix, brace)) = rest.split_once("::{") {
        let prefix = prefix.trim();
        let Some(inner) = brace.strip_suffix('}') else {
            return Vec::new();
        };
        expand_rust_brace(&vis, prefix, inner.trim())
    } else if let Some((full, local)) = rust_single_item(rest) {
        vec![(vis, full, local)]
    } else {
        Vec::new()
    }
}

fn expand_rust_brace(vis: &str, prefix: &str, inner: &str) -> Vec<RustImport> {
    let mut out = Vec::new();
    let parent = prefix.trim();
    for part in split_brace_items(inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part == "self" {
            let local = parent.rsplit("::").next().unwrap_or(parent).to_string();
            out.push((vis.to_string(), parent.to_string(), local));
        } else if part.contains('{') {
            let (sub, brace) = part.split_once("::{").unwrap();
            let sub_prefix = if parent.is_empty() {
                sub.trim().to_string()
            } else {
                format!("{}::{}", parent, sub.trim())
            };
            let inner = brace.strip_suffix('}').unwrap_or(brace);
            out.extend(expand_rust_brace(vis, &sub_prefix, inner));
        } else if let Some((tail, local)) = rust_brace_part(part, parent) {
            out.push((vis.to_string(), tail, local));
        }
    }
    out
}

fn split_brace_items(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in inner.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(inner[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(inner[start..].to_string());
    out
}

fn rust_brace_part(part: &str, parent: &str) -> Option<(String, String)> {
    let part = part.trim();
    let (path, local) = if let Some((p, alias)) = part.split_once(" as ") {
        (p.trim(), alias.trim().to_string())
    } else {
        (part, part.to_string())
    };
    let full = if parent.is_empty() {
        path.to_string()
    } else {
        format!("{}::{}", parent, path)
    };
    Some((full, local))
}

fn rust_single_item(rest: &str) -> Option<(String, String)> {
    let rest = rest.trim();
    if rest.ends_with("::*") {
        let p = rest.strip_suffix("::*")?;
        return Some((format!("{}::*", p), "*".to_string()));
    }
    let (path, local) = if let Some((p, alias)) = rest.rsplit_once(" as ") {
        (p.trim(), alias.trim().to_string())
    } else {
        let local = rest.rsplit("::").next()?.to_string();
        (rest, local)
    };
    Some((path.to_string(), local))
}

fn js_import_items(line: &str) -> Option<Vec<JsImport>> {
    let line = line.trim();
    let is_type = line.starts_with("import type ");
    let rest = if is_type {
        line.strip_prefix("import type ")?
    } else if line.starts_with("import ") {
        line.strip_prefix("import ")?
    } else {
        return None;
    };
    let rest = rest.trim().strip_suffix(';')?.trim();
    let (clause, spec) = rest.rsplit_once(" from ")?;
    let spec = js_module_spec(spec)?;
    parse_js_clause(is_type, &spec, clause.trim())
}

fn js_module_spec(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if (raw.starts_with('\'') && raw.ends_with('\''))
        || (raw.starts_with('"') && raw.ends_with('"'))
    {
        Some(raw[1..raw.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_js_clause(is_type: bool, spec: &str, clause: &str) -> Option<Vec<JsImport>> {
    let mut out = Vec::new();
    if clause.starts_with('*') {
        let rest = clause.strip_prefix('*')?.trim();
        let local = rest.strip_prefix("as")?.trim();
        out.push((
            is_type,
            spec.to_string(),
            "*".to_string(),
            local.to_string(),
        ));
        return Some(out);
    }
    let (default, named) = if clause.starts_with('{') {
        (None, Some(clause))
    } else if let Some((d, n)) = clause.split_once(',') {
        let n = n.trim();
        if n.starts_with('{') {
            (Some(d.trim()), Some(n))
        } else {
            return None;
        }
    } else {
        (Some(clause), None)
    };
    if let Some(d) = default
        && !d.is_empty()
    {
        out.push((
            is_type,
            spec.to_string(),
            "default".to_string(),
            d.to_string(),
        ));
    }
    if let Some(n) = named {
        let inner = n.trim().strip_prefix('{')?.strip_suffix('}')?;
        for part in split_brace_items(inner) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (imported, local) = if let Some((i, alias)) = part.split_once(" as ") {
                (i.trim(), alias.trim().to_string())
            } else {
                (part, part.to_string())
            };
            out.push((
                is_type,
                spec.to_string(),
                imported.to_string(),
                local.to_string(),
            ));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn split_diff_files(diff: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in diff.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn path_from_chunk(chunk: &str) -> Option<String> {
    let mut path = None;
    for line in chunk.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/")
            && rest != "/dev/null"
        {
            return Some(trim_diff_path(rest));
        }
        if let Some(rest) = line.strip_prefix("--- a/") {
            path = Some(trim_diff_path(rest));
        }
    }
    path
}

fn trim_diff_path(path: &str) -> String {
    path.trim_end_matches('\t').to_string()
}

static HUNK_HEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$").unwrap());

fn hunks_in_chunk(chunk: &str) -> Vec<Hunk> {
    let mut out = Vec::new();
    let mut in_hunk = false;
    let mut old_start = 0usize;
    let mut removed = 0usize;
    let mut ctx = String::new();
    let mut removed_lines = Vec::new();
    let mut added_lines = Vec::new();
    for line in chunk.lines() {
        if let Some(cap) = HUNK_HEAD.captures(line) {
            if in_hunk {
                out.push(Hunk {
                    old_start,
                    removed,
                    ctx: ctx.trim().to_string(),
                    removed_lines: std::mem::take(&mut removed_lines),
                    added_lines: std::mem::take(&mut added_lines),
                });
            }
            old_start = cap[1].parse().unwrap_or(1);
            removed = 0;
            ctx = cap
                .get(5)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }
        if line.starts_with('-') && !line.starts_with("---") {
            removed += 1;
            removed_lines.push(line[1..].to_string());
        } else if line.starts_with('+') && !line.starts_with("+++") {
            added_lines.push(line[1..].to_string());
        }
    }
    if in_hunk {
        out.push(Hunk {
            old_start,
            removed,
            ctx: ctx.trim().to_string(),
            removed_lines,
            added_lines,
        });
    }
    out
}

static RUST_FN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)(?:pub\s+)?(?:async\s+)?fn\s+(\w+)").unwrap());
static JS_FN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)(?:function|func)\s+(\w+)").unwrap());
static PY_DEF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)def\s+(\w+)").unwrap());
static TEST_IT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:^|\s)(?:test|it)\(\s*["']([^"']+)["']"#).unwrap());

fn enclosing_test_name(
    repo: &Path,
    base_ref: &str,
    path: &str,
    old_start: usize,
    ctx: &str,
) -> Option<String> {
    if let Some(name) = name_in_line(ctx) {
        return Some(name);
    }
    let tree_ref = merge_base_for_diff(repo, base_ref).unwrap_or_else(|| base_ref.to_string());
    let spec = format!("{tree_ref}:{path}");
    let text = scheduler::git_ok(repo, &["show", &spec]).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let start = old_start
        .saturating_sub(1)
        .min(lines.len().saturating_sub(1));
    for i in (0..=start).rev() {
        if let Some(name) = name_in_line(lines[i]) {
            return Some(name);
        }
    }
    None
}

fn merge_base_for_diff(repo: &Path, base_ref: &str) -> Option<String> {
    let out = scheduler::git_ok(repo, &["merge-base", base_ref, "HEAD"]).ok()?;
    let sha = out.trim();
    if sha.is_empty() {
        None
    } else {
        Some(sha.to_string())
    }
}

fn name_in_line(line: &str) -> Option<String> {
    for re in [&*RUST_FN, &*JS_FN, &*PY_DEF, &*TEST_IT] {
        if let Some(cap) = re.captures(line) {
            return Some(cap[1].to_string());
        }
    }
    None
}

pub fn format_block_body(violations: &[Violation]) -> String {
    let mut body = String::from("**Paths:**\n");
    for v in violations {
        if let Some(name) = &v.test_name {
            body.push_str(&format!("- `{}::{}`\n", v.path, name));
        } else {
            body.push_str(&format!("- `{}`\n", v.path));
        }
    }
    body.push_str(
        "\n**Options:**\n\
         1. `allow` — authorize these paths for the rest of this run\n\
         2. revert the unlisted test edits and answer to retry\n",
    );
    body
}

pub fn violated_paths_from_body(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for line in body.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("- `") else {
            continue;
        };
        let Some(end) = rest.find('`') else {
            continue;
        };
        let token = &rest[..end];
        let path = token.split_once("::").map(|(p, _)| p).unwrap_or(token);
        if seen.insert(path.to_string()) {
            out.push(path.to_string());
        }
    }
    out
}

pub fn sync_allow_answers(run: &mut Run, feature_dir: &Path) -> Result<()> {
    let allowed_ids: HashSet<String> = run
        .events()?
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::BlockAnswered { id, text, .. }
                if text.trim().eq_ignore_ascii_case("allow") =>
            {
                Some(id.clone())
            }
            _ => None,
        })
        .collect();
    if allowed_ids.is_empty() {
        return Ok(());
    }
    let entries = answers::entries(feature_dir)?;
    let mut paths = Vec::new();
    for entry in entries {
        if !entry.title.starts_with(BLOCK_PREFIX) || !entry.resolved {
            continue;
        }
        if !allowed_ids.contains(&entry.id) {
            continue;
        }
        paths.extend(violated_paths_from_body(&entry.body));
    }
    if paths.is_empty() {
        return Ok(());
    }
    run.add_authorized_tests(&paths)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn git(repo: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(repo)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .status()
                .unwrap()
                .success(),
            "git {:?}",
            args
        );
    }

    fn repo_with_base_test() -> (tempfile::TempDir, String) {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "user.email", "t@example.invalid"]);
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/existing_test.rs"),
            "#[test]\nfn sample_test() {\n    assert_eq!(1, 1);\n}\n",
        )
        .unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "base"]);
        let base = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        let base = String::from_utf8_lossy(&base.stdout).trim().to_string();
        (d, base)
    }

    #[test]
    fn is_test_path_matches_expected_patterns() {
        for ok in [
            "tests/foo.rs",
            "test/foo.rs",
            "__tests__/foo.js",
            "spec/a.rs",
            "pkg/tests/foo.rs",
            "pkg/__tests__/foo.js",
            "foo_test.rs",
            "foo.test.js",
            "foo.spec.ts",
            "WidgetTest.java",
            "WidgetsTests.java",
        ] {
            assert!(is_test_path(ok), "{ok}");
        }
        for bad in ["src/config.rs", "src/lib.rs", "README.md", "gate.sh"] {
            assert!(!is_test_path(bad), "{bad}");
        }
    }

    #[test]
    fn additive_and_new_test_files_do_not_violate() {
        let (d, base) = repo_with_base_test();
        let root = d.path();
        fs::write(root.join("tests/new_test.rs"), "#[test]\nfn fresh() {}\n").unwrap();
        fs::write(
            root.join("tests/existing_test.rs"),
            "#[test]\nfn sample_test() {\n    assert_eq!(1, 1);\n}\n#[test]\nfn extra() {}\n",
        )
        .unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "add"]);
        assert!(check(root, &base, &[]).unwrap().is_empty());
    }

    #[test]
    fn removed_lines_violate_unless_authorized() {
        let (d, base) = repo_with_base_test();
        let root = d.path();
        fs::write(
            root.join("tests/existing_test.rs"),
            "#[test]\nfn sample_test() {}\n",
        )
        .unwrap();
        git(root, &["add", "tests/existing_test.rs"]);
        git(root, &["commit", "-q", "-m", "weaken"]);
        let v = check(root, &base, &[]).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].path, "tests/existing_test.rs");
        assert_eq!(v[0].test_name.as_deref(), Some("sample_test"));
        assert!(!v[0].deleted);
        assert!(
            check(root, &base, &["tests/existing_test.rs".into()])
                .unwrap()
                .is_empty()
        );
        assert!(
            check(root, &base, &["tests/existing_test.rs::other_test".into()])
                .unwrap()
                .len()
                == 1
        );
        assert!(
            check(root, &base, &["tests/existing_test.rs::sample_test".into()])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn deleted_test_file_violates_unless_whole_path_authorized() {
        let (d, base) = repo_with_base_test();
        let root = d.path();
        fs::remove_file(root.join("tests/existing_test.rs")).unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "delete"]);
        let v = check(root, &base, &[]).unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].deleted);
        assert!(
            check(root, &base, &["tests/existing_test.rs::sample_test".into()])
                .unwrap()
                .len()
                == 1
        );
        assert!(
            check(root, &base, &["tests/existing_test.rs".into()])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn format_and_parse_block_body_round_trip_paths() {
        let v = vec![Violation {
            path: "tests/existing_test.rs".into(),
            test_name: Some("sample_test".into()),
            deleted: false,
        }];
        let body = format_block_body(&v);
        assert_eq!(
            violated_paths_from_body(&body),
            vec!["tests/existing_test.rs".to_string()]
        );
    }

    #[test]
    fn removed_fixture_lines_count_as_removals() {
        let (d, base) = repo_with_base_test();
        let root = d.path();
        fs::write(
            root.join("tests/existing_test.rs"),
            "#[test]\nfn sample_test() {\n    let yaml = \"- item: 1\\n- item: 2\\n\";\n    assert!(yaml.contains(\"1\"));\n}\n",
        )
        .unwrap();
        git(root, &["add", "tests/existing_test.rs"]);
        git(root, &["commit", "-q", "-m", "fixture"]);
        fs::write(
            root.join("tests/existing_test.rs"),
            "#[test]\nfn sample_test() {\n    let yaml = \"- item: 1\\n\";\n    assert!(yaml.contains(\"1\"));\n}\n",
        )
        .unwrap();
        git(root, &["add", "tests/existing_test.rs"]);
        git(root, &["commit", "-q", "-m", "drop line"]);
        assert_eq!(check(root, &base, &[]).unwrap().len(), 1);
    }

    #[test]
    fn sync_allow_answers_ignores_resolved_blocks_without_block_answered_event() {
        let (d, _) = repo_with_base_test();
        let root = d.path();
        let data = tempfile::tempdir().unwrap();
        let mut run = crate::run::Run::create(data.path(), root, "001", None, 1).unwrap();
        let feature_dir = root.join("docs/features/001-x");
        fs::create_dir_all(&feature_dir).unwrap();
        let body = format_block_body(&[Violation {
            path: "tests/existing_test.rs".into(),
            test_name: None,
            deleted: false,
        }]);
        let title = answers::block_title(&format!("{BLOCK_PREFIX} tests/existing_test.rs"));
        let block_id =
            answers::add_block(&feature_dir, None, Some("checkpoint"), &title, &body).unwrap();
        answers::resolve(
            &feature_dir,
            &answers::find(&feature_dir, &block_id).unwrap().unwrap(),
            "allow",
        )
        .unwrap();
        sync_allow_answers(&mut run, &feature_dir).unwrap();
        assert!(run.state().authorized_tests.is_empty());
    }

    #[test]
    fn subset_use_removal_violates() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "user.email", "t@example.invalid"]);
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/sample.rs"),
            "use foo::{a, b};\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
        )
        .unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "both"]);
        let mid = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        let mid = String::from_utf8_lossy(&mid.stdout).trim().to_string();
        fs::write(
            root.join("tests/sample.rs"),
            "use foo::a;\n\n#[test]\nfn existing_test() {\n    assert!(true);\n}\n",
        )
        .unwrap();
        git(root, &["add", "tests/sample.rs"]);
        git(root, &["commit", "-q", "-m", "remove"]);
        assert!(
            !check(root, &mid, &[]).unwrap().is_empty(),
            "{:?}",
            check(root, &mid, &[]).unwrap()
        );
    }

    #[test]
    fn js_import_brace_merge_does_not_violate() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "user.email", "t@example.invalid"]);
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/sample.test.ts"),
            "import { a } from 'x';\n\ntest('x', () => {});\n",
        )
        .unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "base"]);
        let base = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        let base = String::from_utf8_lossy(&base.stdout).trim().to_string();
        fs::write(
            root.join("tests/sample.test.ts"),
            "import { a, b } from 'x';\n\ntest('x', () => {});\n",
        )
        .unwrap();
        git(root, &["add", "tests/sample.test.ts"]);
        git(root, &["commit", "-q", "-m", "merge"]);
        assert!(
            check(root, &base, &[]).unwrap().is_empty(),
            "{:?}",
            check(root, &base, &[]).unwrap()
        );
    }

    #[test]
    fn top_level_use_merge_does_not_violate() {
        let (d, base) = repo_with_base_test();
        let root = d.path();
        fs::write(
            root.join("tests/existing_test.rs"),
            "use foo::a;\n\n#[test]\nfn sample_test() {\n    assert_eq!(1, 1);\n}\n",
        )
        .unwrap();
        git(root, &["add", "tests/existing_test.rs"]);
        git(root, &["commit", "-q", "-m", "imports"]);
        fs::write(
            root.join("tests/existing_test.rs"),
            "use foo::{a, b};\n\n#[test]\nfn sample_test() {\n    assert_eq!(1, 1);\n}\n",
        )
        .unwrap();
        git(root, &["add", "tests/existing_test.rs"]);
        git(root, &["commit", "-q", "-m", "merge"]);
        assert!(
            check(root, &base, &[]).unwrap().is_empty(),
            "{:?}",
            check(root, &base, &[]).unwrap()
        );
    }

    #[test]
    fn validate_entry_rejects_bad_tokens() {
        for bad in ["", "../x", "/abs", "tests/*.rs", "::only", "path::"] {
            assert!(validate_entry(bad).is_err(), "{bad}");
        }
        assert!(validate_entry("tests/a.rs").is_ok());
        assert!(validate_entry("tests/b.rs::fn_name").is_ok());
    }
}
