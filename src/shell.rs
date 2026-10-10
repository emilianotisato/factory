//! Just enough of a shell command line to read a worker's `Bash` call: the
//! words of its first simple command and what ends that command. Quotes and
//! backslashes are honoured, operators split words even when glued
//! (`a|b`), and a redirection's target is not an argument. Used by the
//! PreToolUse hook (`cli::hook`, slice 13) and by `metrics` for the `ctx-*`
//! kinds and the pack-doc reads; `metrics::first_word` keeps slice 08's
//! whitespace reading so the research's counts still compare.

/// A token: a word with its quotes removed, or a control operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Word(String),
    /// `|`, `|&`, `||`, `&&`, `;`, `&` (a newline reads as `;`).
    Op(&'static str),
    /// `>`, `>>`, `<`, `2>&1`…: the next word is its target.
    Redirect,
}

/// The tokens of `cmd`. Unbalanced quotes run to the end of the line.
pub fn tokens(cmd: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = cmd.chars().peekable();
    let flush = |out: &mut Vec<Tok>, word: &mut String, in_word: &mut bool| {
        if *in_word {
            out.push(Tok::Word(std::mem::take(word)));
            *in_word = false;
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                in_word = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => match chars.peek() {
                            Some(&n @ ('"' | '\\' | '$' | '`')) => {
                                word.push(n);
                                chars.next();
                            }
                            Some('\n') => {
                                chars.next();
                            }
                            _ => word.push('\\'),
                        },
                        _ => word.push(q),
                    }
                }
            }
            '\\' => {
                in_word = true;
                match chars.next() {
                    Some('\n') | None => {}
                    Some(n) => word.push(n),
                }
            }
            ' ' | '\t' => flush(&mut out, &mut word, &mut in_word),
            '\n' => {
                flush(&mut out, &mut word, &mut in_word);
                out.push(Tok::Op(";"));
            }
            '#' if !in_word => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push(Tok::Op(";"));
                        break;
                    }
                }
            }
            ';' => {
                flush(&mut out, &mut word, &mut in_word);
                out.push(Tok::Op(";"));
            }
            '|' => {
                flush(&mut out, &mut word, &mut in_word);
                match chars.peek() {
                    Some('|') => {
                        chars.next();
                        out.push(Tok::Op("||"));
                    }
                    Some('&') => {
                        chars.next();
                        out.push(Tok::Op("|&"));
                    }
                    _ => out.push(Tok::Op("|")),
                }
            }
            '&' => {
                flush(&mut out, &mut word, &mut in_word);
                match chars.peek() {
                    Some('&') => {
                        chars.next();
                        out.push(Tok::Op("&&"));
                    }
                    Some('>') => {
                        if !redirect(&mut chars) {
                            out.push(Tok::Redirect);
                        }
                    }
                    _ => out.push(Tok::Op("&")),
                }
            }
            '>' | '<' => {
                // `2>`: the digits before the arrow are a file descriptor.
                if in_word && word.chars().all(|d| d.is_ascii_digit()) {
                    word.clear();
                    in_word = false;
                }
                flush(&mut out, &mut word, &mut in_word);
                // `>&1`, `<&0`: the descriptor is the target, already eaten.
                if !redirect(&mut chars) {
                    out.push(Tok::Redirect);
                }
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
    }
    flush(&mut out, &mut word, &mut in_word);
    out
}

/// Eat the rest of a redirection operator (`>>`, `>&`, `<<<`) and, for a
/// descriptor duplication (`>&2`), its descriptor; true when that was one,
/// so no target word follows.
fn redirect(chars: &mut std::iter::Peekable<std::str::Chars>) -> bool {
    while let Some(&n) = chars.peek() {
        if n == '>' || n == '<' {
            chars.next();
        } else {
            break;
        }
    }
    if chars.peek() == Some(&'&') {
        chars.next();
        while chars
            .peek()
            .is_some_and(|d| d.is_ascii_digit() || *d == '-')
        {
            chars.next();
        }
        return true;
    }
    false
}

/// The first simple command: its words after leading `cd …`/`pushd …`
/// segments, `env`, `time` and `VAR=value` prefixes (redirection targets
/// left out), and the operator that ends it (`None` at the end of the line).
pub fn first_command(cmd: &str) -> (Vec<String>, Option<&'static str>) {
    let toks = tokens(cmd);
    let mut i = 0;
    loop {
        let mut words = Vec::new();
        let mut end = None;
        let mut skip_target = false;
        while i < toks.len() {
            match &toks[i] {
                Tok::Op(op) => {
                    end = Some(*op);
                    i += 1;
                    break;
                }
                Tok::Redirect => skip_target = true,
                Tok::Word(w) => {
                    if skip_target {
                        skip_target = false;
                    } else if !(words.is_empty() && is_prefix(w)) {
                        words.push(w.clone());
                    }
                }
            }
            i += 1;
        }
        let is_cd = matches!(words.first().map(String::as_str), Some("cd" | "pushd"));
        if is_cd && end.is_some() && i < toks.len() {
            continue;
        }
        return (words, end);
    }
}

fn is_prefix(w: &str) -> bool {
    w == "env" || w == "time" || is_assignment(w)
}

/// `NAME=value`.
pub fn is_assignment(tok: &str) -> bool {
    let Some((name, _)) = tok.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

/// A program word without its directory: `/usr/bin/grep` → `grep`.
pub fn program(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(cmd: &str) -> (Vec<&'static str>, Option<&'static str>) {
        let (w, end) = first_command(cmd);
        (w.into_iter().map(|s| &*s.leak()).collect(), end)
    }

    #[test]
    fn quotes_operators_and_redirections() {
        assert_eq!(
            words("grep -rn \"foo bar\" src 2>/dev/null | head"),
            (vec!["grep", "-rn", "foo bar", "src"], Some("|"))
        );
        assert_eq!(
            words("cd /repo && FOO=1 env grep -n 'a|b' x.php"),
            (vec!["grep", "-n", "a|b", "x.php"], None)
        );
        assert_eq!(words("cat a.md|grep x"), (vec!["cat", "a.md"], Some("|")));
        assert_eq!(
            words("cat a.md >out.txt; ls"),
            (vec!["cat", "a.md"], Some(";"))
        );
        assert_eq!(words("rg foo 2>&1"), (vec!["rg", "foo"], None));
        assert_eq!(
            words("grep -r foo 2>&1 src"),
            (vec!["grep", "-r", "foo", "src"], None)
        );
        assert_eq!(words("echo a\\ b # c"), (vec!["echo", "a b"], None));
        assert_eq!(words("cd src"), (vec!["cd", "src"], None));
        assert_eq!(words(""), (vec![], None));
        assert_eq!(program("/usr/bin/grep"), "grep");
    }
}
