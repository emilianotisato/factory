//! Just enough of a shell command line to read a worker's `Bash` call: the
//! words of its first simple command and what ends that command. Quotes and
//! backslashes are honoured, operators split words even when glued
//! (`a|b`), and a redirection's target is not an argument. Used by the
//! PreToolUse hook (`cli::hook`, slice 13) and by `metrics` for the `ctx-*`
//! kinds and the pack-doc reads.
//!
//! [`script`] reads further, for the hook's guard: every simple command the
//! line can run, at any depth, and whether the reading is sure.

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
    let (words, end, _) = first_command_in(cmd);
    (words, end)
}

/// [`first_command`], and where the leading `cd`/`pushd` segments it skips
/// went, in order: each one's target, `None` when it names none (`cd`).
pub type FirstCommand = (Vec<String>, Option<&'static str>, Vec<Option<String>>);

/// See [`FirstCommand`].
pub fn first_command_in(cmd: &str) -> FirstCommand {
    let toks = tokens(cmd);
    let mut i = 0;
    let mut cds = Vec::new();
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
            let target = words[1..]
                .iter()
                .rfind(|w| !w.starts_with('-') || w.as_str() == "-");
            cds.push(target.cloned());
            continue;
        }
        return (words, end, cds);
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

/// One simple command of a [`Script`]: its words, quotes removed and
/// expansions kept as written (`$x`, `$(…)`), and what here-documents and
/// here-strings hand its stdin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Simple {
    pub words: Vec<String>,
    pub stdin: Vec<String>,
}

/// Every simple command a command line can run, at any depth: `&&`, `||`,
/// `;`, `|` and `&` lists, `( … )` and `{ …; }` groups, and the bodies of
/// `$( … )`, backquotes and `<( … )`/`>( … )` — inside double quotes and
/// unquoted here-documents too. A string a command hands another shell
/// (`sh -c '…'`) stays a word: reading that is the caller's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Script {
    pub commands: Vec<Simple>,
    /// A quote, substitution or here-document never closed, or nesting too
    /// deep to follow: the reading may have missed a command.
    pub unbalanced: bool,
}

/// How deep [`script`] follows substitutions before it gives up.
const MAX_NEST: usize = 32;

/// Read `cmd` as [`Script`].
pub fn script(cmd: &str) -> Script {
    let mut lx = Lexer::new(cmd, 0);
    lx.list(false);
    lx.finish()
}

/// A here-document waiting for the newline its body starts after.
struct Doc {
    delim: String,
    strip_tabs: bool,
    quoted: bool,
    owner: usize,
}

/// A word as [`Lexer::word`] reads it; `quoted` when any of it was quoted
/// or escaped (a here-document delimiter then turns expansion off).
struct Word {
    text: String,
    quoted: bool,
}

struct Lexer {
    s: Vec<char>,
    i: usize,
    nest: usize,
    out: Vec<Simple>,
    docs: Vec<Doc>,
    unbalanced: bool,
}

impl Lexer {
    fn new(text: &str, nest: usize) -> Lexer {
        Lexer {
            s: text.chars().collect(),
            i: 0,
            nest,
            out: Vec::new(),
            docs: Vec::new(),
            unbalanced: false,
        }
    }

    fn peek(&self, k: usize) -> Option<char> {
        self.s.get(self.i + k).copied()
    }

    fn bump(&mut self, k: usize) {
        self.i = (self.i + k).min(self.s.len());
    }

    fn finish(mut self) -> Script {
        if !self.docs.is_empty() {
            self.unbalanced = true;
        }
        self.out
            .retain(|c| !c.words.is_empty() || !c.stdin.is_empty());
        Script {
            commands: self.out,
            unbalanced: self.unbalanced,
        }
    }

    /// Take in what a nested lexer found.
    fn absorb(&mut self, lx: Lexer) {
        let sc = lx.finish();
        self.unbalanced |= sc.unbalanced;
        self.out.extend(sc.commands);
    }

    /// The command being built, opened on first use.
    fn slot(&mut self, cur: &mut Option<usize>) -> usize {
        *cur.get_or_insert_with(|| {
            self.out.push(Simple::default());
            self.out.len() - 1
        })
    }

    /// Commands up to the end, or with `sub` up to the `)` that closes a
    /// `$(`, `<(` or `>(` (true when it came). A `case` pattern's `)` does
    /// not close it.
    fn list(&mut self, sub: bool) -> bool {
        let mut cur: Option<usize> = None;
        let mut depth = 0usize;
        let mut cases = 0usize;
        while let Some(c) = self.peek(0) {
            match c {
                ' ' | '\t' | '\r' => self.bump(1),
                '\n' => {
                    self.bump(1);
                    cur = None;
                    self.bodies();
                }
                '#' => {
                    while self.peek(0).is_some_and(|n| n != '\n') {
                        self.bump(1);
                    }
                }
                '&' if self.peek(1) == Some('>') => {
                    let at = self.slot(&mut cur);
                    self.redirect(at);
                }
                ';' | '&' | '|' => {
                    while matches!(self.peek(0), Some(';' | '&' | '|')) {
                        self.bump(1);
                    }
                    cur = None;
                }
                '(' => {
                    self.bump(1);
                    depth += 1;
                    cur = None;
                }
                ')' => {
                    self.bump(1);
                    cur = None;
                    if depth > 0 {
                        depth -= 1;
                    } else if sub && cases == 0 {
                        return true;
                    }
                }
                '<' | '>' if self.peek(1) != Some('(') => {
                    let at = self.slot(&mut cur);
                    self.redirect(at);
                }
                _ => {
                    let at = self.slot(&mut cur);
                    let from = self.i;
                    let w = self.word();
                    if self.i == from {
                        self.bump(1);
                    }
                    let fd = !w.quoted
                        && !w.text.is_empty()
                        && w.text.bytes().all(|b| b.is_ascii_digit())
                        && matches!(self.peek(0), Some('<' | '>'));
                    if fd {
                        continue;
                    }
                    if self.out[at].words.is_empty() && !w.quoted {
                        match w.text.as_str() {
                            "case" => cases += 1,
                            "esac" => cases = cases.saturating_sub(1),
                            _ => {}
                        }
                    }
                    self.out[at].words.push(w.text);
                }
            }
        }
        false
    }

    /// A redirection for the command at `owner`: the operator, then its
    /// target — a here-document's delimiter, a here-string (the command's
    /// stdin) or a file — unless it duplicates a descriptor (`2>&1`).
    fn redirect(&mut self, owner: usize) {
        if self.peek(0) == Some('&') {
            self.bump(1);
        }
        let mut lt = 0;
        while let Some(c @ ('<' | '>')) = self.peek(0) {
            if c == '<' {
                lt += 1;
            }
            self.bump(1);
        }
        if self.peek(0) == Some('|') {
            self.bump(1);
        }
        if self.peek(0) == Some('&') {
            self.bump(1);
            let from = self.i;
            while self.peek(0).is_some_and(|d| d.is_ascii_digit() || d == '-') {
                self.bump(1);
            }
            if self.i > from {
                return;
            }
        }
        let strip_tabs = lt == 2 && self.peek(0) == Some('-');
        if strip_tabs {
            self.bump(1);
        }
        while matches!(self.peek(0), Some(' ' | '\t')) {
            self.bump(1);
        }
        let ends = |c: char| matches!(c, '\n' | ';' | '&' | '|' | '(' | ')' | '<' | '>');
        if self.peek(0).is_none_or(ends) {
            self.unbalanced = true;
            return;
        }
        let w = self.word();
        match lt {
            2 => self.docs.push(Doc {
                delim: w.text,
                strip_tabs,
                quoted: w.quoted,
                owner,
            }),
            3 => self.out[owner].stdin.push(w.text),
            _ => {}
        }
    }

    /// The bodies of the here-documents opened on the line just ended.
    fn bodies(&mut self) {
        for doc in std::mem::take(&mut self.docs) {
            let mut body = String::new();
            let mut closed = false;
            while self.i < self.s.len() {
                let from = self.i;
                while self.peek(0).is_some_and(|c| c != '\n') {
                    self.bump(1);
                }
                let line: String = self.s[from..self.i].iter().collect();
                self.bump(1);
                let line = if doc.strip_tabs {
                    line.trim_start_matches('\t')
                } else {
                    &line
                };
                if line == doc.delim {
                    closed = true;
                    break;
                }
                body.push_str(line);
                body.push('\n');
            }
            if !closed {
                self.unbalanced = true;
            }
            if !doc.quoted {
                let mut lx = Lexer::new(&body, self.nest + 1);
                lx.dquoted(&mut String::new(), false);
                self.absorb(lx);
            }
            self.out[doc.owner].stdin.push(body);
        }
    }

    /// One word: quotes removed, expansions kept as written, and every
    /// substitution in it read for its commands.
    fn word(&mut self) -> Word {
        let mut w = Word {
            text: String::new(),
            quoted: false,
        };
        let start = self.i;
        while let Some(c) = self.peek(0) {
            match c {
                ' ' | '\t' | '\r' | '\n' | ';' | '&' | '|' | '(' | ')' => break,
                '<' | '>' if self.i == start && self.peek(1) == Some('(') => {
                    self.bump(2);
                    self.subst();
                    w.text.extend(&self.s[start..self.i]);
                }
                '<' | '>' => break,
                '\'' => {
                    w.quoted = true;
                    self.bump(1);
                    self.single(&mut w.text);
                }
                '"' => {
                    w.quoted = true;
                    self.bump(1);
                    self.dquoted(&mut w.text, true);
                }
                '\\' => {
                    w.quoted = true;
                    if let Some(n) = self.peek(1).filter(|n| *n != '\n') {
                        w.text.push(n);
                    }
                    self.bump(2);
                }
                '$' => self.dollar(&mut w.text, false),
                '`' => self.backquote(&mut w.text),
                _ => {
                    w.text.push(c);
                    self.bump(1);
                }
            }
        }
        w
    }

    /// The rest of a single-quoted string.
    fn single(&mut self, text: &mut String) {
        while let Some(c) = self.peek(0) {
            self.bump(1);
            if c == '\'' {
                return;
            }
            text.push(c);
        }
        self.unbalanced = true;
    }

    /// Inside double quotes up to the closing one, or with `closing` false
    /// an unquoted here-document's body to its end.
    fn dquoted(&mut self, text: &mut String, closing: bool) {
        while let Some(c) = self.peek(0) {
            match c {
                '"' if closing => {
                    self.bump(1);
                    return;
                }
                '\\' => match self.peek(1) {
                    Some(n @ ('"' | '\\' | '$' | '`')) => {
                        text.push(n);
                        self.bump(2);
                    }
                    Some('\n') => self.bump(2),
                    _ => {
                        text.push('\\');
                        self.bump(1);
                    }
                },
                '$' => self.dollar(text, true),
                '`' => self.backquote(text),
                _ => {
                    text.push(c);
                    self.bump(1);
                }
            }
        }
        if closing {
            self.unbalanced = true;
        }
    }

    /// A `$`: `$(…)` and `${…}` are read and kept as written; `$'…'`
    /// outside double quotes is ANSI-C quoting, kept as written when an
    /// escape in it could spell anything.
    fn dollar(&mut self, text: &mut String, in_dquotes: bool) {
        let start = self.i;
        match self.peek(1) {
            Some('(') => {
                self.bump(2);
                self.subst();
            }
            Some('{') => {
                self.bump(2);
                if self.nest >= MAX_NEST {
                    self.unbalanced = true;
                    self.i = self.s.len();
                    return;
                }
                self.nest += 1;
                self.param();
                self.nest -= 1;
            }
            Some('\'') if !in_dquotes => {
                self.bump(2);
                let mut lit = String::new();
                self.single(&mut lit);
                if lit.contains('\\') {
                    text.extend(&self.s[start..self.i]);
                } else {
                    text.push_str(&lit);
                }
                return;
            }
            Some('"') if !in_dquotes => {
                self.bump(2);
                self.dquoted(text, true);
                return;
            }
            _ => {
                text.push('$');
                self.bump(1);
                return;
            }
        }
        text.extend(&self.s[start..self.i]);
    }

    /// The body of `$(`, `<(` or `>(` up to its `)`, read as commands.
    fn subst(&mut self) {
        if self.nest >= MAX_NEST {
            self.unbalanced = true;
            self.i = self.s.len();
            return;
        }
        self.nest += 1;
        if !self.list(true) {
            self.unbalanced = true;
        }
        self.nest -= 1;
    }

    /// The rest of `${…}`; a substitution inside it is read.
    fn param(&mut self) {
        let mut sink = String::new();
        while let Some(c) = self.peek(0) {
            match c {
                '}' => {
                    self.bump(1);
                    return;
                }
                '\\' => self.bump(2),
                '\'' => {
                    self.bump(1);
                    self.single(&mut sink);
                }
                '"' => {
                    self.bump(1);
                    self.dquoted(&mut sink, true);
                }
                '$' => self.dollar(&mut sink, false),
                '`' => self.backquote(&mut sink),
                _ => self.bump(1),
            }
        }
        self.unbalanced = true;
    }

    /// A backquoted substitution: kept as written, its body read as
    /// commands.
    fn backquote(&mut self, text: &mut String) {
        let start = self.i;
        self.bump(1);
        let mut body = String::new();
        let mut closed = false;
        while let Some(c) = self.peek(0) {
            match c {
                '`' => {
                    self.bump(1);
                    closed = true;
                    break;
                }
                '\\' => match self.peek(1) {
                    Some(n @ ('`' | '\\' | '$')) => {
                        body.push(n);
                        self.bump(2);
                    }
                    _ => {
                        body.push('\\');
                        self.bump(1);
                    }
                },
                _ => {
                    body.push(c);
                    self.bump(1);
                }
            }
        }
        if !closed {
            self.unbalanced = true;
        }
        text.extend(&self.s[start..self.i]);
        if self.nest >= MAX_NEST {
            self.unbalanced = true;
            return;
        }
        let mut lx = Lexer::new(&body, self.nest + 1);
        lx.list(false);
        self.absorb(lx);
    }
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

    #[test]
    fn leading_cds_are_kept_in_order() {
        let cds = |cmd: &str| first_command_in(cmd).2;
        assert_eq!(
            cds("cd /a && pushd b && cd -P c; ls"),
            [Some("/a"), Some("b"), Some("c")].map(|s| s.map(String::from))
        );
        assert_eq!(cds("cd && ls"), [None]);
        assert_eq!(cds("cd - && ls"), [Some("-".to_string())]);
        assert_eq!(cds("ls && cd x"), Vec::<Option<String>>::new());
    }

    fn all(cmd: &str) -> (Vec<Vec<String>>, bool) {
        let sc = script(cmd);
        (
            sc.commands.into_iter().map(|c| c.words).collect(),
            sc.unbalanced,
        )
    }

    fn w(list: &[&[&str]]) -> Vec<Vec<String>> {
        list.iter()
            .map(|c| c.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn script_finds_every_command_at_any_depth() {
        assert_eq!(
            all("a 1 && b|c; (d) || { e; } & f"),
            (
                w(&[
                    &["a", "1"],
                    &["b"],
                    &["c"],
                    &["d"],
                    &["{", "e"],
                    &["}"],
                    &["f"]
                ]),
                false
            )
        );
        assert_eq!(
            all("echo $(x 1) \"`y 2`\" <(z) >(v) ${a:-$(u)}"),
            (
                w(&[
                    &["echo", "$(x 1)", "`y 2`", "<(z)", ">(v)", "${a:-$(u)}"],
                    &["x", "1"],
                    &["y", "2"],
                    &["z"],
                    &["v"],
                    &["u"],
                ]),
                false
            )
        );
        // Redirections: targets are not words; descriptors are not words.
        assert_eq!(
            all("a 2>&1 >out <in 3>x b &>/dev/null"),
            (w(&[&["a", "b"]]), false)
        );
        // A case pattern's `)` does not close the substitution.
        assert_eq!(
            all("echo \"$(case x in a) y;; esac)\""),
            (
                w(&[
                    &["echo", "$(case x in a) y;; esac)"],
                    &["case", "x", "in", "a"],
                    &["y"],
                    &["esac"],
                ]),
                false
            )
        );
    }

    #[test]
    fn here_documents_feed_stdin_and_expand_unless_quoted() {
        let sc = script("cat <<'EOF' > f\n$(a)\nEOF\nbash <<-X\n\t$(b) c\n\tX\nd <<< 'e f'");
        let words: Vec<_> = sc.commands.iter().map(|c| c.words.join(" ")).collect();
        assert_eq!(words, ["cat", "bash", "b", "d"]);
        assert_eq!(sc.commands[0].stdin, ["$(a)\n"]);
        assert_eq!(sc.commands[1].stdin, ["$(b) c\n"]);
        assert_eq!(sc.commands[3].stdin, ["e f"]);
        assert!(!sc.unbalanced);
        // A commit message inside a substitution: the body is literal.
        let sc = script("git commit -m \"$(cat <<'EOF'\nfix(x): a ) b `c`\nEOF\n)\" && ls");
        let words: Vec<_> = sc.commands.iter().map(|c| c.words[0].clone()).collect();
        assert_eq!(words, ["git", "cat", "ls"]);
        assert!(!sc.unbalanced);
    }

    #[test]
    fn what_never_closes_is_unbalanced() {
        for cmd in [
            "echo 'a",
            "echo \"a",
            "echo $(a",
            "echo `a",
            "echo ${a",
            "cat <<EOF\nx",
            "cat <<EOF",
        ] {
            assert!(script(cmd).unbalanced, "{cmd:?}");
        }
        let deep = format!("{}x{}", "$(".repeat(100), ")".repeat(100));
        assert!(script(&deep).unbalanced);
        let deep = format!("{}x{}", "${".repeat(100), "}".repeat(100));
        assert!(script(&deep).unbalanced);
        assert!(!script("a 'b' \"c\" $(d) `e` ${f}").unbalanced);
    }
}
