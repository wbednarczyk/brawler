use super::*;

#[derive(Debug, Clone)]
pub(super) struct MediaMatchCompany {
    pub(super) id: String,
    pub(super) ticker: String,
    pub(super) qualified_ticker: String,
    pub(super) display_name: String,
}

pub(super) fn list_media_match_companies(
    connection: &Connection,
) -> StorageResult<Vec<MediaMatchCompany>> {
    let mut statement = connection.prepare(
        "
        SELECT id, ticker, qualified_ticker, display_name
        FROM companies
        ORDER BY qualified_ticker
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(MediaMatchCompany {
            id: row.get(0)?,
            ticker: row.get(1)?,
            qualified_ticker: row.get(2)?,
            display_name: row.get(3)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn find_companies_for_media_item(
    companies: &[MediaMatchCompany],
    item: &BankierRssItem,
) -> Vec<MediaMatchCompany> {
    let haystack = normalize_media_match_text(&format!("{} {}", item.title, item.summary));
    let tokens = haystack.split_whitespace().collect::<Vec<_>>();

    companies
        .iter()
        .filter(|company| {
            let company_name = normalized_company_name_signal(&company.display_name);
            let ticker = company.ticker.to_uppercase();

            (!company_name.is_empty() && normalized_text_contains_phrase(&tokens, &company_name))
                || (ticker.chars().count() >= 3 && tokens.iter().any(|token| *token == ticker))
        })
        .cloned()
        .collect()
}

pub(super) fn media_duplicate_signature(
    item: &BankierRssItem,
    matched_companies: &[MediaMatchCompany],
) -> Option<String> {
    if matched_companies.is_empty() {
        return None;
    }

    let normalized_title = normalize_media_match_text(&item.title);
    if normalized_title.chars().count() < 12 {
        return None;
    }

    let mut companies = matched_companies
        .iter()
        .map(|company| company.qualified_ticker.as_str())
        .collect::<Vec<_>>();
    companies.sort_unstable();
    companies.dedup();

    Some(format!(
        "media:{}:{}",
        companies.join("+"),
        slug_part(&normalized_title)
    ))
}

// The pure text-normalization transforms below are the SSOT for how every
// adapter normalizes names/media text before matching (ADR 0050; relocated here
// from the retired `entity_resolution` module by ADR 0080 — its other half, the
// write-only story-key path, was removed). Property-tested in `proptests` at the
// end of this file.

/// Fold a single character toward the matching alphabet: Polish diacritics map
/// to their base Latin letter, everything else upper-cases. The canonical
/// character normalization shared by every name/media matcher.
pub(super) fn normalize_media_character(character: char) -> char {
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
pub(super) fn normalize_media_match_text(value: &str) -> String {
    value
        .chars()
        .map(normalize_media_character)
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
pub(super) fn normalized_company_name_signal(value: &str) -> String {
    let mut normalized = normalize_media_match_text(value);
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
pub(super) fn normalized_text_contains_phrase(tokens: &[&str], phrase: &str) -> bool {
    let phrase_tokens = phrase.split_whitespace().collect::<Vec<_>>();
    if phrase_tokens.is_empty() || phrase_tokens.len() > tokens.len() {
        return false;
    }

    tokens
        .windows(phrase_tokens.len())
        .any(|window| window == phrase_tokens.as_slice())
}

pub(super) struct MatchedCompany {
    pub(super) id: String,
    pub(super) qualified_ticker: String,
    pub(super) match_type: &'static str,
}

pub(super) fn find_company_for_gpw_listing(
    connection: &Connection,
    ticker: &str,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    find_company_for_exchange_listing(connection, "GPW", ticker, isin)
}

pub(super) fn find_company_for_exchange_listing(
    connection: &Connection,
    exchange: &str,
    ticker: &str,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    if let Some(company) = find_company_by_ticker(connection, exchange, ticker)? {
        return Ok(Some(company));
    }

    if let Some(mapped_ticker) = registry_ticker_for_exchange_isin(connection, exchange, isin)? {
        if let Some(company) = find_company_by_ticker(connection, exchange, &mapped_ticker)? {
            return Ok(Some(company));
        }
    }

    if let Some(company) = find_company_by_isin(connection, isin)? {
        return Ok(Some(company));
    }

    Ok(None)
}

pub(super) fn find_company_by_ticker(
    connection: &Connection,
    exchange: &str,
    ticker: &str,
) -> StorageResult<Option<MatchedCompany>> {
    let ticker = ticker.trim();
    if ticker.is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT id, qualified_ticker
            FROM companies
            WHERE exchange = ?1 AND ticker = ?2
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            params![exchange.trim().to_uppercase(), ticker.to_uppercase()],
            |row| {
                Ok(MatchedCompany {
                    id: row.get(0)?,
                    qualified_ticker: row.get(1)?,
                    match_type: "ticker",
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn find_company_by_isin(
    connection: &Connection,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    if isin.trim().is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT id, qualified_ticker
            FROM companies
            WHERE isin = ?1
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            [isin.trim()],
            |row| {
                Ok(MatchedCompany {
                    id: row.get(0)?,
                    qualified_ticker: row.get(1)?,
                    match_type: "isin",
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn registry_ticker_for_exchange_isin(
    connection: &Connection,
    exchange: &str,
    isin: &str,
) -> StorageResult<Option<String>> {
    let isin = isin.trim();
    if isin.is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT ticker
            FROM company_registry_entries
            WHERE exchange = ?1
                AND isin = ?2
                AND active = 1
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            params![exchange.trim().to_uppercase(), isin.to_uppercase()],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::transform_invariants::{
        assert_charset, assert_deterministic_str, assert_idempotent_str,
    };
    use proptest::prelude::*;

    /// Arbitrary text including Polish diacritics, punctuation, and whitespace —
    /// the long tail real source titles exercise.
    fn matchable_text() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z0-9 ąćęłńóśżźĄĆĘŁŃÓŚŻŹ.,/&-]{0,40}").unwrap()
    }

    proptest! {
        #[test]
        fn normalize_media_match_text_is_idempotent_and_bounded(input in matchable_text()) {
            assert_idempotent_str(normalize_media_match_text, &input);
            assert_deterministic_str(normalize_media_match_text, &input);
            // Output charset: upper-case Latin, digits, single spaces only.
            assert_charset(
                &normalize_media_match_text(&input),
                |character| character.is_ascii_uppercase() || character.is_numeric() || character == ' ',
                "normalize_media_match_text",
            );
        }

        #[test]
        fn normalized_company_name_signal_is_idempotent(input in matchable_text()) {
            // The signal is a fixed point once stripped/normalized.
            assert_idempotent_str(normalized_company_name_signal, &input);
        }
    }
}
