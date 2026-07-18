//! Deterministic parser for the **attachment** side of a MAR art. 19 insider
//! notification — the standard KNF/ESMA notification form table that carries the
//! transaction figures the Bankier cover note omits (ADR 0083 Decision 6 + the
//! 2026-07-17 ground-truth amendment; plan v0.57 **T4b**).
//!
//! The cover-note parser ([`super`]) recovers person / role / direction from the
//! ESPI cover note but leaves volume / price / currency / tx_date NULL for ~90% of
//! transactions — those live in the attached "Powiadomienie…"/"Zawiadomienie…"
//! notification document (a PDF, or for some issuers an ESAP-derived xhtml). This
//! module parses that document's standardized form into per-transaction units so
//! the storage merge ([`crate::storage::insider`]) can fill the NULLs.
//!
//! **Input is already-extracted text** — the job runs the shared ADR 0061 tier
//! ([`crate::report_diff::extraction::extract_report`], PDF *or* xhtml) first, so
//! this parser is pure/total over a `&str` and trivially testable without building
//! real PDF bytes (mirrors [`crate::fundamentals::extraction::pdf::parse_pdf_text`]
//! and [`crate::fundamentals::management_holdings`]).
//!
//! **Conservative contract** (never fabricates): the standard form is anchored on
//! its Polish field labels ("Imię i nazwisko"/"Nazwa", "Stanowisko"/"status",
//! "Rodzaj transakcji", "Cena"/"Wolumen", "Data transakcji"). A unit is emitted
//! only when a person is resolved; every figure the form omits stays `None`. A
//! document with no recognizable form yields [`AttachmentParse::NotFound`] — the
//! caller parks it (a scanned/no-text-layer document never reaches here, the
//! extraction tier flags it first). One notification document is one person with
//! one or more transaction rows; each row becomes a unit (person repeated), so the
//! merge can match a row per (person, direction, tx_date).

use super::{
    detect_direction, detect_instrument, normalize_currency, parse_pl_date, Direction, InsiderRole,
    Instrument,
};

/// One transaction row recovered from a notification document: a person plus one
/// transaction's figures. Figures are `None` unless the form stated them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentTxUnit {
    pub person_raw: String,
    pub person_normalized: String,
    pub role: Option<InsiderRole>,
    pub related_pdmr_raw: Option<String>,
    pub related_pdmr_normalized: Option<String>,
    pub direction: Option<Direction>,
    pub instrument: Option<Instrument>,
    pub volume: Option<String>,
    pub price: Option<String>,
    pub currency: Option<String>,
    pub tx_date: Option<String>,
}

/// The result of parsing one notification document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentParse {
    /// At least one transaction row was recovered (each has a resolved person).
    Units(Vec<AttachmentTxUnit>),
    /// No recognizable MAR art. 19 notification form in the text. Never guessed.
    NotFound,
}

/// Parse a notification document's extracted text into transaction units.
/// See the module docs for the conservative contract.
pub fn parse_notification_text(text: &str) -> AttachmentParse {
    if text.trim().is_empty() {
        return AttachmentParse::NotFound;
    }
    let lower = text.to_lowercase();

    // Gate: the text must read as the standard notification form. The form always
    // carries the "Szczegóły transakcji" heading and the transaction-nature label.
    let is_form = (lower.contains("szczegóły transakcji")
        || lower.contains("szczegoly transakcji"))
        || (lower.contains("rodzaj transakcji")
            && (lower.contains("imię i nazwisko")
                || lower.contains("imie i nazwisko")
                || lower.contains("nazwa")));
    if !is_form {
        return AttachmentParse::NotFound;
    }

    let Some(person_raw) = extract_person(text) else {
        return AttachmentParse::NotFound;
    };
    // Names in the standard form are already NOMINATIVE ("Imię i nazwisko: Jan
    // Testowy") — unlike the cover note's genitive. Applying the cover-note
    // genitive-recovery here would corrupt them ("Testowy" → "Testowa"), so the
    // stored key is a plain nominative uppercase; the merge bridges the two sources
    // with lenient (declension-tolerant) matching.
    let person_normalized = normalize_nominative(&person_raw);
    let role = extract_role(&lower, &person_raw);
    let (related_pdmr_raw, related_pdmr_normalized) =
        if role == Some(InsiderRole::CloselyAssociated) {
            let pdmr = extract_related_pdmr(text);
            let norm = pdmr.as_deref().map(normalize_nominative);
            (pdmr, norm)
        } else {
            (None, None)
        };

    // Direction is per-transaction (each block states Nabycie/Zbycie). Instrument is
    // stated once in the "Opis instrumentu finansowego" header field, so it is
    // resolved at the form level (masking "transakcj*" so that word's embedded
    // "akcj" never false-matches shares).
    let form_direction = detect_direction(&mask_transakcja(&lower));
    let form_instrument = detect_form_instrument(text, &lower);

    let blocks = split_transaction_blocks(text);
    let mut units = Vec::new();
    for block in &blocks {
        let block_lower = mask_transakcja(&block.to_lowercase());
        let figures = extract_figures(block);
        units.push(AttachmentTxUnit {
            person_raw: person_raw.clone(),
            person_normalized: person_normalized.clone(),
            role,
            related_pdmr_raw: related_pdmr_raw.clone(),
            related_pdmr_normalized: related_pdmr_normalized.clone(),
            direction: detect_direction(&block_lower).or(form_direction),
            instrument: form_instrument,
            volume: figures.volume,
            price: figures.price,
            currency: figures.currency,
            tx_date: figures.tx_date,
        });
    }

    if units.is_empty() {
        // A form with a person but no parsable transaction block still yields one
        // unit (person + role + whole-form direction/instrument) — its figures fill
        // nothing, but its role/direction can.
        units.push(AttachmentTxUnit {
            person_raw,
            person_normalized,
            role,
            related_pdmr_raw,
            related_pdmr_normalized,
            direction: form_direction,
            instrument: form_instrument,
            volume: None,
            price: None,
            currency: None,
            tx_date: None,
        });
    }

    AttachmentParse::Units(units)
}

// ---------------------------------------------------------------------------
// Person / role
// ---------------------------------------------------------------------------

/// The notifying party: the "Imię i nazwisko / Nazwa" field value in section 1.
fn extract_person(text: &str) -> Option<String> {
    // Prefer the explicit personal-name label; fall back to the entity "Nazwa".
    for label in ["imię i nazwisko", "imie i nazwisko", "nazwa"] {
        if let Some(value) = label_value(text, label) {
            let cleaned = clean_field_value(&value);
            if is_plausible(&cleaned) {
                return Some(cleaned);
            }
        }
    }
    None
}

/// The role/status keyword map over section 2 ("Stanowisko / status"), anchored so
/// a later clause does not bleed in. Nullable — a bare PDMR with no organ stays
/// `None` (never guessed).
fn extract_role(lower: &str, person_raw: &str) -> Option<InsiderRole> {
    // A named entity is a closely-associated vehicle by construction.
    if is_entity(person_raw) {
        return Some(InsiderRole::CloselyAssociated);
    }
    // The organ appears in section 2 ("Powód powiadomienia" → "Stanowisko /
    // status: <organ>"). Scan a bounded window after that anchor (the combined
    // "Stanowisko / status" label defeats a strict label-value cut).
    let anchor = [
        "stanowisko",
        "status",
        "powód powiadomienia",
        "powod powiadomienia",
    ]
    .iter()
    .filter_map(|needle| lower.find(needle))
    .min();
    let window = match anchor {
        Some(at) => {
            let start = super::ceil_char_boundary(lower, at);
            let end = super::ceil_char_boundary(lower, (at + 90).min(lower.len()));
            &lower[start..end]
        }
        None => lower,
    };
    role_from_keywords(window)
}

fn role_from_keywords(lower: &str) -> Option<InsiderRole> {
    if lower.contains("rada nadzorcz")
        || lower.contains("rady nadzorczej")
        || lower.contains("radzie nadzorczej")
        || (lower.contains("przewodnicząc") && lower.contains("nadzorcz"))
    {
        return Some(InsiderRole::Supervisory);
    }
    if lower.contains("blisko związan") || lower.contains("fundacja rodzinna") {
        return Some(InsiderRole::CloselyAssociated);
    }
    if lower.contains("prezes")
        || lower.contains("wiceprezes")
        || lower.contains("członek zarządu")
        || lower.contains("członka zarządu")
        || lower.contains("członkiem zarządu")
        || lower.contains("zarząd")
    {
        return Some(InsiderRole::Management);
    }
    None
}

/// The anchoring PDMR for a closely-associated vehicle: the "osoby blisko związanej
/// z <PDMR>" reference the form carries in section 1.
fn extract_related_pdmr(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let anchor = ["związanej z ", "związanej z osobą ", "związany z "]
        .iter()
        .filter_map(|needle| lower.find(needle).map(|at| at + needle.len()))
        .min()?;
    let after = &text[super::floor_char_boundary(text, anchor)..];
    let window = &after[..super::ceil_char_boundary(after, after.len().min(60))];
    let cleaned = clean_field_value(window);
    // Keep the leading "<Given> <Surname>" run.
    let name: String = cleaned
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    is_plausible(&name).then_some(name)
}

// ---------------------------------------------------------------------------
// Transaction blocks + figures
// ---------------------------------------------------------------------------

/// Split the form into per-transaction blocks on the repeated "Rodzaj transakcji"
/// label (each "Szczegóły transakcji" row starts one). A single-transaction form
/// yields one block. Everything before the first anchor is dropped (person/role
/// header). Returns an empty vec when no anchor is present.
fn split_transaction_blocks(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let anchor = "rodzaj transakcji";
    let starts: Vec<usize> = lower.match_indices(anchor).map(|(i, _)| i).collect();
    if starts.is_empty() {
        return Vec::new();
    }
    let mut blocks = Vec::new();
    for (idx, &start) in starts.iter().enumerate() {
        let end = starts.get(idx + 1).copied().unwrap_or(text.len());
        blocks.push(text[start..end].to_string());
    }
    blocks
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Figures {
    volume: Option<String>,
    price: Option<String>,
    currency: Option<String>,
    tx_date: Option<String>,
}

/// Extract volume / price / currency / tx_date from one transaction block by its
/// form labels. Conservative: a label the block omits leaves its field `None`.
fn extract_figures(block: &str) -> Figures {
    Figures {
        volume: extract_volume(block),
        price: extract_price(block).0,
        currency: extract_price(block).1,
        tx_date: extract_tx_date(block),
    }
}

/// The "Wolumen" field value → a bare integer (Polish `.`/space grouping stripped).
fn extract_volume(block: &str) -> Option<String> {
    let value = label_value(&block.to_lowercase(), "wolumen")?;
    let digits: String = value
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ' ' | '\u{00a0}' | '\u{2009}'))
        .filter(|c| c.is_ascii_digit())
        .collect();
    (1..=12).contains(&digits.len()).then_some(digits)
}

/// The "Cena" field value → decimal-normalized price + optional currency.
fn extract_price(block: &str) -> (Option<String>, Option<String>) {
    let lower = block.to_lowercase();
    // Anchor on "cena" but not "cena i wolumen" (the sub-heading, no value).
    let Some(value) = label_value(&lower, "cena") else {
        return (None, None);
    };
    // A leading number like "12,50 PLN" or "12.50" — grouping stripped, comma→dot.
    let mut digits = String::new();
    let mut seen_sep = false;
    for c in value.chars() {
        match c {
            '0'..='9' => digits.push(c),
            ',' if !seen_sep => {
                digits.push('.');
                seen_sep = true;
            }
            '.' => { /* thousands grouping before a comma; ignore */ }
            ' ' | '\u{00a0}' | '\u{2009}' => {}
            _ => break,
        }
    }
    if digits.is_empty() || !digits.contains('.') || digits.len() > 14 {
        return (None, None);
    }
    let currency = ["pln", "zł", "zl", "eur", "usd"]
        .iter()
        .find(|c| value.to_lowercase().contains(**c))
        .map(|c| normalize_currency(c));
    (Some(digits), currency)
}

/// The "Data transakcji" field value → an ISO date.
fn extract_tx_date(block: &str) -> Option<String> {
    let value = label_value(&block.to_lowercase(), "data transakcji")?;
    // The label value is lowercased; re-slice the same span from the original block
    // so a Polish month name keeps its case-insensitive match in `parse_pl_date`.
    parse_pl_date(value.trim())
}

// ---------------------------------------------------------------------------
// Label parsing shared helpers
// ---------------------------------------------------------------------------

/// The value following `label` (case-insensitive) up to the next line break or the
/// next form label. Tolerates a `:` separator glued or spaced. Operates on the
/// original-case `text` when `text` is original-case; callers pass lowercased text
/// when they only need the value's shape.
fn label_value(text: &str, label: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let at = lower.find(label)?;
    let after = &text[super::ceil_char_boundary(text, at + label.len())..];
    // Skip a separator ':' / ')' / whitespace run.
    let after = after.trim_start_matches([':', ')', ' ', '\t', '-', '–', '\u{00a0}']);
    // Cut at a newline or the next known label token.
    let end = [
        "\n",
        "imię",
        "imie",
        "nazwa",
        "stanowisko",
        "status",
        "rodzaj",
        "opis",
        "kod",
        "cena",
        "wolumen",
        "data",
        "miejsce",
        "powód",
        "powod",
    ]
    .iter()
    .filter_map(|needle| {
        let hay = after.to_lowercase();
        hay.find(needle).and_then(|i| (i > 0).then_some(i))
    })
    .min()
    .unwrap_or(after.len());
    let value = after[..super::floor_char_boundary(after, end.min(after.len()))].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn clean_field_value(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| {
            matches!(c, ':' | ',' | '.' | ')' | '(' | '"' | '„' | '”' | '-' | '–')
        })
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Nominative uppercase join key: collapse whitespace + uppercase, no declension
/// recovery (the form is already nominative).
fn normalize_nominative(raw: &str) -> String {
    clean_field_value(raw)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// Mask the word "transakcj*" so its embedded "akcj" cannot false-match shares.
fn mask_transakcja(lower: &str) -> String {
    lower.replace("transakcj", "###")
}

/// Instrument from the "Opis instrumentu finansowego" field value when present
/// (the authoritative form field), else the whole-form text with "transakcj*"
/// masked. Warrants are checked before shares (a warrant form also mentions akcje).
fn detect_form_instrument(text: &str, lower: &str) -> Option<Instrument> {
    if let Some(value) = label_value(text, "opis instrumentu")
        .or_else(|| label_value(text, "instrumentu finansowego"))
    {
        if let Some(instrument) = detect_instrument(&mask_transakcja(&value.to_lowercase())) {
            return Some(instrument);
        }
    }
    detect_instrument(&mask_transakcja(lower))
}

fn is_plausible(name: &str) -> bool {
    let chars = name.chars().count();
    (3..=90).contains(&chars)
        && name.split_whitespace().count() >= 2
        && name.chars().any(|c| c.is_alphabetic())
}

fn is_entity(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        "fundacja",
        "holding",
        "sp. z",
        "spółka",
        "s.a",
        " asi",
        "alternatywna",
        "inwestycyjna",
    ]
    .iter()
    .any(|m| lower.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine-readable single-transaction notification form (structurally
    /// equivalent to the KNF/ESMA standard annex — never copied from `private/`).
    fn single_tx_form() -> &'static str {
        "POWIADOMIENIE o transakcjach, o których mowa w art. 19 ust. 1 rozporządzenia MAR\n\
         1. Dane osoby pełniącej obowiązki zarządcze\n\
         Imię i nazwisko: Jan Testowy\n\
         2. Powód powiadomienia\n\
         Stanowisko / status: Wiceprezes Zarządu\n\
         4. Szczegóły transakcji\n\
         Opis instrumentu finansowego: Akcje zwykłe na okaziciela\n\
         Kod identyfikacyjny: PLTEST000010\n\
         Rodzaj transakcji: Nabycie\n\
         Cena: 12,50 PLN\n\
         Wolumen: 1 000\n\
         Data transakcji: 2026-07-03\n\
         Miejsce transakcji: XWAR"
    }

    #[test]
    fn parses_single_transaction_all_fields() {
        let AttachmentParse::Units(units) = parse_notification_text(single_tx_form()) else {
            panic!("expected a parsed form");
        };
        assert_eq!(units.len(), 1);
        let u = &units[0];
        assert_eq!(u.person_normalized, "JAN TESTOWY");
        assert_eq!(u.role, Some(InsiderRole::Management));
        assert_eq!(u.direction, Some(Direction::Buy));
        assert_eq!(u.instrument, Some(Instrument::Shares));
        assert_eq!(u.volume.as_deref(), Some("1000"));
        assert_eq!(u.price.as_deref(), Some("12.50"));
        assert_eq!(u.currency.as_deref(), Some("PLN"));
        assert_eq!(u.tx_date.as_deref(), Some("2026-07-03"));
    }

    #[test]
    fn parses_multi_transaction_table() {
        let text = "POWIADOMIENIE art. 19 MAR\n\
             Imię i nazwisko: Robert Tomaszewski\n\
             Stanowisko / status: Prezes Zarządu\n\
             Szczegóły transakcji\n\
             Rodzaj transakcji: Zbycie\n\
             Cena: 30,00 PLN\n\
             Wolumen: 275000\n\
             Data transakcji: 2026-07-03\n\
             Rodzaj transakcji: Zbycie\n\
             Cena: 31,00 PLN\n\
             Wolumen: 40000\n\
             Data transakcji: 2026-07-07";
        let AttachmentParse::Units(units) = parse_notification_text(text) else {
            panic!("expected units");
        };
        assert_eq!(units.len(), 2, "one unit per transaction row");
        assert_eq!(units[0].volume.as_deref(), Some("275000"));
        assert_eq!(units[0].tx_date.as_deref(), Some("2026-07-03"));
        assert_eq!(units[1].volume.as_deref(), Some("40000"));
        assert_eq!(units[1].tx_date.as_deref(), Some("2026-07-07"));
        assert!(units.iter().all(|u| u.direction == Some(Direction::Sell)));
    }

    #[test]
    fn closely_associated_entity_gets_role_and_pdmr() {
        let text = "POWIADOMIENIE art. 19 MAR\n\
             Nazwa: Nitka Fundacja Rodzinna\n\
             osoby blisko związanej z Michał Nitka, Członek Zarządu\n\
             Szczegóły transakcji\n\
             Rodzaj transakcji: Nabycie\n\
             Wolumen: 5000\n\
             Data transakcji: 2026-07-01";
        let AttachmentParse::Units(units) = parse_notification_text(text) else {
            panic!("expected units");
        };
        assert_eq!(units[0].role, Some(InsiderRole::CloselyAssociated));
        assert_eq!(
            units[0].related_pdmr_normalized.as_deref(),
            Some("MICHAŁ NITKA")
        );
        assert_eq!(units[0].volume.as_deref(), Some("5000"));
    }

    #[test]
    fn warrant_instrument_detected() {
        let text = "POWIADOMIENIE art. 19 MAR\n\
             Imię i nazwisko: Andrzej Oślizło\n\
             Stanowisko: Prezes Zarządu\n\
             Szczegóły transakcji\n\
             Opis instrumentu finansowego: Warranty subskrypcyjne serii A\n\
             Rodzaj transakcji: Nabycie\n\
             Wolumen: 12000\n\
             Data transakcji: 2026-07-15";
        let AttachmentParse::Units(units) = parse_notification_text(text) else {
            panic!("expected units");
        };
        assert_eq!(units[0].instrument, Some(Instrument::SubscriptionWarrants));
    }

    #[test]
    fn non_form_text_is_not_found() {
        assert_eq!(
            parse_notification_text("Zwyczajne Walne Zgromadzenie zwołuje się na dzień..."),
            AttachmentParse::NotFound
        );
        assert_eq!(parse_notification_text(""), AttachmentParse::NotFound);
    }

    #[test]
    fn form_without_resolvable_person_is_not_found() {
        // Has the form headings but no name field value.
        let text = "Szczegóły transakcji\nRodzaj transakcji: Nabycie\nWolumen: 100";
        assert_eq!(parse_notification_text(text), AttachmentParse::NotFound);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn never_panics_on_arbitrary_text(text in ".*") {
            let _ = parse_notification_text(&text);
        }

        #[test]
        fn parse_is_deterministic(text in ".*") {
            prop_assert_eq!(parse_notification_text(&text), parse_notification_text(&text));
        }
    }
}
