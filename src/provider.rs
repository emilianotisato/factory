//! The agent CLIs factory knows: their binaries, models and effort levels.
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::{ErrorKind, Read as _, Seek as _, SeekFrom};
use std::os::unix::ffi::OsStrExt;
#[cfg(any(test, not(unix)))]
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::{Duration, Instant};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An agent CLI that can run a worker or a review.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
    Cursor,
    Agy,
    Opencode,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::Claude,
        Provider::Codex,
        Provider::Cursor,
        Provider::Agy,
        Provider::Opencode,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
            Provider::Cursor => "cursor",
            Provider::Agy => "agy",
            Provider::Opencode => "opencode",
        }
    }

    /// The executable names to look for on `PATH`, in order.
    pub fn binaries(self) -> &'static [&'static str] {
        match self {
            Provider::Claude => &["claude"],
            Provider::Codex => &["codex"],
            Provider::Cursor => &["cursor-agent", "agent"],
            Provider::Agy => &["agy"],
            Provider::Opencode => &["opencode"],
        }
    }

    /// Empty when the CLI takes no separate effort flag.
    pub fn effort_levels(self) -> &'static [&'static str] {
        match self {
            Provider::Claude => &["low", "medium", "high", "max"],
            Provider::Codex => &["minimal", "low", "medium", "high", "xhigh"],
            Provider::Cursor => &[],
            Provider::Agy => &["low", "medium", "high", "xhigh", "max"],
            Provider::Opencode => &["low", "medium", "high", "max"],
        }
    }

    /// Empty when the owner must pick one.
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Claude => "opus",
            Provider::Codex => "gpt-5.4",
            Provider::Cursor => "grok-4.7-high",
            Provider::Agy => "gemini-4-neon-high",
            Provider::Opencode => "",
        }
    }

    pub fn default_effort(self) -> Option<&'static str> {
        match self {
            Provider::Claude | Provider::Codex | Provider::Agy => Some("high"),
            Provider::Cursor | Provider::Opencode => None,
        }
    }

    /// Suggestions for a CLI that cannot list its models quickly.
    pub fn curated_models(self) -> &'static [&'static str] {
        match self {
            Provider::Claude => &["opus", "sonnet", "haiku"],
            Provider::Codex => &["gpt-5.4", "o3", "o4-mini"],
            Provider::Cursor | Provider::Agy | Provider::Opencode => &[],
        }
    }

    pub fn login_hint(self) -> &'static str {
        match self {
            Provider::Claude => "claude login",
            Provider::Codex => "codex login",
            Provider::Cursor => "cursor-agent login",
            Provider::Agy => "agy login",
            Provider::Opencode => "opencode auth login",
        }
    }

    /// Takes a bare name or a path; only the file name counts.
    pub fn infer_from_binary(name: &str) -> Option<Provider> {
        let base = Path::new(name.trim()).file_name()?.to_str()?;
        Provider::ALL
            .into_iter()
            .find(|p| p.binaries().contains(&base))
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Provider::ALL
            .into_iter()
            .find(|p| s.eq_ignore_ascii_case(p.as_str()))
            .ok_or_else(|| {
                format!("unknown provider `{s}`: expected claude, codex, cursor, agy or opencode")
            })
    }
}

/// Who gives the second opinion: a provider, or nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReviewProvider {
    Provider(Provider),
    None,
}

impl ReviewProvider {
    pub const ALL: [ReviewProvider; 6] = [
        ReviewProvider::Provider(Provider::Claude),
        ReviewProvider::Provider(Provider::Codex),
        ReviewProvider::Provider(Provider::Cursor),
        ReviewProvider::Provider(Provider::Agy),
        ReviewProvider::Provider(Provider::Opencode),
        ReviewProvider::None,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ReviewProvider::Provider(p) => p.as_str(),
            ReviewProvider::None => "none",
        }
    }

    pub fn provider(self) -> Option<Provider> {
        match self {
            ReviewProvider::Provider(p) => Some(p),
            ReviewProvider::None => None,
        }
    }
}

impl std::fmt::Display for ReviewProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ReviewProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ReviewProvider::ALL
            .into_iter()
            .find(|p| s.eq_ignore_ascii_case(p.as_str()))
            .ok_or_else(|| {
                format!(
                    "unknown review provider `{s}`: expected claude, codex, cursor, agy, opencode or none"
                )
            })
    }
}

impl Serialize for ReviewProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ReviewProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl clap::ValueEnum for ReviewProvider {
    fn value_variants<'a>() -> &'a [Self] {
        &ReviewProvider::ALL
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(self.as_str()))
    }
}

/// A provider whose binary is on `PATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub provider: Provider,
    pub binary: PathBuf,
}

/// Never runs the binary: a lazy shim downloads its tool when invoked.
pub fn resolve_binary_in(provider: Provider, path_env: &OsStr) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::split_paths(path_env).collect();
    provider.binaries().iter().find_map(|name| {
        dirs.iter()
            .filter(|d| !d.as_os_str().is_empty())
            .map(|d| d.join(name))
            .find(|p| is_executable(p))
    })
}

/// A model id a CLI will read as a value: not empty, no leading `-` that would parse as a flag, no space or control character.
pub fn is_model_name(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('-') && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

pub fn resolve_binary(provider: Provider) -> Option<PathBuf> {
    resolve_binary_in(provider, &std::env::var_os("PATH").unwrap_or_default())
}

/// `metadata` follows symlinks, so a linked binary counts as the file it points at.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
        && CString::new(path.as_os_str().as_bytes())
            .is_ok_and(|c| unsafe { libc::access(c.as_ptr(), libc::X_OK) == 0 })
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

pub fn detect_installed_in(path_env: &OsStr) -> Vec<Detected> {
    let dirs: Vec<PathBuf> = std::env::split_paths(path_env)
        .filter(|d| !d.as_os_str().is_empty())
        .collect();
    let mut hits: Vec<(usize, usize, Detected)> = Vec::new();
    for (provider_rank, provider) in Provider::ALL.iter().enumerate() {
        let found = dirs.iter().enumerate().find_map(|(dir_index, dir)| {
            provider.binaries().iter().find_map(|name| {
                let path = dir.join(name);
                is_executable(&path).then_some((dir_index, path))
            })
        });
        if let Some((dir_index, binary)) = found {
            hits.push((
                dir_index,
                provider_rank,
                Detected {
                    provider: *provider,
                    binary,
                },
            ));
        }
    }
    hits.sort_by_key(|(d, r, _)| (*d, *r));
    hits.into_iter().map(|(_, _, det)| det).collect()
}

pub fn detect_installed() -> Vec<Detected> {
    detect_installed_in(&std::env::var_os("PATH").unwrap_or_default())
}

/// The CLI's own listing when it has one and answers within `timeout`; the curated list otherwise.
fn retry_busy<T>(mut f: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    const MAX: u32 = 5;
    for attempt in 0..MAX {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) if e.kind() == ErrorKind::ExecutableFileBusy && attempt + 1 < MAX => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

fn spawn_retry_busy(cmd: &mut Command) -> std::io::Result<std::process::Child> {
    retry_busy(|| cmd.spawn())
}

pub fn discover_models(provider: Provider, bin: &Path, timeout: Duration) -> Vec<String> {
    let curated = || {
        provider
            .curated_models()
            .iter()
            .map(|m| m.to_string())
            .collect()
    };
    let args: &[&str] = match provider {
        Provider::Cursor => &["--list-models"],
        Provider::Agy | Provider::Opencode => &["models"],
        Provider::Claude | Provider::Codex => return curated(),
    };
    match run_bounded(bin, args, timeout) {
        Some(text) => {
            let models = parse_models(provider, &text);
            if models.is_empty() { curated() } else { models }
        }
        None => curated(),
    }
}

/// `None` when it cannot start, exits non-zero or outlives `timeout`.
pub(crate) fn run_bounded(bin: &Path, args: &[&str], timeout: Duration) -> Option<String> {
    let mut out = unlinked_file()?;
    let err = unlinked_file()?;
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out.try_clone().ok()?))
        .stderr(Stdio::from(err))
        // Claude refuses to start inside another Claude session.
        .env_remove("CLAUDECODE")
        // Its own group: a wrapper script's children die with it.
        .process_group(0);
    let mut child = spawn_retry_busy(&mut cmd).ok()?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                // SAFETY: a plain syscall on the group this child leads.
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if !status.success() {
        return None;
    }
    let mut text = String::new();
    out.seek(SeekFrom::Start(0)).ok()?;
    out.read_to_string(&mut text).ok()?;
    Some(text)
}

/// A temp file with no name left on disk, so nothing outlives the probe.
fn unlinked_file() -> Option<File> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        ".factory-models.{}.{nanos}.{n}",
        std::process::id()
    ));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .ok()?;
    let _ = std::fs::remove_file(&path);
    Some(file)
}

fn is_model_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | ':' | '@'))
}

fn parse_models(provider: Provider, text: &str) -> Vec<String> {
    let first_column = |line: &str| -> Option<String> {
        let id = match provider {
            Provider::Cursor => line.split(" - ").next()?,
            Provider::Agy => line.split('\t').next()?,
            _ => line,
        };
        let id = id.trim();
        (is_model_id(id) && !id.ends_with("...")).then(|| id.to_string())
    };
    let mut models: Vec<String> = Vec::new();
    for id in text.lines().filter_map(first_column) {
        if !models.contains(&id) {
            models.push(id);
        }
    }
    models
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        let tmp = dir.join(format!("{name}.tmp"));
        fs::write(&tmp, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).unwrap();
        fs::rename(&tmp, &p).unwrap();
        p
    }

    #[test]
    fn retry_busy_retries_executable_file_busy() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let n = AtomicU32::new(0);
        let ok = retry_busy(|| {
            let k = n.fetch_add(1, Ordering::Relaxed);
            if k == 0 {
                Err(std::io::Error::new(ErrorKind::ExecutableFileBusy, "busy"))
            } else {
                Ok(42)
            }
        })
        .unwrap();
        assert_eq!(ok, 42);
        assert_eq!(n.load(Ordering::Relaxed), 2);

        let miss: std::io::Result<i32> =
            retry_busy(|| Err(std::io::Error::new(ErrorKind::NotFound, "nope")));
        assert_eq!(miss.unwrap_err().kind(), ErrorKind::NotFound);
    }

    #[test]
    fn provider_registry_roundtrips_infers_and_probes_without_executing_shims() {
        #[derive(Serialize, Deserialize)]
        struct Holder {
            p: Provider,
            r: ReviewProvider,
        }
        for p in Provider::ALL {
            assert_eq!(p.to_string().parse::<Provider>(), Ok(p));
            assert_eq!(p.as_str().to_uppercase().parse::<Provider>(), Ok(p));
            let r = ReviewProvider::Provider(p);
            let text = toml::to_string(&Holder { p, r }).unwrap();
            assert_eq!(text, format!("p = \"{p}\"\nr = \"{p}\"\n"));
            let back: Holder = toml::from_str(&text).unwrap();
            assert_eq!((back.p, back.r), (p, r));
            assert_eq!(
                <Provider as clap::ValueEnum>::from_str(p.as_str(), false),
                Ok(p)
            );
        }
        assert_eq!(ReviewProvider::ALL.len(), 6);
        for r in ReviewProvider::ALL {
            assert_eq!(r.to_string().parse::<ReviewProvider>(), Ok(r));
            let json = serde_json::to_string(&r).unwrap();
            assert_eq!(json, format!("\"{r}\""));
            assert_eq!(serde_json::from_str::<ReviewProvider>(&json).unwrap(), r);
            assert_eq!(
                <ReviewProvider as clap::ValueEnum>::from_str(r.as_str(), false),
                Ok(r)
            );
        }
        assert_eq!("none".parse(), Ok(ReviewProvider::None));
        assert!("gemini".parse::<ReviewProvider>().is_err());
        assert!(serde_json::from_str::<ReviewProvider>("\"gemini\"").is_err());
        assert!("none".parse::<Provider>().is_err());

        for (name, want) in [
            ("claude", Provider::Claude),
            ("codex", Provider::Codex),
            ("cursor-agent", Provider::Cursor),
            ("agent", Provider::Cursor),
            ("/opt/bin/cursor-agent", Provider::Cursor),
            ("agy", Provider::Agy),
            ("/home/me/.local/bin/opencode", Provider::Opencode),
        ] {
            assert_eq!(Provider::infer_from_binary(name), Some(want), "{name}");
        }
        assert_eq!(Provider::infer_from_binary("gemini"), None);
        assert_eq!(Provider::infer_from_binary(""), None);

        let tmp = tempfile::tempdir().unwrap();
        let (shims, real, plain) = (
            tmp.path().join("shims"),
            tmp.path().join("real"),
            tmp.path().join("plain"),
        );
        for d in [&shims, &real, &plain] {
            fs::create_dir(d).unwrap();
        }
        let sentinel = tmp.path().join("executed");
        let canary = format!("touch {}\nexit 0", sentinel.display());
        script(&shims, "agy", &canary);
        script(&shims, "opencode", &canary);
        let target = script(&real, "cursor-agent-1.0", &canary);
        std::os::unix::fs::symlink(&target, shims.join("agent")).unwrap();
        fs::write(plain.join("codex"), "not executable").unwrap();
        fs::create_dir(plain.join("claude")).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("gone"), plain.join("cursor-agent")).unwrap();

        let path_env = std::env::join_paths([&plain, &shims]).unwrap();
        let found = detect_installed_in(&path_env);
        assert_eq!(
            found,
            vec![
                Detected {
                    provider: Provider::Cursor,
                    binary: shims.join("agent"),
                },
                Detected {
                    provider: Provider::Agy,
                    binary: shims.join("agy"),
                },
                Detected {
                    provider: Provider::Opencode,
                    binary: shims.join("opencode"),
                },
            ]
        );
        assert_eq!(resolve_binary_in(Provider::Claude, &path_env), None);
        assert_eq!(resolve_binary_in(Provider::Codex, &path_env), None);
        assert!(!sentinel.exists(), "detection ran a binary");
    }

    #[test]
    fn discover_models_parses_live_outputs_and_kills_hung_probe() {
        let agy = "Fetching available models...\n\
                   gemini-4-neon-high\tGemini 4 Neon (high)\n\
                   gemini-4-neon-low\tGemini 4 Neon (low)\n\
                   \n\
                   claude-sonnet-4-5\tClaude Sonnet 4.5\n";
        let cursor = "Available models\n\n\
                      auto - Auto\n\
                      grok-4.7-high - Grok 4.7 High (current)\n\
                      gpt-5.4 - GPT-5.4\n\n\
                      Tip: use --model <id> to switch.\n";
        let opencode = "anthropic/claude-sonnet-4-5\n\nopenai/gpt-5.4\nopencode/grok-code\n";

        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for (provider, listing, want) in [
            (
                Provider::Agy,
                agy,
                &[
                    "gemini-4-neon-high",
                    "gemini-4-neon-low",
                    "claude-sonnet-4-5",
                ][..],
            ),
            (
                Provider::Cursor,
                cursor,
                &["auto", "grok-4.7-high", "gpt-5.4"][..],
            ),
            (
                Provider::Opencode,
                opencode,
                &[
                    "anthropic/claude-sonnet-4-5",
                    "openai/gpt-5.4",
                    "opencode/grok-code",
                ][..],
            ),
        ] {
            let flag = if provider == Provider::Cursor {
                "--list-models"
            } else {
                "models"
            };
            let listing_file = dir.join(format!("{provider}.txt"));
            fs::write(&listing_file, listing).unwrap();
            let bin = script(
                dir,
                provider.as_str(),
                &format!(
                    "[ \"$1\" = \"{flag}\" ] || exit 3\ncat {}",
                    listing_file.display()
                ),
            );
            let got = discover_models(provider, &bin, Duration::from_secs(10));
            assert_eq!(got, want, "{provider}");
        }

        let failing = script(dir, "failing", "echo 'gpt-x'\nexit 1");
        assert!(discover_models(Provider::Cursor, &failing, Duration::from_secs(10)).is_empty());
        let codex = script(dir, "codex", &format!("touch {}/ran", dir.display()));
        assert_eq!(
            discover_models(Provider::Codex, &codex, Duration::from_secs(10)),
            ["gpt-5.4", "o3", "o4-mini"]
        );
        assert!(!dir.join("ran").exists());

        let pid_file = dir.join("grandchild.pid");
        let hung = script(
            dir,
            "hung",
            &format!("sleep 30 &\necho $! > {}\nsleep 30", pid_file.display()),
        );
        let started = Instant::now();
        let got = discover_models(Provider::Agy, &hung, Duration::from_millis(500));
        let took = started.elapsed();
        assert!(got.is_empty(), "{got:?}");
        assert!(took < Duration::from_secs(2), "{took:?}");
        let grandchild: i32 = fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let gone = |pid: i32| {
            // SAFETY: signal 0 only asks whether the pid exists.
            let absent = unsafe { libc::kill(pid, 0) } != 0;
            let zombie = fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
                s.rsplit(')')
                    .next()
                    .unwrap_or("")
                    .trim_start()
                    .starts_with('Z')
            });
            absent || zombie
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        while !gone(grandchild) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(gone(grandchild), "the probe's background child survived");
    }
}
