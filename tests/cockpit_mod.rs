use std::fs;
use std::path::PathBuf;

const DECLARATION: &str = "const MOD_VERSION = '";

fn mod_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mods/cockpit")
}

/// The version `register.tsx` declares, or `None` unless exactly one line declares it.
fn declared_version(source: &str) -> Option<&str> {
    let mut found = source
        .lines()
        .filter_map(|line| line.strip_prefix(DECLARATION)?.strip_suffix('\''));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

#[test]
fn the_mod_carries_the_crates_version() {
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(mod_dir().join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "mods/cockpit/.claude-plugin/plugin.json version"
    );
    let source = fs::read_to_string(mod_dir().join("hooks/register.tsx")).unwrap();
    assert_eq!(
        declared_version(&source),
        Some(env!("CARGO_PKG_VERSION")),
        "mods/cockpit/hooks/register.tsx must declare {DECLARATION}<version>' on one line, once"
    );
}

#[test]
fn a_missing_or_repeated_version_declaration_reads_as_none() {
    assert_eq!(declared_version("const SCHEMA = 1\n"), None);
    assert_eq!(
        declared_version("export const MOD_VERSION = '1.0.0'\n"),
        None
    );
    assert_eq!(
        declared_version("const MOD_VERSION = '1.0.0'\nconst MOD_VERSION = '1.0.0'\n"),
        None
    );
    assert_eq!(
        declared_version("const SCHEMA = 1\nconst MOD_VERSION = '1.0.0'\n"),
        Some("1.0.0")
    );
}
