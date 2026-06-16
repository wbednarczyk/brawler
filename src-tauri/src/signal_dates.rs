//! Deterministic extraction of future dates from Polish ESPI/EBI filing bodies, for deriving
//! calendar events from dividend and general-meeting signals (ADR 0036, milestone v0.41.0).
//!
//! This is the **deterministic-first** half of date extraction: it only returns a date that
//! sits next to a recognized Polish label, so it never guesses. When it returns `None`, the
//! caller may fall back to the opt-in AI extractor; either way the derived event is created
//! `proposed` and requires user confirmation before it reaches the calendar.

/// Polish genitive month names, indexed 1..=12.
const MONTHS_PL: [&str; 12] = [
    "stycznia",
    "lutego",
    "marca",
    "kwietnia",
    "maja",
    "czerwca",
    "lipca",
    "sierpnia",
    "września",
    "października",
    "listopada",
    "grudnia",
];

/// How many characters after a label to scan for a date.
const SCAN_WINDOW: usize = 80;

/// Extract a dividend date from a filing body. Prefers the payment date
/// (`dzień/termin/data wypłaty dywidendy`), falling back to the record date (`dzień dywidendy`).
/// Returns an ISO `YYYY-MM-DD` string, or `None` when no labelled date is found.
pub fn parse_dividend_date(body: &str) -> Option<String> {
    const PAYMENT_LABELS: &[&str] = &[
        "dzień wypłaty dywidendy",
        "termin wypłaty dywidendy",
        "data wypłaty dywidendy",
        "dniem wypłaty dywidendy",
    ];
    const RECORD_LABELS: &[&str] = &[
        "dzień dywidendy",
        "dniem dywidendy",
        "dzień ustalenia prawa",
    ];

    find_date_for_labels(body, PAYMENT_LABELS).or_else(|| find_date_for_labels(body, RECORD_LABELS))
}

/// Extract a general-meeting date from a filing body (`walne zgromadzenie ... w dniu/na dzień ...`).
/// Returns an ISO `YYYY-MM-DD` string, or `None` when no labelled date is found.
pub fn parse_general_meeting_date(body: &str) -> Option<String> {
    const LABELS: &[&str] = &[
        "na dzień",
        "w dniu",
        "odbędzie się w dniu",
        "zwołuje",
        "zwołania",
    ];

    let lower = body.to_lowercase();
    // Only look for a meeting date when the body actually concerns a general meeting.
    if !lower.contains("walne zgromadzenie") && !lower.contains("walnego zgromadzenia") {
        return None;
    }
    find_date_for_labels(body, LABELS)
}

/// Scan for the first date that appears within `SCAN_WINDOW` characters after any of `labels`.
fn find_date_for_labels(body: &str, labels: &[&str]) -> Option<String> {
    let lower = body.to_lowercase();
    let mut best: Option<(usize, String)> = None;

    for label in labels {
        let mut search_from = 0;
        while let Some(found) = lower[search_from..].find(label) {
            let label_start = search_from + found;
            let scan_start = label_start + label.len();
            let scan_end = (scan_start + SCAN_WINDOW).min(lower.len());
            if let Some(date) = parse_first_date(&lower[scan_start..scan_end]) {
                // Keep the earliest-positioned match for determinism.
                if best
                    .as_ref()
                    .map(|(pos, _)| label_start < *pos)
                    .unwrap_or(true)
                {
                    best = Some((label_start, date));
                }
            }
            search_from = scan_start;
        }
    }

    best.map(|(_, date)| date)
}

/// Find the first numeric (`DD.MM.YYYY`, `YYYY-MM-DD`, …) or Polish long-form
/// (`DD <month> YYYY`) date in `text`.
fn parse_first_date(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i].is_ascii_digit() {
            if let Some((date, _next)) = try_parse_date_at(&chars, i) {
                return Some(date);
            } else {
                // Skip the run of digits we failed to parse.
                let mut j = i;
                while j < len && chars[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// Try to parse a date starting at index `start` (which points at a digit). Returns the ISO
/// date and the index just past the match on success.
fn try_parse_date_at(chars: &[char], start: usize) -> Option<(String, usize)> {
    let (first_num, after_first) = take_number(chars, start)?;

    // ISO form: YYYY-MM-DD (first number is a 4-digit year).
    if first_num >= 1000 {
        let (sep1, p1) = take_separator(chars, after_first, &['-', '.', '/'])?;
        let _ = sep1;
        let (month, after_month) = take_number(chars, p1)?;
        let (_, p2) = take_separator(chars, after_month, &['-', '.', '/'])?;
        let (day, after_day) = take_number(chars, p2)?;
        return build_iso(first_num as i64, month, day).map(|d| (d, after_day));
    }

    // Day-first forms. first_num is the day.
    let day = first_num;
    // Skip whitespace before separator/month.
    let p = skip_spaces(chars, after_first);

    // Numeric DD.MM.YYYY / DD-MM-YYYY / DD/MM/YYYY (optionally with a trailing dot after day).
    if let Some((_, p1)) = take_separator(chars, p, &['.', '-', '/']) {
        let p1 = skip_spaces(chars, p1);
        if let Some((month, after_month)) = take_number(chars, p1) {
            let (_, p2) = take_separator(chars, after_month, &['.', '-', '/'])?;
            let p2 = skip_spaces(chars, p2);
            let (year, after_year) = take_number(chars, p2)?;
            return build_iso(year as i64, month, day).map(|d| (d, after_year));
        }
    }

    // Polish long form: DD <month-name> YYYY.
    let p = skip_spaces(chars, after_first);
    let (word, after_word) = take_word(chars, p)?;
    let month = MONTHS_PL.iter().position(|m| *m == word)? + 1;
    let p2 = skip_spaces(chars, after_word);
    let (year, after_year) = take_number(chars, p2)?;
    build_iso(year as i64, month as u32, day).map(|d| (d, after_year))
}

fn take_number(chars: &[char], start: usize) -> Option<(u32, usize)> {
    let mut i = start;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i == start || i - start > 4 {
        return None;
    }
    let value: u32 = chars[start..i].iter().collect::<String>().parse().ok()?;
    Some((value, i))
}

fn take_separator(chars: &[char], start: usize, allowed: &[char]) -> Option<(char, usize)> {
    let c = *chars.get(start)?;
    if allowed.contains(&c) {
        Some((c, start + 1))
    } else {
        None
    }
}

fn take_word(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut i = start;
    while i < chars.len() && chars[i].is_alphabetic() {
        i += 1;
    }
    if i == start {
        return None;
    }
    Some((chars[start..i].iter().collect(), i))
}

fn skip_spaces(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// Validate an `YYYY-MM-DD` string and return it in canonical form, or `None` if it is not a
/// real calendar date. Used to sanitize AI-extracted dates before deriving an event (ADR 0036).
pub fn validate_iso_date(value: &str) -> Option<String> {
    let value = value.trim();
    let mut parts = value.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    build_iso(year, month, day)
}

/// Validate a (year, month, day) triple and format it as `YYYY-MM-DD`.
fn build_iso(year: i64, month: u32, day: u32) -> Option<String> {
    if !(1900..=2100).contains(&year) {
        return None;
    }
    if !(1..=12).contains(&month) {
        return None;
    }
    let max_day = days_in_month(year, month);
    if !(1..=max_day).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dividend_payment_date_numeric() {
        let body = "Zarząd rekomenduje wypłatę dywidendy. Dzień dywidendy: 10.07.2026 r. \
                    Termin wypłaty dywidendy: 24.07.2026 r.";
        assert_eq!(parse_dividend_date(body), Some("2026-07-24".to_owned()));
    }

    #[test]
    fn falls_back_to_record_date_when_no_payment_date() {
        let body = "Ustalono dzień dywidendy na 15.06.2026 r.";
        assert_eq!(parse_dividend_date(body), Some("2026-06-15".to_owned()));
    }

    #[test]
    fn parses_polish_long_form_date() {
        let body = "Dzień wypłaty dywidendy: 5 sierpnia 2026 roku.";
        assert_eq!(parse_dividend_date(body), Some("2026-08-05".to_owned()));
    }

    #[test]
    fn parses_iso_date() {
        let body = "Dzień dywidendy 2026-09-30.";
        assert_eq!(parse_dividend_date(body), Some("2026-09-30".to_owned()));
    }

    #[test]
    fn parses_general_meeting_date() {
        let body = "Zarząd zwołuje Zwyczajne Walne Zgromadzenie na dzień 28 czerwca 2026 r. \
                    o godzinie 11:00.";
        assert_eq!(
            parse_general_meeting_date(body),
            Some("2026-06-28".to_owned())
        );
    }

    #[test]
    fn general_meeting_requires_meeting_context() {
        // No "walne zgromadzenie" → no date, even with a labelled date present.
        let body = "Spółka informuje, że w dniu 28.06.2026 podpisano umowę.";
        assert_eq!(parse_general_meeting_date(body), None);
    }

    #[test]
    fn returns_none_without_labelled_date() {
        let body = "Rekomendacja wypłaty dywidendy w wysokości 1,50 zł na akcję.";
        assert_eq!(parse_dividend_date(body), None);
    }

    #[test]
    fn rejects_invalid_calendar_dates() {
        let body = "Dzień dywidendy: 31.02.2026 r.";
        assert_eq!(parse_dividend_date(body), None);
    }
}
