//! `FACTORY_CLOCK_SCALE` and `FACTORY_CLOCK_ANCHOR` speed tests up; every process reads both, so stub and driver share one clock.
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn wall_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn unix_now() -> u64 {
    let wall = wall_now();
    match scale() {
        Some((anchor, scale)) => {
            anchor.saturating_add(wall.saturating_sub(anchor).saturating_mul(scale))
        }
        None => wall,
    }
}

fn scale() -> Option<(u64, u64)> {
    static SCALE: OnceLock<Option<(u64, u64)>> = OnceLock::new();
    *SCALE.get_or_init(|| {
        let scale: u64 = std::env::var("FACTORY_CLOCK_SCALE")
            .ok()?
            .trim()
            .parse()
            .ok()
            .filter(|s| *s > 1)?;
        let anchor = std::env::var("FACTORY_CLOCK_ANCHOR")
            .ok()
            .and_then(|a| a.trim().parse().ok())
            .unwrap_or_else(wall_now);
        Some((anchor, scale))
    })
}

pub fn now_rfc3339() -> String {
    rfc3339(unix_now())
}

pub fn today() -> String {
    yyyymmdd(unix_now())
}

pub fn rfc3339(secs: u64) -> String {
    let (y, m, d) = civil((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

pub fn yyyymmdd(secs: u64) -> String {
    let (y, m, d) = civil((secs / 86_400) as i64);
    format!("{y:04}{m:02}{d:02}")
}

pub fn from_rfc3339(text: &str) -> Option<u64> {
    let t = text.strip_suffix('Z')?;
    let (date, time) = t.split_once('T')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut c = time.split(':').map(|x| x.parse::<i64>().ok());
    let (h, mi, s) = (c.next()??, c.next()??, c.next()??);
    let days = days_from_civil(y, m, day);
    u64::try_from(days * 86_400 + h * 3600 + mi * 60 + s).ok()
}

/// Hinnant's `days_from_civil`, public domain.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Howard Hinnant's `civil_from_days`, public domain.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(yyyymmdd(1_791_590_400), "20261010");
        assert_eq!(yyyymmdd(1_791_590_399), "20261009");
    }

    #[test]
    fn rfc3339_round_trips() {
        for secs in [0, 951_782_400, 1_000_000_000, 1_700_000_000, 1_791_590_399] {
            assert_eq!(from_rfc3339(&rfc3339(secs)), Some(secs));
        }
        assert_eq!(from_rfc3339("2026-10-09"), None);
        assert_eq!(from_rfc3339("not a time"), None);
    }
}
