//! The worker's PreToolUse hook: no I/O beyond stdin, the pack's first line and a `stat`, so it answers in milliseconds.
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

use crate::Result;
use crate::error::io;
use crate::paths;
use crate::shell;

pub const PACK_ENV: &str = "FACTORY_PACK";

pub const NEVER_HINT: &str = "workers never run gh or git push: the driver pushes the branch and talks to GitHub; commit locally, and put what you need from GitHub in your report";

pub fn grep_hint() -> String {
    format!(
        "use: {} ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output",
        paths::factory_command()
    )
}

const HEADER: &str = "<!-- factory-pack files: ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// No decision: the call goes on to the worker's permission rules.
    Allow,
    Deny(String),
}

/// Only a slice pack carries files in full; a checkpoint pack carries each slice's Prove task.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Held {
    pub files: Vec<String>,
    pub slice_pack: bool,
}

impl Held {
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

    pub fn load(path: &Path) -> Held {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let mut line = String::new();
        if let Ok(f) = std::fs::File::open(path) {
            let _ = BufReader::new(f).read_line(&mut line);
        }
        Held::parse(name, &line)
    }

    /// `prd.md` and `log.jsonl` are never held: the pack carries cuts of them, not the files.
    pub fn section(&self, path: &str, repo: &Path) -> Option<&'static str> {
        if !self.slice_pack {
            return None;
        }
        let repo = repo.to_str()?.trim_end_matches('/');
        let at = self
            .files
            .iter()
            .position(|f| path.strip_prefix(repo).and_then(|r| r.strip_prefix('/')) == Some(f))?;
        let name = shell::program(&self.files[at]);
        if name == "prd.md" {
            return None;
        }
        if at == 0 {
            return Some("Slice");
        }
        if name == "log.jsonl" {
            return None;
        }
        Some("Outline")
    }
}

pub fn pack_hint(section: &str) -> String {
    format!("that file is in your pack under \"# {section}\"; read a range if you need more")
}

pub fn decide(input: &Value, held: &Held, repo: &Path, cwd: &Path) -> Verdict {
    let tool = input.get("tool_name").and_then(Value::as_str).unwrap_or("");
    let args = input.get("tool_input").unwrap_or(&Value::Null);
    match tool {
        "Read" => {
            let whole = ["offset", "limit"]
                .iter()
                .all(|k| args.get(*k).is_none_or(Value::is_null));
            let path = args.get("file_path").and_then(Value::as_str).unwrap_or("");
            match (whole, absolute(path, cwd)) {
                (true, Some(full)) => match held.section(&full, repo) {
                    Some(s) => Verdict::Deny(pack_hint(s)),
                    None => Verdict::Allow,
                },
                _ => Verdict::Allow,
            }
        }
        "Bash" => {
            let command = args.get("command").and_then(Value::as_str).unwrap_or("");
            bash(command, held, repo, cwd)
        }
        _ => Verdict::Allow,
    }
}

fn bash(command: &str, held: &Held, repo: &Path, cwd: &Path) -> Verdict {
    if guard(command) {
        return Verdict::Deny(NEVER_HINT.to_string());
    }
    let (words, end, cds) = shell::first_command_in(command);
    // A `cd` the hook cannot follow (`cd`, `cd -`, `cd ~`, `cd $D`) leaves nothing to judge paths by.
    let mut here = cwd.to_path_buf();
    for target in cds {
        match target
            .filter(|t| t != "-")
            .and_then(|t| absolute(&t, &here))
        {
            Some(dir) => here = PathBuf::from(dir),
            None => return Verdict::Allow,
        }
    }
    let Some(first) = words.first() else {
        return Verdict::Allow;
    };
    let args = &words[1..];
    match shell::program(first) {
        "grep" | "egrep" | "fgrep" if over_repo(grep_paths(args), &here, repo) => {
            Verdict::Deny(grep_hint())
        }
        "rg" if over_repo(rg_paths(args), &here, repo) => Verdict::Deny(grep_hint()),
        // `cat f | head`: a range, not the whole file.
        "cat" if !matches!(end, Some("|" | "|&")) => {
            for a in args.iter().filter(|a| !a.starts_with('-')) {
                if let Some(s) = absolute(a, &here).and_then(|full| held.section(&full, repo)) {
                    return Verdict::Deny(pack_hint(s));
                }
            }
            Verdict::Allow
        }
        _ => Verdict::Allow,
    }
}

/// Script files, aliases and make targets are out of sight: this holds an agent under pressure, not one evading it.
pub fn guard(command: &str) -> bool {
    forbidden(command, 0)
}

const MAX_DEPTH: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Fine,
    Forbidden,
    Unsure,
}

fn forbidden(text: &str, depth: u32) -> bool {
    if depth > MAX_DEPTH {
        return mentions(text);
    }
    let script = shell::script(text);
    let mut unsure = script.unbalanced;
    for c in &script.commands {
        let words: Vec<&str> = c.words.iter().map(String::as_str).collect();
        match reach(&words, &c.stdin, depth) {
            Reach::Forbidden => return true,
            Reach::Unsure => unsure = true,
            Reach::Fine => {}
        }
    }
    unsure && mentions(text)
}

/// Never unsure: the nested read already weighed its own doubt against its own text.
fn nested(text: &str, depth: u32) -> Reach {
    if forbidden(text, depth + 1) {
        Reach::Forbidden
    } else {
        Reach::Fine
    }
}

fn mentions(text: &str) -> bool {
    let b = text.to_ascii_lowercase().into_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    ["gh", "push"].iter().any(|w| {
        let w = w.as_bytes();
        (0..b.len()).any(|i| {
            b[i..].starts_with(w)
                && (i == 0 || !word(b[i - 1]))
                && b.get(i + w.len()).is_none_or(|c| !word(*c))
        })
    })
}

fn expansion(w: &str) -> bool {
    w.contains(['$', '`', '*', '?', '[', '{', '}'])
}

/// Words that open a compound command or negate one; the command is next.
const RESERVED: [&str; 12] = [
    "!", "{", "}", "if", "then", "elif", "else", "fi", "do", "done", "while", "until",
];

fn reach(w: &[&str], stdin: &[String], depth: u32) -> Reach {
    let mut i = 0;
    loop {
        while i < w.len() && (shell::is_assignment(w[i]) || RESERVED.contains(&w[i])) {
            i += 1;
        }
        let Some(&first) = w.get(i) else {
            return Reach::Fine;
        };
        if expansion(first) {
            return Reach::Unsure;
        }
        let args = &w[i + 1..];
        let skip = |short: &str, long: &[&str], positional: usize| {
            i + 1 + skip_opts(args, short, long, positional)
        };
        i = match shell::program(first) {
            "gh" | "git-push" | "git-send-pack" => return Reach::Forbidden,
            "git" => return git(args),
            "for" | "case" | "select" | "function" | "in" | "esac" | "coproc" => {
                return Reach::Fine;
            }
            "alias" | "parallel" => return Reach::Unsure,
            "eval" => return nested(&args.join(" "), depth),
            "trap" => {
                return match args.first() {
                    Some(a) if !a.starts_with('-') => nested(a, depth),
                    _ => Reach::Fine,
                };
            }
            "env" => match env_split(args) {
                Some(line) => return nested(&line, depth),
                None => skip("uCa", &["unset", "chdir", "argv0"], 0),
            },
            "command" | "builtin" | "nohup" | "setsid" | "unbuffer" | "chronic" | "busybox"
            | "fakeroot" | "torsocks" | "nsenter" | "unshare" => skip("", &[], 0),
            "exec" => skip("a", &[], 0),
            "nice" => skip("n", &["adjustment"], 0),
            "time" => skip("fo", &["format", "output"], 0),
            "timeout" => skip("sk", &["signal", "kill-after"], 1),
            "stdbuf" => skip("ioe", &["input", "output", "error"], 0),
            "ionice" => skip("cnpPu", &["class", "classdata", "pid", "pgid", "uid"], 0),
            "taskset" | "chrt" => skip("", &[], 1),
            "proxychains" | "proxychains4" => skip("f", &[], 0),
            "sudo" => skip(
                "CDgpRrTtUu",
                &[
                    "chdir",
                    "close-from",
                    "group",
                    "host",
                    "prompt",
                    "chroot",
                    "role",
                    "type",
                    "command-timeout",
                    "other-user",
                    "user",
                ],
                0,
            ),
            "doas" => skip("uC", &[], 0),
            "xargs" => skip(
                "adEILnPs",
                &[
                    "arg-file",
                    "delimiter",
                    "max-args",
                    "max-procs",
                    "max-chars",
                    "process-slot-var",
                ],
                0,
            ),
            "flock" => {
                let at = skip_opts(args, "wE", &["timeout", "conflict-exit-code"], 1);
                if let Some(s) = option_value(&args[at.min(args.len())..], "-c", "--command") {
                    return nested(s, depth);
                }
                i + 1 + at
            }
            "watch" => {
                let at = skip_opts(args, "nq", &["interval", "equexit"], 0);
                return nested(&args[at.min(args.len())..].join(" "), depth);
            }
            "script" | "su" | "runuser" => {
                return match option_value(args, "-c", "--command") {
                    Some(s) => nested(s, depth),
                    None => Reach::Fine,
                };
            }
            "find" => return find_exec(args, depth),
            "bash" | "sh" | "zsh" | "dash" | "ksh" | "mksh" | "ash" | "yash" | "posh" | "fish" => {
                return shell_call(args, stdin, depth);
            }
            p => return interpreter(p, args),
        };
    }
}

/// `short` and `long` are the wrapper's options that take a value; `positional` operands precede its command.
fn skip_opts(args: &[&str], short: &str, long: &[&str], positional: usize) -> usize {
    let mut i = 0;
    while i < args.len() {
        let a = args[i];
        if a == "--" {
            i += 1;
            break;
        }
        if let Some(name) = a.strip_prefix("--") {
            i += 1;
            if !name.contains('=') && long.contains(&name) {
                i += 1;
            }
            continue;
        }
        match a.strip_prefix('-').filter(|c| !c.is_empty()) {
            Some(cluster) => {
                i += 1;
                if let Some((k, c)) = cluster.char_indices().find(|(_, c)| short.contains(*c))
                    && k + c.len_utf8() == cluster.len()
                {
                    i += 1;
                }
            }
            None => break,
        }
    }
    (i + positional).min(args.len())
}

fn option_value<'a>(args: &[&'a str], short: &str, long: &str) -> Option<&'a str> {
    let long_eq = format!("{long}=");
    for (k, a) in args.iter().enumerate() {
        if *a == short || *a == long {
            return args.get(k + 1).copied();
        }
        if let Some(v) = a.strip_prefix(long_eq.as_str()) {
            return Some(v);
        }
        if let Some(v) = a
            .strip_prefix(short)
            .filter(|v| !v.is_empty() && !a.starts_with("--"))
        {
            return Some(v);
        }
    }
    None
}

fn env_split(args: &[&str]) -> Option<String> {
    for (k, a) in args.iter().enumerate() {
        let (line, rest) = if *a == "-S" || *a == "--split-string" {
            (args.get(k + 1).copied()?, k + 2)
        } else if let Some(v) = a.strip_prefix("--split-string=") {
            (v, k + 1)
        } else if let Some(v) = a.strip_prefix("-S").filter(|v| !v.is_empty()) {
            (v, k + 1)
        } else if a.starts_with('-') {
            continue;
        } else {
            return None;
        };
        let mut out = line.to_string();
        for w in &args[rest.min(args.len())..] {
            out.push(' ');
            out.push_str(&paths::shell_word(w));
        }
        return Some(out);
    }
    None
}

/// An alias defined on the line (`-c alias.x=…`) could spell `push`, so it is forbidden outright.
fn git(args: &[&str]) -> Reach {
    const VALUED: [&str; 8] = [
        "-C",
        "-c",
        "--git-dir",
        "--work-tree",
        "--namespace",
        "--config-env",
        "--super-prefix",
        "--attr-source",
    ];
    let alias = |v: &str| v.trim_start().to_ascii_lowercase().starts_with("alias.");
    let mut i = 0;
    while let Some(&a) = args.get(i) {
        if !a.starts_with('-') {
            break;
        }
        i += 1;
        if VALUED.contains(&a) {
            let v = args.get(i).copied().unwrap_or("");
            i += 1;
            if matches!(a, "-c" | "--config-env") && alias(v) {
                return Reach::Forbidden;
            }
            if a == "-c" && v.contains('$') {
                return Reach::Unsure;
            }
        } else if let Some(v) = a.strip_prefix("--config-env=")
            && alias(v)
        {
            return Reach::Forbidden;
        }
    }
    let Some(&sub) = args.get(i) else {
        return Reach::Fine;
    };
    if expansion(sub) {
        return Reach::Unsure;
    }
    match sub {
        "push" | "send-pack" => Reach::Forbidden,
        "subtree" | "lfs" if args[i + 1..].contains(&"push") => Reach::Forbidden,
        _ => Reach::Fine,
    }
}

fn find_exec(args: &[&str], depth: u32) -> Reach {
    let mut worst = Reach::Fine;
    let mut k = 0;
    while k < args.len() {
        if matches!(args[k], "-exec" | "-execdir" | "-ok" | "-okdir") {
            let from = k + 1;
            let mut to = from;
            while to < args.len() && !matches!(args[to], ";" | "+") {
                to += 1;
            }
            match reach(&args[from..to], &[], depth) {
                Reach::Forbidden => return Reach::Forbidden,
                Reach::Unsure => worst = Reach::Unsure,
                Reach::Fine => {}
            }
            k = to;
        }
        k += 1;
    }
    worst
}

/// A shell reading stdin runs what a pipe brings, which the hook cannot see; a here-document it can.
fn shell_call(args: &[&str], stdin: &[String], depth: u32) -> Reach {
    let mut command = false;
    let mut i = 0;
    while let Some(&a) = args.get(i) {
        if a == "--" || a == "-" {
            i += 1;
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            i += 1;
            match long {
                "command" => command = true,
                "rcfile" | "init-file" => i += 1,
                _ if long.starts_with("command=") => {
                    return nested(&long["command=".len()..], depth);
                }
                _ => {}
            }
            continue;
        }
        let Some(cluster) = a.strip_prefix('-').or_else(|| a.strip_prefix('+')) else {
            break;
        };
        i += 1;
        if a.starts_with('-') && cluster.contains('c') {
            command = true;
        }
        if cluster.contains(['o', 'O']) {
            i += 1;
        }
    }
    if command {
        return match args.get(i) {
            Some(line) => nested(line, depth),
            None => Reach::Fine,
        };
    }
    if i < args.len() {
        return Reach::Fine;
    }
    if stdin.is_empty() {
        return Reach::Unsure;
    }
    if stdin.iter().any(|doc| forbidden(doc, depth + 1)) {
        Reach::Forbidden
    } else {
        Reach::Fine
    }
}

fn interpreter(program: &str, args: &[&str]) -> Reach {
    let inline: &[&str] = match program {
        p if p.starts_with("python") || p.starts_with("pypy") => &["c"],
        "node" | "nodejs" | "bun" => &["e", "p", "-eval", "-print"],
        "perl" => &["e", "E"],
        "ruby" | "lua" | "luajit" | "Rscript" | "osascript" => &["e"],
        "php" => &["r"],
        "deno" => {
            return if args.first() == Some(&"eval") {
                Reach::Unsure
            } else {
                Reach::Fine
            };
        }
        "tclsh" | "wish" => &[],
        _ => return Reach::Fine,
    };
    for a in args {
        let Some(cluster) = a.strip_prefix('-') else {
            return Reach::Fine;
        };
        if cluster.is_empty() {
            return Reach::Unsure;
        }
        let long = cluster.starts_with('-');
        let hit = inline.iter().any(|f| {
            if f.starts_with('-') {
                long && (cluster == *f || cluster.starts_with(&format!("{f}=")))
            } else {
                !long && cluster.contains(*f)
            }
        });
        if hit {
            return Reach::Unsure;
        }
    }
    Reach::Unsure
}

fn over_repo(paths: Option<Vec<&str>>, here: &Path, repo: &Path) -> bool {
    match paths {
        None => false,
        Some(p) if p.is_empty() => here.to_str().is_some_and(|h| under(h, repo)),
        Some(p) => p.iter().any(|p| repo_dir(p, here, repo)),
    }
}

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

/// `rg` always recurses; `--files` and `--type-list` list rather than search.
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

/// A glob ending in `*` or `**` reaches directories; anything else is a directory only when the disk says so.
fn repo_dir(p: &str, here: &Path, repo: &Path) -> bool {
    let Some(full) = absolute(p, here) else {
        return false;
    };
    if !under(&full, repo) {
        return false;
    }
    if p.contains(['*', '?', '[']) {
        return matches!(shell::program(p.trim_end_matches('/')), "*" | "**");
    }
    Path::new(&full).is_dir()
}

fn under(full: &str, repo: &Path) -> bool {
    let Some(repo) = repo.to_str() else {
        return false;
    };
    let repo = repo.trim_end_matches('/');
    full == repo || full.starts_with(&format!("{repo}/"))
}

/// Lexical, never touching the disk; `~` and `$VAR` are not expanded.
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

pub fn output(v: &Verdict) -> Option<Value> {
    match v {
        Verdict::Allow => None,
        Verdict::Deny(reason) => Some(json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        })),
    }
}

/// Never a non-zero exit: a hook bug must not block a worker.
pub fn hook_with(
    stdin: &mut dyn Read,
    pack: Option<&Path>,
    repo: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let mut text = String::new();
    let read = stdin.read_to_string(&mut text).map_err(|e| e.to_string());
    let verdict =
        match read.and_then(|_| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())) {
            Ok(input) if input.is_object() => {
                let held = pack.map(Held::load).unwrap_or_default();
                let cwd = input
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(|| PathBuf::from("/"));
                let root = repo.unwrap_or(&cwd);
                let root = root
                    .to_str()
                    .and_then(|r| absolute(r, Path::new("/")))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| root.to_path_buf());
                decide(&input, &held, &root, &cwd)
            }
            Ok(_) => unread(&text, "stdin is not a JSON object", err),
            Err(e) => unread(&text, &format!("cannot parse stdin ({e})"), err),
        };
    if let Some(v) = output(&verdict) {
        writeln!(out, "{v}").map_err(io("write stdout"))?;
    }
    Ok(0)
}

fn unread(text: &str, why: &str, err: &mut dyn Write) -> Verdict {
    if mentions(text) {
        let _ = writeln!(err, "factory hook: {why}; it names gh or push: denied");
        return Verdict::Deny(NEVER_HINT.to_string());
    }
    let _ = writeln!(err, "factory hook: {why}; allowed (no decision)");
    Verdict::Allow
}

pub fn hook(stdin: &mut dyn Read, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let pack = std::env::var_os(PACK_ENV).map(PathBuf::from);
    let repo = std::env::var_os(crate::worker::REPO_ENV)
        .filter(|r| !r.is_empty())
        .map(PathBuf::from);
    hook_with(stdin, pack.as_deref(), repo.as_deref(), out, err)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLICE: &str = "docs/features/001-x/slice-02-b.md";

    fn held() -> Held {
        Held::parse(
            "slice-02-1.md",
            &format!(
                "<!-- factory-pack files: {SLICE},docs/features/001-x/prd.md,docs/features/001-x/log.jsonl,docs/features/001-x/agent-log.md,src/helper.rs -->\n"
            ),
        )
    }

    /// On disk, so a grep operand can be asked whether it is a directory.
    fn repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for d in ["src/Http", "app", "storage/logs", "docs/features/001-x"] {
            std::fs::create_dir_all(tmp.path().join(d)).unwrap();
        }
        for f in [
            "src/one.php",
            "src/x.php",
            "storage/app.log",
            "storage/logs/laravel.log",
            ".env",
            "Makefile",
            "artisan",
        ] {
            std::fs::write(tmp.path().join(f), "x\n").unwrap();
        }
        tmp
    }

    fn bash_in(cmd: &str, repo: &Path, cwd: &Path) -> Verdict {
        decide(
            &json!({"tool_name": "Bash", "tool_input": {"command": cmd}}),
            &held(),
            repo,
            cwd,
        )
    }

    fn bash(cmd: &str) -> Verdict {
        bash_in(cmd, Path::new("/repo"), Path::new("/repo"))
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
            Path::new("/repo"),
        )
    }

    fn denied(v: Verdict) -> bool {
        matches!(v, Verdict::Deny(_))
    }

    #[test]
    fn recursive_grep_over_the_repo_is_denied_with_the_verb() {
        let tmp = repo();
        let root = tmp.path();
        let r = root.to_str().unwrap();
        let hint = Verdict::Deny(grep_hint());
        for cmd in [
            "grep -rn foo src".to_string(),
            "grep -R foo".to_string(),
            "grep -rn foo .".to_string(),
            format!("cd {r} && grep -rni --include=*.php foo app/"),
            "grep --recursive -e foo src/Http".to_string(),
            "rg foo".to_string(),
            "rg -n 'a b' src".to_string(),
            format!("rg foo {r}/src | head -20"),
            "egrep -r 'a|b' ./".to_string(),
            "grep -r foo *".to_string(),
        ] {
            assert_eq!(bash_in(&cmd, root, root), hint, "{cmd}");
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
            "rg foo *.php",
        ] {
            assert_eq!(bash_in(cmd, root, root), Verdict::Allow, "{cmd}");
        }
    }

    #[test]
    fn grep_operands_are_directories_only_when_they_are() {
        let tmp = repo();
        let root = tmp.path();
        for cmd in [
            "rg KEY .env",
            "rg foo Makefile",
            "grep -r x artisan",
            "grep -r x missing",
        ] {
            assert_eq!(bash_in(cmd, root, root), Verdict::Allow, "{cmd}");
        }
        assert!(denied(bash_in("rg foo storage", root, root)));
    }

    #[test]
    fn a_leading_cd_moves_where_paths_resolve() {
        let tmp = repo();
        let root = tmp.path();
        let r = root.to_str().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let e = elsewhere.path().to_str().unwrap();
        assert!(denied(bash_in(
            &format!("cd {r} && grep -rn foo src"),
            root,
            elsewhere.path()
        )));
        assert!(denied(bash_in("cd src && rg foo", root, root)));
        assert!(denied(bash_in("cd src && rg foo Http", root, root)));
        assert_eq!(
            bash_in(&format!("cd {e} && grep -rn foo ."), root, root),
            Verdict::Allow
        );
        for cmd in [
            "cd && rg foo",
            "cd - && rg foo",
            "cd ~/x && rg foo",
            "cd $D && rg foo",
        ] {
            assert_eq!(bash_in(cmd, root, root), Verdict::Allow, "{cmd}");
        }
        assert_eq!(
            bash_in("cd docs && cat features/001-x/slice-02-b.md", root, root),
            Verdict::Deny(pack_hint("Slice"))
        );
        assert_eq!(
            bash_in(
                &format!("cd {e} && cat docs/features/001-x/slice-02-b.md"),
                root,
                root
            ),
            Verdict::Allow
        );
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
            read("/repo/docs/features/001-x/log.jsonl", json!({})),
            Verdict::Allow
        );
        assert_eq!(
            read("/repo/docs/features/001-x/agent-log.md", json!({})),
            want("Outline")
        );
        assert_eq!(read("/repo/src/other.rs", json!({})), Verdict::Allow);
        assert_eq!(bash(&format!("cat {SLICE}")), want("Slice"));
        assert_eq!(bash("cd /repo && cat src/helper.rs"), want("Outline"));
        assert!(!denied(bash(&format!("cat {SLICE} | head -40"))));
        assert!(!denied(bash(&format!("sed -n 1,40p {SLICE}"))));
        let cp = Held::parse(
            "checkpoint-1-4.md",
            &format!("<!-- factory-pack files: {SLICE} -->"),
        );
        let v = decide(
            &json!({"tool_name": "Read", "tool_input": {"file_path": format!("/repo/{SLICE}")}}),
            &cp,
            Path::new("/repo"),
            Path::new("/repo"),
        );
        assert_eq!(v, Verdict::Allow);
    }

    #[test]
    fn only_the_repo_copy_of_a_held_file_is_held() {
        let h = held();
        let repo = Path::new("/repo");
        assert_eq!(h.section("/repo/src/helper.rs", repo), Some("Outline"));
        assert_eq!(
            h.section("/repo/src/helper.rs", Path::new("/repo/")),
            Some("Outline")
        );
        for other in [
            "/tmp/src/helper.rs",
            "/repo/vendor/x/src/helper.rs",
            "/repo-old/src/helper.rs",
            "src/helper.rs",
        ] {
            assert_eq!(h.section(other, repo), None, "{other}");
        }
        let v = decide(
            &json!({"tool_name": "Read", "tool_input": {"file_path": "../src/helper.rs"}}),
            &h,
            repo,
            Path::new("/repo/app"),
        );
        assert_eq!(v, Verdict::Deny(pack_hint("Outline")));
        let v = decide(
            &json!({"tool_name": "Read", "tool_input": {"file_path": "src/helper.rs"}}),
            &h,
            repo,
            Path::new("/repo/vendor/x"),
        );
        assert_eq!(v, Verdict::Allow);
    }

    #[test]
    fn malformed_input_gets_no_decision_and_a_line_on_stderr() {
        for input in ["", "not json", "[1]"] {
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = hook_with(&mut input.as_bytes(), None, None, &mut out, &mut err).unwrap();
            assert_eq!(code, 0);
            assert!(
                out.is_empty(),
                "{input:?}: {}",
                String::from_utf8_lossy(&out)
            );
            assert!(
                String::from_utf8_lossy(&err).contains("allowed"),
                "{input:?}"
            );
        }
        let (mut out, mut err) = (Vec::new(), Vec::new());
        hook_with(
            &mut "{\"command\": \"git push".as_bytes(),
            None,
            None,
            &mut out,
            &mut err,
        )
        .unwrap();
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
    }

    #[test]
    fn mentions_are_whole_words() {
        assert!(mentions("x gh y"));
        assert!(mentions("git-push"));
        assert!(mentions("'gh'"));
        assert!(mentions("$GH api"));
        assert!(!mentions("github pushd push_x ghost GH_TOKEN"));
    }
}
