//! UTC timestamps without a date crate. `events.jsonl` lines carry RFC 3339
//! seconds and run ids carry the day; nothing in the driver needs a calendar
//! beyond that, so the civil-date arithmetic is the twelve-line public-domain
//! algorithm rather than a dependency (AD-201).
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `2026-10-09T23:12:01Z`.
pub fn now_rfc3339() -> String {
    rfc3339(unix_now())
}

/// `20261009`, the day part of a run id.
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

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day).
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
}
