use std::fs;
use std::process::Command;

use factory::excerpt;
use factory::live::redact::{self, MARKER};
use serde_json::json;

#[test]
fn redact_hardening_and_excerpt_gate_failure_edge_cases() {
    let seg = "a".repeat(45);

    assert_eq!(
        redact::tokens("postgres://app:s3cr3t@db").into_owned(),
        format!("postgres://app:{MARKER}@db")
    );
    assert_eq!(
        redact::tokens("Authorization: Basic dXNlcjpwYXNz").into_owned(),
        format!("Authorization: Basic {MARKER}")
    );
    assert_eq!(
        redact::tokens("PRIVATE-TOKEN: glpat-123456").into_owned(),
        format!("PRIVATE-TOKEN: {MARKER}")
    );
    assert_eq!(
        redact::tokens("Cookie: session=abc").into_owned(),
        format!("Cookie: {MARKER}")
    );
    let pem = "-----BEGIN RSA PRIVATE KEY-----\nline\n-----END RSA PRIVATE KEY-----";
    assert_eq!(redact::tokens(pem).into_owned(), MARKER);
    for secret in [
        "sk-proj-abc",
        "glpat-abc",
        "npm_token123",
        "AIzaSyD123456789012345678901",
        "sk_live_abc123",
        "xapp-1-abc",
    ] {
        assert!(
            redact::tokens(secret).contains(MARKER),
            "expected {:?} redacted",
            secret
        );
    }
    assert_eq!(redact::tokens("bypass=0").into_owned(), "bypass=0");
    assert_eq!(
        redact::tokens("compass=north").into_owned(),
        "compass=north"
    );

    let path = format!("/tmp/{seg}/x.rs");
    let mut snap = json!({
      "evidence": {
        "checkpoints": [{ "files": [{ "path": path.clone() }] }]
      },
      "attention": [{ "answer": format!("run --path /{seg}") }]
    });
    redact::all(&mut snap);
    assert_eq!(
        snap["evidence"]["checkpoints"][0]["files"][0]["path"].as_str(),
        Some(path.as_str())
    );
    assert!(
        snap["attention"][0]["answer"]
            .as_str()
            .unwrap()
            .contains(&seg)
    );

    assert!(excerpt::excerpt(br#"{"result":"passed","failed":0}"#, &[]).is_empty());
    assert!(excerpt::excerpt(b"\x1b[38:2::0:0:31mgreenish\x1b[0m", &[]).is_empty());
    assert!(!excerpt::excerpt(b"\x1b[31merr\x1b[0m", &[]).is_empty());
    assert!(!excerpt::excerpt(b"\x1b[38;2;0;0;31;31merr\x1b[0m", &[]).is_empty());

    for pattern in ["^", ""] {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/index.md"), "# Board\n\n## Features\n").unwrap();
        fs::write(
            dir.path().join("docs/project.toml"),
            format!("[docs]\nframework = 3\ngate_failure = [{pattern:?}]\n"),
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_factory"))
            .args(["docs", "check"])
            .current_dir(dir.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            !out.status.success(),
            "pattern {pattern:?} should fail docs check"
        );
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(combined.contains("matches the empty string"), "{combined}");
    }
}
