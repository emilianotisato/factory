//! The release workflow cannot run before the owner's first tag, so these prove its text.
use std::fs;

const TARGETS: [&str; 4] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
];

fn read(rel: &str) -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/").to_string() + rel)
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// cargo-dist 0.32 writes its config to `dist-workspace.toml`, not `[workspace.metadata.dist]`.
fn dist() -> toml::Table {
    let t: toml::Table = toml::from_str(&read("dist-workspace.toml")).unwrap();
    t["dist"].as_table().expect("[dist]").clone()
}

#[test]
fn dist_builds_four_unix_targets_and_the_shell_installer() {
    let d = dist();
    let mut targets: Vec<&str> = d["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    targets.sort_unstable();
    let mut want = TARGETS.to_vec();
    want.sort_unstable();
    assert_eq!(targets, want);
    assert_eq!(
        d["installers"].as_array().unwrap(),
        &[toml::Value::from("shell")]
    );
    // `factory upgrade` is the updater; dist must not ship a second one.
    assert_eq!(d["install-updater"].as_bool(), Some(false));
}

#[test]
fn the_release_workflow_runs_on_version_tags_for_the_four_targets() {
    let yml = read(".github/workflows/release.yml");
    // The template's "windows longpaths" step is OS-guarded; a Windows build would name a triple.
    for text in [&yml, &read("dist-workspace.toml")] {
        assert!(!text.contains("pc-windows"), "a windows build");
    }
    let on = &yml[yml.find("\non:").expect("on:")..];
    let tags = &on[on.find("tags:").expect("a tag trigger")..];
    let pattern = tags.lines().nth(1).unwrap_or("").trim();
    assert!(
        pattern == "- 'v*'" || pattern == "- '**[0-9]+.[0-9]+.[0-9]+*'",
        "tag pattern {pattern}"
    );
    // The matrix comes from `dist plan` at run time, so the yml names no target.
    assert!(yml.contains("dist build") && yml.contains("dist host"));
    assert!(read("dist-workspace.toml").contains("cargo-dist-version = \"0.32.0\""));
}

#[test]
fn the_readme_offers_the_installer_before_cargo_install() {
    let readme = read("README.md");
    let installer = readme
        .find("releases/latest/download/factory-installer.sh")
        .expect("the installer line");
    let cargo = readme
        .find("cargo install --path .")
        .expect("the cargo install line");
    assert!(installer < cargo);
}
