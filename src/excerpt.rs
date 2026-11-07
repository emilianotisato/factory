//! Finds the failing lines in a gate or CI log, whatever the stack printed them with.
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// At most this many lines come back.
pub const MAX_LINES: usize = 20;
/// Lines of context kept on each side of a match.
pub const CONTEXT: usize = 2;
/// Every line, and a structured failure's message, is cut to this many chars.
pub const WIDTH: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    /// `false` for a context line: a block is titled from a match, never from the line above it.
    pub hit: bool,
}

static GENERIC: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^error",
        r"error[:\[]",
        r"\berror TS\d+",
        r":\s*error\b",
        r"--- FAIL",
        r"^FAIL",
        r"✘",
        r"\[FAIL\]",
        r"Diff in ",
        r"timed out after",
        r"affected by [1-9]\d* vulnerabilit",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("generic pattern"))
    .collect()
});
static FAILED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bfailed\b").unwrap());
static ZERO_FAILED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b0 failed\b").unwrap());
static EXIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"returned with error code|make(\[\d+\])?: \*\*\*.*Error").unwrap()
});
static TIER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^=== \d+\.").unwrap());

/// The failing lines of `bytes` in log order, empty when nothing matches; `project` is tried before the built-in set.
pub fn excerpt(bytes: &[u8], project: &[Regex]) -> Vec<Line> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines: Vec<String> = Vec::new();
    let mut red: Vec<bool> = Vec::new();
    let mut structured: Vec<bool> = Vec::new();
    for raw in strip_bom(&text).split('\n') {
        let is_red = has_red(raw);
        let clean = uncarriage(&strip_ansi(raw));
        match failures(&clean) {
            Some(found) => {
                for f in found {
                    lines.push(f);
                    red.push(is_red);
                    structured.push(true);
                }
            }
            None => {
                lines.push(clean);
                red.push(is_red);
                structured.push(false);
            }
        }
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }

    let pick = |f: &dyn Fn(usize, &str) -> bool| -> Vec<usize> {
        lines
            .iter()
            .enumerate()
            .filter(|(i, l)| f(*i, l))
            .map(|(i, _)| i)
            .collect()
    };
    let mut hits = pick(&|_, l| project.iter().any(|r| r.is_match(l)));
    if hits.is_empty() {
        hits = pick(&|i, _| structured[i]);
    }
    if hits.is_empty() {
        hits = pick(&|i, l| red[i] || generic(l));
    }
    let before_exit = lines
        .iter()
        .rposition(|l| EXIT.is_match(l))
        .and_then(|i| i.checked_sub(1));
    if let Some(i) = before_exit
        && !hits.contains(&i)
    {
        hits.push(i);
        hits.sort_unstable();
    }
    if hits.is_empty() {
        return Vec::new();
    }

    let mut keep = vec![false; lines.len()];
    let mut kept = 0;
    let take = |i: usize, keep: &mut [bool], kept: &mut usize| {
        if !keep[i] && *kept < MAX_LINES {
            keep[i] = true;
            *kept += 1;
        }
    };
    if let Some(t) = lines.iter().rposition(|l| TIER.is_match(l)) {
        take(t, &mut keep, &mut kept);
    }
    let order = before_exit.into_iter().chain(
        hits.iter()
            .rev()
            .copied()
            .filter(|&i| Some(i) != before_exit),
    );
    for i in order {
        take(i, &mut keep, &mut kept);
        let lo = i.saturating_sub(CONTEXT);
        let hi = (i + CONTEXT).min(lines.len() - 1);
        let near = (lo..=hi).filter(|&j| j != i && !keep[j]).count();
        if kept + near > MAX_LINES {
            continue;
        }
        for j in lo..=hi {
            take(j, &mut keep, &mut kept);
        }
    }
    lines
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(i, text)| Line {
            text: cut(&text, WIDTH),
            hit: hits.binary_search(&i).is_ok(),
        })
        .collect()
}

fn generic(line: &str) -> bool {
    GENERIC.iter().any(|r| r.is_match(line)) || FAILED.is_match(&ZERO_FAILED.replace_all(line, ""))
}

/// One line per `failures[]` entry of the JSON objects on `line`, or `None` when it carries none.
fn failures(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !t.starts_with('{') || !t.ends_with('}') {
        return None;
    }
    let mut out = Vec::new();
    for piece in t.split("}{") {
        let open = if piece.starts_with('{') { "" } else { "{" };
        let close = if piece.ends_with('}') { "" } else { "}" };
        let Ok(v) = serde_json::from_str::<Value>(&format!("{open}{piece}{close}")) else {
            continue;
        };
        for f in v["failures"].as_array().into_iter().flatten() {
            let s = |k: &str| f[k].as_str().unwrap_or_default();
            let mut text = s("test").to_string();
            let at = match (s("file"), f["line"].as_u64()) {
                ("", _) => String::new(),
                (file, Some(n)) => format!("{file}:{n}"),
                (file, None) => file.to_string(),
            };
            if !at.is_empty() {
                text = if text.is_empty() {
                    at
                } else {
                    format!("{text} ({at})")
                };
            }
            let message = cut(s("message"), WIDTH);
            if !message.is_empty() {
                text = if text.is_empty() {
                    message
                } else {
                    format!("{text}: {message}")
                };
            }
            if !text.is_empty() {
                out.push(text);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// SGR 31 or 91 anywhere on the line; the colour arguments of 38 and 48 are skipped.
fn has_red(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] != 0x1b || b[i + 1] != b'[' {
            i += 1;
            continue;
        }
        let start = i + 2;
        let mut end = start;
        while end < b.len() && !(0x40..=0x7e).contains(&b[end]) {
            end += 1;
        }
        if end < b.len() && b[end] == b'm' {
            let params: Vec<&str> = line[start..end].split([';', ':']).collect();
            let mut k = 0;
            while k < params.len() {
                match params[k] {
                    "31" | "91" => return true,
                    "38" | "48" => {
                        k += if params.get(k + 1) == Some(&"5") {
                            3
                        } else {
                            5
                        }
                    }
                    _ => k += 1,
                }
            }
        }
        i = end + 1;
    }
    false
}

/// Drops CSI and OSC sequences and the two-byte escapes, such as the charset switch `ESC ( B`.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            Some(c) if ('\u{20}'..='\u{2f}').contains(&c) => {
                chars.next();
            }
            _ => {}
        }
    }
    out
}

pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// The BOM, the escape sequences and every carriage return dropped: what a person reads in the log.
pub fn clean(text: &str) -> String {
    strip_bom(text)
        .split('\n')
        .map(|l| uncarriage(&strip_ansi(l)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a terminal leaves on a line rewritten with `\r`: the last non-empty segment.
fn uncarriage(line: &str) -> String {
    if !line.contains('\r') {
        return line.to_string();
    }
    line.split('\r')
        .rfind(|s| !s.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// The first `width` chars of `text`.
pub fn cut(text: &str, width: usize) -> String {
    match text.char_indices().nth(width) {
        Some((i, _)) => text[..i].to_string(),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/fixture/gate-logs/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn texts(lines: &[Line]) -> Vec<&str> {
        lines.iter().map(|l| l.text.as_str()).collect()
    }

    fn hits(lines: &[Line]) -> Vec<&str> {
        lines
            .iter()
            .filter(|l| l.hit)
            .map(|l| l.text.as_str())
            .collect()
    }

    #[test]
    fn excerpt_finds_a_pest_json_failure() {
        let x = excerpt(&fixture("pest-json.log"), &[]);
        let hits = hits(&x);
        assert_eq!(
            hits,
            [
                "P\\Tests\\Feature\\DataTables\\UserDataTableTest::__pest_evaluable_it_filters_users_by_search (/home/user/.local/share/factory/lanes/prokeycloud-13825a9c/lane-1/tests/Feature/DataTables/UserDataTableTest.php:31): Expecting Illuminate\\Support\\Collection not to contain 'Maria Lopez'."
            ],
            "{x:#?}"
        );
        assert!(x.iter().all(|l| !l.text.contains("\"failures\"")), "{x:#?}");
        assert!(x.len() <= MAX_LINES);
    }

    #[test]
    fn excerpt_finds_a_tsc_error() {
        let x = excerpt(&fixture("tsc.log"), &[]);
        let hits = hits(&x);
        assert_eq!(hits.len(), 1, "{x:#?}");
        assert!(
            hits[0].starts_with(
                "app/Modules/Irsa/resources/js/pages/flujos/Mapas.vue(5,33): error TS2307:"
            ),
            "{x:#?}"
        );
        assert!(texts(&x).iter().all(|t| !t.contains('\u{1b}')));
    }

    #[test]
    fn excerpt_finds_a_red_crap_row() {
        let x = excerpt(&fixture("crap.log"), &[]);
        let hits = hits(&x);
        assert!(
            hits.iter()
                .any(|t| t.starts_with(" Idempotency ") && t.contains("306.00")),
            "{x:#?}"
        );
        assert!(
            hits.iter().all(|t| !t.contains("terminalAdd")),
            "a yellow row is not a failure: {x:#?}"
        );
        let all = texts(&x);
        assert!(
            all.contains(&"=== 6. CRAP (complexity vs coverage) ==="),
            "{x:#?}"
        );
        assert!(all.iter().all(|t| !t.contains("affected by 0")), "{x:#?}");
        assert!(x.len() <= MAX_LINES);
    }

    #[test]
    fn excerpt_finds_a_rustfmt_diff_at_the_top() {
        let mut log = fixture("rustfmt.log");
        for i in 0..200 {
            log.extend_from_slice(
                format!("test tests::case_{i} ... \x1b[32mok\x1b[0m\n").as_bytes(),
            );
        }
        log.extend_from_slice(b"\ntest result: ok. 200 passed; 0 failed; 0 ignored\n");
        let x = excerpt(&log, &[]);
        assert!(
            hits(&x).contains(&"Diff in /home/user/Projects/factory/src/cli/report.rs:842:"),
            "{x:#?}"
        );
        assert!(
            hits(&x)
                .iter()
                .any(|t| t.starts_with("-            or_dash("))
        );
        assert!(texts(&x).iter().all(|t| !t.contains("case_")), "{x:#?}");
    }

    #[test]
    fn excerpt_ignores_zero_failed() {
        assert_eq!(excerpt(&fixture("zero-failed.log"), &[]), []);
        let x = excerpt(b"test result: FAILED. 3 passed; 1 failed; 0 ignored\n", &[]);
        assert_eq!(hits(&x).len(), 1);
    }

    #[test]
    fn excerpt_uses_project_gate_failure_first() {
        let mut log = fixture("rustfmt.log");
        log.extend_from_slice(b"Tier lint: REJECTED (3 problems)\nall done\n");
        let ours = [Regex::new(r"REJECTED").unwrap()];
        assert_eq!(
            hits(&excerpt(&log, &ours)),
            ["Tier lint: REJECTED (3 problems)"]
        );
        assert!(
            hits(&excerpt(&log, &[]))
                .iter()
                .any(|t| t.starts_with("Diff in "))
        );
    }

    #[test]
    fn the_budget_keeps_the_last_matches() {
        let log: String = (0..50).map(|i| format!("error: number {i}\n")).collect();
        let x = excerpt(log.as_bytes(), &[]);
        assert_eq!(x.len(), MAX_LINES);
        assert_eq!(x.last().unwrap().text, "error: number 49");
        assert_eq!(x[0].text, "error: number 30");
    }

    #[test]
    fn ansi_bom_and_carriage_returns_are_stripped() {
        let text = "\u{feff}\x1b]0;title\x07\x1b[1;31mred\x1b[0m\x1b(B\nbar 10%\rbar 100%\r\n";
        assert_eq!(clean(text), "red\nbar 100%\n");
        assert!(has_red("\x1b[1;91mx"));
        assert!(!has_red("\x1b[38;5;31mx"));
        assert!(!has_red("\x1b[32mx"));
    }
}
