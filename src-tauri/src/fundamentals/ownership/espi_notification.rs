//! Deterministic parser for ESPI **major-holdings** threshold notifications
//! (ADR 0072 §2b, plan v0.56 T4 stream 2).
//!
//! Threshold-crossing filings under art. 69 of the public-offering act are
//! formulaic, but the free-text body is still natural language. This parser is
//! **conservative by construction**: it extracts a resulting stake only when the
//! holder and at least one resulting percentage are unambiguous, and returns
//! [`EspiHoldingParse::Ambiguous`] the moment the text carries conflicting
//! candidates (e.g. both a before- and an after-percentage for the same axis
//! that it cannot disambiguate) or no confidently-extractable holder. A caller
//! MUST NOT write a stake for anything but [`EspiHoldingParse::Clean`] — never
//! guess (ADR 0072: silently-wrong ownership is worse than absent ownership).
//!
//! Pure and total: no network, no panics on arbitrary input (proptest-guarded).

use regex::Regex;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::OnceLock;

/// A cleanly-parsed resulting holding from one notification. Capital % and votes
/// % are carried SEPARATELY (preferred-vote shares are common on the GPW and the
/// gap is itself signal); each is optional because a notification may disclose
/// only one axis.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEspiHolding {
    pub holder_name: String,
    pub capital_pct: Option<Decimal>,
    pub votes_pct: Option<Decimal>,
}

/// The outcome of parsing one notification. Only [`Self::Clean`] may be written
/// to `ownership_stakes`; the other two are recorded as an unparsed marker plus a
/// diagnostic and never produce a stake.
#[derive(Debug, Clone, PartialEq)]
pub enum EspiHoldingParse {
    /// Unambiguous holder + at least one resulting percentage.
    Clean(ParsedEspiHolding),
    /// Recognizable as a major-holdings notification but not safely parseable
    /// (conflicting percentages, or no confidently-extractable holder). Carries a
    /// short machine-stable reason for the diagnostic payload.
    Ambiguous(&'static str),
    /// Nothing resembling a resulting holding was found.
    NotFound,
}

/// Parse a resulting stake out of a major-holdings notification's title + body.
/// See the module docs for the conservative contract.
pub fn parse_major_holding_notification(title: &str, body: &str) -> EspiHoldingParse {
    let combined = format!("{title}\n{body}");
    let normalized = normalize_ws(&combined);
    let lower = normalized.to_lowercase();

    let pct = match extract_percentages(&normalized, &lower) {
        PercentOutcome::Clean { capital, votes } => (capital, votes),
        PercentOutcome::Ambiguous(reason) => return EspiHoldingParse::Ambiguous(reason),
        PercentOutcome::None => return EspiHoldingParse::NotFound,
    };

    let holder = match extract_holder(&normalized) {
        Some(holder) => holder,
        None => return EspiHoldingParse::Ambiguous("holder_unresolved"),
    };

    EspiHoldingParse::Clean(ParsedEspiHolding {
        holder_name: holder,
        capital_pct: pct.0,
        votes_pct: pct.1,
    })
}

// ---- percentage extraction ----

enum PercentOutcome {
    Clean {
        capital: Option<Decimal>,
        votes: Option<Decimal>,
    },
    Ambiguous(&'static str),
    None,
}

/// `z A% do B%` — a threshold change: capture the BEFORE (A) and AFTER (B) values.
fn change_form() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bz\s+(\d+(?:[.,]\d+)?)\s*%\s+do\s+(\d+(?:[.,]\d+)?)\s*%").unwrap()
    })
}

/// `... X% kapitału ...` — a resulting capital percentage.
fn capital_form() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(\d+(?:[.,]\d+)?)\s*%\s+(?:kapitału|udziału w kapitale|w kapitale)")
            .unwrap()
    })
}

/// `... X% ... głosów` / `uprawniających do X% ... głosów` — a resulting votes %.
fn votes_form() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(\d+(?:[.,]\d+)?)\s*%\s+(?:ogólnej liczby głosów|głosów|w ogólnej liczbie głosów|udziału w ogólnej liczbie głosów)").unwrap()
    })
}

fn extract_percentages(text: &str, lower: &str) -> PercentOutcome {
    // The change form is the most explicit: "z A% do B%". It legitimately carries
    // two values (before/after) — take the AFTER. Multiple *distinct* after-values
    // mean two different axes/holdings in one body we cannot safely split.
    let change_afters: Vec<(usize, Decimal)> = change_form()
        .captures_iter(text)
        .filter_map(|caps| {
            let at = caps.get(0)?.start();
            let after = parse_decimal(caps.get(2)?.as_str())?;
            Some((at, after))
        })
        .collect();
    if !change_afters.is_empty() {
        let distinct: Vec<Decimal> = dedup_decimals(change_afters.iter().map(|(_, d)| *d));
        if distinct.len() > 1 {
            return PercentOutcome::Ambiguous("multiple_change_values");
        }
        let after = distinct[0];
        // Assign the axis by the keyword nearest the match; default to votes since
        // "udział w ogólnej liczbie głosów zmienił się z A% do B%" is the canonical
        // art. 69 phrasing.
        let at = change_afters[0].0;
        let (capital, votes) =
            if mentions_near(lower, at, "kapitał") && !mentions_near(lower, at, "głos") {
                (Some(after), None)
            } else {
                (None, Some(after))
            };
        return PercentOutcome::Clean { capital, votes };
    }

    // Full form: "posiada N akcji stanowiących X% kapitału ... uprawniających do
    // Y% ogólnej liczby głosów". Conflicting distinct values on one axis are
    // ambiguous (a before/after pair stated as two holdings).
    let capitals = dedup_decimals(
        capital_form()
            .captures_iter(text)
            .filter_map(|c| c.get(1).and_then(|m| parse_decimal(m.as_str()))),
    );
    let votes = dedup_decimals(
        votes_form()
            .captures_iter(text)
            .filter_map(|c| c.get(1).and_then(|m| parse_decimal(m.as_str()))),
    );

    if capitals.len() > 1 || votes.len() > 1 {
        return PercentOutcome::Ambiguous("multiple_holding_values");
    }
    if capitals.is_empty() && votes.is_empty() {
        return PercentOutcome::None;
    }
    PercentOutcome::Clean {
        capital: capitals.first().copied(),
        votes: votes.first().copied(),
    }
}

/// Whether `needle` appears within a window around `at` in the lowercased text.
fn mentions_near(lower: &str, at: usize, needle: &str) -> bool {
    const WINDOW: usize = 60;
    let start = at.saturating_sub(WINDOW);
    let end = (at + WINDOW).min(lower.len());
    // Clamp to char boundaries to avoid slicing mid-UTF8.
    let start = floor_char_boundary(lower, start);
    let end = ceil_char_boundary(lower, end);
    lower[start..end].contains(needle)
}

// ---- holder extraction ----

/// Precise, high-confidence holder patterns. Each captures the notifying
/// shareholder's name; anything not matched cleanly is treated as unresolved
/// (Ambiguous), never guessed.
fn holder_forms() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        vec![
            Regex::new(r"(?i)zawiadomieni\w*\s+od\s+(.+?)(?:\s+o\s+|\s+w\s+trybie|\s+dotycząc|,|\.|$)")
                .unwrap(),
            Regex::new(r"(?i)otrzyma\w+\s+od\s+(.+?)\s+(?:zawiadomieni|informacj)").unwrap(),
            Regex::new(r"(?i)\bakcjonariusz(?:a|)\s+(.+?)\s+(?:posiada|zawiadam|przekroczy|zwiększy|zmniejszy)")
                .unwrap(),
        ]
    })
}

fn extract_holder(text: &str) -> Option<String> {
    for form in holder_forms().iter() {
        if let Some(caps) = form.captures(text) {
            if let Some(candidate) = caps.get(1) {
                let name = clean_holder(candidate.as_str());
                if is_plausible_holder(&name) {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn clean_holder(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| c == ',' || c == '.' || c == '"' || c == '„' || c == '”')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A plausible holder name: non-empty, not sentence-like, bounded length.
fn is_plausible_holder(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let chars = name.chars().count();
    if !(2..=80).contains(&chars) {
        return false;
    }
    // Sentence fragments (a stray subordinate clause) are not names.
    let lower = name.to_lowercase();
    !lower.contains(" że ") && !lower.contains(" który ") && !lower.contains("informuje")
}

// ---- helpers ----

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_decimal(raw: &str) -> Option<Decimal> {
    Decimal::from_str(&raw.replace(',', ".")).ok()
}

fn dedup_decimals(values: impl Iterator<Item = Decimal>) -> Vec<Decimal> {
    let mut out: Vec<Decimal> = Vec::new();
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn change_form_takes_the_after_value() {
        // "udział w ogólnej liczbie głosów zmienił się z A% do B%" → after = B (votes).
        let title = "Zawiadomienie o zmianie udziału w ogólnej liczbie głosów";
        let body = "Fundusz otrzymał zawiadomienie od Nationale-Nederlanden OFE, \
                    zgodnie z którym udział w ogólnej liczbie głosów zmienił się z 5,15% do 4,77%.";
        match parse_major_holding_notification(title, body) {
            EspiHoldingParse::Clean(holding) => {
                assert_eq!(
                    holding.votes_pct,
                    Some(dec("4.77")),
                    "after value, votes axis"
                );
                assert_eq!(holding.capital_pct, None);
                assert_eq!(holding.holder_name, "Nationale-Nederlanden OFE");
            }
            other => panic!("expected clean parse, got {other:?}"),
        }
    }

    #[test]
    fn full_form_extracts_capital_and_votes() {
        let title = "Znaczne pakiety akcji — zawiadomienie w trybie art. 69";
        let body = "Spółka otrzymała od Marcin Iwiński zawiadomienie, że posiada \
                    12 650 000 akcji stanowiących 12,66% kapitału zakładowego, \
                    uprawniających do 12,66% ogólnej liczby głosów.";
        match parse_major_holding_notification(title, body) {
            EspiHoldingParse::Clean(holding) => {
                assert_eq!(holding.capital_pct, Some(dec("12.66")));
                assert_eq!(holding.votes_pct, Some(dec("12.66")));
                assert_eq!(holding.holder_name, "Marcin Iwiński");
            }
            other => panic!("expected clean parse, got {other:?}"),
        }
    }

    #[test]
    fn preferred_vote_gap_is_preserved() {
        let title = "Zawiadomienie w trybie art. 69";
        let body = "Otrzymano od Skarb Państwa zawiadomienie: posiada akcje stanowiące \
                    30,00% kapitału zakładowego uprawniające do 45,00% ogólnej liczby głosów.";
        match parse_major_holding_notification(title, body) {
            EspiHoldingParse::Clean(holding) => {
                assert_eq!(holding.capital_pct, Some(dec("30")));
                assert_eq!(holding.votes_pct, Some(dec("45")));
                assert_ne!(
                    holding.capital_pct, holding.votes_pct,
                    "the gap is the signal"
                );
            }
            other => panic!("expected clean parse, got {other:?}"),
        }
    }

    #[test]
    fn conflicting_capital_values_are_ambiguous() {
        // A body stating both a before- and an after-holding on the capital axis
        // must NOT be guessed — never silently wrong.
        let title = "Zawiadomienie o zmianie stanu posiadania";
        let body = "Przed transakcją posiadał akcje stanowiące 5,15% kapitału zakładowego. \
                    Po transakcji posiada akcje stanowiące 4,77% kapitału zakładowego.";
        assert_eq!(
            parse_major_holding_notification(title, body),
            EspiHoldingParse::Ambiguous("multiple_holding_values"),
        );
    }

    #[test]
    fn no_holder_is_ambiguous_not_clean() {
        let title = "Znaczne pakiety akcji";
        let body = "Posiadacz posiada akcje stanowiące 7,00% kapitału zakładowego.";
        // No confidently-extractable holder → Ambiguous, never a stake.
        assert_eq!(
            parse_major_holding_notification(title, body),
            EspiHoldingParse::Ambiguous("holder_unresolved"),
        );
    }

    #[test]
    fn unrelated_text_is_not_found() {
        let title = "Rekomendacja Zarządu w sprawie wypłaty dywidendy";
        let body = "Zarząd rekomenduje wypłatę dywidendy w wysokości 1,00 zł na akcję.";
        assert_eq!(
            parse_major_holding_notification(title, body),
            EspiHoldingParse::NotFound,
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn never_panics_on_arbitrary_text(title in ".*", body in ".*") {
            let _ = parse_major_holding_notification(&title, &body);
        }
    }
}
