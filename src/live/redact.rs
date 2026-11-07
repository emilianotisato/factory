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
            r#"(?i)\b(\w*(?:key|secret|token|password|pass))=(?:"[^"]*"|'[^']*'|[^\s"'&,;]+)"#,
            format!("${{1}}={m}"),
        ),
        (r"/[0-9A-Za-z]{41,}", format!("/{m}")),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).expect("redaction pattern compiles"), r))
    .collect()
});

/// Runs over the whole snapshot, before `rev` and the cap. The top-level `repo` keeps its stored path.
pub fn all(v: &mut Value) {
    let home = paths::home();
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                walk(x, &home, k != "repo");
            }
        }
        v => walk(v, &home, true),
    }
}

fn walk(v: &mut Value, home: &Path, tilde: bool) {
    match v {
        Value::String(s) => {
            if let Cow::Owned(t) = string(s, home, tilde) {
                *s = t;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| walk(x, home, tilde)),
        Value::Object(o) => o.values_mut().for_each(|x| walk(x, home, tilde)),
        _ => {}
    }
}

fn string<'a>(s: &'a str, home: &Path, tilde: bool) -> Cow<'a, str> {
    let mut out = tokens(s);
    if tilde {
        let t = paths::home_to_tilde(&out, home);
        if t != out {
            out = Cow::Owned(t);
        }
    }
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
            "gate": {"tail": ["/srv/me/code/app/src/x.rs:3 failed", "/tmp/srv/me/x"]},
            "workers": [{"repo": "/srv/me/a"}],
        });
        if let Value::Object(o) = &mut v {
            for (k, x) in o.iter_mut() {
                walk(x, home, k != "repo");
            }
        }
        assert_eq!(
            v,
            json!({
                "repo": format!("/srv/me/code/app GITHUB_TOKEN={MARKER}"),
                "gate": {"tail": ["~/code/app/src/x.rs:3 failed", "/tmp/srv/me/x"]},
                "workers": [{"repo": "~/a"}],
            })
        );
    }
}
