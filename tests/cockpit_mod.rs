use std::fs;
use std::path::PathBuf;

fn mod_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mods/cockpit")
}

/// The version `source` declares as `{name} = '…'`, or `None` unless exactly one line declares it.
fn declared_version<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let declaration = format!("const {name} = '");
    let mut found = source
        .lines()
        .filter_map(|line| line.strip_prefix(declaration.as_str())?.strip_suffix('\''));
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
    for (file, name) in [
        ("hooks/register.tsx", "MOD_VERSION"),
        ("tests/cockpit.test.tsx", "VERSION"),
    ] {
        let source = fs::read_to_string(mod_dir().join(file)).unwrap();
        assert_eq!(
            declared_version(&source, name),
            Some(env!("CARGO_PKG_VERSION")),
            "mods/cockpit/{file} must declare const {name} = '<version>' on one line, once"
        );
    }
}

#[test]
fn a_missing_or_repeated_version_declaration_reads_as_none() {
    assert_eq!(declared_version("const SCHEMA = 1\n", "MOD_VERSION"), None);
    assert_eq!(
        declared_version("export const MOD_VERSION = '1.0.0'\n", "MOD_VERSION"),
        None
    );
    assert_eq!(
        declared_version(
            "const MOD_VERSION = '1.0.0'\nconst MOD_VERSION = '1.0.0'\n",
            "MOD_VERSION"
        ),
        None
    );
    assert_eq!(
        declared_version(
            "const SCHEMA = 1\nconst MOD_VERSION = '1.0.0'\n",
            "MOD_VERSION"
        ),
        Some("1.0.0")
    );
}
