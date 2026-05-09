use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// A simple calendar date in the proleptic Gregorian calendar.
///
/// Constructed either from year/month/day directly (in tests) or from the
/// system clock via [`Date::today_utc`]. We use plain `i32`/`u32` and don't
/// validate ranges — the only "real" producer is [`Date::today_utc`], which
/// derives values from `SystemTime`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Date {
    /// Today's date in **UTC**.
    ///
    /// Note: this returns the UTC calendar date, not the local one. For a
    /// changelog header — which is approximate by nature and only ever read
    /// by humans — UTC is fine, and avoids any libc/FFI/process-spawn cost.
    /// It can be off by up to ~12 hours from local time near midnight.
    pub fn today_utc() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            // If the system clock is somehow before the epoch, fall back to
            // 1970-01-01 rather than panicking. This branch is essentially
            // unreachable in practice.
            .unwrap_or(0);
        let days = secs.div_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Self { year, month, day }
    }
}

/// Howard Hinnant's `civil_from_days` algorithm.
///
/// Given a count of days since 1970-01-01 (negative for dates before),
/// returns `(year, month, day)` in the proleptic Gregorian calendar.
///
/// Reference: https://howardhinnant.github.io/date_algorithms.html#civil_from_days
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    // Shift epoch from 1970-03-01 (start of the algorithm's "era" math).
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    (year, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_epoch() {
        // 1970-01-01 is day 0.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_leap_day() {
        // 2024-02-29: 2024 is a leap year. Days from 1970-01-01:
        // 54 years * 365 + 14 leap days (1972,76,80,84,88,92,96,2000,04,08,
        // 12,16,20,24=before mar 1 only counts up to 28th) +
        // january (31) + 28 = compute by trusting the algorithm and
        // verifying the round trip via days_from_civil instead.
        let d = days_from_civil(2024, 2, 29);
        assert_eq!(civil_from_days(d), (2024, 2, 29));
    }

    #[test]
    fn civil_from_days_year_boundary() {
        let last = days_from_civil(2025, 12, 31);
        assert_eq!(civil_from_days(last), (2025, 12, 31));
        assert_eq!(civil_from_days(last + 1), (2026, 1, 1));
    }

    #[test]
    fn display_zero_pads() {
        let d = Date { year: 2026, month: 5, day: 9 };
        assert_eq!(d.to_string(), "2026-05-09");
        let d = Date { year: 1, month: 1, day: 1 };
        assert_eq!(d.to_string(), "0001-01-01");
    }

    /// Inverse of `civil_from_days`, used only for testing. Same source.
    fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
        let y = y as i64 - if m <= 2 { 1 } else { 0 };
        let era = y.div_euclid(400);
        let yoe = (y - era * 400) as u32; // [0, 399]
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
        era * 146_097 + doe as i64 - 719_468
    }
}
