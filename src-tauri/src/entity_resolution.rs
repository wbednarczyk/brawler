//! Entity resolution — canonical company identity and cross-source story keys
//! (Architecture v2 / ADR 0050).
//!
//! Brawler ingests many sources into one unified set. Two correctness-critical
//! transforms decide how items collapse together:
//!
//! - **Company identity** — resolving a parsed item's issuer to a single
//!   canonical company. The structured path (ticker → registry ISIN → ISIN)
//!   needs the database and stays in `storage::feed_matching`; the **pure
//!   text-normalization** that media/name matching depends on lives here, as the
//!   single source of truth, so every adapter normalizes identically.
//! - **Story identity** — a canonical, stable [`story_key`] that groups items
//!   about the same underlying event across sources. This is the direct enabler
//!   for story clustering (`v0.46`): two sources reporting the same event for the
//!   same companies on the same day produce the same key regardless of wording
//!   order or source.
//!
//! These are exactly the order-dependent / long-tail transforms ADR 0049 says
//! must be tested by their algebraic properties, not just examples — see the
//! `proptests` and golden snapshot below.

/// Fold a single character toward the matching alphabet: Polish diacritics map to
/// their base Latin letter, everything else upper-cases. The canonical character
/// normalization shared by every name/media matcher.
pub fn normalize_match_char(character: char) -> char {
    match character {
        'ą' | 'Ą' => 'A',
        'ć' | 'Ć' => 'C',
        'ę' | 'Ę' => 'E',
        'ł' | 'Ł' => 'L',
        'ń' | 'Ń' => 'N',
        'ó' | 'Ó' => 'O',
        'ś' | 'Ś' => 'S',
        'ż' | 'Ż' | 'ź' | 'Ź' => 'Z',
        other => other.to_uppercase().next().unwrap_or(other),
    }
}

/// Normalize free text for matching: fold diacritics, upper-case, replace every
/// non-alphanumeric run with a single space, and trim. Idempotent and
/// order-preserving over the input's tokens.
pub fn normalize_match_text(value: &str) -> String {
    value
        .chars()
        .map(normalize_match_char)
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reduce a company display name to a comparable signal: normalized, with common
/// Polish joint-stock suffixes (`SPÓŁKA AKCYJNA` / `S A` / `SA`) stripped. Returns
/// an empty string when the remaining signal is too short to match safely.
pub fn company_name_signal(value: &str) -> String {
    let mut normalized = normalize_match_text(value);
    for suffix in [" SPOLKA AKCYJNA", " S A", " SA"] {
        if let Some(stripped) = normalized.strip_suffix(suffix) {
            normalized = stripped.trim().to_owned();
        }
    }

    if normalized.chars().count() < 4 {
        String::new()
    } else {
        normalized
    }
}

/// Whether the whitespace-tokenized `phrase` appears as a contiguous run within
/// `tokens`.
pub fn text_contains_phrase(tokens: &[&str], phrase: &str) -> bool {
    let phrase_tokens = phrase.split_whitespace().collect::<Vec<_>>();
    if phrase_tokens.is_empty() || phrase_tokens.len() > tokens.len() {
        return false;
    }

    tokens
        .windows(phrase_tokens.len())
        .any(|window| window == phrase_tokens.as_slice())
}

/// Minimum normalized-title length before a story key is meaningful — short
/// titles (e.g. "Komunikat") would over-cluster unrelated items.
const MIN_STORY_TITLE_LEN: usize = 12;

/// Derive a **canonical story key** grouping items about the same event across
/// sources. The key is built from the matched companies (sorted + deduped, so
/// arrival order is irrelevant), the publication day (`YYYY-MM-DD` bucket), and a
/// slug of the normalized title.
///
/// Returns `None` when the title is too short to be a reliable cluster anchor or
/// when no company matched (an unmatched item is not clustered). The result is
/// deterministic and order-independent over `qualified_tickers` — the properties
/// story clustering (`v0.46`) relies on.
pub fn story_key(title: &str, qualified_tickers: &[String], published_at: &str) -> Option<String> {
    let normalized = normalize_match_text(title);
    if normalized.chars().count() < MIN_STORY_TITLE_LEN {
        return None;
    }

    let mut tickers = qualified_tickers
        .iter()
        .map(|ticker| ticker.trim().to_uppercase())
        .filter(|ticker| !ticker.is_empty())
        .collect::<Vec<_>>();
    tickers.sort_unstable();
    tickers.dedup();
    if tickers.is_empty() {
        return None;
    }

    // Day bucket: the first 10 chars of an ISO-8601 timestamp are `YYYY-MM-DD`.
    let day = published_at.get(0..10).unwrap_or(published_at);
    let slug = normalized
        .chars()
        .map(|character| if character == ' ' { '-' } else { character })
        .collect::<String>()
        .to_lowercase();

    Some(format!("story:{}:{}:{}", tickers.join("+"), day, slug))
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::transform_invariants::{
        assert_charset, assert_deterministic_str, assert_idempotent_str, assert_order_independent,
    };
    use proptest::prelude::*;

    /// Arbitrary text including Polish diacritics, punctuation, and whitespace —
    /// the long tail real source titles exercise.
    fn matchable_text() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z0-9 ąćęłńóśżźĄĆĘŁŃÓŚŻŹ.,/&-]{0,40}").unwrap()
    }

    proptest! {
        #[test]
        fn normalize_match_text_is_idempotent_and_bounded(input in matchable_text()) {
            assert_idempotent_str(normalize_match_text, &input);
            assert_deterministic_str(normalize_match_text, &input);
            // Output charset: upper-case Latin, digits, single spaces only.
            assert_charset(
                &normalize_match_text(&input),
                |character| character.is_ascii_uppercase() || character.is_numeric() || character == ' ',
                "normalize_match_text",
            );
        }

        #[test]
        fn company_name_signal_is_idempotent(input in matchable_text()) {
            // The signal is a fixed point once stripped/normalized.
            assert_idempotent_str(company_name_signal, &input);
        }

        #[test]
        fn story_key_is_order_independent_over_companies(
            title in "[a-zA-Z ]{12,40}",
            mut tickers in proptest::collection::vec("[A-Z]{2,5}:GPW", 1..6),
            day in "20[0-9]{2}-[01][0-9]-[0-3][0-9]",
        ) {
            tickers.sort();
            tickers.dedup();
            prop_assume!(!tickers.is_empty());
            // Same companies in any order produce the same key.
            assert_order_independent(
                |list: Vec<String>| story_key(&title, &list, &day),
                tickers,
            );
        }
    }
}

#[cfg(test)]
mod golden {
    use super::*;

    #[test]
    fn story_key_examples_are_stable() {
        let cases = vec![
            story_key(
                "Spółka ABC podpisała znaczącą umowę z kontrahentem",
                &["ABC:GPW".to_owned()],
                "2026-06-08T10:00:00Z",
            ),
            // Order of companies must not change the key.
            story_key(
                "Wezwanie do zapisywania się na sprzedaż akcji",
                &["XYZ:GPW".to_owned(), "ABC:GPW".to_owned()],
                "2026-06-08",
            ),
            // Too-short title → no clustering anchor.
            story_key("Komunikat", &["ABC:GPW".to_owned()], "2026-06-08"),
            // No company match → not clustered.
            story_key("A reasonably long market headline here", &[], "2026-06-08"),
        ];

        insta::assert_debug_snapshot!(cases);
    }
}
