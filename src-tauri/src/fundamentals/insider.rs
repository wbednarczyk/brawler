//! Deterministic parser for MAR **art. 19** insider-transaction notification
//! bodies (ADR 0083 Decision 6 + the 2026-07-17 ground-truth amendment; plan
//! v0.57 T4).
//!
//! The Bankier `body_text` is only the ESPI **cover note** (a KNF "Raport
//! bieżący"): it names the person, their role, and usually the transaction
//! direction, but the volume/price/date table lives in an attached PDF for ~90%
//! of transactions (filled later by T4b). This parser therefore extracts
//! **person / role / direction / instrument** from the cover note and leaves the
//! figures NULL unless the note restates them inline — it **never guesses**
//! (mirrors the [`ownership::espi_notification`](super::ownership::espi_notification)
//! conservative contract). When the operative sentence *does* carry figures they
//! are recovered per-field (see [`extract_inline_figures`]): a `N akcji|warrantów`
//! volume, a `cenie N,NN PLN|zł` price/currency, and a transaction date from the
//! ESAP metadata block or — only when an inline volume anchors it — a `w dniu …`
//! date (so a bare receipt date is never mistaken for a transaction date).
//!
//! One filing commonly carries **several** notifications (multiple persons /
//! several closely-associated vehicles), each a separate unit with a stable
//! index. Units are recovered from, in priority order: bracket enumerators
//! (`[i]`/`[ii]`…), the attachment-description block, an "od X … oraz osoby
//! blisko związanej Y" conjunction, else a single unit.
//!
//! Pure and total: no network, no panics on arbitrary input (proptest-guarded).

use std::sync::OnceLock;

use regex::Regex;

pub mod attachment;

#[cfg(test)]
mod tests;

/// A PDMR / associated-party role. Nullable at the row level — a bare "osoba
/// pełniąca obowiązki zarządcze" with no board qualifier stays `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsiderRole {
    Management,
    Supervisory,
    CloselyAssociated,
}

impl InsiderRole {
    pub fn as_str(self) -> &'static str {
        match self {
            InsiderRole::Management => "management",
            InsiderRole::Supervisory => "supervisory",
            InsiderRole::CloselyAssociated => "closely_associated",
        }
    }
}

/// Transaction direction. Nullable — absent in ~1/4 of cover notes (PDF-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Buy,
    Sell,
    Other,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Buy => "buy",
            Direction::Sell => "sell",
            Direction::Other => "other",
        }
    }
}

/// Traded instrument. Nullable when the note does not name one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Instrument {
    Shares,
    SubscriptionWarrants,
    Other,
}

impl Instrument {
    pub fn as_str(self) -> &'static str {
        match self {
            Instrument::Shares => "shares",
            Instrument::SubscriptionWarrants => "subscription_warrants",
            Instrument::Other => "other",
        }
    }
}

/// One cleanly-extracted notification unit. Figures are `None` unless the cover
/// note restated them inline (never fabricated).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedInsiderUnit {
    pub person_raw: String,
    pub person_normalized: String,
    pub role: Option<InsiderRole>,
    pub related_pdmr_raw: Option<String>,
    pub related_pdmr_normalized: Option<String>,
    pub related_pdmr_role: Option<InsiderRole>,
    pub direction: Option<Direction>,
    pub instrument: Option<Instrument>,
    pub volume: Option<String>,
    pub price: Option<String>,
    pub currency: Option<String>,
    pub tx_date: Option<String>,
}

/// Outcome for one notification unit within a filing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum InsiderUnit {
    /// A confidently-extracted unit (has at least a person). Only these are written.
    Clean(ParsedInsiderUnit),
    /// A unit slot was recognized but its person could not be resolved. Carries a
    /// machine-stable reason. Never written.
    Ambiguous { reason: &'static str },
}

/// The result of parsing one filing's cover note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsiderNotificationParse {
    /// At least one notification unit was recognized (each `Clean` or `Ambiguous`).
    Units(Vec<InsiderUnit>),
    /// Nothing resembling a MAR art. 19 notification was found in the body.
    NotFound,
}

/// Parse a MAR art. 19 notification cover note into its transaction units.
/// See the module docs for the conservative contract.
pub fn parse_insider_notification(title: &str, body: &str) -> InsiderNotificationParse {
    let operative = extract_operative(body);
    if operative.trim().is_empty() {
        return InsiderNotificationParse::NotFound;
    }

    // Whole-filing signals (title carries direction/instrument words too).
    let combined_lower = format!("{title}\n{operative}").to_lowercase();

    // Gate: the body must actually read as a MAR art. 19 notification before any
    // unit is emitted (a filing merely mentioning "Zarząd rekomenduje" is not one).
    let is_notification = combined_lower.contains("art. 19")
        || combined_lower.contains("obowiązki zarządcze")
        || combined_lower.contains("obowiązków zarządczych")
        || combined_lower.contains("blisko związan")
        || ((combined_lower.contains("powiadomieni") || combined_lower.contains("zawiadomieni"))
            && combined_lower.contains("transakcj"));
    if !is_notification {
        return InsiderNotificationParse::NotFound;
    }
    let filing_direction = detect_direction(&combined_lower);
    let filing_instrument = detect_instrument(&combined_lower);

    let units = if let Some(units) = split_by_enumerators(&operative) {
        units
    } else if let Some(units) = units_from_attachments(body, &operative) {
        units
    } else if let Some(units) = split_conjunction(&operative) {
        units
    } else {
        single_unit(&operative)
    };

    if units.is_empty() {
        return InsiderNotificationParse::NotFound;
    }

    // Fold whole-filing direction/instrument into units that lack their own.
    let units = units
        .into_iter()
        .map(|unit| match unit {
            InsiderUnit::Clean(mut parsed) => {
                if parsed.direction.is_none() {
                    parsed.direction = filing_direction;
                }
                if parsed.instrument.is_none() {
                    parsed.instrument = filing_instrument;
                }
                InsiderUnit::Clean(parsed)
            }
            other => other,
        })
        .collect();

    InsiderNotificationParse::Units(units)
}

// ---------------------------------------------------------------------------
// Operative-text isolation
// ---------------------------------------------------------------------------

/// The Polish operative window: from "Treść raportu" (the report body) up to the
/// bilingual / entity-metadata boundary. Falls back to the whole body when the
/// "Treść raportu" marker is absent. The attachment table is kept (its
/// descriptions name the parties) — unit builders trim it where needed.
fn extract_operative(body: &str) -> String {
    let lower = body.to_lowercase();
    let start = lower
        .find("treść raportu")
        .map(|at| at + "treść raportu".len())
        // Skip a possible trailing ':' after the marker.
        .map(|at| body[at..].find(':').map(|c| at + c + 1).unwrap_or(at))
        .unwrap_or(0);

    // End at the first bilingual / entity-metadata boundary after the start.
    let rest_lower = &lower[start.min(lower.len())..];
    let end_rel = ["message (english version)", "informacje o podmiocie"]
        .iter()
        .filter_map(|needle| rest_lower.find(needle))
        .min();
    let end = match end_rel {
        Some(rel) => start + rel,
        None => body.len(),
    };
    body[start.min(body.len())..end.min(body.len())].to_string()
}

/// The attachment-description block ("ZałącznikiPlikOpis…") — a `(file, desc)`
/// stream where the descriptions name the parties in the NOMINATIVE. Returns the
/// descriptions with their `Powiadomienie`/`Zawiadomienie` prefix stripped.
fn attachment_descriptions(body: &str) -> Vec<String> {
    let lower = body.to_lowercase();
    // The section starts at the operative-trailing "Załączniki" and ends at the
    // bilingual boundary.
    let Some(start) = lower
        .rfind("załącznikiplikopis")
        .or_else(|| lower.rfind("załącznikiplik"))
    else {
        return Vec::new();
    };
    let rest = &body[start..];
    let rest_lower = &lower[start..];
    let end = ["message (english version)", "informacje o podmiocie"]
        .iter()
        .filter_map(|needle| rest_lower.find(needle))
        .min()
        .unwrap_or(rest.len());
    let block = &rest[..end];

    // Each ".pdf" is followed by its description up to the next filename or end.
    let mut descriptions = Vec::new();
    let block_lower = block.to_lowercase();
    let mut cursor = 0usize;
    while let Some(rel) = block_lower[cursor..].find(".pdf") {
        let after = cursor + rel + ".pdf".len();
        // The description runs until the next ".pdf" (or the block end).
        let next = block_lower[after..]
            .find(".pdf")
            .map(|n| after + n)
            .unwrap_or(block.len());
        // Trim back to the start of the trailing filename token if present.
        let desc_end = block[after..next]
            .rfind(['/', '\\'])
            .map(|_| next)
            .unwrap_or(next);
        let desc = strip_notification_prefix(block[after..desc_end.min(block.len())].trim());
        // The linearized stream glues each description to the NEXT filename with no
        // separator ("…Mikołaj PodgórskiPowiadomienie_PFR"): cut at the next
        // filename prefix / underscore so the party name survives intact.
        let desc = cut_at_next_filename(&desc);
        if is_name_like(&desc) {
            descriptions.push(desc);
        }
        cursor = after;
    }
    descriptions
}

/// Cut a description at the next glued filename token — the next
/// "Powiadomienie"/"Zawiadomienie" prefix (after the first character) or an
/// underscore (filenames use them; party names never do).
fn cut_at_next_filename(desc: &str) -> String {
    let lower = desc.to_lowercase();
    let mut cut = desc.len();
    for marker in ["powiadomienie", "zawiadomienie"] {
        // Skip a marker at position 0 (already handled by the prefix strip).
        if let Some(pos) = lower.match_indices(marker).map(|(i, _)| i).find(|&i| i > 0) {
            cut = cut.min(pos);
        }
    }
    if let Some(pos) = desc.find('_') {
        cut = cut.min(pos);
    }
    desc[..floor_char_boundary(desc, cut)].trim().to_string()
}

fn strip_notification_prefix(text: &str) -> String {
    let trimmed = text.trim();
    for prefix in [
        "Powiadomienie",
        "Zawiadomienie",
        "powiadomienie",
        "zawiadomienie",
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    trimmed.to_string()
}

/// A description names a party when it has ≥2 alphabetic tokens and does not
/// look like a generic "Załącznik nr N" / file-stem row.
fn is_name_like(desc: &str) -> bool {
    let lower = desc.to_lowercase();
    if lower.is_empty() || lower.contains(" nr ") || lower.starts_with("załącznik") {
        return false;
    }
    // Reject file-stem noise (underscores, long digit runs).
    if desc.contains('_') || desc.chars().filter(|c| c.is_ascii_digit()).count() > 3 {
        return false;
    }
    let words = desc
        .split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphabetic()))
        .count();
    words >= 2 && desc.chars().count() <= 90
}

// ---------------------------------------------------------------------------
// Unit builders
// ---------------------------------------------------------------------------

fn enumerator_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // [i] [ii] [iii] [iv] [v] … or [1] [2] …
    RE.get_or_init(|| Regex::new(r"\[(?:i{1,3}v?|iv|vi{0,3}|[1-9])\]").unwrap())
}

/// Split the operative on bracket enumerators, one unit per `[i]…` segment.
fn split_by_enumerators(operative: &str) -> Option<Vec<InsiderUnit>> {
    let matches: Vec<usize> = enumerator_re()
        .find_iter(operative)
        .map(|m| m.start())
        .collect();
    if matches.len() < 2 {
        return None;
    }
    let mut segments = Vec::new();
    for (idx, &start) in matches.iter().enumerate() {
        let end = matches.get(idx + 1).copied().unwrap_or(operative.len());
        segments.push(&operative[start..end]);
    }
    Some(segments.iter().map(|seg| unit_from_segment(seg)).collect())
}

/// A conjunction of exactly two parties: "od <N1> <role1> … (i|oraz) osob(y|ą)
/// blisko [z nim ]związan(ej|ą) <N2>" (the GKI shape).
fn split_conjunction(operative: &str) -> Option<Vec<InsiderUnit>> {
    let lower = operative.to_lowercase();
    let anchor = [
        "i osoby blisko",
        "oraz osoby blisko",
        "i osobą blisko",
        "oraz osobą blisko",
    ]
    .iter()
    .filter_map(|needle| lower.find(needle))
    .min()?;
    let first = &operative[..anchor];
    let second = &operative[anchor..];
    // The first segment must carry an "od <name>" person for this to be a real
    // two-party conjunction (not a single note that merely mentions an associate).
    extract_od_person(first)?;
    let first_unit = unit_from_segment(first);
    let mut second_unit = unit_from_segment(second);
    // The associate's anchoring PDMR is the first party ("osoby blisko z nim
    // związanej" refers back to it) — fill it in when the second segment named none.
    if let (InsiderUnit::Clean(first_parsed), InsiderUnit::Clean(second_parsed)) =
        (&first_unit, &mut second_unit)
    {
        if second_parsed.role == Some(InsiderRole::CloselyAssociated)
            && second_parsed.related_pdmr_raw.is_none()
        {
            second_parsed.related_pdmr_raw = Some(first_parsed.person_raw.clone());
            second_parsed.related_pdmr_normalized = Some(first_parsed.person_normalized.clone());
            second_parsed.related_pdmr_role = first_parsed.role;
        }
    }
    Some(vec![first_unit, second_unit])
}

/// One unit per name-like attachment description (the SCW shape); role /
/// direction / anchoring PDMR come from the operative sentence.
fn units_from_attachments(body: &str, operative: &str) -> Option<Vec<InsiderUnit>> {
    let descriptions = attachment_descriptions(body);
    if descriptions.len() < 2 {
        return None;
    }
    let op_lower = operative.to_lowercase();
    let filing_direction = detect_direction(&op_lower);
    let mut units = Vec::new();
    for desc in descriptions {
        let is_entity = is_entity_name(&desc);
        let (role, related_pdmr_raw, related_pdmr_role) = if is_entity {
            let pdmr = pdmr_for_entity(&desc, operative);
            (
                Some(InsiderRole::CloselyAssociated),
                pdmr.clone(),
                pdmr.and_then(|p| role_near(operative, &p)),
            )
        } else {
            (role_near(operative, &desc), None, None)
        };
        units.push(InsiderUnit::Clean(ParsedInsiderUnit {
            person_normalized: normalize_person(&desc),
            person_raw: desc,
            role,
            related_pdmr_normalized: related_pdmr_raw.as_deref().map(normalize_person),
            related_pdmr_raw,
            related_pdmr_role,
            direction: filing_direction,
            instrument: None,
            volume: None,
            price: None,
            currency: None,
            tx_date: None,
        }));
    }
    Some(units)
}

fn single_unit(operative: &str) -> Vec<InsiderUnit> {
    vec![unit_from_segment(operative)]
}

/// Build one unit from a text segment: find the party, classify its role, and
/// (for a closely-associated vehicle) resolve the anchoring PDMR.
fn unit_from_segment(segment: &str) -> InsiderUnit {
    let Some(person_raw) = extract_party(segment) else {
        return InsiderUnit::Ambiguous {
            reason: "person_unresolved",
        };
    };
    let lower = segment.to_lowercase();
    let entity = is_entity_name(&person_raw);
    let closely = entity
        || lower.contains("blisko związan")
        || lower.contains("blisko z nim związan")
        || lower.contains("fundacja rodzinna");

    let (role, related_pdmr_raw, related_pdmr_role) = if closely {
        let pdmr = extract_related_pdmr(segment);
        (
            Some(InsiderRole::CloselyAssociated),
            pdmr.clone(),
            pdmr.and_then(|p| role_near(segment, &p)),
        )
    } else {
        (role_from_keywords(&lower), None, None)
    };

    let figures = extract_inline_figures(segment);
    InsiderUnit::Clean(ParsedInsiderUnit {
        person_normalized: normalize_person(&person_raw),
        person_raw,
        role,
        related_pdmr_normalized: related_pdmr_raw.as_deref().map(normalize_person),
        related_pdmr_raw,
        related_pdmr_role,
        direction: detect_direction(&lower),
        instrument: detect_instrument(&lower),
        volume: figures.volume,
        price: figures.price,
        currency: figures.currency,
        tx_date: figures.tx_date,
    })
}

// ---------------------------------------------------------------------------
// Inline transaction figures (present only when the cover note restates them)
// ---------------------------------------------------------------------------

/// Figures a cover note may restate inline. Every field is independently nullable
/// and never guessed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct InlineFigures {
    volume: Option<String>,
    price: Option<String>,
    currency: Option<String>,
    tx_date: Option<String>,
}

/// Extract volume / price / currency / tx_date from a unit segment when — and only
/// when — the cover note restates them inline (the ~10% of filings whose operative
/// sentence carries figures; the rest live in the attached PDF, filled by T4b).
///
/// Conservative, per-field:
/// - **volume**: the first `N akcji|warrantów` figure (Polish `.`/space grouping
///   stripped) — a post-transaction holdings figure only ever follows it.
/// - **price / currency**: an explicit `cenie N,NN PLN|zł` (decimal comma → dot,
///   grouping stripped); absent → both NULL.
/// - **tx_date**: the ESAP metadata block's ISO date when present, else — only when
///   an inline volume anchors it — the first `w dniu <date>` (Polish long-form or
///   `DD.MM.YYYY`). The volume gate rejects the bare *receipt* date ("otrzymał w
///   dniu …") of the ~2/3 of notes that carry no figures.
///
/// Never fabricates: any field the note omits (or states ambiguously) stays NULL.
fn extract_inline_figures(segment: &str) -> InlineFigures {
    let volume = extract_volume(segment);
    let (price, currency) = extract_price(segment);
    let tx_date = extract_esap_date(segment).or_else(|| {
        // A "w dniu" date is a transaction date only when an inline volume anchors
        // it; otherwise it is the notification-receipt date (never a tx date).
        volume
            .is_some()
            .then(|| extract_transaction_date(segment))
            .flatten()
    });
    InlineFigures {
        volume,
        price,
        currency,
        tx_date,
    }
}

/// Digit run (with Polish `.`/space grouping) immediately preceding `akcj`/`warrant`.
fn volume_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d[\d.\u{00a0}\u{2009} ]*)(?:akcj|warrant)").unwrap())
}

fn extract_volume(segment: &str) -> Option<String> {
    let caps = volume_re().captures(segment)?;
    let digits: String = caps
        .get(1)?
        .as_str()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    (1..=12).contains(&digits.len()).then_some(digits)
}

/// `cenie N,NN <ccy>` — an explicit average/unit price with decimal comma.
fn price_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)cenie\s+(\d[\d.\u{00a0}\u{2009} ]*,\d+)\s*(pln|zł|zl|eur|usd)?").unwrap()
    })
}

fn extract_price(segment: &str) -> (Option<String>, Option<String>) {
    let Some(caps) = price_re().captures(segment) else {
        return (None, None);
    };
    let Some(raw) = caps.get(1) else {
        return (None, None);
    };
    // Strip thousands grouping (dots/spaces), decimal comma → dot.
    let normalized: String = raw
        .as_str()
        .chars()
        .filter_map(|c| match c {
            ',' => Some('.'),
            '.' | ' ' | '\u{00a0}' | '\u{2009}' => None,
            other if other.is_ascii_digit() => Some(other),
            _ => None,
        })
        .collect();
    // Sanity: exactly one decimal point, plausible length.
    if normalized.matches('.').count() != 1 || normalized.len() > 14 {
        return (None, None);
    }
    let currency = caps.get(2).map(|m| normalize_currency(m.as_str()));
    (Some(normalized), currency)
}

pub(crate) fn normalize_currency(raw: &str) -> String {
    match raw.to_lowercase().as_str() {
        "zł" | "zl" | "pln" => "PLN".to_string(),
        other => other.to_uppercase(),
    }
}

/// The ESAP metadata block's transaction date ("… dotyczą informacje<ISO>"), the
/// authoritative in-body date for filings that carry the block (e.g. warrant grants).
fn esap_date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)dotyczą informacje\s*(\d{4}-\d{2}-\d{2})").unwrap())
}

fn extract_esap_date(segment: &str) -> Option<String> {
    let caps = esap_date_re().captures(segment)?;
    let iso = caps.get(1)?.as_str();
    valid_iso(iso).then(|| iso.to_string())
}

/// The first `w dniu <date>` after which a real date parses (Polish long-form or
/// `DD.MM.YYYY`). Callers gate this on an inline volume so a receipt date is not
/// taken as a transaction date.
fn w_dniu_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)w\s+dniu\s+").unwrap())
}

fn extract_transaction_date(segment: &str) -> Option<String> {
    for m in w_dniu_re().find_iter(segment) {
        let after = &segment[m.end()..];
        let window = &after[..ceil_char_boundary(after, after.len().min(28))];
        if let Some(iso) = parse_pl_date(window) {
            return Some(iso);
        }
    }
    None
}

/// Parse a date at the start of `text`: Polish long-form ("6 lipca 2026"),
/// `DD.MM.YYYY`, or ISO. Returns `YYYY-MM-DD` or `None`.
pub(crate) fn parse_pl_date(text: &str) -> Option<String> {
    let t = text.trim_start();
    // Long-form: "<day> <miesiąc(genitive)> <year>".
    static LONG: OnceLock<Regex> = OnceLock::new();
    let long = LONG.get_or_init(|| Regex::new(r"^(\d{1,2})\s+([\p{L}]+)\s+(\d{4})").unwrap());
    if let Some(caps) = long.captures(t) {
        let day: u32 = caps.get(1)?.as_str().parse().ok()?;
        let month = polish_month(caps.get(2)?.as_str())?;
        let year: u32 = caps.get(3)?.as_str().parse().ok()?;
        return build_iso(year, month, day);
    }
    // Numeric "DD.MM.YYYY".
    static NUM: OnceLock<Regex> = OnceLock::new();
    let num = NUM.get_or_init(|| Regex::new(r"^(\d{1,2})\.(\d{1,2})\.(\d{4})").unwrap());
    if let Some(caps) = num.captures(t) {
        let day: u32 = caps.get(1)?.as_str().parse().ok()?;
        let month: u32 = caps.get(2)?.as_str().parse().ok()?;
        let year: u32 = caps.get(3)?.as_str().parse().ok()?;
        return build_iso(year, month, day);
    }
    // ISO "YYYY-MM-DD".
    static ISO: OnceLock<Regex> = OnceLock::new();
    let iso = ISO.get_or_init(|| Regex::new(r"^(\d{4})-(\d{2})-(\d{2})").unwrap());
    if let Some(caps) = iso.captures(t) {
        let s = format!(
            "{}-{}-{}",
            caps.get(1)?.as_str(),
            caps.get(2)?.as_str(),
            caps.get(3)?.as_str()
        );
        return valid_iso(&s).then_some(s);
    }
    None
}

fn build_iso(year: u32, month: u32, day: u32) -> Option<String> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || !(2000..=2100).contains(&year) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn valid_iso(s: &str) -> bool {
    let mut parts = s.split('-');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    matches!(
        (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>()),
        (Ok(2000..=2100), Ok(1..=12), Ok(1..=31))
    )
}

/// Genitive Polish month name → month number (the form that follows a day number).
fn polish_month(name: &str) -> Option<u32> {
    Some(match name.to_lowercase().as_str() {
        "stycznia" => 1,
        "lutego" => 2,
        "marca" => 3,
        "kwietnia" => 4,
        "maja" => 5,
        "czerwca" => 6,
        "lipca" => 7,
        "sierpnia" => 8,
        "września" => 9,
        "października" => 10,
        "listopada" => 11,
        "grudnia" => 12,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Party extraction
// ---------------------------------------------------------------------------

/// The primary party in a segment: an "od <Name>" / "przez <Name>" anchor, or an
/// entity name if one is spelled out. Enumerated segments lead with the party
/// name right after the `[i]` marker.
fn extract_party(segment: &str) -> Option<String> {
    // Enumerated segment: name follows the bracket marker.
    if let Some(m) = enumerator_re().find(segment) {
        let after = segment[m.end()..].trim_start();
        if let Some(name) = leading_name(after) {
            return Some(name);
        }
    }
    if let Some(name) = extract_od_person(segment) {
        return Some(name);
    }
    // Fall back to the first entity name in the segment.
    leading_name(segment.trim_start())
}

fn od_person_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "od [Pana|Pani|p.] <Name…>" up to a role/clause boundary.
    RE.get_or_init(|| {
        // The notifying party is "od <party>" ("received FROM …") or, in the
        // "dokonaniem transakcji przez <party>" phrasing, "przez <party>". A
        // leading honorific / "podmiotu"/"spółki" is stripped; boilerplate "przez"
        // captures (KRS registry / legal-basis clauses) are filtered downstream.
        Regex::new(
            r"(?i)\b(?:od|przez)\s+(?:pan[aiąeą]*\s+|p\.\s+|podmiot[u]?\s+|spółki\s+|firmy\s+)?([^,\(]+?)(?:\s+(?:jako|członk|osob[yaąę]|będąc|pełniąc|z\s+siedzib|–|zawiadomien|powiadomien)|[,\(]|\stj\.|$)",
        )
        .unwrap()
    })
}

fn extract_od_person(segment: &str) -> Option<String> {
    // Iterate all "od/przez <…>" matches and take the first plausible party: the
    // earliest match is often KRS/legal boilerplate ("prowadzonego przez Sąd
    // Rejonowy…", "wykonywanych przez osoby pełniące…"), not the notifying party.
    for caps in od_person_re().captures_iter(segment) {
        let Some(m) = caps.get(1) else { continue };
        let cleaned = clean_name(m.as_str());
        let starts_upper = cleaned.chars().next().is_some_and(|c| c.is_uppercase());
        if !starts_upper || is_boilerplate_capture(&cleaned) {
            continue;
        }
        // The capture is a bare role phrase — the party name is either after a
        // dash ("od Członka Zarządu – Adama Łodygowskiego") or the trailing tokens
        // of the capture itself ("od Członka Zarządu KRUK S.A. Piotra Kowalewskiego").
        if is_role_phrase(&cleaned) {
            let after = segment[m.end()..].trim_start();
            if after.starts_with('–') || after.starts_with('-') {
                if let Some(name) = leading_name(after.trim_start_matches(['–', '-', ' '])) {
                    if !is_role_phrase(&name) {
                        return Some(name);
                    }
                }
            }
            if let Some(name) = trailing_person_name(&cleaned) {
                return Some(name);
            }
            continue;
        }
        if is_plausible_party(&cleaned) {
            return Some(cleaned);
        }
    }
    None
}

/// The trailing "<Given> <Surname>" of a role-phrase capture (the last two
/// name-ish tokens — alphabetic, capitalized, no dots), when the person is
/// embedded after the role/issuer. Returns `None` unless two adjacent name
/// tokens close the phrase.
fn trailing_person_name(capture: &str) -> Option<String> {
    let tokens: Vec<&str> = capture.split_whitespace().collect();
    let name_ish = |t: &str| {
        let chars = t.chars().count();
        chars >= 3
            && t.chars().all(|c| c.is_alphabetic())
            && t.chars().next().is_some_and(|c| c.is_uppercase())
    };
    let mut trailing: Vec<&str> = Vec::new();
    for token in tokens.iter().rev() {
        if name_ish(token) {
            trailing.push(token);
            if trailing.len() == 2 {
                break;
            }
        } else if !trailing.is_empty() {
            break;
        }
    }
    if trailing.len() != 2 {
        return None;
    }
    trailing.reverse();
    let name = clean_name(&trailing.join(" "));
    (is_plausible_party(&name) && !is_role_phrase(&name)).then_some(name)
}

/// Whether a capture is a bare organ/role phrase (not a party name).
fn is_role_phrase(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("zarządu")
        || lower.contains("rady nadzorczej")
        || lower.starts_with("prezes")
        || lower.starts_with("wiceprezes")
        || lower.starts_with("członk")
        || lower.starts_with("przewodnicz")
}

/// The leading capitalized name token-run of a segment (for enumerated segments
/// and entity fallbacks).
fn leading_name(text: &str) -> Option<String> {
    let mut collected: Vec<&str> = Vec::new();
    for token in text.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '_');
        // Accept mixed-case / underscore entity leads too ("cyber_Folks").
        let starts_upper = clean.chars().next().is_some_and(|c| c.is_uppercase())
            || clean.contains('_')
            || clean.chars().any(|c| c.is_uppercase());
        let is_conn = matches!(
            clean.to_lowercase().as_str(),
            "s.a."
                | "s.a"
                | "sa"
                | "sp."
                | "z"
                | "o.o."
                | "spółka"
                | "akcyjna"
                | "alternatywna"
                | "inwestycyjna"
                | "asi"
                | "holding"
                | "fundacja"
                | "rodzinna"
                | "rodziny"
                | "w"
                | "organizacji"
        );
        if starts_upper || (is_conn && !collected.is_empty()) {
            collected.push(clean);
        } else if !collected.is_empty() {
            break;
        }
        if collected.len() >= 8 {
            break;
        }
    }
    let name = clean_name(&collected.join(" "));
    is_plausible_party(&name).then_some(name)
}

fn clean_name(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches(|c: char| {
        c == ',' || c == '.' || c == '"' || c == '„' || c == '”' || c == '-' || c == '–'
    });
    // Drop a leading honorific left in the capture.
    let without_honorific = ["Pan ", "Pani ", "Pana ", "Panu ", "Panią ", "p. ", "Panem "]
        .iter()
        .find_map(|h| without_prefix_ci(trimmed, h))
        .unwrap_or(trimmed);
    without_honorific
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn without_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        Some(&text[prefix.len()..])
    } else {
        None
    }
}

fn is_plausible_party(name: &str) -> bool {
    let chars = name.chars().count();
    if !(3..=90).contains(&chars) {
        return false;
    }
    let lower = name.to_lowercase();
    // A party name is at least two tokens (given+surname, or entity words) — a
    // lone generic token ("Zarząd", "Spółka") is never a party.
    if name.split_whitespace().count() < 2 {
        return false;
    }
    // A sentence fragment is not a party name.
    !lower.contains(" że ")
        && !lower.contains(" który ")
        && !lower.contains("informuje")
        && !lower.contains("otrzyma")
        && name.chars().any(|c| c.is_alphabetic())
}

/// A capture that is KRS-registry / court boilerplate ("prowadzonego przez Sąd
/// Rejonowy…"), never a notifying party.
fn is_boilerplate_capture(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["sąd", "rejonow", "rejestr", "wydział", "krajow"]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn is_entity_name(name: &str) -> bool {
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
        "cyber_",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

// ---------------------------------------------------------------------------
// Related-PDMR (anchor for closely-associated vehicles)
// ---------------------------------------------------------------------------

fn extract_related_pdmr(segment: &str) -> Option<String> {
    // "…związan… z <PDMR-instrumental>, <role>" or "…związan… z <PDMR> (<role>)".
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)związan\w*\s+(?:z\s+(?:osobą\s+pełniącą\s+obowiązki\s+zarządcze\s+w\s+spółce\s*[-–]?\s*)?)(.+?)(?:,|\(|\s+prezes|\s+wiceprezes|\s+członk|\s+przewodnicz|$)").unwrap()
    });
    let caps = re.captures(segment)?;
    let cleaned = clean_name(caps.get(1)?.as_str());
    is_plausible_party(&cleaned).then_some(cleaned)
}

/// Resolve the PDMR anchoring an entity vehicle whose name embeds or references a
/// natural person: prefer a "<Given> <Surname>" run inside the entity name,
/// otherwise the surname stem matched against the operative.
fn pdmr_for_entity(entity: &str, operative: &str) -> Option<String> {
    // A leading "<Given> <Surname>" before a "Fundacja"/"Holding" keyword.
    let lower = entity.to_lowercase();
    if let Some(kw) = ["fundacja", "holding", "sp."]
        .iter()
        .filter_map(|k| lower.find(k))
        .min()
    {
        let head = entity[..kw].trim();
        let cleaned = clean_name(head);
        if head.split_whitespace().count() == 2 && is_plausible_party(&cleaned) {
            return Some(cleaned);
        }
    }
    // Otherwise a surname stem shared with an instrumental name in the operative.
    let surname = entity
        .split_whitespace()
        .find(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))?;
    let stem: String = surname.chars().take(5).collect::<String>().to_lowercase();
    for token in operative.split_whitespace() {
        if token.to_lowercase().starts_with(&stem) && token.len() >= surname.len() {
            // Pair with the preceding given name.
            if let Some(pos) = operative.find(token) {
                let before: Vec<&str> = operative[..pos].split_whitespace().collect();
                if let Some(given) = before.last() {
                    let candidate = clean_name(&format!("{given} {token}"));
                    if is_plausible_party(&candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Role / direction / instrument keyword maps
// ---------------------------------------------------------------------------

fn role_from_keywords(lower: &str) -> Option<InsiderRole> {
    if lower.contains("rada nadzorcz")
        || lower.contains("rady nadzorczej")
        || lower.contains("radzie nadzorczej")
        || lower.contains("przewodnicząc") && lower.contains("nadzorcz")
    {
        return Some(InsiderRole::Supervisory);
    }
    if lower.contains("blisko związan") || lower.contains("fundacja rodzinna") {
        return Some(InsiderRole::CloselyAssociated);
    }
    if lower.contains("prezes")
        || lower.contains("wiceprezes")
        || lower.contains("członka zarządu")
        || lower.contains("członek zarządu")
        || lower.contains("członkiem zarządu")
        || lower.contains("zarządu spółki")
    {
        return Some(InsiderRole::Management);
    }
    // Bare "osoba pełniąca obowiązki zarządcze" with no board qualifier → None.
    None
}

/// The role keyword nearest an occurrence of `name`'s surname in `text`.
fn role_near(text: &str, name: &str) -> Option<InsiderRole> {
    let surname = name.split_whitespace().last()?;
    let stem: String = surname.chars().take(5).collect::<String>().to_lowercase();
    let lower = text.to_lowercase();
    let at = lower.find(&stem)?;
    // Tight window so a following party's clause does not bleed in.
    let start = floor_char_boundary(&lower, at.saturating_sub(6));
    let end = ceil_char_boundary(&lower, (at + 45).min(lower.len()));
    role_from_keywords(&lower[start..end])
}

pub(crate) fn detect_direction(lower: &str) -> Option<Direction> {
    // Order matters: an explicit sale/transfer keyword wins over a generic
    // "nabycie" that may appear in boilerplate.
    if lower.contains("zbyci") || lower.contains("sprzedaż") || lower.contains("sprzedaz") {
        return Some(Direction::Sell);
    }
    if lower.contains("przeniesieni") {
        return Some(Direction::Other);
    }
    if lower.contains("nabyci") {
        return Some(Direction::Buy);
    }
    None
}

pub(crate) fn detect_instrument(lower: &str) -> Option<Instrument> {
    if lower.contains("warrant") {
        return Some(Instrument::SubscriptionWarrants);
    }
    if lower.contains("akcj") {
        return Some(Instrument::Shares);
    }
    None
}

// ---------------------------------------------------------------------------
// Polish personal-name normalization (genitive / instrumental → nominative)
// ---------------------------------------------------------------------------

/// Uppercase, whitespace-collapsed nominal key (join key). For a two-token
/// personal name it recovers the nominative case with conservative suffix rules;
/// entity names are kept as-is (uppercased). Never guesses beyond the rules.
pub fn normalize_person(raw: &str) -> String {
    let cleaned = clean_name(raw);
    let nominative = to_nominative(&cleaned);
    nominative
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// Recover the nominative of a declined two-token personal name. Entities and
/// anything that is not a clean two-token name are returned unchanged.
fn to_nominative(name: &str) -> String {
    if is_entity_name(name) {
        return name.to_string();
    }
    let tokens: Vec<&str> = name.split_whitespace().collect();
    if tokens.len() != 2 {
        return name.to_string();
    }
    let given = nominative_given(tokens[0]);
    let surname = nominative_surname(tokens[1]);
    format!("{given} {surname}")
}

fn ends_with(word: &str, suffix: &str) -> bool {
    word.to_lowercase().ends_with(suffix)
}

fn strip_suffix_chars(word: &str, suffix_char_len: usize) -> String {
    let n = word.chars().count();
    word.chars()
        .take(n.saturating_sub(suffix_char_len))
        .collect()
}

fn nominative_given(token: &str) -> String {
    if ends_with(token, "ą") {
        return format!("{}a", strip_suffix_chars(token, 1));
    }
    if ends_with(token, "em") {
        return strip_suffix_chars(token, 2);
    }
    if ends_with(token, "y") {
        // Feminine genitive (Martyny → Martyna, Joanny → Joanna).
        return format!("{}a", strip_suffix_chars(token, 1));
    }
    if ends_with(token, "a") {
        // Masculine genitive/accusative (Andrzeja → Andrzej, Piotra → Piotr).
        let stripped = strip_suffix_chars(token, 1);
        return apply_fleeting_e(&stripped);
    }
    token.to_string()
}

/// Fleeting-e restoration for the common `-eł` class (Pawł → Paweł).
fn apply_fleeting_e(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n >= 2 && chars[n - 1] == 'ł' && is_consonant(chars[n - 2]) {
        let mut out: String = chars[..n - 1].iter().collect();
        out.push('e');
        out.push('ł');
        return out;
    }
    word.to_string()
}

fn is_consonant(c: char) -> bool {
    let lower = c.to_lowercase().next().unwrap_or(c);
    c.is_alphabetic() && !"aeiouyąęó".contains(lower)
}

fn nominative_surname(token: &str) -> String {
    if ends_with(token, "ą") {
        return format!("{}a", strip_suffix_chars(token, 1));
    }
    if ends_with(token, "kiem") {
        return strip_suffix_chars(token, 3); // Stasikiem → Stasik
    }
    if ends_with(token, "giem") {
        return strip_suffix_chars(token, 3);
    }
    if ends_with(token, "im") {
        return strip_suffix_chars(token, 1); // Podgórskim → Podgórski
    }
    if ends_with(token, "iego") {
        return strip_suffix_chars(token, 3); // Wysockiego → Wysocki
    }
    if ends_with(token, "ego") {
        return format!("{}y", strip_suffix_chars(token, 3)); // Testowego → Testowy
    }
    if ends_with(token, "ej") {
        return format!("{}a", strip_suffix_chars(token, 2)); // Testowej → Testowa
    }
    if ends_with(token, "em") {
        return strip_suffix_chars(token, 2); // Lewkowiczem → Lewkowicz
    }
    if ends_with(token, "y") && token.chars().count() > 3 {
        return format!("{}a", strip_suffix_chars(token, 1)); // Zięby → Zięba
    }
    // Surnames ending in -a (Kulessa) or -i (Kowalewski) or -o (Oślizło) are kept
    // as-is: stripping them would corrupt indeclinable / -ski forms.
    token.to_string()
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
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn never_panics_on_arbitrary_text(title in ".*", body in ".*") {
            let _ = parse_insider_notification(&title, &body);
        }

        #[test]
        fn parse_is_deterministic(title in ".*", body in ".*") {
            let a = parse_insider_notification(&title, &body);
            let b = parse_insider_notification(&title, &body);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn normalize_person_never_panics(name in ".*") {
            let _ = normalize_person(&name);
        }
    }
}
