//! Plain-text helpers shared by [`super::collect`] and [`super::render`].
//!
//! Kept deliberately tiny: each function takes a `&str` (and possibly a
//! caller-supplied truncation budget) and returns a fresh `String`. No
//! DB access, no allocations beyond what the output needs.
//!
//! The split exists so `render.rs` can focus on layout (which currently
//! still hosts the bulk of end-to-end tests) while `collect.rs` can
//! reuse `short_date` for `ProjectContext.last_active` without pulling
//! in the renderer.

use chrono::{DateTime, Utc};

/// Reduce an RFC3339 timestamp to `YYYY-MM-DD` in UTC.
///
/// Falls back to the substring before the first `T` if parsing fails,
/// so a malformed but readable string still shows *something* sensible.
pub(super) fn short_date(rfc3339: &str) -> String {
    DateTime::parse_from_rfc3339(rfc3339)
        .map(|d| d.with_timezone(&Utc).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| rfc3339.split('T').next().unwrap_or(rfc3339).to_string())
}

/// First non-empty line of `s`, truncated to `max` Unicode chars.
pub(super) fn first_line(s: &str, max: usize) -> String {
    let line = s.lines().next().unwrap_or(s).trim();
    truncate(line, max)
}

/// First paragraph of `s` (text up to the first blank line) flattened
/// onto a single line, truncated to `max` Unicode chars.
pub(super) fn first_paragraph(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    let para = trimmed
        .split("\n\n")
        .next()
        .unwrap_or(trimmed)
        .replace('\n', " ")
        .trim()
        .to_string();
    truncate(&para, max)
}

/// Truncate `s` to `max` Unicode chars, appending `…` if any chars
/// were dropped.
pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_date_parses_rfc3339() {
        assert_eq!(short_date("2026-06-04T12:34:56Z"), "2026-06-04");
        assert_eq!(short_date("2026-06-04T08:00:00+08:00"), "2026-06-04");
    }

    #[test]
    fn short_date_falls_back_on_garbage() {
        assert_eq!(short_date("not-a-date"), "not-a-date");
        assert_eq!(short_date("2026-06-04T??"), "2026-06-04");
    }

    #[test]
    fn truncate_respects_unicode_char_boundary() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 3), "hel…");
        assert_eq!(truncate("你好世界", 2), "你好…");
    }

    #[test]
    fn first_line_strips_trailing_lines_and_trims() {
        assert_eq!(first_line("  abc\nrest", 10), "abc");
        assert_eq!(first_line("very long line here", 6), "very l…");
    }

    #[test]
    fn first_paragraph_collapses_newlines_until_blank_line() {
        let s = "one\ntwo\n\nbody";
        assert_eq!(first_paragraph(s, 100), "one two");
    }
}
