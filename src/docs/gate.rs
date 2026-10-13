//! `factory docs gate` (PRD-002 FR-15): the verify gate `[docs] gate` and the
//! inner loop `[docs] inner` of `docs/project.toml`, and the foundations the
//! board reports beside them. A port of `docs.mjs:383-606`: the tool checks
//! that the entry point exists and never runs the command.
//!
//! The driver lifts `gate` and `gate --inner` stdout whole
//! (`Docs::gate`, `Docs::gate_inner`), so stdout carries the bare command and
//! every warning goes to stderr.
use std::fs;
use std::io::Write;
use std::path::Path;

use serde_json::Value;

use super::project::{self, Project};
use super::{Gate, Harness};
use crate::error::io;
use crate::{Error, Result};

/// Gate script names, best first (`GATE_NAMES`).
pub const NAMES: [&str; 6] = ["verify", "check", "ci", "ci:check", "qa", "test"];

const RUNNERS: [&str; 17] = [
    "composer", "npm", "pnpm", "yarn", "bun", "make", "just", "mise", "cargo", "go", "mix", "mvn",
    "gradle", "gradlew", "swift", "flutter", "dart",
];

/// Toolchains whose tasks the tool cannot enumerate cheaply: the manifest
/// proves the runner fits the repo (`GATE_MANIFESTS`).
fn manifests(runner: &str) -> &'static [&'static str] {
    match runner {
        "cargo" => &["Cargo.toml"],
        "go" => &["go.mod"],
        "mix" => &["mix.exs"],
        "mvn" => &["pom.xml"],
        "gradle" => &["build.gradle", "build.gradle.kts"],
        "gradlew" => &["gradlew"],
        "swift" => &["Package.swift"],
        "flutter" | "dart" => &["pubspec.yaml"],
        _ => &[],
    }
}

/// One shell-level part of the gate: `ok`, `missing` or `unverified`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub status: &'static str,
    pub detail: String,
}

fn part(status: &'static str, detail: impl Into<String>) -> Part {
    Part {
        status,
        detail: detail.into(),
    }
}

/// Likely gate commands by [`NAMES`] order, and `verify` hints from the agent
/// instruction files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Candidates {
    pub commands: Vec<String>,
    pub hints: Vec<String>,
}

fn basename(t: &str) -> &str {
    t.rsplit('/').next().unwrap_or(t)
}

fn first_existing(root: &Path, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find(|n| root.join(n).exists())
        .map(|n| n.to_string())
}

/// `scripts` keys of a JSON manifest; `None` when it is absent or unparseable.
fn json_scripts(root: &Path, file: &str) -> Option<Vec<String>> {
    let text = fs::read_to_string(root.join(file)).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    Some(
        v.get("scripts")
            .and_then(Value::as_object)
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default(),
    )
}

fn name_len(s: &str, extra: &[char]) -> usize {
    s.len()
        - s.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || extra.contains(&c))
            .len()
}

/// `:` not followed by `=`.
fn colon(rest: &str) -> bool {
    rest.starts_with(':') && !rest[1..].starts_with('=')
}

/// `^([A-Za-z0-9_.-]+)\s*:(?!=)`, per line.
fn make_targets(root: &Path) -> Option<Vec<String>> {
    let f = first_existing(root, &["Makefile", "makefile", "GNUmakefile"])?;
    let text = fs::read_to_string(root.join(f)).unwrap_or_default();
    Some(
        text.lines()
            .filter_map(|l| {
                let n = name_len(l, &['_', '.', '-']);
                (n > 0 && colon(l[n..].trim_start())).then(|| l[..n].to_string())
            })
            .collect(),
    )
}

/// `^([A-Za-z0-9_-]+)(?:\s+[^:=\n]*)?:(?!=)`, per line.
fn just_recipes(root: &Path) -> Option<Vec<String>> {
    let f = first_existing(root, &["justfile", "Justfile", ".justfile"])?;
    let text = fs::read_to_string(root.join(f)).unwrap_or_default();
    Some(
        text.lines()
            .filter_map(|l| {
                let n = name_len(l, &['_', '-']);
                if n == 0 {
                    return None;
                }
                let rest = &l[n..];
                let hit = colon(rest)
                    || (rest.starts_with(char::is_whitespace)
                        && rest.find([':', '=']).is_some_and(|i| colon(&rest[i..])));
                hit.then(|| l[..n].to_string())
            })
            .collect(),
    )
}

/// `[tasks.<name>]` headers and the keys of a bare `[tasks]` table.
fn mise_tasks(root: &Path) -> Option<Vec<String>> {
    let f = first_existing(root, &["mise.toml", ".mise.toml"])?;
    let text = fs::read_to_string(root.join(f)).unwrap_or_default();
    // `["']?name["']?` → the name and what follows the closing quote.
    let task = |s: &str| -> Option<(String, String)> {
        let s = s.strip_prefix(['"', '\'']).unwrap_or(s);
        let n = name_len(s, &['_', ':', '.', '-']);
        let rest = &s[n..];
        (n > 0).then(|| {
            let rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
            (s[..n].to_string(), rest.to_string())
        })
    };
    let mut out: Vec<String> = Vec::new();
    for l in text.lines() {
        if let Some((n, rest)) = l.strip_prefix("[tasks.").and_then(task)
            && rest.starts_with(']')
            && !out.contains(&n)
        {
            out.push(n);
        }
    }
    let mut lines = text.lines().skip_while(|l| l.trim_end() != "[tasks]");
    if lines.next().is_some() {
        for l in lines.take_while(|l| !l.starts_with('[')) {
            if let Some((n, rest)) = task(l)
                && rest.trim_start().starts_with('=')
                && !out.contains(&n)
            {
                out.push(n);
            }
        }
    }
    Some(out)
}

/// pnpm, yarn or bun by lockfile, else npm (`jsRunner`).
fn js_runner(root: &Path) -> &'static str {
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm run"
    } else if root.join("yarn.lock").exists() {
        "yarn"
    } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        "bun run"
    } else {
        "npm run"
    }
}

/// `resolveGatePart` (`docs.mjs:431-481`).
pub fn resolve_part(root: &Path, text: &str) -> Part {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let Some(idx) = tokens.iter().position(|t| RUNNERS.contains(&basename(t))) else {
        let first = tokens.first().copied().unwrap_or("");
        if first.contains('/') {
            return if root.join(first.trim_start_matches('/')).exists() {
                part("ok", "resolves")
            } else {
                part("missing", format!("{first} not found"))
            };
        }
        return part(
            "unverified",
            format!("cannot check \"{first}\" — declared as-is"),
        );
    };
    let runner = basename(tokens[idx]);
    // `mise exec -- composer verify`: the real gate is what follows the wrapper.
    if runner == "mise" && matches!(tokens.get(idx + 1), Some(&("exec" | "x"))) {
        let from = match tokens[idx..].iter().position(|t| *t == "--") {
            Some(j) => idx + j + 1,
            None => idx + 2,
        };
        return resolve_part(root, &tokens[from.min(tokens.len())..].join(" "));
    }
    let args: Vec<&str> = tokens[idx + 1..]
        .iter()
        .copied()
        .filter(|t| !t.starts_with('-') && !t.contains('='))
        .collect();
    let script = match args.first() {
        Some(&("run" | "run-script")) => args.get(1).copied(),
        a => a.copied(),
    };
    let lookup = |names: Option<Vec<String>>, source: &str, what: &str| {
        let Some(names) = names else {
            return part("missing", format!("no {source}"));
        };
        let wanted: Vec<&str> = if runner == "make" || runner == "just" {
            args.clone()
        } else {
            script.into_iter().collect()
        };
        match wanted.iter().find(|w| !names.iter().any(|n| n == *w)) {
            Some(gone) => part("missing", format!("{source} has no \"{gone}\" {what}")),
            None => part("ok", "resolves"),
        }
    };
    match runner {
        "composer" => lookup(
            json_scripts(root, "composer.json"),
            "composer.json",
            "script",
        ),
        "npm" | "pnpm" | "yarn" | "bun" => {
            lookup(json_scripts(root, "package.json"), "package.json", "script")
        }
        "make" => lookup(make_targets(root), "Makefile", "target"),
        "just" => lookup(just_recipes(root), "justfile", "recipe"),
        "mise" => lookup(mise_tasks(root), "mise.toml", "task"),
        _ => match first_existing(root, manifests(runner)) {
            Some(m) => part(
                "ok",
                format!("resolves ({} present, task not checked)", basename(&m)),
            ),
            None => part(
                "missing",
                format!("no {} for {runner}", manifests(runner).join(" / ")),
            ),
        },
    }
}

/// `` `[^`]*\bverify\b[^`]*` ``, case-insensitive: any span between two
/// backticks holding the word.
fn verify_hint(line: &str) -> bool {
    let spans: Vec<&str> = line.split('`').collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    spans.len() > 2
        && spans[1..spans.len() - 1].iter().any(|s| {
            let lower = s.to_lowercase();
            lower
                .match_indices("verify")
                .any(|(i, _)| !lower[..i].ends_with(word) && !lower[i + 6..].starts_with(word))
        })
}

/// How one manifest's task becomes a command line.
type Runner = Box<dyn Fn(&str) -> String>;

/// `gateCandidates` (`docs.mjs:483-510`), the hints capped at three.
pub fn candidates(root: &Path) -> Candidates {
    let mut sources: Vec<(Vec<String>, Runner)> = Vec::new();
    if let Some(s) = json_scripts(root, "composer.json") {
        sources.push((s, Box::new(|n| format!("composer {n}"))));
    }
    if let Some(s) = json_scripts(root, "package.json") {
        let r = js_runner(root);
        sources.push((s, Box::new(move |n| format!("{r} {n}"))));
    }
    if let Some(s) = make_targets(root) {
        sources.push((s, Box::new(|n| format!("make {n}"))));
    }
    if let Some(s) = just_recipes(root) {
        sources.push((s, Box::new(|n| format!("just {n}"))));
    }
    if let Some(s) = mise_tasks(root) {
        sources.push((s, Box::new(|n| format!("mise run {n}"))));
    }
    let mut commands: Vec<String> = Vec::new();
    for n in NAMES {
        for (names, fmt) in &sources {
            let c = fmt(n);
            if names.iter().any(|x| x == n) && !commands.contains(&c) {
                commands.push(c);
            }
        }
    }
    let mut hints = Vec::new();
    for f in ["CLAUDE.md", "AGENTS.md", ".ai/guidelines/verification.md"] {
        let Ok(text) = fs::read_to_string(root.join(f)) else {
            continue;
        };
        for line in text.split('\n').filter(|l| verify_hint(l)) {
            if hints.len() >= 3 {
                break;
            }
            let short: String = line.trim().chars().take(110).collect();
            hints.push(format!("{f}: {short}"));
        }
    }
    Candidates { commands, hints }
}

/// Split on `&&`, `||` and `;`, the parts trimmed, empty ones dropped.
fn parts(cmd: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = cmd;
    loop {
        let cut = ["&&", "||", ";"]
            .iter()
            .filter_map(|sep| rest.find(sep).map(|i| (i, sep.len())))
            .min();
        let (piece, next) = match cut {
            Some((i, len)) => (&rest[..i], Some(&rest[i + len..])),
            None => (rest, None),
        };
        if !piece.trim().is_empty() {
            out.push(piece.trim());
        }
        match next {
            Some(n) => rest = n,
            None => return out,
        }
    }
}

/// `gateStatus` (`docs.mjs:512-526`): the worst part wins. `candidates` is
/// `null`, as the board prints it.
pub fn status(root: &Path, declared: Option<&str>) -> Gate {
    let declared = declared.filter(|g| !parts(g).is_empty());
    let Some(declared) = declared else {
        return Gate {
            declared: None,
            status: "none".into(),
            detail: Some("no gate declared".into()),
            candidates: Value::Null,
        };
    };
    let results: Vec<Part> = parts(declared)
        .into_iter()
        .map(|p| resolve_part(root, p))
        .collect();
    let rank = |s: &str| match s {
        "ok" => 0,
        "unverified" => 1,
        _ => 2,
    };
    let worst = results.iter().fold(&results[0], |a, r| {
        if rank(r.status) > rank(a.status) {
            r
        } else {
            a
        }
    });
    let detail = if worst.status == "ok" {
        results
            .iter()
            .find(|r| r.detail != "resolves")
            .map_or("resolves".to_string(), |r| r.detail.clone())
    } else {
        results
            .iter()
            .filter(|r| r.status != "ok")
            .map(|r| r.detail.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    };
    Gate {
        declared: Some(declared.to_string()),
        status: worst.status.into(),
        detail: Some(detail),
        candidates: Value::Null,
    }
}

/// `gateLine` (`docs.mjs:528`).
pub fn line(g: &Gate) -> String {
    let declared = g.declared.as_deref().unwrap_or("");
    let detail = g.detail.as_deref().unwrap_or("");
    match g.status.as_str() {
        "none" => "none declared — run /gate, or `factory docs gate set \"<cmd>\"`".into(),
        "ok" => format!("{declared} ({detail})"),
        "unverified" => format!("{declared} (declared; {detail})"),
        _ => format!("{declared} — {detail}"),
    }
}

/// `harnessStatus` (`docs.mjs:592-606`): `verify-*` skills holding a
/// `SKILL.md`, each with its feature-map count.
pub fn harness(root: &Path) -> Harness {
    let none = || Harness {
        present: false,
        detail: Some("none — run /harness".into()),
    };
    let dir = root.join(".claude/skills");
    let Ok(rd) = fs::read_dir(&dir) else {
        return none();
    };
    let mut found: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("verify-") && dir.join(n).join("SKILL.md").exists())
        .collect();
    if found.is_empty() {
        return none();
    }
    found.sort();
    let detail: Vec<String> = found
        .iter()
        .map(|n| {
            let count = fs::read_dir(dir.join(n).join("features")).map_or(0, |rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|f| f.ends_with(".md") && f != "README.md")
                    .count()
            });
            if count > 0 {
                format!("{n} ({count} features mapped)")
            } else {
                format!("{n} — no feature map")
            }
        })
        .collect();
    Harness {
        present: true,
        detail: Some(detail.join(", ")),
    }
}

/// `docs/ux-rubric.md` is there.
pub fn rubric(root: &Path) -> bool {
    root.join("docs/ux-rubric.md").exists()
}

fn loaded(root: &Path) -> Result<Project> {
    project::load(root)?.ok_or_else(|| {
        Error::Msg("no [docs] table in docs/project.toml — run `factory docs init`".into())
    })
}

/// The verb: `gate [set <cmd…>] [--inner] [--candidates]` (`gateCmd`,
/// `docs.mjs:543-578`). Exit 1 when nothing is declared to print.
pub fn run(
    root: &Path,
    args: &[String],
    inner: bool,
    with_candidates: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let p = loaded(root)?;
    let say = |w: &mut dyn Write, s: &str| writeln!(w, "{s}").map_err(io("write"));
    match args.first().map(String::as_str) {
        Some("set") => {
            let cmd = args[1..].join(" ").trim().to_string();
            if cmd.is_empty() {
                return Err(Error::Msg(
                    "usage: factory docs gate set \"<command>\"  ·  factory docs gate set --inner \"<command>\"".into(),
                ));
            }
            let key = if inner { "inner" } else { "gate" };
            project::set(root, key, Some(&toml::Value::String(cmd.clone())))?;
            if inner {
                // The fast subset a slice's Verify task runs; the full gate runs at checkpoints.
                say(out, &format!("inner: {cmd}"))?;
                return Ok(0);
            }
            let g = status(root, Some(&cmd));
            say(out, &format!("gate: {}", line(&g)))?;
            if g.status == "missing" {
                say(
                    out,
                    "declared anyway — fix the entry point, or the first Verify task fails on the spot",
                )?;
            }
            return Ok(0);
        }
        Some(other) => {
            return Err(Error::Msg(format!(
                "unknown gate argument \"{other}\"; usage: factory docs gate [set <cmd>] [--inner] [--candidates]"
            )));
        }
        None => {}
    }
    if inner {
        // Bare command on stdout; falls back to the full gate so a Verify
        // task always has something to run.
        if let Some(i) = &p.inner {
            say(out, i)?;
            return Ok(0);
        }
        let Some(g) = &p.gate else { return Ok(1) };
        say(
            err,
            "! no inner loop declared — running the full gate per slice (`factory docs gate set --inner \"<cmd>\"`)",
        )?;
        say(out, g)?;
        return Ok(0);
    }
    let g = status(root, p.gate.as_deref());
    if with_candidates || g.status == "none" {
        let c = candidates(root);
        let none = g.status == "none";
        say(
            out,
            &if none {
                "no gate declared".to_string()
            } else {
                format!("declared: {}", line(&g))
            },
        )?;
        say(
            out,
            &if c.commands.is_empty() {
                "candidates: none found (no verify/check/ci/qa script, target, recipe or task)"
                    .to_string()
            } else {
                format!("candidates: {}", c.commands.join(", "))
            },
        )?;
        for h in &c.hints {
            say(out, &format!("  hint · {h}"))?;
        }
        if none {
            say(
                out,
                "declare one with `factory docs gate set \"<cmd>\"`, or run /gate to build it properly",
            )?;
        }
        return Ok(i32::from(none));
    }
    // Bare command on stdout so a skill can lift it; problems go to stderr.
    say(out, g.declared.as_deref().unwrap_or(""))?;
    if g.status != "ok" {
        say(err, &format!("! {}", g.detail.as_deref().unwrap_or("")))?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (f, text) in files {
            let p = d.path().join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        d
    }

    fn st(root: &Path, cmd: &str) -> (String, String) {
        let g = status(root, Some(cmd));
        (g.status, g.detail.unwrap_or_default())
    }

    #[test]
    fn a_path_resolves_when_the_file_exists() {
        let d = dir(&[("gate.sh", "#!/bin/sh\n")]);
        assert_eq!(st(d.path(), "./gate.sh").0, "ok");
        assert_eq!(
            st(d.path(), "./nope.sh"),
            ("missing".into(), "./nope.sh not found".into())
        );
        assert_eq!(st(d.path(), "a && ./nope.sh").0, "missing");
        assert_eq!(st(d.path(), "frob").0, "unverified");
    }

    #[test]
    fn runners_are_checked_against_their_manifest() {
        let d = dir(&[("composer.json", r#"{"scripts":{"test":"pest"}}"#)]);
        assert_eq!(
            st(d.path(), "composer verify"),
            (
                "missing".into(),
                "composer.json has no \"verify\" script".into()
            )
        );
        assert_eq!(st(d.path(), "composer test").0, "ok");

        let d = dir(&[("Cargo.toml", "[package]\n")]);
        let (s, detail) = st(d.path(), "cargo test");
        assert_eq!(s, "ok");
        assert!(detail.contains("task not checked"), "{detail}");
        assert_eq!(
            st(d.path(), "make verify"),
            ("missing".into(), "no Makefile".into())
        );

        let d = dir(&[
            ("Makefile", "verify: lint\n\tcargo test\nX := 1\n"),
            ("mise.toml", "[tasks.ci]\nrun = 'x'\n[tasks]\nqa = 'y'\n"),
        ]);
        assert_eq!(st(d.path(), "make verify").0, "ok");
        assert_eq!(st(d.path(), "make X").0, "missing");
        assert_eq!(st(d.path(), "mise run ci").0, "ok");
        assert_eq!(st(d.path(), "mise run qa").0, "ok");
        assert_eq!(st(d.path(), "mise exec -- make verify").0, "ok");
    }

    #[test]
    fn candidates_follow_the_name_order_and_the_lockfile() {
        let d = dir(&[
            (
                "package.json",
                r#"{"scripts":{"test":"vitest","verify":"x"}}"#,
            ),
            ("pnpm-lock.yaml", ""),
            ("justfile", "check arg:\n  echo\n"),
            (
                "CLAUDE.md",
                "Run `make verify` before a commit.\nno hint here\n",
            ),
        ]);
        let c = candidates(d.path());
        assert_eq!(
            c.commands,
            ["pnpm run verify", "just check", "pnpm run test"]
        );
        assert_eq!(c.hints, ["CLAUDE.md: Run `make verify` before a commit."]);
    }

    #[test]
    fn the_harness_counts_mapped_features() {
        let d = dir(&[
            (".claude/skills/verify-app/SKILL.md", "x"),
            (".claude/skills/verify-app/features/a.md", "x"),
            (".claude/skills/verify-app/features/README.md", "x"),
            (".claude/skills/other/SKILL.md", "x"),
        ]);
        let h = harness(d.path());
        assert!(h.present);
        assert_eq!(h.detail.as_deref(), Some("verify-app (1 features mapped)"));
        assert!(!harness(dir(&[]).path()).present);
    }
}
