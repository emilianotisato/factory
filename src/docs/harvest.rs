//! A batch's lessons, read-only: each shipped feature's retro without its Numbers, then the ideas it spawned.
use std::fs;
use std::io::Write;
use std::path::Path;

use super::board::{self, Feature};
use super::ideas::{self, IdeaRow};
use crate::error::io;
use crate::{Error, Result};

/// No references means every live shipped feature, in number order.
pub fn harvest(root: &Path, references: &[String], out: &mut dyn Write) -> Result<()> {
    let board = board::scan(root)?;
    let batch: Vec<&Feature> = if references.is_empty() {
        let mut v: Vec<&Feature> = board
            .live
            .iter()
            .filter(|f| f.status().as_deref() == Some("shipped"))
            .collect();
        v.sort_by_key(|f| f.number);
        v
    } else {
        references
            .iter()
            .map(|r| {
                let f = board.resolve(r)?;
                match f.status() {
                    Some(s) if s == "shipped" => Ok(f),
                    s => Err(Error::Msg(format!(
                        "refusing: PRD-{:03} is {}, not shipped",
                        f.number,
                        s.as_deref().unwrap_or("without a status")
                    ))),
                }
            })
            .collect::<Result<_>>()?
    };
    if batch.is_empty() {
        return say(out, "no shipped feature to harvest\n");
    }
    let (rows, _) = ideas::list(root);
    let mut text = String::new();
    for (i, f) in batch.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        text.push_str(&feature(f, &rows)?);
    }
    say(out, &text)
}

fn feature(f: &Feature, rows: &[IdeaRow]) -> Result<String> {
    let nnn = format!("PRD-{:03}", f.number);
    let mut text = format!("══ {nnn} · {} ══\n\n", f.dir_name);
    let retro = f.dir.join("retro.md");
    if retro.is_file() {
        let raw = fs::read_to_string(&retro).map_err(io(format!("read {}", retro.display())))?;
        let lessons = lessons(&raw);
        if lessons.is_empty() {
            text.push_str("retro.md holds no lessons\n");
        } else {
            text.push_str(&lessons);
            text.push('\n');
        }
    } else {
        text.push_str("no retro.md\n");
    }
    let spawned: Vec<&IdeaRow> = rows
        .iter()
        .filter(|r| r.origin.as_deref().is_some_and(|o| names(o, &nnn)))
        .collect();
    text.push_str(&format!("\nideas from {nnn}: "));
    if spawned.is_empty() {
        text.push_str("none\n");
    } else {
        text.push_str(&format!("{}\n", spawned.len()));
        for r in spawned {
            text.push_str(&format!("- {} · {}\n", r.id, r.first_sentence));
        }
    }
    Ok(text)
}

/// The text under the `# ` heading, with the `## Numbers` section cut to the next `## `.
fn lessons(retro: &str) -> String {
    let lines: Vec<&str> = strip_frontmatter(retro).lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("# "))
        .map_or(0, |i| i + 1);
    let mut kept = Vec::new();
    let mut cutting = false;
    for &l in &lines[start..] {
        if l.starts_with("## ") {
            cutting = l.trim_end() == "## Numbers";
        }
        if !cutting {
            kept.push(l);
        }
    }
    kept.join("\n").trim().to_string()
}

fn strip_frontmatter(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("---\n") else {
        return text;
    };
    match rest.find("\n---\n") {
        Some(i) => &rest[i + 5..],
        None => text,
    }
}

fn names(origin: &str, nnn: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    origin.match_indices(nnn).any(|(i, m)| {
        let before = origin[..i].chars().next_back();
        let after = origin[i + m.len()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

fn say(out: &mut dyn Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes()).map_err(io("write stdout"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_origin_names_a_feature_only_as_a_whole_token() {
        assert!(names("TKT-007 polish", "TKT-007"));
        assert!(names("factory TKT-007 run 007-20261015-c8fa", "TKT-007"));
        assert!(!names("TKT-0070 polish", "TKT-007"));
        assert!(!names("TKT-007x", "TKT-007"));
        assert!(!names("XTKT-007", "TKT-007"));
    }

    #[test]
    fn lessons_drop_the_frontmatter_the_heading_and_the_numbers_section() {
        let retro = "---\nscore: 4\n---\n\n# Retro — sample\n\nprose\n\n## Cost vs catch\n\nlesson\n\n## Numbers\n\n<!-- factory:numbers -->\nrows\n<!-- /factory:numbers -->\n\n## After\n\n### Numbers\n\nkept\n";
        let got = lessons(retro);
        assert!(got.starts_with("prose"), "{got}");
        assert!(got.contains("## Cost vs catch\n\nlesson"), "{got}");
        assert!(!got.contains("factory:numbers"), "{got}");
        assert!(!got.contains("rows"), "{got}");
        assert!(got.contains("## After\n\n### Numbers\n\nkept"), "{got}");
    }
}
