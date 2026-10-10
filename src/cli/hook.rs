//! `factory hook` (PRD §14 FR-12 decision 3, §4 FR-2 as corrected
//! 2026-10-09): the PreToolUse command hook the driver injects into every
//! worker through `--settings` (`worker::settings`). It reads the tool call
//! as JSON on stdin and answers with a `hookSpecificOutput` decision on
//! stdout, exit 0, in two cases a deny with a one-line hint the model reads:
//!
//! - `Bash` whose first command is `grep -r`/`-R` or `rg` over a directory
//!   under the cwd: use `factory ctx refs` (or `def`) instead.
//! - a whole-file `Read`, or `Bash cat`, of a file a slice pack holds: name
//!   the pack section that already carries it.
//!
//! Everything else is allowed, and so is anything the hook cannot parse: a
//! hook bug must never block a worker. No I/O beyond stdin and the pack's
//! first line (`FACTORY_PACK`), so it answers in milliseconds.
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

use crate::Result;
use crate::error::io;
use crate::shell;

/// The worker env var naming the pack file (`worker::spawn`).
pub const PACK_ENV: &str = "FACTORY_PACK";

/// The deny reason for a recursive grep over the repo (§14 decision 3).
pub const GREP_HINT: &str = "use: factory ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output";

/// The pack's first line, as `pack::Pack` writes it (slice 03 task 2).
const HEADER: &str = "<!-- factory-pack files: ";

/// The hook's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny(String),
}

/// What the hook knows of the worker's pack: the files its header lists and
/// whether it is a build worker's slice pack, the only kind that carries
/// files in full (a checkpoint pack carries each slice's Prove task, the
/// others carry no file).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Held {
    pub files: Vec<String>,
    pub slice_pack: bool,
}

impl Held {
    /// From the pack's file name and first line.
    pub fn parse(pack_name: &str, first_line: &str) -> Held {
        let files = first_line
            .trim_end()
            .strip_prefix(HEADER)
            .and_then(|rest| rest.strip_suffix("-->"))
            .map(|list| {
                list.split(',')
                    .map(str::trim)
                    .filter(|f| !f.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Held {
            files,
            slice_pack: pack_name.starts_with("slice-"),
        }
    }

    /// Read the first line of the pack at `path`; nothing held when it
    /// cannot be read.
    pub fn load(path: &Path) -> Held {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let mut line = String::new();
        if let Ok(f) = std::fs::File::open(path) {
            let _ = BufReader::new(f).read_line(&mut line);
        }
        Held::parse(name, &line)
    }

    /// The section of the pack that carries `path` (absolute, or relative
    /// to nothing in particular: matched as the held path or a suffix of
    /// it), when a whole-file read of it is one to refuse. `prd.md` is
    /// never one (the pack has cuts, the file is longer), nor is the log
    /// file of an answered block (the pack has the entry only).
    pub fn section(&self, path: &str) -> Option<&'static str> {
        if !self.slice_pack {
            return None;
        }
        let at = self
            .files
            .iter()
            .position(|f| path == f || path.ends_with(&format!("/{f}")))?;
        let name = shell::program(&self.files[at]);
        if name == "prd.md" {
            return None;
        }
        if at == 0 {
            return Some("Slice");
        }
        if name == "agent-log.md" || (name.starts_with("log-") && name.ends_with(".md")) {
            return None;
        }
        Some("Outline")
    }
}

/// The reason for a whole read of a held file.
pub fn pack_hint(section: &str) -> String {
    format!("that file is in your pack under \"# {section}\"; read a range if you need more")
}

/// The verdict on one PreToolUse input.
pub fn decide(input: &Value, held: &Held, cwd: &Path) -> Verdict {
    let tool = input.get("tool_name").and_then(Value::as_str).unwrap_or("");
    let args = input.get("tool_input").unwrap_or(&Value::Null);
    match tool {
        "Read" => {
            let whole = ["offset", "limit"]
                .iter()
                .all(|k| args.get(*k).is_none_or(Value::is_null));
            let path = args.get("file_path").and_then(Value::as_str).unwrap_or("");
            match (whole, absolute(path, cwd)) {
                (true, Some(full)) => match held.section(&full) {
                    Some(s) => Verdict::Deny(pack_hint(s)),
                    None => Verdict::Allow,
                },
                _ => Verdict::Allow,
            }
        }
        "Bash" => {
            let command = args.get("command").and_then(Value::as_str).unwrap_or("");
            bash(command, held, cwd)
        }
        _ => Verdict::Allow,
    }
}

fn bash(command: &str, held: &Held, cwd: &Path) -> Verdict {
    let (words, end) = shell::first_command(command);
    let Some(first) = words.first() else {
        return Verdict::Allow;
    };
    let args = &words[1..];
    match shell::program(first) {
        "grep" | "egrep" | "fgrep" if over_repo(grep_paths(args), cwd) => {
            Verdict::Deny(GREP_HINT.to_string())
        }
        "rg" if over_repo(rg_paths(args), cwd) => Verdict::Deny(GREP_HINT.to_string()),
        // `cat f | head`: a range, not the whole file.
        "cat" if !matches!(end, Some("|" | "|&")) => {
            for a in args.iter().filter(|a| !a.starts_with('-')) {
                if let Some(s) = absolute(a, cwd).and_then(|full| held.section(&full)) {
                    return Verdict::Deny(pack_hint(s));
                }
            }
            Verdict::Allow
        }
        _ => Verdict::Allow,
    }
}

/// A recursive search reaches the repo when it names no path (the cwd) or
/// a path under the cwd that reads as a directory.
fn over_repo(paths: Option<Vec<&str>>, cwd: &Path) -> bool {
    match paths {
        None => false,
        Some(p) if p.is_empty() => true,
        Some(p) => p.iter().any(|p| repo_dir(p, cwd)),
    }
}

/// `grep`'s path operands when the call is recursive, else `None`.
fn grep_paths(args: &[String]) -> Option<Vec<&str>> {
    const WITH_ARG: &[&str] = &[
        "--regexp",
        "--file",
        "--max-count",
        "--after-context",
        "--before-context",
        "--context",
        "--include",
        "--exclude",
        "--exclude-dir",
        "--exclude-from",
        "--label",
        "--binary-files",
        "--devices",
        "--directories",
        "--group-separator",
    ];
    let mut recursive = false;
    let mut pattern_given = false;
    let mut positional = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if a == "--" {
            positional.extend(args[i..].iter().map(String::as_str));
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or(long);
            match name {
                "recursive" | "dereference-recursive" => recursive = true,
                "regexp" | "file" => pattern_given = true,
                _ => {}
            }
            if !long.contains('=') && WITH_ARG.contains(&format!("--{name}").as_str()) {
                i += 1;
            }
            continue;
        }
        if let Some(cluster) = a.strip_prefix('-').filter(|c| !c.is_empty()) {
            for (k, c) in cluster.char_indices() {
                match c {
                    'r' | 'R' => recursive = true,
                    'e' | 'f' | 'm' | 'A' | 'B' | 'C' | 'd' | 'D' => {
                        if matches!(c, 'e' | 'f') {
                            pattern_given = true;
                        }
                        if k + c.len_utf8() == cluster.len() {
                            i += 1;
                        }
                        break;
                    }
                    _ => {}
                }
            }
            continue;
        }
        positional.push(a);
    }
    if !recursive {
        return None;
    }
    if !pattern_given && !positional.is_empty() {
        positional.remove(0);
    }
    Some(positional)
}

/// `rg`'s path operands (it always recurses); `None` for `--files` and
/// `--type-list`, which list rather than search.
fn rg_paths(args: &[String]) -> Option<Vec<&str>> {
    const SHORT_ARG: &str = "efgtTmABCjMrEd";
    const WITH_ARG: &[&str] = &[
        "regexp",
        "file",
        "glob",
        "iglob",
        "type",
        "type-not",
        "type-add",
        "max-count",
        "after-context",
        "before-context",
        "context",
        "threads",
        "max-columns",
        "max-depth",
        "max-filesize",
        "replace",
        "encoding",
        "ignore-file",
        "pre",
        "pre-glob",
        "sort",
        "sortr",
        "color",
        "colors",
        "path-separator",
        "context-separator",
        "engine",
    ];
    let mut pattern_given = false;
    let mut positional = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if a == "--" {
            positional.extend(args[i..].iter().map(String::as_str));
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or(long);
            match name {
                "files" | "type-list" => return None,
                "regexp" | "file" => pattern_given = true,
                _ => {}
            }
            if !long.contains('=') && WITH_ARG.contains(&name) {
                i += 1;
            }
            continue;
        }
        if let Some(cluster) = a.strip_prefix('-').filter(|c| !c.is_empty()) {
            for (k, c) in cluster.char_indices() {
                if SHORT_ARG.contains(c) {
                    if matches!(c, 'e' | 'f') {
                        pattern_given = true;
                    }
                    if k + c.len_utf8() == cluster.len() {
                        i += 1;
                    }
                    break;
                }
            }
            continue;
        }
        positional.push(a);
    }
    if !pattern_given && !positional.is_empty() {
        positional.remove(0);
    }
    Some(positional)
}

/// A path under the cwd that reads as a directory: `.`, a trailing `/`, or
/// a last component with no extension (`src`, `app`). No `stat`: the hook
/// does no I/O beyond the pack's first line, and a missing path is a
/// directory as often as not.
fn repo_dir(p: &str, cwd: &Path) -> bool {
    let Some(full) = absolute(p, cwd) else {
        return false;
    };
    let Some(cwd) = cwd.to_str() else {
        return false;
    };
    let cwd = cwd.trim_end_matches('/');
    if full != cwd && !full.starts_with(&format!("{cwd}/")) {
        return false;
    }
    if p.ends_with('/') || full == cwd {
        return true;
    }
    let name = shell::program(&full);
    match name.rfind('.') {
        Some(0) | None => true,
        Some(dot) => dot + 1 == name.len(),
    }
}

/// `p` made absolute against `cwd` and normalised lexically (`.` and `..`
/// resolved); `None` for `~`, `$VAR` and the empty path, which the hook
/// does not expand.
fn absolute(p: &str, cwd: &Path) -> Option<String> {
    if p.is_empty() || p.starts_with('~') || p.starts_with('$') {
        return None;
    }
    let joined: PathBuf = cwd.join(p);
    let mut parts: Vec<&str> = Vec::new();
    for c in joined.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?),
            Component::ParentDir => {
                parts.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// The `hookSpecificOutput` line for `v`.
pub fn output(v: &Verdict) -> Value {
    let mut out = json!({
        "hookEventName": "PreToolUse",
        "permissionDecision": "allow",
    });
    if let Verdict::Deny(reason) = v {
        out["permissionDecision"] = json!("deny");
        out["permissionDecisionReason"] = json!(reason);
    }
    json!({ "hookSpecificOutput": out })
}

/// Read the call on `stdin`, answer on `out`; never a non-zero exit.
pub fn hook_with(
    stdin: &mut dyn Read,
    pack: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let mut text = String::new();
    let verdict = match stdin
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())
        .and_then(|_| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()))
    {
        Ok(input) if input.is_object() => {
            let held = pack.map(Held::load).unwrap_or_default();
            let cwd = input
                .get("cwd")
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("/"));
            decide(&input, &held, &cwd)
        }
        Ok(_) => {
            let _ = writeln!(err, "factory hook: stdin is not a JSON object; allowed");
            Verdict::Allow
        }
        Err(e) => {
            let _ = writeln!(err, "factory hook: cannot parse stdin ({e}); allowed");
            Verdict::Allow
        }
    };
    writeln!(out, "{}", output(&verdict)).map_err(io("write stdout"))?;
    Ok(0)
}

/// `factory hook`: the pack from `FACTORY_PACK`.
pub fn hook(stdin: &mut dyn Read, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let pack = std::env::var_os(PACK_ENV).map(PathBuf::from);
    hook_with(stdin, pack.as_deref(), out, err)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLICE: &str = "docs/features/001-x/slice-02-b.md";

    fn held() -> Held {
        Held::parse(
            "slice-02-1.md",
            &format!(
                "<!-- factory-pack files: {SLICE},docs/features/001-x/prd.md,docs/features/001-x/agent-log.md,src/helper.rs -->\n"
            ),
        )
    }

    fn bash(cmd: &str) -> Verdict {
        decide(
            &json!({"tool_name": "Bash", "tool_input": {"command": cmd}}),
            &held(),
            Path::new("/repo"),
        )
    }

    fn read(path: &str, extra: Value) -> Verdict {
        let mut input = json!({"file_path": path});
        for (k, v) in extra.as_object().unwrap() {
            input[k] = v.clone();
        }
        decide(
            &json!({"tool_name": "Read", "tool_input": input}),
            &held(),
            Path::new("/repo"),
        )
    }

    fn denied(v: Verdict) -> bool {
        matches!(v, Verdict::Deny(_))
    }

    #[test]
    fn recursive_grep_over_the_repo_is_denied_with_the_verb() {
        for cmd in [
            "grep -rn foo src",
            "grep -R foo",
            "grep -rn foo .",
            "cd /repo && grep -rni --include=*.php foo app/",
            "grep --recursive -e foo src/Http",
            "rg foo",
            "rg -n 'a b' src",
            "rg foo /repo/src | head -20",
            "egrep -r 'a|b' ./",
        ] {
            assert_eq!(bash(cmd), Verdict::Deny(GREP_HINT.to_string()), "{cmd}");
        }
        for cmd in [
            "grep -n foo src/one.php",
            "grep -rn foo src/one.php",
            "cat src/x.php | grep foo",
            "rg foo src/one.php",
            "grep -rn foo /var/log",
            "grep -rn foo ../other",
            "rg --files src",
            "git grep -n foo",
            "grep -m 3 foo storage/logs/laravel.log",
            "grep -A2 -r foo storage/app.log",
        ] {
            assert_eq!(bash(cmd), Verdict::Allow, "{cmd}");
        }
    }

    #[test]
    fn a_whole_read_of_a_held_file_names_the_section() {
        let want = |s: &str| Verdict::Deny(pack_hint(s));
        assert_eq!(read(&format!("/repo/{SLICE}"), json!({})), want("Slice"));
        assert_eq!(read(SLICE, json!({"offset": null})), want("Slice"));
        assert_eq!(read("/repo/src/helper.rs", json!({})), want("Outline"));
        assert_eq!(
            read(
                &format!("/repo/{SLICE}"),
                json!({"offset": 10, "limit": 40})
            ),
            Verdict::Allow
        );
        assert_eq!(
            read("/repo/src/helper.rs", json!({"limit": 40})),
            Verdict::Allow
        );
        assert_eq!(
            read("/repo/docs/features/001-x/prd.md", json!({})),
            Verdict::Allow
        );
        assert_eq!(
            read("/repo/docs/features/001-x/agent-log.md", json!({})),
            Verdict::Allow
        );
        assert_eq!(read("/repo/src/other.rs", json!({})), Verdict::Allow);
        assert_eq!(bash(&format!("cat {SLICE}")), want("Slice"));
        assert_eq!(bash("cd /repo && cat src/helper.rs"), want("Outline"));
        assert!(!denied(bash(&format!("cat {SLICE} | head -40"))));
        assert!(!denied(bash(&format!("sed -n 1,40p {SLICE}"))));
        // A checkpoint pack carries no file in full.
        let cp = Held::parse(
            "checkpoint-1-4.md",
            &format!("<!-- factory-pack files: {SLICE} -->"),
        );
        let v = decide(
            &json!({"tool_name": "Read", "tool_input": {"file_path": format!("/repo/{SLICE}")}}),
            &cp,
            Path::new("/repo"),
        );
        assert_eq!(v, Verdict::Allow);
    }

    #[test]
    fn malformed_input_is_allowed_with_a_line_on_stderr() {
        for input in ["", "not json", "[1]"] {
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = hook_with(&mut input.as_bytes(), None, &mut out, &mut err).unwrap();
            assert_eq!(code, 0);
            let v: Value = serde_json::from_slice(&out).unwrap();
            assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "allow");
            assert!(
                String::from_utf8_lossy(&err).contains("allowed"),
                "{input:?}"
            );
        }
    }
}
