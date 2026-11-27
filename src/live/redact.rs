//! What leaves the run before it prints: token shapes become a marker and home paths `~`.
//! Best effort over every string; a secret this misses still prints.
use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::paths;

pub const MARKER: &str = "[redacted]";

/// Each pattern and its replacement; a replacement keeps the name or prefix that told the secret apart.
static RULES: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    let m = MARKER;
    [
        (r"\bsk-ant-[A-Za-z0-9_-]+", format!("sk-ant-{m}")),
        (r"\b(gh[pousr]_)[A-Za-z0-9]+", format!("${{1}}{m}")),
        (r"\bgithub_pat_[A-Za-z0-9_]+", format!("github_pat_{m}")),
        (r"\b(A[KS]IA)[0-9A-Z]{16}\b", format!("${{1}}{m}")),
        (r"\b(xox[a-z]-)[A-Za-z0-9-]+", format!("${{1}}{m}")),
        (
            r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*",
            m.to_string(),
        ),
        (r"(?i)\b(bearer\s+)\S+", format!("${{1}}{m}")),
        (r"(?i)\b(x-api-key\s*:\s*)\S+", format!("${{1}}{m}")),
        (
            r#"(?i)("[\w-]*(?:password|secret|token|key)"\s*:\s*")(?:[^"\\]|\\.)*""#,
            format!("${{1}}{m}\""),
        ),
        (
            r#"(?i)\b((?:[A-Za-z0-9]+_)*(?:key|secret|token|password|pass))=(?:"[^"]*"|'[^']*'|[^\s"'&,;]+)"#,
            format!("${{1}}={m}"),
        ),
        (
            r"(?i)([a-z][a-z0-9+.-]*://[^/\s:@]+:)[^@\s/]+(@)",
            format!("${{1}}{m}${{2}}"),
        ),
        (
            r"(?i)\b(authorization\s*:\s*(?:basic|token)\s+)\S+",
            format!("${{1}}{m}"),
        ),
        (
            r"(?i)\b(private-token\s*:\s*)\S+",
            format!("${{1}}{m}"),
        ),
        (
            r"(?i)\b((?:set-)?cookie\s*:\s*)[^\r\n;]+",
            format!("${{1}}{m}"),
        ),
        (
            r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|$)",
            m.to_string(),
        ),
        (r"\bsk-proj-[A-Za-z0-9_-]+", format!("sk-proj-{m}")),
        (r"\bglpat-[A-Za-z0-9_-]+", format!("glpat-{m}")),
        (r"\bnpm_[A-Za-z0-9]+", format!("npm_{m}")),
        (r"\bAIza[0-9A-Za-z_-]{20,}", format!("AIza{m}")),
        (r"\bsk_(?:live|test)_[0-9A-Za-z]+", format!("sk_{m}")),
        (r"\bxapp-[0-9A-Za-z-]+", format!("xapp-{m}")),
        (r"/[0-9A-Za-z]{41,}", format!("/{m}")),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).expect("redaction pattern compiles"), r))
    .collect()
});

#[derive(Clone, Copy, Default)]
struct WalkCtx {
    evidence: bool,
    checkpoints: bool,
    files: bool,
    attention: bool,
}

impl WalkCtx {
    fn child(self, key: &str) -> Self {
        let mut c = self;
        match key {
            "evidence" => {
                c.evidence = true;
                c.checkpoints = false;
                c.files = false;
            }
            "checkpoints" if self.evidence => c.checkpoints = true,
            "files" if self.evidence && self.checkpoints => c.files = true,
            "attention" => c.attention = true,
            _ => {}
        }
        c
    }
}

/// Runs over the whole snapshot, before `rev` and the cap. The top-level `repo` keeps its stored path.
pub fn all(v: &mut Value) {
    let home = paths::home();
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                walk(x, &home, k != "repo", WalkCtx::default().child(k), false);
            }
        }
        v => walk(v, &home, true, WalkCtx::default(), false),
    }
}

fn walk(v: &mut Value, home: &Path, tilde: bool, ctx: WalkCtx, skip_tokens: bool) {
    match v {
        Value::String(s) => {
            if let Cow::Owned(t) = string(s, home, tilde, skip_tokens) {
                *s = t;
            }
        }
        Value::Array(a) => {
            for x in a.iter_mut() {
                walk(x, home, tilde, ctx, false);
            }
        }
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                let child = ctx.child(k);
                let skip = skip_tokens
                    || (child.evidence && child.checkpoints && child.files && k == "path")
                    || (child.attention && k == "answer");
                walk(x, home, tilde, child, skip);
            }
        }
        _ => {}
    }
}

fn string<'a>(s: &'a str, home: &Path, tilde: bool, skip_tokens: bool) -> Cow<'a, str> {
    let mut out = if skip_tokens {
        Cow::Borrowed(s)
    } else {
        tokens(s)
    };
    if tilde {
        let t = bare_home(&paths::home_to_tilde(&out, home), home);
        if t != out {
            out = Cow::Owned(t);
        }
    }
    out
}

/// The home directory named alone, as in `cd /home/x` or `/home/x:`, also becomes `~`.
fn bare_home(text: &str, home: &Path) -> String {
    let Some(home) = home.to_str().map(|h| h.trim_end_matches('/')) else {
        return text.to_string();
    };
    if home.is_empty() {
        return text.to_string();
    }
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || "._-~/".contains(c));
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(home) {
        let end = at + home.len();
        let alone = !word(rest[..at].chars().next_back()) && !word(rest[end..].chars().next());
        out.push_str(&rest[..at]);
        out.push_str(if alone { "~" } else { home });
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `text` with every token shape replaced by [`MARKER`].
pub fn tokens(text: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(text);
    for (re, with) in RULES.iter() {
        if let Cow::Owned(t) = re.replace_all(&out, with.as_str()) {
            out = Cow::Owned(t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn r(s: &str) -> String {
        tokens(s).into_owned()
    }

    #[test]
    fn each_prefixed_token_keeps_its_prefix() {
        for (secret, kept) in [
            ("sk-ant-api03-AbC_d-123xyz", "sk-ant-"),
            ("ghp_FAKEt0kenN0tRealAbCdEfGh1234567890", "ghp_"),
            ("gho_abcDEF1234567890", "gho_"),
            ("github_pat_11ABCDEFG_0123456789abcdef", "github_pat_"),
            ("AKIAABCDEFGHIJKLMNOP", "AKIA"),
            ("xoxb-1234-5678-AbCdEf", "xoxb-"),
            ("xoxp-99-abc", "xoxp-"),
        ] {
            assert_eq!(
                r(&format!("got {secret} back")),
                format!("got {kept}{MARKER} back")
            );
        }
    }

    #[test]
    fn a_jwt_goes_whole_and_a_dotted_name_stays() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.SflKxwRJSMeKKF2QT4fwpMeJf36P_k-x";
        assert_eq!(r(&format!("jwt={jwt}")), format!("jwt={MARKER}"));
        assert_eq!(r(&format!("token {jwt}.")), format!("token {MARKER}."));
        for s in [
            "factory 1.2.3",
            "v10.20.30-rc.1",
            "src/live/redact.rs",
            "package.lock.json",
            "eyJ.only",
        ] {
            assert_eq!(r(s), s);
        }
    }

    #[test]
    fn bearer_keeps_its_word() {
        assert_eq!(
            r("Authorization: Bearer abc.def-123"),
            format!("Authorization: Bearer {MARKER}")
        );
        assert_eq!(
            r("authorization: bearer xyz"),
            format!("authorization: bearer {MARKER}")
        );
    }

    #[test]
    fn an_api_key_header_keeps_its_name() {
        assert_eq!(
            r("curl -H 'X-API-Key: s3cr3tValue' https://x"),
            format!("curl -H 'X-API-Key: {MARKER} https://x")
        );
        assert_eq!(r("x-api-key:abc"), format!("x-api-key:{MARKER}"));
    }

    #[test]
    fn env_style_names_match_in_any_case_and_as_a_suffix() {
        for name in [
            "KEY",
            "SECRET",
            "TOKEN",
            "PASSWORD",
            "PASS",
            "APP_KEY",
            "GITHUB_TOKEN",
            "DB_PASSWORD",
            "api_key",
            "Client_Secret",
            "token",
            "my_secret",
        ] {
            assert_eq!(
                r(&format!("run {name}=hunter2 now")),
                format!("run {name}={MARKER} now")
            );
        }
        assert_eq!(
            r(r#"DB_PASSWORD="two words" next"#),
            format!("DB_PASSWORD={MARKER} next")
        );
        assert_eq!(
            r("https://x/cb?token=abc123&page=2"),
            format!("https://x/cb?token={MARKER}&page=2")
        );
        assert_eq!(r("PATH=/usr/bin COLOR=1"), "PATH=/usr/bin COLOR=1");
        assert_eq!(r("bypass=0"), "bypass=0");
        assert_eq!(r("compass=north"), "compass=north");
    }

    #[test]
    fn high_signal_url_auth_cookie_pem_and_vendor_prefixes() {
        assert_eq!(
            r("postgres://app:s3cr3t@db"),
            format!("postgres://app:{MARKER}@db")
        );
        assert_eq!(
            r("Authorization: Basic dXNlcjpwYXNz"),
            format!("Authorization: Basic {MARKER}")
        );
        assert_eq!(
            r("PRIVATE-TOKEN: glpat-123456"),
            format!("PRIVATE-TOKEN: {MARKER}")
        );
        assert_eq!(r("Cookie: session=abc"), format!("Cookie: {MARKER}"));
        let pem = "-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----";
        assert_eq!(r(pem), MARKER);
        for secret in [
            "sk-proj-abc123",
            "glpat-abcdef",
            "npm_AbCdEf123",
            "AIzaSyD123456789012345678901",
            "sk_live_abc123",
            "xapp-1-abc-def",
        ] {
            assert!(r(secret).contains(MARKER), "expected redaction of {secret}");
        }
    }

    #[test]
    fn a_json_secret_keeps_its_key() {
        assert_eq!(
            r(r#"{"password": "p@ss w\"rd", "user": "me"}"#),
            format!(r#"{{"password": "{MARKER}", "user": "me"}}"#)
        );
        for key in ["secret", "Token", "key", "access_token", "api-key"] {
            assert_eq!(
                r(&format!(r#"{{"{key}":"v4lue"}}"#)),
                format!(r#"{{"{key}":"{MARKER}"}}"#)
            );
        }
        assert_eq!(r(r#"{"name": "v4lue"}"#), r#"{"name": "v4lue"}"#);
    }

    #[test]
    fn a_long_path_segment_goes_and_a_sha_stays() {
        let url = format!("https://github.com/o/r/commit/{SHA}");
        assert_eq!(r(&url), url);
        assert_eq!(r(SHA), SHA);
        let long = format!("{SHA}9");
        assert_eq!(
            r(&format!("https://hooks.example/t/{long}?x=1")),
            format!("https://hooks.example/t/{MARKER}?x=1")
        );
        let base62 = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789AbCdEfGh";
        assert_eq!(
            r(&format!("https://x/{base62}/y")),
            format!("https://x/{MARKER}/y")
        );
        assert_eq!(r(&format!("head {SHA}9")), format!("head {SHA}9"));
    }

    #[test]
    fn plain_text_is_borrowed_untouched() {
        let s = "gate 1 attempt 2 red after 1.5 min";
        assert!(matches!(tokens(s), Cow::Borrowed(_)));
    }

    #[test]
    fn home_becomes_a_tilde_everywhere_but_the_top_level_repo() {
        let home = Path::new("/srv/me");
        let mut v = json!({
            "repo": "/srv/me/code/app GITHUB_TOKEN=abc",
            "gate": {"tail": ["/srv/me/code/app/src failed", "/tmp/srv/me/x", "cd /srv/me: denied", "/srv/meg"]},
            "workers": [{"repo": "/srv/me/a"}],
        });
        if let Value::Object(o) = &mut v {
            for (k, x) in o.iter_mut() {
                walk(x, home, k != "repo", WalkCtx::default(), false);
            }
        }
        assert_eq!(
            v,
            json!({
                "repo": format!("/srv/me/code/app GITHUB_TOKEN={MARKER}"),
                "gate": {"tail": ["~/code/app/src failed", "/tmp/srv/me/x", "cd ~: denied", "/srv/meg"]},
                "workers": [{"repo": "~/a"}],
            })
        );
    }

    #[test]
    fn evidence_path_and_attention_answer_skip_token_patterns() {
        let seg = "a".repeat(45);
        let path = format!("/tmp/{seg}/file.rs");
        let mut v = json!({
            "evidence": {
                "checkpoints": [{
                    "files": [{ "path": path }]
                }]
            },
            "attention": [{
                "answer": format!("/bin/run --path /{seg}")
            }]
        });
        all(&mut v);
        let kept_path = v["evidence"]["checkpoints"][0]["files"][0]["path"]
            .as_str()
            .unwrap();
        assert!(
            kept_path.contains(&seg),
            "path must not be mangled by token rules: {kept_path}"
        );
        let kept_answer = v["attention"][0]["answer"].as_str().unwrap();
        assert!(
            kept_answer.contains(&seg),
            "answer must stay runnable: {kept_answer}"
        );
    }
}
