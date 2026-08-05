//! Deterministic management-holdings section parser. See the module doc.
//!
//! Real GPW filings taught the same hard lessons the ownership parser learned,
//! plus a few of their own, all handled by *general* mechanisms (never
//! per-company rules):
//!
//! 1. **Word-per-line fragmentation.** Some xhtml exports put each token on its
//!    own line (ACP). Detected by a single-token-line ratio; the whole document
//!    is then *reflowed* by re-segmenting a flat token stream at alpha↔digit
//!    transitions, so a fragmented heading and a fragmented "31 458" both
//!    reconstruct before anchoring.
//! 2. **Glyph guard first.** Custom-font digits mapped into the Unicode PUA
//!    (CDR half-year) yield [`MgmtHoldingsState::GlyphEncoded`] — the deterministic
//!    tier refuses rather than emit blanks, parking for the OCR path.
//! 3. **Anchoring needs a holdings token co-occurring with an organ phrase.**
//!    Board-composition (`skład osobowy`), remuneration (`wynagrodzenia`), and
//!    diversity-policy (`polityka różnorodności`) sections name the same organs
//!    but are not holdings tables; a TOC line (dotted leaders / a trailing bare
//!    page number) is furniture, not the section.
//! 4. **One unified row assembler.** Cell-stream (one cell per line), positional
//!    multi-date columns, role-inline name cells, and organ subheaders converge
//!    on the same person→role→numbers→note pipeline.
//! 5. **Conservative shares.** A row's `shares` is emitted only when its numeric
//!    cells carry exactly one distinct readable integer (all-equal columns, or a
//!    single column); a disagreeing multi-column row yields `None` — never a
//!    guessed as-of figure. A dash / `nd.` / `nie dotyczy` cell is absent, not `0`.
//! 6. **Prose zero is signal.** "nie posiadają akcji" (per-organ or aggregate)
//!    is an explicit [`MgmtHoldingsState::ZeroHoldingAggregate`] zero picture.
//! 7. **Subsidiary tables are excluded.** A related-entity holdings heading
//!    (`akcje jednostek powiązanych …`) terminates the issuer's own table.

use std::sync::OnceLock;

use regex::Regex;

use crate::report_diff::extraction::{Section, SourceFormat};

/// A board organ a holdings row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MgmtRole {
    Management,
    Supervisory,
}

impl MgmtRole {
    pub fn as_str(self) -> &'static str {
        match self {
            MgmtRole::Management => "management",
            MgmtRole::Supervisory => "supervisory",
        }
    }
}

/// One parsed by-person holdings row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MgmtHoldingRow {
    pub person_raw: String,
    pub role: Option<MgmtRole>,
    /// Decimal-exact share count. `Some("0")` is a real zero holding; `None` means
    /// the figure was stated but not deterministically recoverable (never guessed).
    pub shares: Option<String>,
    pub indirect_via_raw: Option<String>,
    pub prior_shares: Option<String>,
    pub prior_as_of: Option<String>,
}

/// A prose zero-holding statement for one organ (or the whole board when
/// `role` is `None`) — an explicit zero picture, not a table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroOrgan {
    pub role: Option<MgmtRole>,
}

/// Outcome state of a parse attempt (mirrors ownership extraction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MgmtHoldingsState {
    /// A holdings section was located and at least one person row parsed.
    Parsed,
    /// A prose "nie posiadają akcji" statement (per-organ or aggregate) — an
    /// explicit zero picture, no by-person rows.
    ZeroHoldingAggregate,
    /// The text layer is glyph-mangled (custom font in the Unicode PUA) — an
    /// AI/OCR-tier residual, never guessed.
    GlyphEncoded,
    /// No holdings anchor located (only false anchors, or none at all). When
    /// `matched_heading` is `Some`, an anchor was found but no row parsed
    /// (`table_unparsable` at the job seam).
    SectionMissing,
}

/// Full result of one [`parse_management_holdings`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MgmtHoldingsOutcome {
    pub state: MgmtHoldingsState,
    pub matched_heading: Option<String>,
    pub rows: Vec<MgmtHoldingRow>,
    pub zero_organs: Vec<ZeroOrgan>,
}

/// How many lines after an anchor form its candidate block.
const WINDOW_LINES: usize = 150;
/// Most anchors considered per document.
const MAX_ANCHORS: usize = 60;
/// A word-per-line document has at least this many single-token lines fraction.
/// Set high so only *extreme* fragmentation (ACP, ~1 token/line) reflows — a
/// cell-stream table (2-token name / role cells) must stay per-line.
const WORD_PER_LINE_RATIO: f64 = 0.75;

/// Parse the management-holdings section out of a report's ordered text sections.
/// Deterministic and panic-free for arbitrary input.
pub fn parse_management_holdings(
    sections: &[Section],
    _format: SourceFormat,
) -> MgmtHoldingsOutcome {
    let raw = flatten(sections);
    let lines = if is_word_per_line(&raw) {
        reflow_whole(&raw)
    } else {
        raw
    };

    let anchors: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_holdings_anchor(l))
        .map(|(i, _)| i)
        .take(MAX_ANCHORS)
        .collect();

    // Glyph guard: a mangled text layer with a real anchor is an OCR residual.
    if !anchors.is_empty() && is_glyph_encoded(&lines) {
        return MgmtHoldingsOutcome {
            state: MgmtHoldingsState::GlyphEncoded,
            matched_heading: Some(lines[anchors[0]].trim().to_owned()),
            rows: Vec::new(),
            zero_organs: Vec::new(),
        };
    }

    // Assemble every anchor's window into a block (each bounded by the prose-stop /
    // subsidiary-heading terminators), then merge, dedup by normalized name in
    // document order (first wins). Multi-anchor collection is what recovers tables
    // split across a caption + heading + organ subtables (PKO/KRU/SNT); the
    // per-block terminators keep report narrative out of each block.
    // Collect every anchor's block, then dedup by normalized name keeping the
    // INFO-RICHEST row per person (a clean holdings-table row carries shares + role;
    // a narrative/notification mention carries neither and must not win). First-seen
    // order is preserved for output stability.
    let mut order: Vec<String> = Vec::new();
    let mut best_by_person: std::collections::HashMap<String, MgmtHoldingRow> =
        std::collections::HashMap::new();
    let mut anchor_heading: Option<String> = None;
    for &idx in &anchors {
        let end = (idx + 1 + WINDOW_LINES).min(lines.len());
        let initial = organ_from_heading(&lines[idx]);
        let block = assemble(&lines[idx + 1..end], initial);
        if anchor_heading.is_none() && !block.is_empty() {
            anchor_heading = Some(lines[idx].trim().to_owned());
        }
        for row in block {
            let key = row
                .person_raw
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_uppercase();
            match best_by_person.get(&key) {
                None => {
                    order.push(key.clone());
                    best_by_person.insert(key, row);
                }
                Some(existing) if row_info_score(&row) > row_info_score(existing) => {
                    best_by_person.insert(key, row);
                }
                Some(_) => {}
            }
        }
    }
    let rows: Vec<MgmtHoldingRow> = order
        .into_iter()
        .filter_map(|k| best_by_person.remove(&k))
        .collect();

    if !rows.is_empty() {
        return MgmtHoldingsOutcome {
            state: MgmtHoldingsState::Parsed,
            matched_heading: anchor_heading
                .or_else(|| anchors.first().map(|&i| lines[i].trim().to_owned())),
            rows,
            zero_organs: Vec::new(),
        };
    }

    // No by-person rows: a prose zero-holding statement is still an explicit zero.
    let zero_organs = collect_zero_organs(&lines);
    if !zero_organs.is_empty() {
        return MgmtHoldingsOutcome {
            state: MgmtHoldingsState::ZeroHoldingAggregate,
            matched_heading: anchors.first().map(|&i| lines[i].trim().to_owned()),
            rows: Vec::new(),
            zero_organs,
        };
    }

    // An anchor without rows and without a zero statement is a table the
    // deterministic tier could not read (image / displaced) — carry the heading so
    // the job records `table_unparsable`; no anchor at all is `section_missing`.
    MgmtHoldingsOutcome {
        state: MgmtHoldingsState::SectionMissing,
        matched_heading: anchors.first().map(|&i| lines[i].trim().to_owned()),
        rows: Vec::new(),
        zero_organs: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Text normalization
// ---------------------------------------------------------------------------

fn flatten(sections: &[Section]) -> Vec<String> {
    let mut lines = Vec::new();
    for section in sections {
        if section.heading != "<preamble>" {
            lines.push(section.heading.clone());
        }
        for line in section.body.lines() {
            lines.push(line.to_owned());
        }
    }
    lines
}

/// Whitespace-removed, lowercased form — the substrate for keyword matching.
fn deflate(line: &str) -> String {
    line.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// Custom-font detection (copied from the ownership parser — same PUA threshold).
fn is_glyph_encoded(lines: &[String]) -> bool {
    let mut pua = 0usize;
    let mut total = 0usize;
    for line in lines {
        for c in line.chars() {
            if !c.is_whitespace() {
                total += 1;
                if ('\u{E000}'..='\u{F8FF}').contains(&c) {
                    pua += 1;
                }
            }
        }
    }
    pua > 500 && total > 0 && (pua as f64 / total as f64) > 0.02
}

/// Is the document word-per-line fragmented (each token on its own line)?
fn is_word_per_line(lines: &[String]) -> bool {
    let mut single = 0usize;
    let mut nonempty = 0usize;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        nonempty += 1;
        if !trimmed.chars().any(char::is_whitespace) {
            single += 1;
        }
    }
    nonempty >= 8 && (single as f64 / nonempty as f64) > WORD_PER_LINE_RATIO
}

/// Reflow a fragmented token stream back into logical cell lines: re-segment at
/// alpha↔digit transitions, so an alpha run (name + role) and a digit run (a
/// number or a numeric column) each land on one line.
fn reflow_whole(lines: &[String]) -> Vec<String> {
    let tokens: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_digit: Option<bool> = None;
    let mut prev_lower = false;
    for token in tokens {
        let first = token.chars().next();
        let is_digit = first.is_some_and(|c| c.is_ascii_digit());
        let is_cap = first.is_some_and(|c| c.is_uppercase());
        let is_lower_alpha = first.is_some_and(|c| c.is_lowercase() && c.is_alphabetic());
        // Break at an alpha↔digit transition, or before a proper noun that follows
        // a lowercase word (a new name segment begins after a heading / prose tail).
        let break_here = match current_digit {
            None => false,
            Some(prev_digit) => prev_digit != is_digit || (!is_digit && is_cap && prev_lower),
        };
        if break_here && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(token);
        current_digit = Some(is_digit);
        if !is_digit {
            prev_lower = is_lower_alpha;
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

// ---------------------------------------------------------------------------
// Anchors
// ---------------------------------------------------------------------------

fn is_holdings_anchor(line: &str) -> bool {
    let d = deflate(line);
    if d.len() > 400 || d.is_empty() {
        return false;
    }
    if is_toc_line(line) || is_false_anchor(&d) || is_related_entity_heading(&d) {
        return false;
    }
    has_holdings_token(&d) && has_organ_token(&d)
}

/// A holdings token: the section is about *holding* shares, not composing a board
/// or paying it.
fn has_holdings_token(d: &str) -> bool {
    const TOKENS: [&str; 8] = [
        "posiadaniaakcji",
        "posiadaniuakcji",
        "wposiadaniu",
        "stanposiadania",
        "posiadaneprzez",
        "akcjewposiadaniu",
        "akcjespółki",
        "zestawieniestanuposiadania",
    ];
    TOKENS.iter().any(|t| d.contains(t))
}

/// An organ token: the holders are managing/supervising persons.
fn has_organ_token(d: &str) -> bool {
    const TOKENS: [&str; 7] = [
        "zarządzając",
        "nadzorując",
        "członkówzarządu",
        "członkówrady",
        "radynadzorczej",
        "władzbanku",
        "osóbzarządzających",
    ];
    TOKENS.iter().any(|t| d.contains(t))
}

/// Board-composition / remuneration / diversity-policy sections name the same
/// organs but disclose no holdings.
fn is_false_anchor(d: &str) -> bool {
    const TOKENS: [&str; 7] = [
        "składosobowy",
        "wynagrodz",
        "różnorodnoś",
        "roznorodnos",
        "regulaminwynagr",
        "opispolityki",
        "zasadydziałania",
    ];
    TOKENS.iter().any(|t| d.contains(t))
}

/// A table-of-contents entry: dotted leaders, or an anchor phrase with a trailing
/// bare page number.
fn is_toc_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.contains("....") || trimmed.contains(". . .") {
        return true;
    }
    // Trailing " <page>" after the phrase (a heading has no trailing number).
    trailing_page_regex().is_match(trimmed)
}

fn trailing_page_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[A-Za-zżźćńółęąśŻŹĆŃÓŁĘĄŚ]\s+\d{1,4}\s*$").expect("valid regex"))
}

/// A related-entity holdings heading (subsidiary shares) — terminates the
/// issuer's own table.
fn is_related_entity_heading(d: &str) -> bool {
    (d.contains("jednostekpowiązanych")
        || d.contains("jednostkachpowiązanych")
        || d.contains("podmiotówpowiązanych")
        || d.contains("spółkachzależnych")
        || d.contains("spółkachpowiązanych"))
        && (d.contains("akcj") || d.contains("posiadan") || d.contains("udział"))
}

/// The organ a single-organ table caption implies (both organs → `None`).
fn organ_from_heading(line: &str) -> Option<MgmtRole> {
    let d = deflate(line);
    let mgmt = d.contains("zarząd") || d.contains("zarządzając") || d.contains("władzbanku");
    let sup = d.contains("nadzor");
    match (mgmt, sup) {
        (true, false) => Some(MgmtRole::Management),
        (false, true) => Some(MgmtRole::Supervisory),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Zero-holding statements
// ---------------------------------------------------------------------------

/// Collect the distinct organ(s) a document declares as holding zero, from prose
/// "nie posiada(ją) akcji" statements. An aggregate statement (both organs, or
/// neither disambiguated) yields a single `role: None` marker.
fn collect_zero_organs(lines: &[String]) -> Vec<ZeroOrgan> {
    let mut mgmt = false;
    let mut sup = false;
    let mut aggregate = false;
    for line in lines {
        let d = deflate(line);
        if !is_zero_statement(&d) {
            continue;
        }
        let has_mgmt = d.contains("zarządzając") || d.contains("osobyzarządzające");
        let has_sup = d.contains("nadzorując") || d.contains("osobynadzorujące");
        if has_mgmt && has_sup {
            aggregate = true;
        } else if has_sup {
            sup = true;
        } else if has_mgmt {
            mgmt = true;
        } else {
            aggregate = true;
        }
    }
    let mut organs = Vec::new();
    if aggregate && !(mgmt || sup) {
        organs.push(ZeroOrgan { role: None });
        return organs;
    }
    if aggregate {
        // A mixed document that also named organs individually: keep the explicit
        // per-organ markers plus the aggregate is redundant — prefer per-organ.
    }
    if sup {
        organs.push(ZeroOrgan {
            role: Some(MgmtRole::Supervisory),
        });
    }
    if mgmt {
        organs.push(ZeroOrgan {
            role: Some(MgmtRole::Management),
        });
    }
    if organs.is_empty() && aggregate {
        organs.push(ZeroOrgan { role: None });
    }
    organs
}

fn is_zero_statement(d: &str) -> bool {
    const TOKENS: [&str; 6] = [
        "nieposiadająakcji",
        "nieposiadałyakcji",
        "nieposiadaliakcji",
        "nieposiadaakcji",
        "nieposiadałakcji",
        "nieposiadałaakcji",
    ];
    // Require an organ reference so an unrelated "nie posiada akcji" elsewhere is
    // not mistaken for the board statement.
    TOKENS.iter().any(|t| d.contains(t))
        && (d.contains("zarządzając")
            || d.contains("nadzorując")
            || d.contains("osobyzarządzające")
            || d.contains("osobynadzorujące"))
}

// ---------------------------------------------------------------------------
// Row assembler
// ---------------------------------------------------------------------------

/// A pending row accumulator.
struct Pending {
    person_raw: String,
    role: Option<MgmtRole>,
    numbers: Vec<NumCell>,
    indirect: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
enum NumCell {
    Value(String),
    Absent,
}

fn assemble(window: &[String], initial_organ: Option<MgmtRole>) -> Vec<MgmtHoldingRow> {
    let mut organ = initial_organ;
    let mut pending: Option<Pending> = None;
    let mut rows: Vec<MgmtHoldingRow> = Vec::new();

    for raw_line in window {
        let line = strip_row_ordinal(raw_line);
        let line = line.as_str();
        let d = deflate(line);
        if d.is_empty() {
            continue;
        }
        // A subsidiary-share heading ends the issuer's own table.
        if is_related_entity_heading(&d) {
            flush(&mut pending, &mut rows);
            break;
        }
        // Organ subheader → switch context, close any pending row.
        if let Some(role) = organ_subheader(&d) {
            flush(&mut pending, &mut rows);
            organ = Some(role);
            continue;
        }
        // A footnote / note line carrying the indirect vehicle attaches to the
        // most recent row.
        if let Some(vehicle) = indirect_vehicle(line) {
            if let Some(p) = pending.as_mut() {
                if p.indirect.is_none() {
                    p.indirect = Some(vehicle);
                }
            } else if let Some(last) = rows.last_mut() {
                if last.indirect_via_raw.is_none() {
                    last.indirect_via_raw = Some(vehicle);
                }
            }
            continue;
        }
        // A person line opens a new row.
        if let Some((name, inline_role)) = person_name(line) {
            flush(&mut pending, &mut rows);
            let nums = lex_numbers(line);
            pending = Some(Pending {
                person_raw: name,
                role: inline_role.or(organ),
                numbers: nums,
                indirect: None,
            });
            continue;
        }
        // Otherwise: numeric cells / role-only line for the pending row.
        if let Some(p) = pending.as_mut() {
            let nums = lex_numbers(line);
            if !nums.is_empty() {
                p.numbers.extend(nums);
            } else if p.role.is_none() {
                if let Some(role) = role_from_line(&d) {
                    p.role = Some(role);
                }
            }
        }
    }
    flush(&mut pending, &mut rows);
    rows
}

/// Strip a leading table row-ordinal ("1 Szymon Midera …", "1) …") — a number
/// that precedes an alpha token, so it is neither a name nor a share cell. A
/// leading number followed by another number (a split thousands group like
/// "3 366 250") is left intact.
fn strip_row_ordinal(line: &str) -> String {
    let mut tokens = line.split_whitespace();
    let Some(first) = tokens.next() else {
        return line.to_owned();
    };
    let core = first.trim_end_matches(['.', ')', ':']);
    let is_ordinal =
        !core.is_empty() && core.len() <= 3 && core.chars().all(|c| c.is_ascii_digit());
    if is_ordinal {
        if let Some(second) = tokens.clone().next() {
            if second.chars().next().is_some_and(char::is_alphabetic) {
                return tokens.collect::<Vec<_>>().join(" ");
            }
        }
    }
    line.to_owned()
}

/// Information richness of a row, for dedup: a clean holdings-table row has both a
/// readable share value and a role; a bare narrative mention has neither.
fn row_info_score(row: &MgmtHoldingRow) -> u8 {
    u8::from(row.shares.is_some()) * 2
        + u8::from(row.role.is_some())
        + u8::from(row.indirect_via_raw.is_some())
}

fn flush(pending: &mut Option<Pending>, rows: &mut Vec<MgmtHoldingRow>) {
    if let Some(p) = pending.take() {
        // A person row must carry at least one numeric/dash cell — a table row
        // always does. A bare capitalized phrase with no nearby figure (a company
        // name, a section title) is furniture, not a holder.
        if p.numbers.is_empty() {
            return;
        }
        let person_raw = strip_title_prefix(&p.person_raw);
        let shares = shares_from(&p.numbers);
        // Person-plausibility gate (F-A2): a name that is a company / legal
        // form / counterparty / street address / office / role-boilerplate
        // string, a genitive-case surname fragment, or a YEAR mis-captured as
        // a share count is NEVER a natural person — drop the row rather than
        // emit junk into the founder-stamping substrate.
        if is_implausible_person(&person_raw, shares.as_deref()) {
            return;
        }
        rows.push(MgmtHoldingRow {
            person_raw,
            role: p.role,
            shares,
            indirect_via_raw: p.indirect,
            prior_shares: None,
            prior_as_of: None,
        });
    }
}

/// Polish diacritic fold + uppercase for a single name token, so the
/// implausible-token sets match regardless of case/diacritics ("Państwa" →
/// "PANSTWA", "Sp" → "SP").
fn fold_upper(token: &str) -> String {
    token
        .chars()
        .map(|c| match c {
            'ą' | 'Ą' => 'A',
            'ć' | 'Ć' => 'C',
            'ę' | 'Ę' => 'E',
            'ł' | 'Ł' => 'L',
            'ń' | 'Ń' => 'N',
            'ó' | 'Ó' => 'O',
            'ś' | 'Ś' => 'S',
            'ż' | 'Ż' | 'ź' | 'Ź' => 'Z',
            other => other,
        })
        .flat_map(char::to_uppercase)
        .collect()
}

/// Courtesy-title tokens a table sometimes prefixes to a name ("Pan Wojciech
/// Niesyto") — never part of the identity; stripped from the leading token(s).
const PERSON_TITLE_PREFIXES: &[&str] = &["PAN", "PANI", "PANA", "PANIA", "PANNA"];

/// Legal-form / company suffix tokens (whole-token). A name carrying one is an
/// entity, not a person ("Sales Limited", "Rekman Sp.").
const LEGAL_FORM_TOKENS: &[&str] = &[
    "SP", "SPZOO", "SPK", "SA", "LTD", "LIMITED", "GMBH", "PLC", "INC", "SARL", "LLC", "OYJ", "BV",
    "NV",
];

/// Street / estate address abbreviations — rejected only as the FIRST token
/// ("ul. Europejska" → "Ul Europejska").
const ADDRESS_TOKENS: &[&str] = &["UL", "AL", "OS", "ULICA", "ALEJA", "OSIEDLE"];

/// Institution / state / office / role-boilerplate / generic-business tokens a
/// holdings-adjacent table can capitalize — none is part of a natural-person name.
/// Conservative on purpose: every entry is a word no Polish surname or given name
/// is, so the ground-truth person-recall floor (≥95%) is never at risk.
const NON_PERSON_TOKENS: &[&str] = &[
    // State / institution.
    "SKARB",
    "PANSTWA",
    "PANSTWO",
    "GMINA",
    "MINISTERSTWO",
    "URZAD",
    "IZBA",
    "IZBY",
    // Office / role-boilerplate.
    "DYREKTOR",
    "KIEROWNIK",
    "NACZELNIK",
    "ZLECENIODAWCA",
    "WYSTAWCA",
    "NABYWCA",
    "ZBYWCA",
    "POSIADACZ",
    "EMITENT",
    // Generic company / brand words.
    "MARKETING",
    "TRADING",
    "LOGISTICS",
    "DISTRIBUTION",
    "SALES",
    "GROUP",
    "HOLDING",
    "TECHNOLOGIES",
    "SOLUTIONS",
    "SYSTEMS",
    "CAPITAL",
    "INTERNATIONAL",
];

/// Accounting / reporting-domain stems (prefix-matched, so every Polish inflection
/// is covered): a prose fragment like "…Międzynarodowymi Standardami
/// Sprawozdawczości Finansowej…" is title-cased and swept up as a "name" near a
/// figure. No Polish personal name begins with any of these stems, so a prefix
/// hit is an unambiguous non-person (real-corpus harvest: SNT, F-A2 2026-07-17).
const NON_PERSON_PREFIXES: &[&str] = &[
    "SPRAWOZDAW",
    "FINANSOW",
    "STANDARD",
    "RACHUNKOW",
    "SKONSOLIDOW",
    "JEDNOSTKOW",
    "MIEDZYNARODOW",
    // Role/section boilerplate in any declension ("Stanowiska Dyrektorów" —
    // the PZU section-heading leak the corpus junk harness caught).
    "STANOWISK",
    "DYREKTOR",
];

/// Strip leading courtesy-title tokens from an assembled name.
fn strip_title_prefix(name: &str) -> String {
    let tokens: Vec<&str> = name.split_whitespace().collect();
    let mut start = 0;
    while start < tokens.len()
        && PERSON_TITLE_PREFIXES.contains(&fold_upper(tokens[start]).as_str())
    {
        start += 1;
    }
    tokens[start..].join(" ")
}

/// Whether an assembled name is NOT a natural person. Conservative: it fires only
/// on shapes that are unmistakably an entity / address / office / role phrase, a
/// genitive-case surname fragment, or a mis-parsed year — so real holders are
/// never dropped (the ≥95% ground-truth person-recall floor is the guard).
pub(crate) fn is_implausible_person(name: &str, shares: Option<&str>) -> bool {
    let tokens: Vec<String> = name.split_whitespace().map(fold_upper).collect();
    // A single surviving token is not a person name (mirrors `person_name`'s
    // 2-token minimum, now applied after title-prefix stripping).
    if tokens.len() < 2 {
        return true;
    }
    // A legal-form / company suffix or fragment anywhere in the name.
    if tokens
        .iter()
        .any(|t| LEGAL_FORM_TOKENS.contains(&t.as_str()))
    {
        return true;
    }
    // A street/estate address as the leading token.
    if ADDRESS_TOKENS.contains(&tokens[0].as_str()) {
        return true;
    }
    // An institution / office / role-boilerplate / generic-business token anywhere.
    if tokens
        .iter()
        .any(|t| NON_PERSON_TOKENS.contains(&t.as_str()))
    {
        return true;
    }
    // An accounting / reporting-domain stem anywhere (prose fragment swept up).
    if tokens
        .iter()
        .any(|t| NON_PERSON_PREFIXES.iter().any(|p| t.starts_with(p)))
    {
        return true;
    }
    // A genitive-case surname fragment ("… Wnorowskiego …") — a prose mention swept
    // up as a name. Adjectival Polish surnames are `-ski/-cki/-i` in the nominative;
    // an `-ego`/`-iego` tail is the genitive/accusative, i.e. not a table's own row.
    if tokens
        .iter()
        .any(|t| t.chars().count() > 4 && (t.ends_with("IEGO") || t.ends_with("EGO")))
    {
        return true;
    }
    // A YEAR mis-captured as a share count ("APPLE REALME OPPO … 2024"): a share
    // value inside the plausible-year band on a 3+-token all-ASCII (no Polish
    // diacritics) name is a product/counterparty row, not a holder. Two-token
    // Polish names — the overwhelming majority of real holders — are never touched,
    // so a genuine person holding e.g. 2024 shares stays.
    if let Some(year) = shares.and_then(|s| s.parse::<i64>().ok()) {
        if (1990..=2035).contains(&year) && tokens.len() >= 3 && name.is_ascii() {
            return true;
        }
    }
    false
}

/// The unambiguous share value of a row's numeric cells: exactly one distinct
/// readable integer → that value; empty or disagreeing → `None` (never guessed).
fn shares_from(cells: &[NumCell]) -> Option<String> {
    let mut distinct: Vec<&str> = Vec::new();
    for cell in cells {
        if let NumCell::Value(v) = cell {
            if !distinct.contains(&v.as_str()) {
                distinct.push(v);
            }
        }
    }
    if distinct.len() == 1 {
        Some(distinct[0].to_owned())
    } else {
        None
    }
}

/// Is this deflated line exactly an organ subheader ("Zarząd" / "Rada Nadzorcza")?
fn organ_subheader(d: &str) -> Option<MgmtRole> {
    match d {
        "zarząd" | "zarzad" | "członkowiezarządu" | "osobyzarządzające" => {
            Some(MgmtRole::Management)
        }
        "radanadzorcza" | "radanadzorczej" | "członkowieradynadzorczej" | "osobynadzorujące" => {
            Some(MgmtRole::Supervisory)
        }
        _ => None,
    }
}

/// Role keywords → organ, for a role-only line or the tail of a person cell.
fn role_from_line(d: &str) -> Option<MgmtRole> {
    let sup = d.contains("radanadzorcz")
        || d.contains("radynadzorczej")
        || d.contains("przewodnicząc") && d.contains("nadzor")
        || d.contains("nadzorczej");
    let mgmt = d.contains("prezeszarządu")
        || d.contains("zarządu")
        || d.contains("wiceprezes")
        || d.contains("prezes")
        || d.contains("chief")
        || d.contains("officer");
    // Supervisory keywords are more specific; check them first.
    if sup {
        Some(MgmtRole::Supervisory)
    } else if mgmt {
        Some(MgmtRole::Management)
    } else {
        None
    }
}

/// Role/organ/header stopwords a name token must never be (lowercased).
fn is_stopword(token_lower: &str) -> bool {
    const WORDS: &[&str] = &[
        "prezes",
        "wiceprezes",
        "przewodniczący",
        "przewodnicząca",
        "wiceprzewodniczący",
        "wiceprzewodnicząca",
        "współprzewodniczący",
        "współprzewodnicząca",
        "członek",
        "członkini",
        "członkowie",
        "zarząd",
        "zarządu",
        "rada",
        "rady",
        "nadzorcza",
        "nadzorczej",
        "chief",
        "executive",
        "financial",
        "operating",
        "joint",
        "officer",
        "imię",
        "nazwisko",
        "stanowisko",
        "funkcja",
        "liczba",
        "akcji",
        "wartość",
        "udział",
        "kapitale",
        "głosów",
        "stan",
        "posiadania",
        "wynagrodzenie",
        "rok",
        "razem",
        "ogółem",
        "pozostali",
        // Organizational / boilerplate phrase heads that a table can capitalize —
        // never part of a natural-person name (real-corpus false positives).
        "grupa",
        "grupy",
        "kapitałowa",
        "kapitałowej",
        "spółka",
        "spółki",
        "spółek",
        "spółką",
        "bank",
        "banku",
        "polskiego",
        "walne",
        "zgromadzenie",
        "praktyki",
        "dobre",
        "portfel",
        "nabycie",
        "zbycie",
        "strategia",
        "tabela",
        "nominalna",
        "fundacja",
        "rodzinna",
        "warranty",
        "subskrypcyjne",
        "źródło",
        "notyfikacji",
        "zestawienie",
    ];
    WORDS.contains(&token_lower)
}

/// Extract a person name (2–3 leading capitalized non-stopword tokens) and an
/// inline role from a line. Returns `None` when the line is not a person cell.
fn person_name(line: &str) -> Option<(String, Option<MgmtRole>)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut name_tokens: Vec<String> = Vec::new();
    for token in &tokens {
        let cleaned: String = token.trim_matches(|c: char| !c.is_alphabetic()).to_owned();
        if cleaned.is_empty() {
            break;
        }
        let lower = cleaned.to_lowercase();
        let first_upper = cleaned.chars().next().is_some_and(|c| c.is_uppercase());
        let all_alpha = cleaned.chars().all(|c| c.is_alphabetic() || c == '-');
        // Reject an all-uppercase token (an acronym / ticker / SHOUTED heading like
        // "PKO", "PLN", "ZACIĄGNIĘTE") — a person name is title-case.
        let all_upper = cleaned.chars().filter(|c| c.is_alphabetic()).count() >= 2
            && cleaned
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_uppercase);
        if first_upper
            && all_alpha
            && !all_upper
            && !is_stopword(&lower)
            && cleaned.chars().count() > 1
        {
            name_tokens.push(cleaned);
            if name_tokens.len() == 3 {
                break;
            }
        } else {
            break;
        }
    }
    if name_tokens.len() < 2 {
        return None;
    }
    let name = name_tokens.join(" ");
    let rest = deflate(&tokens[name_tokens.len().min(tokens.len())..].join(" "));
    let role = role_from_line(&rest);
    Some((name, role))
}

/// Lex the integer share-count cells of a line. Decimals (percentages, nominal
/// values with a comma) are ignored; dashes / `nd.` / `nie dotyczy` are `Absent`.
fn lex_numbers(line: &str) -> Vec<NumCell> {
    let mut cells: Vec<NumCell> = Vec::new();
    let d = deflate(line);
    let has_absent = d.contains("niedotyczy") || d == "nd." || d == "nd" || d.contains("brakakcji");
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 0usize;
    while i < tokens.len() {
        let token = tokens[i];
        // A dash cell.
        if token == "-" || token == "–" || token == "—" || token == "nd." || token == "nd" {
            cells.push(NumCell::Absent);
            i += 1;
            continue;
        }
        // A decimal (percentage / comma-nominal) → not a share count.
        if is_decimal(token) {
            i += 1;
            continue;
        }
        if is_int(token) {
            // Collect the maximal run of pure-integer tokens, then split it into
            // numbers (see `split_number_run` — repeated same-value columns like
            // "692 640 692 640 692 640" must not merge into one giant number).
            let start = i;
            while i < tokens.len() && is_int(tokens[i]) {
                i += 1;
            }
            for number in split_number_run(&tokens[start..i]) {
                cells.push(NumCell::Value(number));
            }
            continue;
        }
        i += 1;
    }
    if cells.is_empty() && has_absent {
        cells.push(NumCell::Absent);
    }
    cells
}

/// Split a run of space-separated integer groups into share-count numbers.
///
/// Space is both the Polish thousands separator AND the column separator, so
/// "692 640 692 640 692 640" is ambiguous: one 18-digit number, or three columns
/// of 692 640. A holdings table repeats a value across columns, so a run that is a
/// base pattern repeated `k ≥ 2` times is split by that period (each column is one
/// number). A short single group repeated ("0 0 0 0", "10 10") splits per group.
/// Otherwise a number is a leading 1–3-digit group followed by 3-digit groups.
fn split_number_run(run: &[&str]) -> Vec<String> {
    let n = run.len();
    if n == 0 {
        return Vec::new();
    }
    for p in 1..n {
        if !n.is_multiple_of(p) {
            continue;
        }
        let k = n / p;
        if k < 2 {
            continue;
        }
        if !run.chunks(p).all(|chunk| chunk == &run[0..p]) {
            continue;
        }
        // A repeated single 3-digit group ("143 143") is genuinely ambiguous with a
        // 6-digit number — leave it to the greedy merge; a repeated multi-group
        // pattern or a short group is a real column repetition.
        if p == 1 && run[0].len() > 2 {
            break;
        }
        let base: String = run[0..p].concat();
        return (0..k).map(|_| strip_leading_zeros(&base)).collect();
    }
    greedy_merge_numbers(run)
}

/// Greedy thousands merge for a non-periodic run: each number is a leading group
/// followed by exactly-3-digit continuation groups.
fn greedy_merge_numbers(run: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < run.len() {
        let mut number = run[i].to_owned();
        i += 1;
        while i < run.len() && is_thousands_group(run[i]) {
            number.push_str(run[i]);
            i += 1;
        }
        if number.chars().count() <= 15 {
            out.push(strip_leading_zeros(&number));
        }
    }
    out
}

fn is_int(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|c| c.is_ascii_digit())
}

fn is_thousands_group(token: &str) -> bool {
    token.len() == 3 && token.chars().all(|c| c.is_ascii_digit())
}

fn is_decimal(token: &str) -> bool {
    let mut has_sep = false;
    let mut has_digit = false;
    for c in token.chars() {
        if c.is_ascii_digit() {
            has_digit = true;
        } else if c == ',' || c == '.' || c == '%' {
            has_sep = true;
        } else {
            return false;
        }
    }
    has_digit && has_sep
}

fn strip_leading_zeros(number: &str) -> String {
    let trimmed = number.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Extract the "pośrednio poprzez <vehicle>" / family-foundation vehicle from a
/// note line, if present.
fn indirect_vehicle(line: &str) -> Option<String> {
    let d = deflate(line);
    if !d.contains("pośrednio") && !d.contains("fundacjarodzinna") {
        return None;
    }
    if let Some(caps) = indirect_regex().captures(line) {
        let vehicle = caps.get(1)?.as_str().trim();
        let vehicle = vehicle.trim_end_matches(['.', ',', ';']).trim();
        if !vehicle.is_empty() && vehicle.chars().count() <= 80 {
            return Some(vehicle.to_owned());
        }
    }
    None
}

fn indirect_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "pośrednio poprzez X" / "pośrednio przez X" / "przez X Fundacja Rodzinna".
    RE.get_or_init(|| {
        Regex::new(r"(?i)po[śs]rednio\s+(?:poprzez|przez)\s+(.+)").expect("valid regex")
    })
}
