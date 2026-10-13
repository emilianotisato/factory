//! The frontmatter reader: a port of `docs.mjs:62-88 parseFrontmatter`
//! (`key: value`, flow arrays, integers, quoted strings, full-line comments),
//! plus the one YAML form v3 adds, a `key: |` block scalar for the PRD's
//! `summary` (FR-13). It reads v2 files exactly as `docs.mjs` does: a v2 file
//! never holds a `|` value, so the extension changes nothing for them.

/// One scalar, as `parseScalar` types it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Num(i64),
    Str(String),
    List(Vec<Value>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    /// A string or a number as text; `None` for null and lists.
    pub fn text(&self) -> Option<String> {
        match self {
            Value::Str(s) => Some(s.clone()),
            Value::Num(n) => Some(n.to_string()),
            _ => None,
        }
    }
}

/// `data` is `None` when there is no frontmatter or it does not parse; then
/// `body` is the whole text and `error` says why when it was a parse failure.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub data: Option<Vec<(String, Value)>>,
    pub body: String,
    pub error: Option<String>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data
            .as_ref()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }
}

pub fn parse(text: &str) -> Frontmatter {
    let whole = || Frontmatter {
        data: None,
        body: text.to_string(),
        error: None,
    };
    if !text.starts_with("---\n") {
        return whole();
    }
    // `text.indexOf('\n---', 3)`: the first fence after the opening one.
    let Some(end) = text[3..].find("\n---").map(|i| i + 3) else {
        return whole();
    };
    let block = if end >= 4 { &text[4..end] } else { "" };
    let rest = &text[end + 4..];
    let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();
    let lines: Vec<&str> = block.split('\n').collect();
    let mut data: Vec<(String, Value)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, raw)) = key_line(line) else {
            return Frontmatter {
                data: None,
                body: text.to_string(),
                error: Some(format!("unparseable frontmatter line: {}", line.trim())),
            };
        };
        let raw = raw.trim();
        let value = if matches!(raw, "|" | "|-" | "|+") {
            let start = i;
            while i < lines.len()
                && (lines[i].trim().is_empty() || lines[i].starts_with([' ', '\t']))
            {
                i += 1;
            }
            block_scalar(&lines[start..i], raw)
        } else {
            scalar(raw)
        };
        match data.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => data.push((key.to_string(), value)),
        }
    }
    Frontmatter {
        data: Some(data),
        body,
        error: None,
    }
}

/// `^([A-Za-z_][\w-]*):\s*(.*)$`.
fn key_line(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once(':')?;
    let mut chars = key.chars();
    let first = chars.next()?;
    let ok = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ok.then_some((key, rest))
}

/// `parseScalar`.
fn scalar(raw: &str) -> Value {
    if raw.is_empty() {
        return Value::Null;
    }
    if let Some(inner) = raw.strip_prefix('[') {
        let inner = inner.strip_suffix(']').unwrap_or(inner).trim();
        if inner.is_empty() {
            return Value::List(Vec::new());
        }
        return Value::List(inner.split(',').map(|t| scalar(t.trim())).collect());
    }
    let quote = |c: char| c == '"' || c == '\'';
    if raw.len() >= 2 && raw.starts_with(quote) && raw.ends_with(quote) {
        return Value::Str(raw[1..raw.len() - 1].to_string());
    }
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(n) = raw.parse() {
            return Value::Num(n);
        }
    }
    Value::Str(raw.to_string())
}

/// The indented lines under `key: |`, dedented by the first line's indent.
/// `|` keeps one final newline, `|-` none, `|+` every trailing blank line.
fn block_scalar(lines: &[&str], style: &str) -> Value {
    let indent = lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map_or(0, |l| l.len() - l.trim_start().len());
    let mut out: Vec<&str> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                ""
            } else {
                &l[indent.min(l.len() - l.trim_start().len())..]
            }
        })
        .collect();
    let blank_tail = out.iter().rev().take_while(|l| l.is_empty()).count();
    out.truncate(out.len() - blank_tail);
    if out.is_empty() {
        return Value::Str(String::new());
    }
    let mut s = out.join("\n");
    match style {
        "|-" => {}
        "|+" => s.push_str(&"\n".repeat(blank_tail + 1)),
        _ => s.push('\n'),
    }
    Value::Str(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_frontmatter_reads_as_docs_mjs_reads_it() {
        let fm = parse(
            "---\nnumber: 7\nslug: x-y\nstatus: draft\n# a comment\nowns_fr: [13, \"14-19\"]\nretired_fr: []\nreviewed:\ntitle: 'A: b'\n---\n\n# PRD\n",
        );
        assert_eq!(fm.get("number"), Some(&Value::Num(7)));
        assert_eq!(fm.str("slug"), Some("x-y"));
        assert_eq!(
            fm.get("owns_fr"),
            Some(&Value::List(vec![
                Value::Num(13),
                Value::Str("14-19".into())
            ]))
        );
        assert_eq!(fm.get("retired_fr"), Some(&Value::List(vec![])));
        assert_eq!(fm.get("reviewed"), Some(&Value::Null));
        assert_eq!(fm.str("title"), Some("A: b"));
        // `docs.mjs` drops one newline after the closing `---`, not the blank line.
        assert_eq!(fm.body, "\n# PRD\n");

        let bad = parse("---\nnot a key line\n---\nbody");
        assert!(bad.data.is_none());
        assert!(bad.error.unwrap().contains("not a key line"));
        assert!(parse("# no frontmatter\n").data.is_none());
        assert!(parse("---\nunterminated: 1\n").data.is_none());
    }

    #[test]
    fn a_block_scalar_reads_its_indented_lines() {
        let fm = parse(
            "---\nnumber: 1\nsummary: |\n  First line: with a colon.\n\n    indented more\n  # not a comment\ntype: feature\nnote: |-\n  no newline\n---\nbody\n",
        );
        assert_eq!(
            fm.str("summary"),
            Some("First line: with a colon.\n\n  indented more\n# not a comment\n")
        );
        assert_eq!(fm.str("type"), Some("feature"));
        assert_eq!(fm.str("note"), Some("no newline"));
        assert_eq!(parse("---\ns: |\n---\n").str("s"), Some(""));
    }
}
