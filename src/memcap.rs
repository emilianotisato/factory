//! Optional `systemd-run --user --scope` memory limits for gate and lane setup commands.
use std::path::Path;
use std::process::Command;

pub fn normalize_limit(raw: &str) -> std::result::Result<Option<String>, String> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    if let Some(pct) = raw.strip_suffix('%') {
        let n: u32 = pct
            .trim()
            .parse()
            .map_err(|_| format!("invalid percentage {raw:?}"))?;
        if !(1..=100).contains(&n) {
            return Err(format!("percentage must be 1..=100, got {n}"));
        }
        return Ok(Some(format!("{n}%")));
    }
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return Err(format!("unrecognized memory limit {raw:?}"));
    }
    let n: u64 = raw[..i]
        .parse()
        .map_err(|_| format!("invalid number in {raw:?}"))?;
    if n < 1 {
        return Err(format!("size must be at least 1, got {n}"));
    }
    let rest = &raw[i..];
    let (letter, consumed) = if rest.len() >= 3 {
        let tail = &rest[1..];
        if rest.starts_with(['k', 'K']) && (tail.starts_with("iB") || tail.starts_with('B')) {
            ('K', if tail.starts_with("iB") { 3 } else { 2 })
        } else if rest.starts_with(['m', 'M']) && (tail.starts_with("iB") || tail.starts_with('B'))
        {
            ('M', if tail.starts_with("iB") { 3 } else { 2 })
        } else if rest.starts_with(['g', 'G']) && (tail.starts_with("iB") || tail.starts_with('B'))
        {
            ('G', if tail.starts_with("iB") { 3 } else { 2 })
        } else if rest.starts_with(['t', 'T']) && (tail.starts_with("iB") || tail.starts_with('B'))
        {
            ('T', if tail.starts_with("iB") { 3 } else { 2 })
        } else if !rest.is_empty() {
            let c = rest.as_bytes()[0];
            if matches!(c, b'k' | b'K' | b'm' | b'M' | b'g' | b'G' | b't' | b'T') {
                (c.to_ascii_uppercase() as char, 1)
            } else {
                return Err(format!("unrecognized unit in {raw:?}"));
            }
        } else {
            return Err(format!("missing unit in {raw:?}"));
        }
    } else if rest.len() == 2 {
        let b = rest.as_bytes();
        if b[1] == b'B' && matches!(b[0], b'k' | b'K' | b'm' | b'M' | b'g' | b'G' | b't' | b'T') {
            (b[0].to_ascii_uppercase() as char, 2)
        } else {
            return Err(format!("unrecognized unit in {raw:?}"));
        }
    } else if rest.len() == 1 {
        let c = rest.as_bytes()[0];
        if matches!(c, b'k' | b'K' | b'm' | b'M' | b'g' | b'G' | b't' | b'T') {
            (c.to_ascii_uppercase() as char, 1)
        } else {
            return Err(format!("unrecognized unit in {raw:?}"));
        }
    } else {
        return Err(format!("missing unit in {raw:?}"));
    };
    if rest.len() != consumed {
        return Err(format!("trailing junk in {raw:?}"));
    }
    Ok(Some(format!("{n}{letter}")))
}

pub fn parse_memtotal_kb(meminfo: &str) -> Option<u64> {
    for line in meminfo.lines() {
        let Some(rest) = line.strip_prefix("MemTotal:") else {
            continue;
        };
        let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
        return Some(kb);
    }
    None
}

pub fn default_memory_max() -> String {
    let text = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let Some(kb) = parse_memtotal_kb(&text) else {
        return "8G".into();
    };
    let gib = (kb / 1024 / 2).max(2);
    format!("{gib}G")
}

pub fn resolve_limit(configured: Option<&str>) -> Option<String> {
    if std::env::var("FACTORY_GATE_MEMCAP")
        .ok()
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("off"))
    {
        return None;
    }
    match configured {
        Some(raw) => match normalize_limit(raw) {
            Ok(Some(v)) => Some(v),
            Ok(None) => None,
            Err(_) => Some(default_memory_max()),
        },
        None => Some(default_memory_max()),
    }
}

fn systemd_bin() -> String {
    std::env::var("FACTORY_SYSTEMD_RUN")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "systemd-run".into())
}

#[cfg(target_os = "linux")]
fn memory_controller_delegated() -> bool {
    let Ok(text) = std::fs::read_to_string("/proc/self/cgroup") else {
        return false;
    };
    let path = text
        .lines()
        .find_map(|l| l.strip_prefix("0::"))
        .unwrap_or("");
    let mut cgroup = Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    loop {
        let ctrl = cgroup.join("cgroup.controllers");
        if ctrl.is_file()
            && let Ok(s) = std::fs::read_to_string(&ctrl)
            && s.split_whitespace().any(|c| c == "memory")
        {
            return true;
        }
        if !cgroup.pop() {
            break;
        }
    }
    false
}

#[cfg(not(target_os = "linux"))]
fn memory_controller_delegated() -> bool {
    false
}

pub fn systemd_run_available() -> bool {
    let bin = systemd_bin();
    if std::env::var("FACTORY_SYSTEMD_RUN")
        .ok()
        .filter(|s| !s.is_empty())
        .is_none()
        && cfg!(target_os = "linux")
        && !memory_controller_delegated()
    {
        return false;
    }
    Command::new(&bin)
        .args([
            "--user",
            "--scope",
            "--quiet",
            "-p",
            "MemoryMax=1G",
            "--",
            "true",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn shell_command(
    cmd: &str,
    limit: Option<&str>,
    unit: Option<&str>,
) -> (Command, Option<String>) {
    if let Some(cap) = limit.filter(|_| systemd_run_available()) {
        let bin = systemd_bin();
        let scope = unit.map(str::to_string);
        let mut c = Command::new(&bin);
        c.arg("--user").arg("--scope").arg("--quiet");
        if let Some(u) = unit {
            c.arg(format!("--unit={u}"));
        }
        c.arg("-p")
            .arg(format!("MemoryMax={cap}"))
            .arg("-p")
            .arg("MemorySwapMax=0")
            .arg("-p")
            .arg("OOMPolicy=kill")
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg(cmd);
        (c, scope)
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd);
        (c, None)
    }
}

pub fn detect_oom(status: &std::process::ExitStatus, unit: Option<&str>, log_tail: &str) -> bool {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(unix)]
    if let Some(code) = status.code()
        && code == 137
    {
        return true;
    }
    #[cfg(unix)]
    if status.signal() == Some(9) {
        return true;
    }
    if log_tail.contains("oom-kill") || log_tail.contains("MemoryMax") {
        return true;
    }
    if std::env::var("FACTORY_SYSTEMD_RUN")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return false;
    }
    let Some(u) = unit else {
        return false;
    };
    let out = Command::new("systemctl")
        .args(["--user", "show", "-p", "Result", "--value", u])
        .output()
        .ok();
    out.is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "oom-kill")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_memtotal_reads_kb() {
        let info = "MemTotal:       16384000 kB\n";
        assert_eq!(parse_memtotal_kb(info), Some(16_384_000));
    }

    #[test]
    fn normalize_limit_accepts_common_forms() {
        assert_eq!(normalize_limit("off").unwrap(), None);
        assert_eq!(normalize_limit("OFF").unwrap(), None);
        assert_eq!(normalize_limit("50%").unwrap(), Some("50%".into()));
        assert_eq!(normalize_limit("12g").unwrap(), Some("12G".into()));
        assert_eq!(normalize_limit("12GiB").unwrap(), Some("12G".into()));
        assert_eq!(normalize_limit("512M").unwrap(), Some("512M".into()));
        assert_eq!(normalize_limit("12GB").unwrap(), Some("12G".into()));
    }

    #[test]
    fn normalize_limit_rejects_garbage() {
        assert!(normalize_limit("").is_err());
        assert!(normalize_limit("101%").is_err());
        assert!(normalize_limit("12x").is_err());
    }

    #[test]
    fn default_memory_max_floors_at_two_gib() {
        let info = "MemTotal:       2097152 kB\n";
        let gib = parse_memtotal_kb(info).unwrap() / 1024 / 2;
        assert!(gib >= 2);
    }

    fn with_gate_memcap_off(f: impl FnOnce()) {
        // SAFETY: serialized by the test harness lock; restored after the closure.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap();
        let saved = std::env::var("FACTORY_GATE_MEMCAP").ok();
        unsafe {
            std::env::set_var("FACTORY_GATE_MEMCAP", "off");
        }
        f();
        match saved {
            Some(v) => unsafe { std::env::set_var("FACTORY_GATE_MEMCAP", v) },
            None => unsafe { std::env::remove_var("FACTORY_GATE_MEMCAP") },
        }
    }

    #[test]
    fn resolve_limit_honors_env_off() {
        with_gate_memcap_off(|| {
            assert_eq!(resolve_limit(None), None);
            assert_eq!(resolve_limit(Some("12G")), None);
        });
    }

    #[test]
    fn resolve_limit_defaults_without_config() {
        with_gate_memcap_off(|| ());
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap();
        let saved = std::env::var("FACTORY_GATE_MEMCAP").ok();
        unsafe {
            std::env::remove_var("FACTORY_GATE_MEMCAP");
        }
        assert!(resolve_limit(None).is_some());
        match saved {
            Some(v) => unsafe { std::env::set_var("FACTORY_GATE_MEMCAP", v) },
            None => unsafe { std::env::remove_var("FACTORY_GATE_MEMCAP") },
        }
    }
}
