//! Classify stored report documents into diffable financial statements and order
//! them chronologically (v0.47.0, ADR 0052). Pure, deterministic, and tested:
//! consecutive same-type pairs are what the diff compares.
//!
//! `period_id` is frequently null on real backfilled documents, so the period is
//! parsed from the title/URL; documents whose period cannot be parsed fall back to
//! `created_at` ordering so they still pair, just less precisely.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::fundamentals::extraction::classify::{classify_doc_kind, DocKind};

/// The financial-statement type the diff aligns within (never across types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum StatementType {
    /// Skonsolidowane sprawozdanie finansowe — consolidated.
    Ssf,
    /// Jednostkowe sprawozdanie finansowe — standalone.
    Jsf,
}

impl StatementType {
    pub fn as_str(self) -> &'static str {
        match self {
            StatementType::Ssf => "ssf",
            StatementType::Jsf => "jsf",
        }
    }
}

/// Classify a document as a diffable financial statement, or `None` if it is not
/// one (announcements, audit agreements, management reports, etc. are excluded).
///
/// A thin projection of the document taxonomy (ADR 0077 §1): only the two
/// periodic-statement kinds are diffable, everything else — auditor work
/// products, governance documents, presentations, filing companions — is not.
/// The exclusion/marker rationale and its G-2 contract corpus live in
/// [`crate::fundamentals::extraction::classify`], so the diff's notion of "a
/// statement" cannot drift from the taxonomy's.
pub fn classify_statement(title: &str, url: &str) -> Option<StatementType> {
    // DIFF capability limit, NOT taxonomy: the report diff compares parsed
    // statement bodies, and an ESEF package (`.xbri`) or a zipped filing is not a
    // body the diff can read. The taxonomy (`classify_doc_kind`, ADR 0077 §1
    // amendment 2026-07-09) now classifies a bare `.xbri` package as a periodic
    // statement, but the diff must still exclude it — hence this projection-side
    // exclusion, documented and tested here, which does not touch the taxonomy.
    let raw = format!("{title} {url}").to_lowercase();
    if raw.contains(".xbri") || raw.contains(".zip") {
        return None;
    }
    match classify_doc_kind(title, url) {
        DocKind::PeriodicSsf => Some(StatementType::Ssf),
        DocKind::PeriodicJsf => Some(StatementType::Jsf),
        _ => None,
    }
}

/// A sortable chronological key parsed from the title/URL: `(year, period_index)`
/// where period_index is 1..=4 for quarters, 2 for H1, 4 for annual. `None` when
/// no period can be parsed.
pub fn period_sort_key(title: &str, url: &str) -> Option<(i32, u8)> {
    period_index_from_text(&format!("{} {}", title, url))
}

/// Which carrier feeds the grammar, and therefore which pass leads.
#[derive(Clone, Copy)]
enum PeriodTextMode {
    /// Document title/URL and cover-page text: an explicit calendar date is the
    /// most reliable signal and is tried FIRST (the tuned v0.59 order). The
    /// filename/document route depends on this order — do not change it.
    DateFirst,
    /// ESPI feed-title carrier: a bare date in a feed title is typically the
    /// publication date, so an explicit period MARKER (`za I kwartał 2025`,
    /// `1H2025`, `za rok 2024`) is tried before the dates pass (C3). The
    /// publication-date idiom (`z dnia <date>`) is stripped by the caller.
    MarkerFirst,
}

/// The grammar behind [`period_sort_key`], over one free-text carrier.
///
/// Two carriers feed it and there is deliberately **one** grammar for both
/// (v0.59, card `fc692da`): a document's title+URL, and — when those name no
/// period — the document's own cover page (`jobs::structured_extraction::
/// derive_report_period`). Adding a form here fixes it for both routes at once.
fn period_index_from_text(text: &str) -> Option<(i32, u8)> {
    period_index_from_text_mode(text, PeriodTextMode::DateFirst)
}

fn period_index_from_text_mode(text: &str, mode: PeriodTextMode) -> Option<(i32, u8)> {
    let lowered = text.to_lowercase();
    let tokens = tokenize(&lowered);

    // Feed-title carrier: an explicit marker wins over a lone bare date (C3). The
    // filename/document carrier keeps the date-first order untouched.
    if matches!(mode, PeriodTextMode::MarkerFirst) {
        if let Some(resolved) = marker_period(&tokens, &lowered) {
            return Some(resolved);
        }
    }

    match dates_period(&tokens) {
        DatePeriod::NonCalendarWindow => {
            // The document states its reporting window and the window is not a
            // calendar one (a non-December fiscal year): the calendar mapping
            // below would be a guess, so abstain on the intra-year period while
            // still ordering by year (ADR 0061 decision 1).
            return parse_year(&lowered).map(|year| (year, 0));
        }
        DatePeriod::Resolved(year, index) => return Some((year, index)),
        DatePeriod::None => {}
    }
    // Marker pass. When no marker names an intra-year period we still keep the
    // year (index `0`) as the sort key — unchanged from v0.59.
    if let Some(resolved) = marker_period(&tokens, &lowered) {
        return Some(resolved);
    }
    parse_year(&lowered).map(|year| (year, 0))
}

/// Period resolution from an explicit marker only: a glued compact form
/// (`1Q2026`, `1HY2025`), or a period marker (`QSr 3`, `III kwartał`, `roczny`)
/// plus the first standalone year. `None` when nothing explicitly names an
/// intra-year period — the caller decides whether that abstains or falls through
/// to a year-only key.
fn marker_period(tokens: &[String], lowered: &str) -> Option<(i32, u8)> {
    if let Some(resolved) = compact_period(tokens) {
        return Some(resolved);
    }
    // The intra-year index keeps its EXISTING precedence unchanged: an
    // explicit q1-q4/annual marker (`rok obrotowy`, `QSr 3`, …) still wins
    // over a bare day+month calendar date — this is what lets a marker like
    // "rok obrotowy" correctly override a non-calendar fiscal year-end (e.g.
    // Synektik's 30 September) being misread as a mere Q3.
    let normalized = normalize(tokens);
    let (index, source) = parse_period_with_source(&normalized);
    if index == 0 {
        return None;
    }
    // Bug #325, tightened (adversarial-review blocker #5): prefer the year
    // paired with the SAME day+month calendar marker that named THIS index,
    // over `parse_year`'s "first standalone year anywhere in the text" below
    // — but ONLY when the index itself came from that calendar marker
    // (`PeriodIndexSource::MonthCalendarMarker`). When the index instead came
    // from an EXPLICIT marker (`rok obrotowy`, `QSr 4`, `roczny`, …), an
    // unrelated day+month mention elsewhere in the same text must never
    // donate ITS year to that marker's index: a non-calendar filer's cover
    // page can read "rok obrotowy zakończony 30 września 2025 [...] 31
    // grudnia 2024" (the true FYE stated next to September, a comparative or
    // otherwise unrelated December date mentioned separately) — pairing the
    // "rok obrotowy" index with the December marker's 2024 would resolve
    // FY2024, silently wrong by a year. See `month_end_year_for_index`.
    if matches!(source, PeriodIndexSource::MonthCalendarMarker) {
        if let Some(year) = month_end_year_for_index(&normalized, index) {
            return Some((year, index));
        }
    }
    let year = parse_year(lowered)?;
    Some((year, index))
}

/// Which pass inside [`parse_period_with_source`] resolved the intra-year
/// index — an EXPLICIT marker (`rok obrotowy`, `QSr 4`, `roczny`, a quarter
/// word) or the day+month CALENDAR fallback (`months`, e.g. bare "31
/// grudnia"). [`marker_period`] uses this to decide whether
/// [`month_end_year_for_index`] may safely donate its year to this index
/// (adversarial-review blocker #5, tightening bug #325's fix): the calendar
/// marker's own nearby year is trustworthy evidence about ITS OWN index, but
/// says nothing about an index an unrelated EXPLICIT marker already settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeriodIndexSource {
    ExplicitMarker,
    MonthCalendarMarker,
}

/// The year paired with the day+month calendar marker for THIS `index`
/// (1..=4), tolerant of exactly one embedded space in the year — the
/// pdf2htmlEX glyph-run artifact that `tokenize`/`normalize` cannot repair (a
/// stray space mid-digit survives as a token boundary, so "2025" re-emerges
/// as two tokens "20"+"25", rejoined by `normalize` with exactly one space).
/// `None` when this index names no day+month marker (e.g. it came from an
/// explicit `QSr`/`rok obrotowy` marker instead) or that marker has no year
/// right next to it.
///
/// Bug #325 (DNP, 202512 Dino Polska SF UoR.xhtml): the cover page states the
/// reporting year TWICE as "31 grudnia 20 25" (broken by the extraction
/// artifact in both places), while an unrelated signing dateline nearby
/// states "26 marca 2026" with a CLEAN "2026". The old grammar paired
/// whichever index `parse_period` found with `parse_year`'s document-wide
/// "first clean year anywhere" — which skipped past both broken "2025"s and
/// landed on the signing date's "2026" instead (fiscal_year 2026 / period_end
/// 2026-12-31, wrong). Tying the year to the SAME marker that named the index
/// is the fix. When the nearby year is ALSO broken beyond this one-space
/// tolerance (or absent), this returns `None` and the caller falls back to
/// the old global scan — the same "never guess" doctrine as every other pass,
/// never worse than before the fix.
fn month_end_year_for_index(normalized: &str, index: u8) -> Option<i32> {
    let g = grammar();
    let (regex, _) = g.months_with_year.iter().find(|(_, i)| *i == index)?;
    let captures = regex.captures(normalized)?;
    let year_text = captures[1].replace(' ', "");
    let year: i32 = year_text.parse().ok()?;
    (2000..=2099).contains(&year).then_some(year)
}

/// The full reporting period `(fiscal_year, period_type, period_end)` a
/// title/URL names, assuming a calendar fiscal year: index 1→`Q1`/`-03-31`,
/// 2→`H1`/`-06-30`, 3→`Q3`/`-09-30`, 4→`FY`/`-12-31`.
///
/// The single home of the title/URL period derivation for the **document/
/// filename** carrier — the fetched-document route
/// ([`crate::jobs::structured_extraction::derive_report_period`]). Here an
/// explicit calendar date is the most reliable signal and leads (date-first, the
/// tuned v0.59 order). The ESPI **feed-title** carrier uses the sibling
/// [`period_from_feed_title`], which strips the publication-date idiom and lets a
/// period marker win over a lone date. An unparseable year or an ambiguous
/// intra-year period (index `0`) yields `None`: **never guessed** (ADR 0061
/// decision 1 / ADR 0084 — no tier invents a period).
pub fn period_from_title_url(title: &str, url: &str) -> Option<(i64, &'static str, String)> {
    to_period(period_sort_key(title, url))
}

/// The period an **ESPI feed title** names — the ingest-time cover-note tier's
/// carrier, which has a feed item's title/link, never a stored document (ADR
/// 0061 tier 2a). Two things distinguish it from the document/filename route
/// ([`period_from_title_url`]) and both are title-specific (C3):
///
/// 1. The Polish publication-date idiom `z dnia <date>` is stripped first — that
///    phrase is ALWAYS a filing/publication reference, never the reporting
///    period, and a lone date on a calendar boundary would otherwise be read as
///    the period.
/// 2. An explicit period marker (`za I kwartał 2025`, `1H2025`, `za rok 2024`)
///    wins over a lone bare date, because a bare date in a feed title is usually
///    the publication date (`MarkerFirst`). The document/filename order is
///    deliberately left unchanged.
pub fn period_from_feed_title(title: &str, url: &str) -> Option<(i64, &'static str, String)> {
    let cleaned = strip_publication_date_idiom(title);
    let text = format!("{} {}", cleaned, url);
    to_period(period_index_from_text_mode(
        &text,
        PeriodTextMode::MarkerFirst,
    ))
}

/// Removes the Polish `z dnia <date>` publication-date idiom (and its close
/// separator/month-word variants) from a feed title, so the publication date it
/// carries cannot be mistaken for the reporting period. Only the idiom's own date
/// is removed; any period marker or reporting-period date in the title stays.
fn strip_publication_date_idiom(title: &str) -> std::borrow::Cow<'_, str> {
    static IDIOM: OnceLock<Regex> = OnceLock::new();
    let idiom = IDIOM.get_or_init(|| {
        Regex::new(
            r"(?i)\bz\s+dnia\s+(?:\d{1,2}[.\-/]\d{1,2}[.\-/]\d{2,4}|\d{4}[.\-/]\d{1,2}[.\-/]\d{1,2}|\d{1,2}\s+\p{L}+\s+\d{4})",
        )
        .expect("static publication-date idiom pattern")
    });
    idiom.replace_all(title, " ")
}

/// The same derivation applied to free document text — the cover page of a
/// periodic report, which states the period the title/URL failed to name
/// (v0.59, card `fc692da`). Same grammar, same abstention contract.
pub fn period_from_text(text: &str) -> Option<(i64, &'static str, String)> {
    to_period(period_index_from_text(text))
}

fn to_period(key: Option<(i32, u8)>) -> Option<(i64, &'static str, String)> {
    let (year, period_index) = key?;
    let period_type = match period_index {
        1 => "Q1",
        2 => "H1",
        3 => "Q3",
        4 => "FY",
        _ => return None,
    };
    let period_end = match period_type {
        "Q1" => format!("{year}-03-31"),
        "H1" => format!("{year}-06-30"),
        "Q3" => format!("{year}-09-30"),
        _ => format!("{year}-12-31"),
    };
    Some((i64::from(year), period_type, period_end))
}

/// A short human label for the period, for the candidate read model.
pub fn period_label(title: &str, url: &str) -> Option<String> {
    let (year, period) = period_sort_key(title, url)?;
    let suffix = match period {
        1 => "Q1",
        2 => "Q2/H1",
        3 => "Q3",
        4 => "Q4/FY",
        // Unknown intra-year period: show just the year rather than a bare "?".
        _ => return Some(year.to_string()),
    };
    Some(format!("{year} {suffix}"))
}

fn parse_year(t: &str) -> Option<i32> {
    // First standalone 4-digit year in 2000..=2099.
    let bytes = t.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i].is_ascii_digit() {
            // `i` is a char boundary (it points at an ASCII digit), but `i + 4` is a raw
            // byte offset that can land inside a multi-byte char (e.g. a Polish diacritic
            // shortly after a digit). A 4-digit year can only ever be ASCII, so when the
            // window isn't a valid boundary it can't be a match anyway — skip it instead
            // of slicing into a multi-byte char and panicking.
            if t.is_char_boundary(i + 4) {
                let chunk = &t[i..i + 4];
                if let Ok(y) = chunk.parse::<i32>() {
                    if (2000..=2099).contains(&y) {
                        return Some(y);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Period grammar (v0.59, card `fc692da`).
//
// Derived from the REAL title/URL forms in the maintainer's database, in three
// ordered passes — cheapest and least ambiguous first:
//
//   1. an explicit calendar period-end date (`30.06.2025`, `2025_09_30`),
//   2. a glued quarter/half token carrying its own year (`1Q2026`, `Q125`,
//      `2023q3`, `1HY2025`),
//   3. a period marker (`QSr 3`, `III kwartał`, `HY`, `9M`, `roczny`, …) plus
//      the first standalone year in the text.
//
// Every pass abstains rather than guesses: an unstated period stays index `0`,
// and an explicitly NON-calendar reporting window (a non-December fiscal year,
// e.g. Synektik's 1 Oct – 30 Sep) abstains outright — the calendar mapping the
// contract assumes would be wrong there.
// ---------------------------------------------------------------------------

/// Calendar period ends: `(month, day)` → period index.
const CALENDAR_PERIOD_ENDS: [((u32, u32), u8); 4] =
    [((3, 31), 1), ((6, 30), 2), ((9, 30), 3), ((12, 31), 4)];

/// Lowercased alphanumeric runs, in order. Titles and URLs separate their parts
/// with `_`, `-`, `.`, `/`, spaces and brackets interchangeably, so the grammar
/// works on tokens rather than on raw substrings.
fn tokenize(lowered: &str) -> Vec<String> {
    lowered
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Tokens joined by single spaces, with the two Polish words that *contain*
/// `roczn` but do not mean "annual" rewritten first: `śródroczny` (interim) and
/// `półroczny` (half-year). Without this the annual arm swallowed every interim
/// report — a measured defect on the maintainer's real titles.
fn normalize(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| {
            if token.starts_with("srodroczn") || token.starts_with("śródroczn") {
                "interim"
            } else if token.starts_with("polrocz") || token.starts_with("półrocz") {
                "polrocze"
            } else {
                token.as_str()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

enum DatePeriod {
    Resolved(i32, u8),
    /// A stated reporting window that does not start on 1 January.
    NonCalendarWindow,
    None,
}

fn numeric(token: &str) -> Option<(usize, u32)> {
    if token.chars().all(|c| c.is_ascii_digit()) {
        token.parse().ok().map(|value| (token.len(), value))
    } else {
        None
    }
}

/// A `(year, month, day)` starting at `i`, in either `YYYY ? ?` or `? ? YYYY`
/// order, when three adjacent tokens are all numeric and form a valid date.
fn date_at(tokens: &[String], i: usize) -> Option<(i32, u32, u32)> {
    let a = numeric(tokens.get(i)?)?;
    let b = numeric(tokens.get(i + 1)?)?;
    let c = numeric(tokens.get(i + 2)?)?;
    let (year, month, day) = if a.0 == 4 {
        (a.1, b.1, c.1)
    } else if c.0 == 4 {
        (c.1, b.1, a.1)
    } else {
        return None;
    };
    if !(2000..=2099).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year as i32, month, day))
}

fn calendar_period_index(month: u32, day: u32) -> Option<u8> {
    CALENDAR_PERIOD_ENDS
        .iter()
        .find(|((m, d), _)| *m == month && *d == day)
        .map(|(_, index)| *index)
}

fn dates_period(tokens: &[String]) -> DatePeriod {
    let mut ends: BTreeSet<(i32, u8)> = BTreeSet::new();
    for i in 0..tokens.len() {
        let Some((year, month, day)) = date_at(tokens, i) else {
            continue;
        };
        let this_index = calendar_period_index(month, day);
        // A range (`01.10.2024-31.03.2025`) or a comparative header pair
        // (`31.03.2025 31.12.2024`) is six adjacent numeric tokens: two dates back
        // to back.
        if let Some((year2, month2, day2)) = date_at(tokens, i + 3) {
            // A comparative balance-sheet header pairs the current period end with
            // the prior one: both are calendar boundaries and the current is listed
            // FIRST (and is therefore the LATER date). It names the period
            // explicitly, so resolve to the current (first) period rather than
            // abstaining — abstaining lost real statements (card `fc692da`). Only
            // this shape is rescued; anything else keeps the abstention below.
            if let (Some(index), Some(_)) = (this_index, calendar_period_index(month2, day2)) {
                if (year, month, day) > (year2, month2, day2) {
                    return DatePeriod::Resolved(year, index);
                }
            }
            // A window that does not open on 1 January is not a calendar period,
            // whatever its end date looks like.
            if (month, day) != (1, 1) {
                return DatePeriod::NonCalendarWindow;
            }
        }
        if let Some(index) = this_index {
            ends.insert((year, index));
        }
    }
    // Exactly one calendar period end is a statement; several are a comparison.
    if ends.len() == 1 {
        let (year, index) = ends.into_iter().next().expect("one element");
        DatePeriod::Resolved(year, index)
    } else {
        DatePeriod::None
    }
}

struct Grammar {
    /// Glued forms carrying their own year, matched per token.
    quarter_then_year: Regex,
    year_then_quarter_prefix: Regex,
    quarter_after_year: Regex,
    half_then_year: Regex,
    year_then_half: Regex,
    q1: Regex,
    q2: Regex,
    q3: Regex,
    q4: Regex,
    months: Vec<(Regex, u8)>,
    /// The SAME day+month calendar markers as `months`, but additionally
    /// capturing the year immediately following — tolerant of exactly one
    /// embedded space in the 4 digits (bug #325: `tokenize`/`normalize`
    /// rejoin every original token gap as a single space, so a pdf2htmlEX
    /// glyph-run artifact that split a year mid-digit re-emerges this way).
    /// See [`month_end_year`].
    months_with_year: Vec<(Regex, u8)>,
}

fn grammar() -> &'static Grammar {
    static GRAMMAR: OnceLock<Grammar> = OnceLock::new();
    GRAMMAR.get_or_init(|| {
        let compile = |pattern: &str| Regex::new(pattern).expect("static period pattern");
        Grammar {
            // `1Q2026`, `raport1h24`  → quarter, then a 2- or 4-digit year.
            quarter_then_year: compile(r"^[a-z]*([1-4])q(\d{2}|\d{4})[a-z]*$"),
            // `Q125`, `Q324` → quarter after a leading `q`, then the year.
            year_then_quarter_prefix: compile(r"^[a-z]*q([1-4])(\d{2}|\d{4})[a-z]*$"),
            // `2023q3` → year first.
            quarter_after_year: compile(r"^(\d{4})q([1-4])[a-z]*$"),
            // `HY25`, `1HY2025`, `1h2025`, `HYR2025`.
            half_then_year: compile(r"^[a-z]*1?hy?r?(\d{2}|\d{4})[a-z]*$"),
            // `2024H1SSF`.
            year_then_half: compile(r"^(\d{4})h1[a-z]*$"),
            q1: compile(
                r"\bq1\b|\b1q\b|\bi q\b|\biq\b|\bi kwarta|\b1 kwarta|\bpierwsz\w* kwarta|\bqsr ?1\b|\bqsr i\b|\bsa q1\b|\b3m\b|\b3 miesi|\bthree months\b",
            ),
            q2: compile(
                r"\bq2\b|\b2q\b|\bii q\b|\biiq\b|\bii kwarta|\b2 kwarta|\bdrug\w* kwarta|\bqsr ?2\b|\bqsr ii\b|\bpsr\d*\b|\bpolrocze\b|\bh1\b|\b1h\b|\bhy\d*\b|\bhyr\d*\b|\bsa p\b|\bpssf\b|\bpsf\b|\bpjsf\b|\b6m\b|\b6 miesi|\bsix months\b|\bhalf year\b",
            ),
            q3: compile(
                r"\bq3\b|\b3q\b|\biii q\b|\biiiq\b|\biii kwarta|\b3 kwarta|\btrzeci\w* kwarta|\bqsr ?3\b|\bqsr iii\b|\bsa q3\b|\b9m\b|\b9 miesi",
            ),
            q4: compile(
                r"\bq4\b|\b4q\b|\biv q\b|\bivq\b|\biv kwarta|\b4 kwarta|\bczwart\w* kwarta|\bqsr ?4\b|\broczn\w*|\bannual\b|\brok obrotowy\b|\bza \d{4} r\b|\bsa r\b|\b12m\b|\b12 miesi",
            ),
            months: vec![
                (compile(r"\b31 (marca|march)\b"), 1),
                (compile(r"\b30 (czerwca|june)\b"), 2),
                (compile(r"\b30 (wrzesnia|września|september)\b"), 3),
                (compile(r"\b31 (grudnia|december)\b"), 4),
            ],
            months_with_year: vec![
                (compile(r"\b31 (?:marca|march) (\d{4}|\d{2} \d{2})\b"), 1),
                (compile(r"\b30 (?:czerwca|june) (\d{4}|\d{2} \d{2})\b"), 2),
                (
                    compile(r"\b30 (?:wrzesnia|września|september) (\d{4}|\d{2} \d{2})\b"),
                    3,
                ),
                (compile(r"\b31 (?:grudnia|december) (\d{4}|\d{2} \d{2})\b"), 4),
            ],
        }
    })
}

fn two_or_four_digit_year(raw: &str) -> i32 {
    let value: i32 = raw.parse().expect("digits");
    if value < 100 {
        2000 + value
    } else {
        value
    }
}

/// Glued `quarter+year` filename forms. Ambiguity (two different readings in one
/// string) abstains rather than picking one.
fn compact_period(tokens: &[String]) -> Option<(i32, u8)> {
    let g = grammar();
    let mut hits: BTreeSet<(i32, u8)> = BTreeSet::new();
    for token in tokens {
        for (regex, quarter_first) in [
            (&g.quarter_then_year, true),
            (&g.year_then_quarter_prefix, true),
            (&g.quarter_after_year, false),
        ] {
            if let Some(captures) = regex.captures(token) {
                let (quarter, year) = if quarter_first {
                    (&captures[1], &captures[2])
                } else {
                    (&captures[2], &captures[1])
                };
                hits.insert((
                    two_or_four_digit_year(year),
                    quarter.parse::<u8>().expect("1..=4"),
                ));
            }
        }
        for regex in [&g.half_then_year, &g.year_then_half] {
            if let Some(captures) = regex.captures(token) {
                hits.insert((two_or_four_digit_year(&captures[1]), 2));
            }
        }
    }
    if hits.len() == 1 {
        hits.into_iter().next()
    } else {
        None
    }
}

/// The intra-year period a marker names, or `0` when none does — the abstention
/// [`period_from_title_url`] turns into `None`. Quarter and half-year markers are
/// tested before the annual arm: an interim report often carries both. Also
/// reports WHICH pass matched ([`PeriodIndexSource`]) — `marker_period` uses
/// this to gate `month_end_year_for_index` (adversarial-review blocker #5).
/// A `0` index pairs with `PeriodIndexSource::ExplicitMarker` as a
/// placeholder; callers must check the index, never branch on the source
/// alone.
fn parse_period_with_source(normalized: &str) -> (u8, PeriodIndexSource) {
    let g = grammar();
    for (regex, index) in [(&g.q1, 1), (&g.q3, 3), (&g.q2, 2), (&g.q4, 4)] {
        if regex.is_match(normalized) {
            return (index, PeriodIndexSource::ExplicitMarker);
        }
    }
    for (regex, index) in &g.months {
        if regex.is_match(normalized) {
            return (*index, PeriodIndexSource::MonthCalendarMarker);
        }
    }
    // Unknown intra-year period; sort before Q1 so it is not mistaken for newer.
    (0, PeriodIndexSource::ExplicitMarker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_consolidated_and_standalone() {
        assert_eq!(
            classify_statement("cyber_Folks 2026 Q1 SSF", "x/SSF.pdf"),
            Some(StatementType::Ssf)
        );
        assert_eq!(
            classify_statement("cyber_Folks 2026 Q1 JSF", "x/JSF.pdf"),
            Some(StatementType::Jsf)
        );
    }

    #[test]
    fn excludes_non_statements() {
        assert_eq!(
            classify_statement("Podpisanie umowy o badanie", "x.xhtml"),
            None
        );
        assert_eq!(
            classify_statement("Opóźnienie publikacji raportu rocznego", "x.pdf"),
            None
        );
        assert_eq!(
            classify_statement("Szacunkowa wysokość przychodów", "x.pdf"),
            None
        );
        assert_eq!(classify_statement("Projekty uchwał ZWZ", "x.pdf"), None);
    }

    #[test]
    fn excludes_real_cbf_annual_filing_components() {
        // Supervisory-board report, audit report, and signature/data files bundled in
        // CBF's real annual filing must NOT be treated as the financial statement.
        assert_eq!(
            classify_statement("26_03_17_sprawozdanie_RN_cyber_FolksSA_z_oceny", "x.pdf"),
            None
        );
        assert_eq!(
            classify_statement("SzB_GK_cyber_Folks Sprawozdanie z badania", "x.xhtml"),
            None
        );
        assert_eq!(
            classify_statement("JSF_CBF-2025-12-31-1-pl.xhtml.xades", "x"),
            None
        );
        assert_eq!(classify_statement("CBF-2025-12-31-1-pl.xbri", "x"), None);
        // ...but the actual standalone ESEF statement is kept.
        assert_eq!(
            classify_statement("JSF_CBF Jednostkowe Sprawozdanie finansowe", "x.xhtml"),
            Some(StatementType::Jsf)
        );
    }

    #[test]
    fn esef_package_is_a_statement_in_taxonomy_but_excluded_from_diff() {
        // The taxonomy classifies a bare ESEF `.xbri` package as a periodic
        // statement (ADR 0077 §1 amendment), but the diff cannot compare a
        // package body, so `classify_statement` must still return `None` for it.
        // This is the no-diff-regression proof paired with the unchanged golden.
        assert_eq!(
            classify_doc_kind("CBF-2025-12-31-1-pl.xbri", ""),
            DocKind::PeriodicSsf
        );
        assert_eq!(classify_statement("CBF-2025-12-31-1-pl.xbri", ""), None);
        assert_eq!(
            classify_statement("Skonsolidowany raport 2025", "x.zip"),
            None
        );
        // A `.xbri.xades` signature is already `Other` in the taxonomy — excluded
        // by both layers.
        assert_eq!(
            classify_statement("CBF-2025-12-31-1-pl.xbri.xades", "x"),
            None
        );
    }

    #[test]
    fn classification_corpus_golden() {
        // A representative slice of real GPW/NewConnect filing-component title shapes
        // (the diffable statements + the surrounding filing noise the diff must reject),
        // captured as a golden snapshot. Per ADR 0049 a classification transform ships a
        // golden snapshot; this closes the "denylist rot silently reclassifies" regression
        // class from the v0.47.0 retro — an edit to the exclusion list or the financial
        // markers that flips a known-good classification now surfaces as a snapshot diff,
        // forcing a conscious review instead of a silent behavior change.
        let corpus = [
            // --- must classify (the diffable statements) ---
            ("cyber_Folks 2026 Q1 SSF", "raport/SSF.pdf"),
            ("cyber_Folks 2026 Q1 JSF", "raport/JSF.pdf"),
            ("Skonsolidowane sprawozdanie finansowe GK za 2025", "x.pdf"),
            (
                "Jednostkowe sprawozdanie finansowe za I półrocze 2025",
                "x.pdf",
            ),
            ("Raport kwartalny QSr 1/2026", "raport_kwartalny.pdf"),
            ("Raport okresowy PSr 2025", "psr_2025.pdf"),
            ("Consolidated financial statement 2025", "x.pdf"),
            (
                "JSF_CBF Jednostkowe Sprawozdanie finansowe",
                "CBF-2025-12-31.xhtml",
            ),
            // no explicit SSF/JSF marker → defaults to consolidated
            (
                "Skonsolidowany raport okresowy grupy 2025",
                "raport_okresowy.pdf",
            ),
            // --- must be rejected (real filing noise bundled in ESPI/ESEF filings) ---
            ("26_03_17_sprawozdanie_RN_cyber_FolksSA_z_oceny", "x.pdf"),
            ("SzB_GK_cyber_Folks Sprawozdanie z badania", "x.xhtml"),
            ("Sprawozdanie z przeglądu śródrocznego skróconego", "x.pdf"),
            ("JSF_CBF-2025-12-31-1-pl.xhtml.xades", "x"),
            ("CBF-2025-12-31-1-pl.xbri", "x"),
            (
                "Podpisanie umowy o badanie sprawozdania finansowego",
                "x.xhtml",
            ),
            ("Opóźnienie publikacji raportu rocznego", "x.pdf"),
            ("Szacunkowa wysokość przychodów w 2025", "x.pdf"),
            ("Projekty uchwał ZWZ", "x.pdf"),
            ("Ogłoszenie o zwołaniu ZWZ", "x.pdf"),
            ("List Prezesa Zarządu do akcjonariuszy", "x.pdf"),
            ("Wybrane dane finansowe", "x.pdf"),
            ("Opinia i raport biegłego rewidenta", "x.pdf"),
        ];
        let rows: Vec<_> = corpus
            .iter()
            .map(|(title, url)| {
                (
                    *title,
                    classify_statement(title, url).map(StatementType::as_str),
                    period_label(title, url),
                )
            })
            .collect();
        insta::assert_debug_snapshot!(rows);
    }

    #[test]
    fn orders_periods_chronologically() {
        let q3_25 = period_sort_key("cyber_Folks 2025 Q3 SSF", "x").unwrap();
        let q1_26 = period_sort_key("cyber_Folks 2026 Q1 SSF", "x").unwrap();
        assert!(q3_25 < q1_26);
        assert_eq!(period_label("2026 Q1 SSF", "x").as_deref(), Some("2026 Q1"));
    }

    #[test]
    fn unparseable_period_is_none() {
        assert_eq!(period_sort_key("some report", "x.pdf"), None);
    }

    #[test]
    fn parses_1h_title_form_as_half_year() {
        // T-A1: the title form "za_1H_2023" lowercases to "1h", which no arm
        // matched before — it collapsed to index 0 (unknown) and lost the period.
        // It is the mirror of the already-handled "h1" and must map to H1.
        assert_eq!(period_sort_key("za_1H_2023", ""), Some((2023, 2)));
        assert_eq!(
            period_label("za_1H_2023", "").as_deref(),
            Some("2023 Q2/H1")
        );
    }

    #[test]
    fn parse_year_does_not_panic_on_multibyte_char_after_digit_run() {
        // "1abł " lowercased: an ASCII digit run (just "1") is followed within the
        // 4-byte scan window by the multi-byte 'ł', so `i + 4` lands on its second
        // byte — reproducing the char-boundary slicing panic (same class as
        // signal_dates::find_date_for_labels).
        assert_eq!(period_sort_key("1abł", ""), None);
    }

    // -----------------------------------------------------------------------
    // Real-issuer title forms (v0.59, card fc692da).
    //
    // Every literal below is a REAL title/URL from the maintainer's database:
    // the A4 sweep measured 1 144 of 1 544 eligible stored documents never
    // reaching extraction because no period could be derived, and 83 of those
    // (31.7 % of the kind) were `periodic_ssf` — genuine financial statements
    // invisible to the pipeline purely for want of a period parse.
    // -----------------------------------------------------------------------

    #[test]
    fn derives_period_from_calendar_period_end_date_in_title() {
        // The single most common real form: the statement names its balance-sheet
        // date instead of a quarter label (PKO BP, PZU, GPW, Cognor, Decora, …).
        assert_eq!(
            period_from_title_url("SSF_PZU_SA_Raport_30.06.2025.pdf", ""),
            Some((2025, "H1", "2025-06-30".to_owned()))
        );
        assert_eq!(
            period_from_title_url("GK_DECORA_SSF_2025_09_30_PL.pdf", ""),
            Some((2025, "Q3", "2025-09-30".to_owned()))
        );
        assert_eq!(
            period_from_title_url("Text_SA_31_03_2025_BSSF_MSSF_PL.xhtml", ""),
            Some((2025, "Q1", "2025-03-31".to_owned()))
        );
        assert_eq!(
            period_from_title_url("SSF Cognor IFRS skonsolidowane 30-09-2025.pdf", ""),
            Some((2025, "Q3", "2025-09-30".to_owned()))
        );
    }

    #[test]
    fn derives_qsr_and_glued_quarter_forms() {
        // `QSr N` (KGHM, Comp, Develia, Inter Cars) and the glued `NQyyyy` /
        // `QNyy` / `yyyyQN` filename forms (GPW, Benefit Systems, Asseco, Text).
        assert_eq!(
            period_sort_key("Skonsolidowany raport kwartalny QSr_3_2025.pdf", ""),
            Some((2025, 3))
        );
        assert_eq!(period_sort_key("Comp_Qsr1_2026.pdf", ""), Some((2026, 1)));
        assert_eq!(
            period_sort_key("SSF GK GPW 1Q2026-sig-sig-sig-sig.pdf", ""),
            Some((2026, 1))
        );
        assert_eq!(
            period_sort_key("Q125_Raport_kwartalny_Grupa_Asseco.pdf", ""),
            Some((2025, 1))
        );
        assert_eq!(
            period_sort_key("txt_2023q3_ssf_signed.pdf", ""),
            Some((2023, 3))
        );
    }

    #[test]
    fn derives_roman_and_word_quarter_forms_across_separators() {
        // Underscore-separated roman numerals never matched (the old arms assumed a
        // space), and `III kwartał` fell into the `i kwarta` arm when it did.
        assert_eq!(
            period_sort_key("Raport_kwartalny_Grupa_ABS_III_kwartal_2025.pdf", ""),
            Some((2025, 3))
        );
        assert_eq!(
            period_sort_key("ABS_Sprawozdanie_finansowe_I_kwartal_2024.pdf", ""),
            Some((2024, 1))
        );
        assert_eq!(
            period_sort_key("Raport_kwartalny_GK_Mo-BRUK_SA_IIIQ_2025.pdf", ""),
            Some((2025, 3))
        );
        assert_eq!(
            period_sort_key(
                "Skonsolidowany_raport_kwartalny_Grupy_Kapitalowej_Echo_Investment_za_pierwszy_kwartal_2024_r.pdf",
                ""
            ),
            Some((2024, 1))
        );
    }

    #[test]
    fn derives_half_year_and_month_count_forms() {
        assert_eq!(
            period_sort_key("ABS_Sprawozdanie_finansowe_HY_2024.pdf", ""),
            Some((2024, 2))
        );
        assert_eq!(
            period_sort_key(
                "SKONSOLIDOWANE_SPRAWOZDANIE_FINANSOWE_GK_BS_1HY2025-sig.pdf",
                ""
            ),
            Some((2025, 2))
        );
        // `PSSF` = półroczne skonsolidowane sprawozdanie finansowe.
        assert_eq!(
            period_sort_key("Elektrotim_2025_PSSF_MSSF_PL-sig.pdf", ""),
            Some((2025, 2))
        );
        assert_eq!(
            period_sort_key("Sprawozdanie finansowe za 9M 2025.pdf", ""),
            Some((2025, 3))
        );
    }

    #[test]
    fn interim_marker_is_not_read_as_annual() {
        // Regression: `śródroczny`/`półroczny` both contain `roczn`, so the annual
        // arm swallowed every interim report whose quarter marker sorted later.
        assert_eq!(
            period_sort_key("Srodroczny_skonsolidowany_raport_za_1Q_2025-sig.pdf", ""),
            Some((2025, 1))
        );
        assert_eq!(
            period_sort_key("Skonsolidowany_raport_polroczny_PSr_2025.pdf", ""),
            Some((2025, 2))
        );
        assert_eq!(
            period_sort_key(
                "Srodroczne_skonsolidowane_sprawozdanie_finansowe_Grupy_Kapitalowej_PZU_SA_za_III_kwartal_2024.pdf",
                ""
            ),
            Some((2024, 3))
        );
    }

    #[test]
    fn abstains_on_a_non_calendar_reporting_window() {
        // Synektik's fiscal year starts 1 October: `31.03.2026` is its H1 end, not
        // Q1. The window is stated, it disagrees with the calendar assumption, so
        // the derivation must abstain rather than guess (ADR 0061 decision 1).
        assert_eq!(
            period_from_title_url(
                "Sprawozdanie_finansowe_SYNEKTIK_SA_i_Grupy_Kapitalowej_SYNEKTIK_za_okres_01.10.2024-31.03.2025.pdf",
                ""
            ),
            None
        );
        // A calendar-year-to-date window is unambiguous and IS derived.
        assert_eq!(
            period_from_title_url("Sprawozdanie za okres 01.01.2025-30.06.2025.pdf", ""),
            Some((2025, "H1", "2025-06-30".to_owned()))
        );
    }

    #[test]
    fn abstains_when_no_period_is_derivable() {
        // Real bare attachment names: nothing in the title or URL names a period.
        assert_eq!(period_from_title_url("SSF.pdf", ""), None);
        assert_eq!(
            period_from_title_url("Benefit_Systems_SSF_Raport_signed.pdf", ""),
            None
        );
        // Two different calendar period ends in one string: ambiguous, never guessed.
        assert_eq!(
            period_from_title_url("Porownanie 30.06.2024 oraz 31.12.2024.pdf", ""),
            None
        );
    }

    // -----------------------------------------------------------------------
    // A1 — comparative balance-sheet header (v0.59 recall regression fix).
    //
    // A stored statement's SFP header pairs the current period-end with the prior
    // one — both calendar boundaries, current listed FIRST (and therefore later):
    // "31.03.2025 31.12.2024", or annual "31.12.2024 31.12.2023". This explicitly
    // names the period; the old code abstained (NonCalendarWindow) and lost it on
    // exactly the card-fc692da documents this milestone recovered.
    // -----------------------------------------------------------------------

    #[test]
    fn comparative_header_resolves_to_current_period() {
        // Quarter vs prior year-end → Q1 of the current (first, later) date.
        assert_eq!(
            period_sort_key("31.03.2025 31.12.2024", ""),
            Some((2025, 1))
        );
        assert_eq!(
            period_from_text("SPRAWOZDANIE Z SYTUACJI FINANSOWEJ 31.03.2025 31.12.2024"),
            Some((2025, "Q1", "2025-03-31".to_owned()))
        );
        // Annual comparative → FY of the current (first, later) date.
        assert_eq!(
            period_sort_key("31.12.2024 31.12.2023", ""),
            Some((2024, 4))
        );
        assert_eq!(
            period_from_text("Bilans 31.12.2024 31.12.2023"),
            Some((2024, "FY", "2024-12-31".to_owned()))
        );
    }

    #[test]
    fn comparative_guards_still_abstain_or_resolve_as_before() {
        // A NON-boundary first date is still a non-calendar window → abstain.
        assert_eq!(period_from_text("15.02.2025 31.12.2024"), None);
        // A calendar-year-to-date range opening 1 January resolves as before.
        assert_eq!(
            period_from_text("okres 01.01.2025 31.03.2025"),
            Some((2025, "Q1", "2025-03-31".to_owned()))
        );
        // A non-December fiscal-year window (Synektik) still abstains.
        assert_eq!(period_from_text("01.10.2024 31.03.2025"), None);
    }

    // -----------------------------------------------------------------------
    // Bug #325 — a day+month calendar marker pairs with ITS OWN nearby year,
    // not the first clean year anywhere in the (possibly extraction-mangled)
    // text.
    // -----------------------------------------------------------------------

    /// Real cover-page shape from the flagged DNP (Dino Polska) document
    /// (`202512 Dino Polska SF UoR.xhtml`, an html_positional pdf2htmlEX
    /// render): the reporting year is stated as "31 grudnia 20 25" — a stray
    /// space pdf2htmlEX's glyph-run rendering inserted mid-digit, TWICE on the
    /// cover page — while an unrelated signing dateline a few words later
    /// states "26 marca 2026" with a clean, unbroken year. Before the fix this
    /// derived fiscal_year=2026 / period_end=2026-12-31 (the exact wrong
    /// values recorded on the maintainer's DB); the correct period is
    /// FY 2025-12-31.
    #[test]
    fn positional_cover_text_pairs_the_year_next_to_its_own_month_marker() {
        let cover_text = "sprawozdanie finansowe za rok zakonczony dnia 31 grudnia 20 25 roku \
             Krotoszyn, dn. 26 marca 2026 roku \
             \u{201e}dino polska\u{201d} s.a. sprawozdanie finansowe za rok zakonczony \
             dnia 31 grudnia 202 5 roku";
        assert_eq!(
            period_from_text(cover_text),
            Some((2025, "FY", "2025-12-31".to_owned())),
            "the year next to '31 grudnia' must win over the clean-but-unrelated \
             signing-date year found later in the text"
        );
    }

    /// A month-year pairing that is broken beyond the one-space tolerance (or
    /// simply absent) still abstains rather than falling back to a
    /// wrong-but-clean year elsewhere — "never guess" holds even for this new
    /// pass. `marker_period`'s existing fallback (first standalone year +
    /// `parse_period`) still applies once `month_end_year` itself finds
    /// nothing, so a document naming ONLY a clean global year is unaffected
    /// (`period_from_text`'s many other passing tests already pin that).
    #[test]
    fn a_year_split_other_than_two_plus_two_next_to_the_month_marker_still_abstains_there() {
        // "202 5" (a 3+1 digit split — the OTHER real occurrence on this same
        // DNP cover page, "dnia 31 grudnia 202 5 roku") is not the single
        // clean-2+2-split shape the tolerance covers; nothing else in the text
        // names a year or an intra-year marker, so the whole derivation
        // abstains rather than guessing.
        assert_eq!(
            period_from_text("zakonczony dnia 31 grudnia 202 5 roku"),
            None
        );
    }

    /// Non-calendar fiscal year end (Synektik: 1 October – 30 September) with
    /// a CLEAN year must resolve exactly as before this fix — the new
    /// month+year pairing only changes WHICH year a match uses, never which
    /// index wins or introduces a new interpretation. Mapping a bare Sep-30
    /// date to period_type `Q3` (rather than a non-calendar FY) is a
    /// pre-existing grammar limitation, unrelated to bug #325.
    #[test]
    fn synektik_like_non_calendar_year_end_with_a_clean_year_is_unaffected() {
        assert_eq!(
            period_from_text("sprawozdanie finansowe zakonczone dnia 30 wrzesnia 2025 roku"),
            Some((2025, "Q3", "2025-09-30".to_owned())),
            "a clean year next to the month marker must resolve the same as \
             the pre-existing (year-anywhere) grammar did"
        );
    }

    /// An explicit annual/fiscal-year MARKER ("rok obrotowy") still wins over
    /// a bare day+month date, unaffected by this fix — the intra-year INDEX
    /// keeps its existing precedence (`parse_period` checked first, unchanged
    /// by `month_end_year_for_index`, which only ever supplies a YEAR for
    /// whichever index that precedence already settled on). This is what lets
    /// a non-calendar filer's "rok obrotowy zakończony 30 września" name its
    /// annual marker rather than being misread as a mere Q3 quarter.
    #[test]
    fn an_explicit_annual_marker_still_overrides_a_bare_month_end_date() {
        assert_eq!(
            period_from_text("sprawozdanie za rok obrotowy zakonczony 30 wrzesnia 2025 roku"),
            Some((2025, "FY", "2025-12-31".to_owned())),
            "'rok obrotowy' must still win the index over '30 wrzesnia', \
             exactly as before this fix"
        );
    }

    /// Adversarial-review blocker #5, tightening bug #325's fix: an EXPLICIT
    /// annual marker ("rok obrotowy") settles the index at FY — but the text
    /// ALSO happens to mention an unrelated December date ("31 grudnia 2024",
    /// e.g. a comparative or otherwise unrelated calendar-year figure) with
    /// its OWN nearby year, different from the true fiscal year end stated
    /// next to September. `month_end_year_for_index` must NOT donate that
    /// December marker's year to the "rok obrotowy" index — that pairing
    /// never came from the December marker in the first place. Before this
    /// fix the December mention silently won, resolving FY2024 (wrong); the
    /// correct resolution keeps the pre-#325 fallback (first standalone year
    /// anywhere), landing on 2025 — the year actually stated next to the
    /// real (non-calendar) fiscal year end.
    #[test]
    fn an_explicit_annual_marker_ignores_an_unrelated_december_mentions_year() {
        assert_eq!(
            period_from_text(
                "rok obrotowy zakonczony 30 wrzesnia 2025 roku, \
                 poprzedni rok obrotowy zakonczony 31 grudnia 2024 roku"
            ),
            Some((2025, "FY", "2025-12-31".to_owned())),
            "the 'rok obrotowy' index must resolve the YEAR via the pre-#325 \
             fallback (first standalone year anywhere), never via the \
             unrelated '31 grudnia 2024' marker's own nearby year"
        );
    }

    // -----------------------------------------------------------------------
    // C3 — ESPI feed-title carrier: markers win over a publication date.
    //
    // Feed titles state their period with an explicit marker and often carry a
    // "z dnia <date>" PUBLICATION reference — never the reporting period. The
    // marker must win, and the publication date must be stripped so it cannot be
    // mistaken for a balance-sheet date on the no-marker fall-through.
    // -----------------------------------------------------------------------

    #[test]
    fn feed_title_marker_wins_over_publication_date() {
        // Marker present + publication date on a boundary → the marker's period.
        assert_eq!(
            period_from_feed_title("Raport okresowy za I kwartał 2025 z dnia 30.06.2025", ""),
            Some((2025, "Q1", "2025-03-31".to_owned()))
        );
        // No marker, only the publication date → stripped, so it abstains rather
        // than mis-keying to that date's calendar period.
        assert_eq!(
            period_from_feed_title("Skonsolidowany raport z dnia 30.06.2025", ""),
            None
        );
        // A bare boundary date with NO marker and NO publication idiom is still the
        // period — feed-title mode must not regress the lone-date resolution.
        assert_eq!(
            period_from_feed_title("SSF_PZU_SA_Raport_30.06.2025.pdf", ""),
            Some((2025, "H1", "2025-06-30".to_owned()))
        );
    }
}

#[cfg(test)]
mod proptests {
    //! Invariant (property-based) coverage of the pure classification/period
    //! transforms (ADR 0049): `parse_year`'s byte-window digit scan panicked on
    //! multi-byte chars shortly after a digit (see
    //! `parse_year_does_not_panic_on_multibyte_char_after_digit_run`) — the same bug
    //! class as `signal_dates::find_date_for_labels`. Arbitrary title/url text,
    //! including unicode, must never panic.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn classify_statement_never_panics(title in ".*", url in ".*") {
            let _ = classify_statement(&title, &url);
        }

        #[test]
        fn period_sort_key_never_panics(title in ".*", url in ".*") {
            let _ = period_sort_key(&title, &url);
        }

        #[test]
        fn period_label_never_panics(title in ".*", url in ".*") {
            let _ = period_label(&title, &url);
        }
    }
}
