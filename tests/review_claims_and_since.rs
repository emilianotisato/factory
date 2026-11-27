//! Structured claims index in collect output and unanchored `# Collected`.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn branch_two_pass(name: &str) -> Fixture {
    let fx = Fixture::new(name);
    fx.git(&["checkout", "-q", "-b", "feat/review"]);
    for (f, n) in [("a", 400_usize), ("b", 300)] {
        let body: String = (1..=n).map(|i| format!("{f} line {i}\n")).collect();
        fs::write(fx.path().join(f), body).unwrap();
        fx.git(&["add", f]);
    }
    fx.git(&["commit", "-q", "-m", "feat: the branch"]);
    let cfg = fx.env()["FACTORY_CONFIG_DIR"].clone();
    fs::write(
        format!("{cfg}/config.toml"),
        "[review]\ntimeout_secs = 20\n",
    )
    .unwrap();
    fx
}

fn write_claims_stub(bin: &Path) {
    fs::create_dir_all(bin).unwrap();
    let script = r#"#!/usr/bin/env node
'use strict'
const argv = process.argv.slice(2)
if (argv.includes('--list-models')) {
  console.log('Available models\n\ngrok-4.7-high - Grok 4.7 High')
  process.exit(0)
}
const m = /review-(\d+)\.diff/.exec(argv[argv.length - 1] ?? '')
const pass = m ? Number(m[1]) : 1
const prose = process.env.CLAIMS_STUB_MODE === 'prose'
let text
if (prose) {
  text = 'Plain review with no summary table.\n'
} else if (pass === 1) {
  text = [
    'Pass one prose.',
    '',
    '### Summary',
    '',
    '| file:line | severity | claim |',
    '|---|---|---|',
    '| src/a.rs:1 | low | low claim pass1 |',
    '| src/b.rs:2 | medium | medium claim pass1 |',
    '',
  ].join('\n')
} else {
  text = [
    'Pass two prose.',
    '',
    '### Summary',
    '',
    '| file:line | severity | claim |',
    '|---|---|---|',
    '| src/c.rs:3 | high | high claim pass2 |',
    '| src/d.rs:4 | low | low claim pass2 |',
    '',
  ].join('\n')
}
const assistant = {
  type: 'assistant',
  message: { role: 'assistant', content: [{ type: 'text', text }] },
}
const result = { type: 'result', result: text }
process.stdout.write(`${JSON.stringify(assistant)}\n${JSON.stringify(result)}\n`)
"#;
    fs::write(bin.join("cursor-agent"), script).unwrap();
    fs::set_permissions(bin.join("cursor-agent"), fs::Permissions::from_mode(0o755)).unwrap();
}

fn collect_with_stub(fx: &Fixture, mode: Option<&str>) -> String {
    let bin = fx.stub_dir().parent().unwrap().join("bin");
    write_claims_stub(&bin);
    let path = format!("{}:{}", bin.display(), fx.env()["PATH"]);
    let mut cmd = fx.command(BIN);
    cmd.env("PATH", &path);
    if let Some(m) = mode {
        cmd.env("CLAIMS_STUB_MODE", m);
    }
    let o = cmd
        .args(["review", "collect", "--base", "main", "--out", "collect.md"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    fx.read("collect.md")
}

fn second_opinion(file: &str) -> &str {
    let start = file
        .find("\n## Second opinion\n")
        .expect("second opinion section");
    &file[start + 1..]
}

#[test]
fn collect_merges_structured_claims_index_sorted_by_severity() {
    let fx = branch_two_pass("claims-merge");
    let file = collect_with_stub(&fx, None);
    let second = second_opinion(&file);
    let idx = second.find("### Claims index").expect("claims index");
    let pass1 = second.find("### Pass 1").expect("pass 1");
    assert!(idx < pass1, "claims index must precede pass 1:\n{second}");

    let table = &second[idx..pass1];
    let rows: Vec<&str> = table.lines().filter(|l| l.starts_with("| high")).collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("| 2 |"));
    assert!(rows[0].contains("high claim pass2"));

    let medium = table
        .lines()
        .find(|l| l.starts_with("| medium"))
        .expect("medium row");
    assert!(medium.contains("medium claim pass1"));
    assert!(medium.contains("| 1 |"));

    let lows: Vec<&str> = table.lines().filter(|l| l.starts_with("| low")).collect();
    assert_eq!(lows.len(), 2);

    let packed = factory::pack::collected(&file, Path::new("collect.md"));
    assert!(packed.contains("- Pass 1"));
    assert!(packed.contains("- Pass 2"));
    assert!(!packed.contains("### Claims index"));
    assert!(!packed.contains("high claim pass2"));
    assert!(!packed.contains("medium claim pass1"));
    assert!(!packed.contains("low claim pass1"));
    assert!(!packed.contains("low claim pass2"));

    let fx_prose = branch_two_pass("claims-prose");
    let prose_file = collect_with_stub(&fx_prose, Some("prose"));
    let prose = second_opinion(&prose_file);
    assert!(!prose.contains("### Claims index"));
    assert!(prose.contains("### Pass 1"));
    assert!(prose.contains("Plain review with no summary table."));
}
