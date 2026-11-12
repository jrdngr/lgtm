//! Times in issue files: UTC to the minute, written `2026-11-12T23:06Z`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Minutes since the Unix epoch.
pub type Minute = i64;

pub fn now() -> Minute {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    (secs / 60) as Minute
}

/// Days since the epoch to (year, month, day), after Howard Hinnant's `civil_from_days`.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn format(t: Minute) -> String {
    let (y, mo, d) = civil(t.div_euclid(1440));
    let m = t.rem_euclid(1440);
    format!("{y:04}-{mo:02}-{d:02}T{:02}:{:02}Z", m / 60, m % 60)
}

pub fn parse(s: &str) -> Option<Minute> {
    let s = s.strip_suffix('Z')?;
    let (date, clock) = s.split_once('T')?;
    let mut date = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, mo, d) = (date.next()??, date.next()??, date.next()??);
    let (h, mi) = clock.split_once(':')?;
    let (h, mi) = (h.parse::<i64>().ok()?, mi.parse::<i64>().ok()?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 {
        return None;
    }
    Some(days(y, mo, d) * 1440 + h * 60 + mi)
}

/// How long ago, as `now`, `4m`, `3h` or `2d`.
pub fn age(since: Minute, now: Minute) -> String {
    match (now - since).max(0) {
        0 => "now".into(),
        m if m < 60 => format!("{m}m"),
        m if m < 1440 => format!("{}h", m / 60),
        m => format!("{}d", m / 1440),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_round_trip_across_leap_days_and_years() {
        for s in [
            "2026-11-12T23:06Z",
            "2024-02-29T00:00Z",
            "1999-12-31T23:59Z",
        ] {
            assert_eq!(format(parse(s).unwrap()), s);
        }
        assert_eq!(parse("1970-01-01T00:01Z"), Some(1));
    }

    #[test]
    fn bad_stamps_are_refused() {
        for s in [
            "2026-11-12T23:06",
            "2026-13-01T00:00Z",
            "2026-11-12 23:06Z",
            "soon",
        ] {
            assert_eq!(parse(s), None, "{s}");
        }
    }

    #[test]
    fn ages_pick_the_largest_whole_unit() {
        assert_eq!(age(100, 100), "now");
        assert_eq!(age(100, 159), "59m");
        assert_eq!(age(0, 1439), "23h");
        assert_eq!(age(0, 2 * 1440 + 5), "2d");
    }
}
