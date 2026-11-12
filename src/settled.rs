//! What a feature already settled, for reviewers: its live decisions and its PRD's Decisions log and Non-goals
//! sections. A missing folder, log or section leaves that part out; nothing here is an error.
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::docs::{log, state};
use crate::{pack, retro};

/// The block rides inside one argv string, which Linux caps at 128 KiB.
pub const CAP: usize = 32 * 1024;

/// The settled block of `feature` as checked out in `checkout`, or none when it has nothing settled.
pub fn for_feature(checkout: &Path, feature: &str) -> Option<String> {
    block(&retro::feature_dir(checkout, feature).ok()?)
}

/// The settled block of the feature folder `dir`, cut to `CAP`.
pub fn block(dir: &Path) -> Option<String> {
    let prd = fs::read_to_string(dir.join("prd.md")).unwrap_or_default();
    let section = |title: &str| pack::cut_titled(&prd, title).map(|s| s.trim_end().to_string());
    fit(
        &decisions(dir),
        section("Decisions log"),
        section("Non-goals"),
    )
}

/// The newest commit that changed the `reviewed` line of `feature`'s state file, in `checkout`, while `reviewed`
/// is set: clearing it asks for the whole branch again.
pub fn last_reviewed(checkout: &Path, feature: &str) -> Option<String> {
    let dir = retro::feature_dir(checkout, feature).ok()?;
    state::read(&dir).ok()??.reviewed?;
    let file = dir.join(state::FILE);
    let file = file.strip_prefix(checkout).unwrap_or(&file);
    let out = Command::new("git")
        .args(["log", "-1", "--format=%H", "-G", "^reviewed", "--"])
        .arg(file)
        .current_dir(checkout)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !sha.is_empty()).then_some(sha)
}

/// One `(id, line)` per decision no later entry supersedes, oldest first.
fn decisions(dir: &Path) -> Vec<(String, String)> {
    let Ok((entries, _)) = log::entries(dir) else {
        return Vec::new();
    };
    let mut overturned: HashMap<String, usize> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        for id in supersedes(&e.body) {
            overturned.insert(id, i);
        }
    }
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == log::Kind::Decision)
        .filter_map(|(i, e)| {
            let id = e.id.clone()?;
            if overturned.get(&id).is_some_and(|&by| by > i) {
                return None;
            }
            let owner = entries.iter().rev().find(|r| {
                r.kind == log::Kind::Review
                    && r.reference.as_deref() == Some(id.as_str())
                    && r.verdict == Some(log::Verdict::Changed)
            });
            let chose = match owner {
                Some(r) => Some(match r.body.trim() {
                    "" => "changed by the owner".to_string(),
                    b => format!("changed by the owner — {b}"),
                }),
                None => e
                    .body
                    .lines()
                    .find_map(|l| l.trim().strip_prefix("**Chose:**"))
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .map(str::to_string),
            };
            let line = match chose {
                Some(c) => format!("- {id} — {}: {c}", e.title.trim()),
                None => format!("- {id} — {}", e.title.trim()),
            };
            Some((id, line))
        })
        .collect()
}

/// The ids named on a body's `**Supersedes:**` lines.
fn supersedes(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|l| l.split_once("**Supersedes:**").map(|(_, rest)| rest))
        .flat_map(|rest| {
            rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .filter(|w| {
                    w.strip_prefix("AD-")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                })
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn render(
    decisions: &[(String, String)],
    log: Option<&str>,
    non_goals: Option<&str>,
    cut: &[String],
) -> String {
    let mut parts = Vec::new();
    if !decisions.is_empty() {
        let lines: Vec<&str> = decisions.iter().map(|(_, l)| l.as_str()).collect();
        parts.push(format!("Decisions taken:\n{}", lines.join("\n")));
    }
    parts.extend(log.map(str::to_string));
    parts.extend(non_goals.map(str::to_string));
    if !cut.is_empty() {
        parts.push(format!(
            "Cut to fit {} KiB: {}.",
            CAP / 1024,
            cut.join(", ")
        ));
    }
    parts.join("\n\n") + "\n"
}

/// Drops the Non-goals first, then the oldest decisions, then the end of the Decisions log, until the block fits.
fn fit(
    decisions: &[(String, String)],
    log: Option<String>,
    non_goals: Option<String>,
) -> Option<String> {
    if decisions.is_empty() && log.is_none() && non_goals.is_none() {
        return None;
    }
    let full = render(decisions, log.as_deref(), non_goals.as_deref(), &[]);
    if full.len() <= CAP {
        return Some(full);
    }
    let mut cut = Vec::new();
    if non_goals.is_some() {
        cut.push("the PRD's Non-goals section".to_string());
    }
    let mut dropped = 0;
    loop {
        let mut said = cut.clone();
        if dropped > 0 {
            said.push(oldest(&decisions[..dropped]));
        }
        let text = render(&decisions[dropped..], log.as_deref(), None, &said);
        if text.len() <= CAP {
            return Some(text);
        }
        if dropped == decisions.len() {
            said.push("the end of the PRD's Decisions log section".to_string());
            let tail = render(&[], None, None, &said);
            let log = log.unwrap_or_default();
            let mut end = CAP.saturating_sub(tail.len() + 2).min(log.len());
            while !log.is_char_boundary(end) {
                end -= 1;
            }
            return Some(format!("{}\n\n{tail}", &log[..end]));
        }
        dropped += 1;
    }
}

fn oldest(dropped: &[(String, String)]) -> String {
    match dropped {
        [(only, _)] => format!("the oldest decision ({only})"),
        [(first, _), .., (last, _)] => {
            format!("the {} oldest decisions ({first} to {last})", dropped.len())
        }
        [] => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::log::{Entry, Kind};

    fn id(n: u32) -> String {
        format!("AD-{n}")
    }

    fn decision(id: &str, title: &str, body: &str) -> String {
        let mut e = Entry::new(Kind::Decision, title, body);
        e.id = Some(id.to_string());
        serde_json::to_string(&e).unwrap()
    }

    fn feature(log: &[String], prd: Option<&str>) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        if !log.is_empty() {
            fs::write(log::path(tmp.path()), log.join("\n") + "\n").unwrap();
        }
        if let Some(prd) = prd {
            fs::write(tmp.path().join("prd.md"), prd).unwrap();
        }
        tmp
    }

    const PRD: &str = "# PRD\n\n## 1. Why\n\nwhy\n\n## 12. Decisions log\n\n- kept the queue\n\n## 15. Non-goals\n\n- no web UI\n\n## 16. Notes\n\nnotes\n";

    #[test]
    fn a_superseded_decision_is_left_out() {
        let dir = feature(
            &[
                decision(
                    &id(1),
                    "Retry three times",
                    "**Chose:** three retries\n**Over:** one",
                ),
                decision(
                    &id(2),
                    "Retry five times",
                    &format!(
                        "**Chose:** five retries\n**Supersedes:** {}\n**Over:** three",
                        id(1)
                    ),
                ),
            ],
            None,
        );
        let block = block(dir.path()).unwrap();
        assert!(
            block.contains(&format!("- {} — Retry five times: five retries", id(2))),
            "{block}"
        );
        assert!(!block.contains(&id(1)), "{block}");
        assert!(!block.contains("three retries"), "{block}");
    }

    #[test]
    fn a_decision_the_owner_changed_shows_the_change() {
        let review = |verdict: log::Verdict, body: &str| {
            let mut e = Entry::new(Kind::Review, "", body);
            e.reference = Some(id(1));
            e.verdict = Some(verdict);
            serde_json::to_string(&e).unwrap()
        };
        let dir = feature(
            &[
                decision(&id(1), "Retry three times", "**Chose:** three retries"),
                review(log::Verdict::Kept, ""),
                review(log::Verdict::Changed, "retry five times"),
            ],
            None,
        );
        let block = block(dir.path()).unwrap();
        assert!(
            block.contains(&format!(
                "- {} — Retry three times: changed by the owner — retry five times",
                id(1)
            )),
            "{block}"
        );
        assert!(!block.contains("three retries"), "{block}");
    }

    #[test]
    fn the_block_names_what_the_cap_cut() {
        let chose = "x".repeat(300);
        let log: Vec<String> = (1..=150)
            .map(|n| {
                decision(
                    &id(n),
                    &format!("Choice {n}"),
                    &format!("**Chose:** {chose}"),
                )
            })
            .collect();
        let prd = format!(
            "## 1. Decisions log\n\n- kept\n\n## 2. Non-goals\n\n{}\n",
            "- nothing more\n".repeat(1000)
        );
        let dir = feature(&log, Some(&prd));
        let block = block(dir.path()).unwrap();
        assert!(block.len() <= CAP, "{}", block.len());
        assert!(!block.contains("nothing more"));
        assert!(block.contains("## 1. Decisions log"), "{block}");
        assert!(block.contains(&format!("- {} — Choice 150: ", id(150))));
        assert!(!block.contains(&format!("- {} — ", id(1))));
        let cut = block.lines().last().unwrap();
        assert!(
            cut.starts_with("Cut to fit 32 KiB: the PRD's Non-goals section, the "),
            "{cut}"
        );
        assert!(
            cut.contains(&format!(" oldest decisions ({} to AD-", id(1))),
            "{cut}"
        );

        let small = feature(&log[..2], Some(PRD));
        let whole = super::block(small.path()).unwrap();
        assert!(!whole.contains("Cut to fit"), "{whole}");
    }

    #[test]
    fn no_log_or_section_is_no_block_part() {
        assert_eq!(block(feature(&[], None).path()), None);
        assert_eq!(
            block(feature(&[], Some("# PRD\n\n## 1. Why\n")).path()),
            None
        );

        let prd_only = block(feature(&[], Some(PRD)).path()).unwrap();
        assert!(!prd_only.contains("Decisions taken"), "{prd_only}");
        assert!(
            prd_only.contains("## 12. Decisions log\n\n- kept the queue"),
            "{prd_only}"
        );
        assert!(
            prd_only.contains("## 15. Non-goals\n\n- no web UI"),
            "{prd_only}"
        );
        assert!(!prd_only.contains("notes"), "{prd_only}");

        let no_goals = "## 12. Decisions log\n\n- kept the queue\n";
        let log_only = block(feature(&[decision(&id(1), "T", "")], Some(no_goals)).path()).unwrap();
        assert!(
            log_only.starts_with(&format!("Decisions taken:\n- {} — T\n", id(1))),
            "{log_only}"
        );
        assert!(!log_only.contains("Non-goals"), "{log_only}");
    }

    #[test]
    fn last_reviewed_is_none_outside_a_feature() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(last_reviewed(tmp.path(), "001"), None);
    }
}
