//! Date-range helpers shared by [`super::reports`].
//!
//! Pure functions — no DB, no LLM, no IO — so they're trivial to unit
//! test and can be reused if another module ever needs to parse
//! `YYYY-MM` / `YYYY-Www` strings.

use anyhow::{Result, anyhow};
use chrono::NaiveDate;

/// Parse an ISO-8601 week string of the form `YYYY-WNN` into `(year,
/// week)`.
pub(super) fn parse_iso_week(s: &str) -> Result<(i32, u32)> {
    let parts: Vec<&str> = s.split("-W").collect();
    if parts.len() != 2 {
        return Err(anyhow!("invalid week format: {}", s));
    }
    let year: i32 = parts[0].parse()?;
    let week: u32 = parts[1].parse()?;
    Ok((year, week))
}

/// Parse a `YYYY-MM` string into `(year, month)`. Rejects months
/// outside `1..=12`.
pub(super) fn parse_year_month(s: &str) -> Result<(i32, u32)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err(anyhow!("invalid month format: {}", s));
    }
    let year: i32 = parts[0].parse()?;
    let month: u32 = parts[1].parse()?;
    if !(1..=12).contains(&month) {
        return Err(anyhow!("month out of range: {}", month));
    }
    Ok((year, month))
}

/// Compute the ISO-8601 timestamp range for one calendar month:
/// `[YYYY-MM-01T00:00:00+00:00, YYYY-(M+1)-01T00:00:00+00:00)`.
/// Crosses to the next year on December.
pub(super) fn month_range(year: i32, month: u32) -> Option<(String, String)> {
    let start = NaiveDate::from_ymd_opt(year, month, 1)?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
    Some((
        format!("{}T00:00:00+00:00", start),
        format!("{}T00:00:00+00:00", end),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_year_month_valid_inputs() {
        assert_eq!(parse_year_month("2026-06").unwrap(), (2026, 6));
        assert_eq!(parse_year_month("2025-01").unwrap(), (2025, 1));
        assert_eq!(parse_year_month("2025-12").unwrap(), (2025, 12));
    }

    #[test]
    fn parse_year_month_rejects_bad_format() {
        assert!(parse_year_month("2026").is_err());
        assert!(parse_year_month("2026-13").is_err(), "13 月不应通过");
        assert!(parse_year_month("2026-00").is_err(), "0 月不应通过");
        assert!(parse_year_month("abcd-06").is_err());
    }

    #[test]
    fn month_range_normal_month() {
        let (a, b) = month_range(2026, 6).unwrap();
        assert_eq!(a, "2026-06-01T00:00:00+00:00");
        assert_eq!(b, "2026-07-01T00:00:00+00:00");
    }

    #[test]
    fn month_range_december_crosses_year() {
        let (a, b) = month_range(2026, 12).unwrap();
        assert_eq!(a, "2026-12-01T00:00:00+00:00");
        assert_eq!(b, "2027-01-01T00:00:00+00:00");
    }

    #[test]
    fn month_range_february_is_normal_28_or_29() {
        let (a, b) = month_range(2026, 2).unwrap();
        assert_eq!(a, "2026-02-01T00:00:00+00:00");
        assert_eq!(b, "2026-03-01T00:00:00+00:00");
    }

    #[test]
    fn parse_iso_week_valid() {
        assert_eq!(parse_iso_week("2026-W23").unwrap(), (2026, 23));
        assert_eq!(parse_iso_week("2025-W01").unwrap(), (2025, 1));
    }

    #[test]
    fn parse_iso_week_rejects_bad_format() {
        assert!(parse_iso_week("2026").is_err());
        assert!(parse_iso_week("2026W23").is_err());
        assert!(parse_iso_week("abcd-Wxx").is_err());
    }
}
