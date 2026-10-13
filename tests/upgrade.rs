//! `factory upgrade` (PRD-004 FR-8, slice 06): a binary with no receipt, or a
//! receipt for another install dir, prints the line that rebuilds it; one
//! with a receipt for its own dir asks the release server, here a stub on a
//! local socket. The install path itself is not driven: it would replace the
//! test binary (the owner's qa-guide row).
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use factory::cli::upgrade::INSTALLER;

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const V: &str = env!("CARGO_PKG_VERSION");
const LATEST: &str = "/api/v3/repos/emilianotisato/factory/releases/latest";

/// A config dir and a receipt dir, both empty, under one temp dir.
struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Env {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("config")).unwrap();
        fs::create_dir_all(dir.path().join("receipt")).unwrap();
        Env { dir }
    }

    fn config(&self, toml: &str) {
        fs::write(self.dir.path().join("config/config.toml"), toml).unwrap();
    }

    fn receipt(&self, install_prefix: &Path) {
        let receipt = serde_json::json!({
            "install_prefix": install_prefix,
            "binaries": ["factory"],
            "source": {
                "release_type": "github",
                "owner": "emilianotisato",
                "name": "factory",
                "app_name": "factory",
            },
            "version": V,
            "provider": { "source": "cargo-dist", "version": "0.32.0" },
        });
        fs::write(
            self.dir.path().join("receipt/factory-receipt.json"),
            receipt.to_string(),
        )
        .unwrap();
    }

    fn upgrade(&self, args: &[&str], server: Option<&str>) -> Output {
        let mut cmd = Command::new(BIN);
        cmd.arg("upgrade")
            .args(args)
            .env("FACTORY_CONFIG_DIR", self.dir.path().join("config"))
            .env("AXOUPDATER_CONFIG_PATH", self.dir.path().join("receipt"))
            .env_remove("AXOUPDATER_CONFIG_WORKING_DIR")
            .env_remove("FACTORY_INSTALLER_GHE_BASE_URL")
            .env("NO_PROXY", "127.0.0.1");
        if let Some(url) = server {
            cmd.env("FACTORY_INSTALLER_GHE_BASE_URL", url);
        }
        cmd.output().expect("spawn factory")
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// The canonical dir the test binary runs from: what a receipt for this
/// binary names as its `install_prefix`.
fn bin_dir() -> PathBuf {
    fs::canonicalize(BIN)
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Serves `tag` as the latest release until the test ends; any other path is
/// a 404. Returns the base URL.
fn release_server(tag: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let release = serde_json::json!({
        "tag_name": tag,
        "name": tag,
        "url": format!("{base}/release"),
        "prerelease": false,
        "assets": [{
            "name": "factory-installer.sh",
            "url": format!("{base}/asset"),
            "browser_download_url": format!("{base}/factory-installer.sh"),
        }],
    })
    .to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut req = Vec::new();
            let mut buf = [0u8; 1024];
            while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => req.extend_from_slice(&buf[..n]),
                }
            }
            let line = String::from_utf8_lossy(&req);
            let path = line.split_whitespace().nth(1).unwrap_or("");
            let (status, body) = if path == LATEST {
                ("200 OK", release.as_str())
            } else {
                ("404 Not Found", "{}")
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    base
}

#[test]
fn no_receipt_prints_the_source_line_from_config() {
    let env = Env::new();
    env.config("[source]\npath = \"/src/factory\"\n");
    let out = env.upgrade(&[], None);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out),
        "installed from source — cargo install --path /src/factory\n"
    );
}

#[test]
fn no_receipt_and_no_source_prints_the_installer() {
    let env = Env::new();
    for args in [&[][..], &["--check"][..]] {
        let out = env.upgrade(args, None);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert_eq!(
            stdout(&out),
            format!("installed from source — {INSTALLER}\n")
        );
    }
}

#[test]
fn a_receipt_for_another_prefix_is_from_source() {
    let env = Env::new();
    env.receipt(Path::new("/nonexistent"));
    let out = env.upgrade(&["--check"], Some(&release_server("v999.0.0")));
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out),
        format!("installed from source — {INSTALLER}\n")
    );
}

#[test]
fn check_prints_the_newer_release_and_installs_nothing() {
    let env = Env::new();
    env.receipt(&bin_dir());
    let before = fs::metadata(BIN).unwrap().modified().unwrap();
    let out = env.upgrade(&["--check"], Some(&release_server("v999.0.0")));
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout(&out), format!("{V} → 999.0.0\n"));
    assert_eq!(fs::metadata(BIN).unwrap().modified().unwrap(), before);
}

#[test]
fn check_on_the_latest_release_is_already_current() {
    let env = Env::new();
    env.receipt(&bin_dir());
    let out = env.upgrade(&["--check"], Some(&release_server(&format!("v{V}"))));
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout(&out), "already current\n");
}
