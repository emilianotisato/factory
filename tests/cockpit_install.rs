#[path = "harness/mod.rs"]
mod harness;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use factory::mods::COCKPIT_FILES;
use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn mod_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mods/cockpit")
}

/// Every file under `dir` by its `/`-joined relative path, minus the types the engine writes into a loaded mod.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(base).unwrap();
            let parts: Vec<_> = rel.iter().map(|p| p.to_string_lossy()).collect();
            let rel = parts.join("/");
            if rel == ".claude-plugin/types" {
                continue;
            }
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn every_file_under_mods_cockpit_is_in_the_bundle_byte_for_byte() {
    let on_disk = files(&mod_dir());
    let bundled: BTreeMap<String, Vec<u8>> = COCKPIT_FILES
        .iter()
        .map(|(p, c)| (p.to_string(), c.as_bytes().to_vec()))
        .collect();
    let missing: Vec<_> = on_disk
        .keys()
        .filter(|k| !bundled.contains_key(*k))
        .collect();
    assert!(
        missing.is_empty(),
        "on disk, not in the bundle: {missing:?}"
    );
    let gone: Vec<_> = bundled
        .keys()
        .filter(|k| !on_disk.contains_key(*k))
        .collect();
    assert!(gone.is_empty(), "in the bundle, not on disk: {gone:?}");
    for (path, bytes) in &bundled {
        assert!(on_disk[path] == *bytes, "{path} differs from its entry");
    }
}

fn install(fx: &Fixture, home: &Path, path: Option<&Path>) -> Output {
    let mut cmd = fx.command(BIN);
    cmd.args(["cockpit", "install"]).env("HOME", home);
    if let Some(p) = path {
        cmd.env("PATH", p);
    }
    cmd.output().unwrap()
}

fn plugin_calls(fx: &Fixture) -> Vec<Vec<String>> {
    fx.calls()
        .iter()
        .map(|c| {
            c["argv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn install_writes_the_marketplace_once_and_rewrites_the_mod() {
    let fx = Fixture::new("cockpit-install");
    let home = tempfile::tempdir().unwrap();
    let mods = fx.data_dir().join("mods");
    let mods_arg = mods.to_str().unwrap();

    let first = install(&fx, home.path(), None);
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert!(
        first.status.success(),
        "first install: {stdout}{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        stdout.ends_with("run /reload-plugins in open sessions\n"),
        "{stdout}"
    );
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(mods.join(".claude-plugin/marketplace.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["name"], "factory");
    assert!(manifest["owner"]["name"].is_string());
    assert_eq!(
        manifest["plugins"],
        serde_json::json!([{ "name": "factory-cockpit", "source": "./cockpit" }])
    );
    let list = argv(&["plugin", "marketplace", "list", "--json"]);
    let plugins = argv(&["plugin", "list", "--json"]);
    assert_eq!(
        plugin_calls(&fx),
        vec![
            list.clone(),
            argv(&["plugin", "marketplace", "add", mods_arg]),
            plugins.clone(),
            argv(&[
                "plugin",
                "install",
                "factory-cockpit@factory",
                "--scope",
                "user"
            ]),
        ]
    );

    fs::write(mods.join("cockpit/stray.txt"), "left over\n").unwrap();
    fs::write(mods.join("cockpit/hooks/register.tsx"), "edited\n").unwrap();
    let second = install(&fx, home.path(), None);
    assert!(
        second.status.success(),
        "second install: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(!mods.join("cockpit/stray.txt").exists());
    assert_eq!(plugin_calls(&fx)[4..], [list, plugins]);
    assert!(
        files(&mods.join("cockpit")) == files(&mod_dir()),
        "the written mod differs from mods/cockpit/"
    );
}

#[test]
fn install_without_claude_writes_the_folder_and_prints_the_commands() {
    let fx = Fixture::new("cockpit-install-no-claude");
    let home = tempfile::tempdir().unwrap();
    let empty = tempfile::tempdir().unwrap();
    let mods = fx.data_dir().join("mods");

    let out = install(&fx, home.path(), Some(empty.path()));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "  claude plugin marketplace add {}\n",
            mods.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("  claude plugin install factory-cockpit@factory --scope user\n"),
        "{stdout}"
    );
    assert!(mods.join(".claude-plugin/marketplace.json").is_file());
    assert!(files(&mods.join("cockpit")) == files(&mod_dir()));
    assert!(fx.calls().is_empty());
}
