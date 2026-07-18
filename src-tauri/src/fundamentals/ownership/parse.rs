//! Deterministic shareholders-table parser (v0.56 T1 spike, iterated against
//! the 14-document hand-labeled real corpus). See the module doc for the
//! design rationale.
//!
//! Real GPW filings taught this parser eight hard lessons (all handled by
//! *general* mechanisms, never per-company rules):
//!
//! 1. **Intra-word spaces everywhere.** Both `pdf-extract` and positional
//!    xhtml emit "Akcjonar iusze p osiadający" and even "6 0 ,9 3%". All
//!    heading and header matching happens on a *deflated* (whitespace-removed,
//!    lowercased) form; number lexing re-joins thousands groups and split
//!    decimals.
//! 2. **Tables fragment across sections.** All-caps holder names ("NN PTE",
//!    "OFE PZU") become section headings in `split_sections`, so the parser
//!    flattens every section (heading line + body lines) back into one
//!    document-ordered line stream and works on windows of that stream.
//! 3. **Many table shapes.** One unified row assembler covers the cell-stream
//!    shape (one cell per line), the positional shape (name line + numeric
//!    line), the classic same-line shape, and narrative bullets ("Skarb
//!    Państwa …, który posiada … co stanowi 34,19% kapitału …").
//! 4. **Several anchors per document.** Every candidate heading opens a
//!    scored window; the best-scoring parse wins (post-balance-date blocks
//!    are penalized so the as-of table beats the "na dzień przekazania
//!    raportu" restatement).
//! 5. **Some documents are deterministically unparsable.** Custom-font
//!    filings map glyphs into the Unicode private-use area; a high PUA ratio
//!    yields [`OwnershipParseState::GlyphEncoded`] (AI/OCR-tier residual)
//!    instead of silently swapped capital/votes columns.
//! 6. **Aggregate labels are data, not anchors or headers.** "Pozostali
//!    akcjonariusze" mid-table must close the previous holder's row — treated
//!    as a nested anchor its percentage leaks into that holder. Likewise a
//!    dash-prefixed qualifier cell ("- akcje własne") and a value line whose
//!    leading digit run is share-count-sized ("295 217 300 akcji, co …") must
//!    not be eaten by the column-header prefixes.
//! 7. **Charts and multi-period restatements masquerade as tables.** A
//!    "Źródło: …" caption before any row marks a pie-chart block (poisoned);
//!    a line carrying ≥3 as-of dates marks a multi-period comparison table
//!    whose column mapping is ambiguous (scored down so a single-period block
//!    wins).
//! 8. **Running prose poisons the name accumulator.** A pending holder name
//!    that overflows [`PENDING_NAME_CAP`] is narrative, not a table cell; the
//!    values that later attach to it must not form a row.

use std::sync::OnceLock;

use regex::Regex;

use crate::report_diff::extraction::{Section, SourceFormat};

/// One disclosed shareholder row. Percentages are normalized decimal strings
/// (comma→dot, `%`/whitespace stripped); either side may be `None` when the
/// filing discloses only one of capital/votes. `holder_raw` keeps the source
/// text (lightly cleaned of footnote markers/qualifiers) — full name
/// normalization/classification is a later task (T5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipRow {
    pub holder_raw: String,
    pub capital_pct: Option<String>,
    pub votes_pct: Option<String>,
}

/// An aggregate/summary row ("razem", "pozostali", "free float", "ogółem")
/// that is *not* a single holder. Kept out of [`OwnershipParseOutcome::rows`]
/// but retained so the harness can eyeball what was set aside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateRow {
    pub label_raw: String,
    pub capital_pct: Option<String>,
    pub votes_pct: Option<String>,
}

/// Outcome state of a parse attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipParseState {
    /// A shareholders section was located and at least one row was parsed.
    Found,
    /// No shareholders heading was located in any section.
    SectionMissing,
    /// A section was located but no holder/aggregate row could be parsed
    /// (e.g. the table is rendered as an image).
    TableUnparsable,
    /// The document's text layer is glyph-mangled (custom font mapped into
    /// the Unicode private-use area): deterministic column mapping would be
    /// unreliable, so the parser refuses rather than risk swapping capital
    /// and votes. AI/OCR-tier residual by design.
    GlyphEncoded,
}

/// Full result of one `parse_shareholders` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipParseOutcome {
    pub state: OwnershipParseState,
    /// The heading line that anchored the winning block (verbatim), if any.
    pub matched_heading: Option<String>,
    pub rows: Vec<OwnershipRow>,
    pub aggregates: Vec<AggregateRow>,
}

/// How many lines after an anchor are considered part of its candidate block.
const WINDOW_LINES: usize = 90;
/// Most anchors considered per document (guards pathological inputs).
const MAX_ANCHORS: usize = 80;
/// Pending multi-line holder-name accumulator cap; on overflow the oldest
/// text is dropped (prose never forms a valid holder anyway).
const PENDING_NAME_CAP: usize = 200;
/// Maximum plausible holder-name length after cleanup.
const NAME_CAP: usize = 120;

/// Normalize Mistral-OCR markdown into the plain, deflatable line stream the
/// deterministic shareholders parser consumes (tier-4, v0.57 T8, ADR 0077).
/// OCR v4 returns markdown tables (`| cell | cell |`), separator rules
/// (`|---|:--:|`), and `#`-prefixed headings; the deterministic parser already
/// handles arbitrary intra-word spacing and the same-line "name  numbers"
/// shape, so we only turn the table syntax into whitespace-separated columns
/// and drop the noise — reusing the whole battle-tested parser rather than a
/// fragile new markdown-table reader. Pure and deterministic (a golden pins it).
pub fn normalize_ocr_markdown(markdown: &str) -> String {
    let mut out = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        // Drop markdown table separator rows (|---|:--:|---|) — pure syntax.
        if is_table_separator(trimmed) {
            continue;
        }
        // Strip a leading heading marker but KEEP the heading text (the parser
        // anchors on the shareholders heading).
        let without_heading = trimmed.trim_start_matches('#').trim_start();
        // Turn table cell pipes into spaces so each table row becomes the
        // same-line "name  col  col" shape the parser already assembles.
        let cells = without_heading.replace('|', " ");
        out.push(cells.trim().to_owned());
    }
    out.join("\n")
}

/// A markdown table separator row (`|---|:--:|---|` / `---`): every pipe-split
/// cell is non-empty and made only of `-`/`:`.
fn is_table_separator(line: &str) -> bool {
    let body = line.trim_matches('|');
    if body.trim().is_empty() {
        return false;
    }
    body.split('|').all(|cell| {
        let c = cell.trim();
        !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')
    })
}

/// Parse a shareholders table out of OCR markdown (tier-4, v0.57 T8). Normalizes
/// the markdown ([`normalize_ocr_markdown`]), wraps it as one document section,
/// and runs the SAME deterministic parser the report-extraction path uses. OCR
/// defeats the glyph encoding (it reads the rendered pixels), so a residual that
/// was `glyph_encoded` on the raw text layer now yields real, readable text.
pub fn parse_ocr_shareholders(markdown: &str) -> OwnershipParseOutcome {
    let normalized = normalize_ocr_markdown(markdown);
    let section = Section {
        ordinal: 0,
        heading: "<preamble>".to_owned(),
        body: normalized,
    };
    parse_shareholders(&[section], SourceFormat::Pdf)
}

/// Parse the shareholders table out of a report's ordered text sections.
///
/// Deterministic and panic-free for arbitrary input. Both formats go through
/// the same unified line-stream pipeline (real xhtml and PDF text layers
/// converge on the same shapes); the parameter is kept for API stability and
/// future format-specific tuning.
pub fn parse_shareholders(sections: &[Section], _format: SourceFormat) -> OwnershipParseOutcome {
    let lines = flatten(sections);

    let anchors: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| anchor_strength(l).is_some())
        .map(|(i, _)| i)
        .take(MAX_ANCHORS)
        .collect();

    if anchors.is_empty() {
        return OwnershipParseOutcome {
            state: OwnershipParseState::SectionMissing,
            matched_heading: None,
            rows: Vec::new(),
            aggregates: Vec::new(),
        };
    }

    if is_glyph_encoded(&lines) {
        return OwnershipParseOutcome {
            state: OwnershipParseState::GlyphEncoded,
            matched_heading: Some(lines[anchors[0]].trim().to_owned()),
            rows: Vec::new(),
            aggregates: Vec::new(),
        };
    }

    let mut best: Option<(i64, usize, ParsedBlock)> = None;
    for &idx in &anchors {
        let window_end = (idx + 1 + WINDOW_LINES).min(lines.len());
        let block = parse_block(&lines[idx], &lines[idx + 1..window_end]);
        let score = score_block(&lines[idx], &block);
        let better = match &best {
            None => true,
            Some((best_score, _, _)) => score > *best_score,
        };
        if better {
            best = Some((score, idx, block));
        }
    }

    let (_, anchor_idx, block) = best.expect("anchors is non-empty");
    let state = if block.rows.is_empty() && block.aggregates.is_empty() {
        OwnershipParseState::TableUnparsable
    } else {
        OwnershipParseState::Found
    };
    OwnershipParseOutcome {
        state,
        matched_heading: Some(lines[anchor_idx].trim().to_owned()),
        rows: block.rows,
        aggregates: block.aggregates,
    }
}

// ---------------------------------------------------------------------------
// Flattening + document-level checks
// ---------------------------------------------------------------------------

/// Rebuild the document-ordered line stream: each section contributes its
/// heading line (except the synthetic `<preamble>`) followed by its body
/// lines. Undoes the section fragmentation described in the module doc.
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

/// Custom-font detection: ratio of Unicode private-use-area codepoints.
/// Measured on the real corpus: a glyph-mangled filing sits at ~50% while
/// every clean filing is below 0.02%, so the threshold has a wide margin.
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

/// Whitespace-removed, lowercased form — the substrate for all keyword
/// matching, because real text layers break words with arbitrary spaces.
fn deflate(line: &str) -> String {
    line.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// Strip leading numbering/punctuation ("1.", "a)", "5.2", "–") from a
/// deflated line so heading variants match regardless of outline numbering.
fn strip_leading_numbering(deflated: &str) -> &str {
    let mut s = deflated;
    loop {
        let trimmed =
            s.trim_start_matches(|c: char| c.is_ascii_digit() || ".,-–—)(:;/\\|•*".contains(c));
        // "a)" / "b)" outline markers.
        let mut chars = trimmed.chars();
        let stripped = match (chars.next(), chars.next()) {
            (Some(l), Some(')')) if l.is_alphabetic() => chars.as_str(),
            _ => trimmed,
        };
        if stripped == s {
            return s;
        }
        s = stripped;
    }
}

// ---------------------------------------------------------------------------
// Anchors
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnchorStrength {
    Strong,
    Weak,
}

/// Is this line a shareholders-section anchor, and how strong?
fn anchor_strength(line: &str) -> Option<AnchorStrength> {
    let d = deflate(line);
    if d.len() > 600 {
        return None;
    }
    let s = strip_leading_numbering(&d);
    if s.contains("listdoakcjonariusz") || s.starts_with("listdo") {
        return None;
    }
    // Geographic/regional breakdowns of the shareholder base are charts, not
    // the stakes table.
    if s.contains("geograficzn") {
        return None;
    }
    // An aggregate/total label ("Pozostali akcjonariusze", "Razem") is table
    // *data*, never a section anchor. Without this, the free-float row inside
    // a block is skipped as nested-anchor furniture and its percentage leaks
    // into the preceding holder's row (real-corpus lesson: XTB, ACP).
    if is_aggregate_label(line) {
        return None;
    }
    const STRONG: [&str; 9] = [
        "strukturaakcjonariatu",
        "znacznepakietyakcji",
        "znacznychpakietówakcji",
        "znacznychpakietowakcji",
        "wykazakcjonariuszy",
        "akcjonariuszeposiadając",
        "akcjonariuszeposiadajac",
        "akcjonariuszamiposiadając",
        "akcjonariuszamiposiadajac",
    ];
    if STRONG.iter().any(|p| s.contains(p)) {
        return Some(AnchorStrength::Strong);
    }
    if s.starts_with("akcjonariusze") || s.starts_with("akcjonariat") {
        return Some(AnchorStrength::Weak);
    }
    // Short heading-like lines such as "Nasi akcjonariusze".
    if s.len() <= 40 && (s.contains("akcjonariusze") || s.contains("akcjonariat")) {
        return Some(AnchorStrength::Weak);
    }
    None
}

/// Blocks stated to be as of a date *after* the reporting period (post-balance
/// restatements) lose to the as-of-period table.
fn is_post_balance_anchor(line: &str) -> bool {
    let d = deflate(line);
    d.contains("podacie")
        || d.contains("podniubilansowym")
        || d.contains("nadzieńprzekazania")
        || d.contains("nadzienprzekazania")
        || d.contains("nadzieńpublikacji")
        || d.contains("nadzienpublikacji")
        || d.contains("zmianywstrukturze")
}

fn score_block(anchor_line: &str, block: &ParsedBlock) -> i64 {
    // Rows backed by a share *count* are real disclosure rows; chart labels
    // and narrative percentages come alone, so they weigh less.
    let mut score: i64 = block
        .rows
        .iter()
        .zip(&block.row_has_count)
        .map(|(_, has_count)| if *has_count { 3 } else { 1 })
        .sum();
    score += block.aggregates.len() as i64;
    if anchor_strength(anchor_line) == Some(AnchorStrength::Strong) {
        score += 4;
    }
    if block.saw_header {
        score += 1;
    }
    if is_post_balance_anchor(anchor_line) {
        score -= 6;
    }
    // A multi-period comparison table cannot be column-mapped
    // deterministically — let a single-period block win instead.
    if block.multi_period_header {
        score -= 10;
    }
    score
}

// ---------------------------------------------------------------------------
// Line classification
// ---------------------------------------------------------------------------

/// Column-header / table-furniture line prefixes (deflated). Tuned on the
/// real corpus where header cells arrive fragmented one-per-line.
const HEADER_PREFIXES: [&str; 38] = [
    "akcjonariusz",
    "akcjonariat",
    "liczba",
    "udział",
    "udzial",
    "%",
    "wartość",
    "wartosc",
    "ilość",
    "ilosc",
    "akcje",
    "akcji",
    "głos",
    "glos",
    "kapita",
    "wkapitale",
    "zakładow",
    "zakladow",
    "nawz",
    "ogóln",
    "ogoln",
    "wogóln",
    "wogoln",
    "liczbie",
    "wliczbie",
    "stanna",
    "nadzień",
    "nadzien",
    "zmiana",
    "nominaln",
    "posiadanych",
    "wyszczególnienie",
    "wyszczegolnienie",
    "lp.",
    "seria",
    "tabela",
    "łączn",
    "laczn",
];

fn is_header_line(line: &str) -> bool {
    // A header cell carries words; a value line ("16.89 %") deflates to
    // digits+% and must never be mistaken for the "%" header prefix.
    if !line.chars().any(char::is_alphabetic) {
        return false;
    }
    // A leading dash marks a qualifier/continuation cell ("- akcje własne"),
    // never a column header — without this the "akcje" header prefix eats the
    // treasury-shares qualifier row (real-corpus lesson: ACP).
    if line.trim_start().starts_with(['-', '–', '—']) {
        return false;
    }
    let d = deflate(line);
    if d.is_empty() {
        return false;
    }
    // A leading digit run of share-count size is a data value ("295 217 300
    // akcji, co stanowi…"), not outline numbering — stripping it would let
    // the "akcji" header prefix eat a narrative value line (real-corpus
    // lesson: PZU).
    if d.chars().take_while(char::is_ascii_digit).count() >= 4 {
        return false;
    }
    let s = strip_leading_numbering(&d);
    if s.is_empty() {
        return false;
    }
    HEADER_PREFIXES.iter().any(|p| s.starts_with(p))
        || s.starts_with("(pln")
        || s.starts_with("pln")
        || s.starts_with("wtys")
        || s.starts_with("(wtys")
        // Column-header fragments left when a header wraps mid-phrase.
        || matches!(s, "wz" | "wza" | "kz" | "nawza")
}

/// A standalone footnote index / page number ("1)", "(1)*", "19", "**").
fn is_footnote_marker_line(line: &str) -> bool {
    let d = deflate(line);
    if d.is_empty() || d.len() > 8 {
        return false;
    }
    let digits = d.chars().filter(char::is_ascii_digit).count();
    digits <= 3 && d.chars().all(|c| c.is_ascii_digit() || "()*†.".contains(c))
}

/// A punctuation-only line (table borders, stray colons).
fn is_punct_only_line(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.chars().all(|c| !c.is_alphanumeric())
}

/// Footnote *text* ("(*) Dane na podstawie…", "* Zgodnie z…", "1) XX ZW…",
/// "Źródło: …"): once rows exist this ends the table.
fn is_footnote_text_line(line: &str) -> bool {
    footnote_text_regex().is_match(line.trim_start()) && !is_footnote_marker_line(line)
}

/// A chart/figure source caption ("Źródło: Raport bieżący 25/2022"), checked
/// on the deflated line.
fn is_source_caption_line(deflated: &str) -> bool {
    deflated.starts_with("źródło:") || deflated.starts_with("zrodlo:")
}

/// A line consisting only of date expressions ("31 grudnia 2025 r. 17 marca
/// 2026 r."), common as a two-period column header row.
fn is_date_line(line: &str) -> bool {
    let d = deflate(line);
    if d.is_empty() || !d.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    let residue = date_regex().replace_all(&d, "");
    let residue = residue
        .replace("r.", "")
        .replace("roku", "")
        .replace(['.', ','], "");
    residue.is_empty()
}

/// Distinct date strings on a line (deflated), for two-period detection.
fn dates_on_line(line: &str) -> Vec<String> {
    let d = deflate(line);
    date_regex()
        .find_iter(&d)
        .map(|m| m.as_str().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// Numeric lexing
// ---------------------------------------------------------------------------

/// One line, lexed: the text prefix before the first numeric token, plus the
/// percent-marked ("25,5%") and plain-decimal ("14,30") value tokens found.
/// Counts ("3 897 645") are recognized (thousands regrouping) and dropped.
struct LexedLine {
    name_prefix: String,
    marked: Vec<String>,
    plain: Vec<String>,
    has_numeric: bool,
    /// The line carried a share-count-sized integer (≥4 digits).
    has_count: bool,
}

fn lex_line(line: &str) -> LexedLine {
    // Whole-line spaced percent ("6 0 ,9 3%") — only when the line carries no
    // letters, so squeezing cannot conflate separate columns.
    let squeezed = deflate(line);
    if !squeezed.is_empty()
        && !squeezed.chars().any(char::is_alphabetic)
        && spaced_pct_regex().is_match(&squeezed)
    {
        let mut lexed = LexedLine {
            name_prefix: String::new(),
            marked: Vec::new(),
            plain: Vec::new(),
            has_numeric: true,
            has_count: false,
        };
        classify_numeric_token(&squeezed, &mut lexed);
        return lexed;
    }

    let atoms: Vec<&str> = line.split_whitespace().collect();
    let mut name_atoms: Vec<&str> = Vec::new();
    let mut in_numbers = false;
    let mut lexed = LexedLine {
        name_prefix: String::new(),
        marked: Vec::new(),
        plain: Vec::new(),
        has_numeric: false,
        has_count: false,
    };
    let mut current = String::new();

    for atom in &atoms {
        let numeric_ish = *atom == "%" || atom.chars().any(|c| c.is_ascii_digit());
        if !in_numbers {
            if numeric_ish {
                in_numbers = true;
            } else {
                name_atoms.push(atom);
                continue;
            }
        }
        if !numeric_ish {
            // Prose between numbers ("akcji, co stanowi") — flush and skip.
            if !current.is_empty() {
                classify_numeric_token(&current, &mut lexed);
                current.clear();
            }
            continue;
        }
        lexed.has_numeric = true;
        let a = atom.trim_matches(|c: char| "()".contains(c));
        if a.is_empty() {
            continue;
        }
        if current.is_empty() {
            current = a.to_owned();
        } else if a == "%"
            || a.starts_with(',')
            || a.starts_with('.')
            || current.ends_with(',')
            || current.ends_with('.')
        {
            current.push_str(a);
        } else if a.len() == 3
            && a.chars().all(|c| c.is_ascii_digit())
            && current.chars().all(|c| c.is_ascii_digit() || c == '-')
        {
            // Thousands group continuation ("3" "897" "645").
            current.push_str(a);
        } else {
            classify_numeric_token(&current, &mut lexed);
            current = a.to_owned();
        }
        if current.contains('%') {
            classify_numeric_token(&current, &mut lexed);
            current.clear();
        }
    }
    if !current.is_empty() {
        classify_numeric_token(&current, &mut lexed);
    }
    lexed.name_prefix = name_atoms.join(" ");
    lexed
}

/// Classify a joined numeric token into marked-percent / plain-decimal /
/// count (dropped).
fn classify_numeric_token(token: &str, lexed: &mut LexedLine) {
    let squeezed: String = token.chars().filter(|c| !c.is_whitespace()).collect();
    let marked = squeezed.contains('%');
    let body = squeezed.trim_end_matches('%');
    if !marked && body.len() >= 4 && body.chars().all(|c| c.is_ascii_digit()) {
        lexed.has_count = true;
        return;
    }
    let dotted = body.replace(',', ".");
    let Ok(value) = dotted.parse::<f64>() else {
        return;
    };
    if !(0.0..=100.0).contains(&value) {
        return;
    }
    if marked {
        lexed.marked.push(dotted);
    } else if plain_value_regex().is_match(body) {
        // A genuine decimal ("14,30"), not a trailing-dot ordinal ("4.").
        lexed.plain.push(dotted);
    }
    // Bare integers without '%' are counts/years/footnotes — dropped.
}

// ---------------------------------------------------------------------------
// Block parsing (unified row assembler)
// ---------------------------------------------------------------------------

struct RawRow {
    name: String,
    marked: Vec<String>,
    plain: Vec<String>,
    has_count: bool,
    /// Two as-of-date columns were in effect when this row was assembled.
    two_period: bool,
}

struct ParsedBlock {
    rows: Vec<OwnershipRow>,
    /// Parallel to `rows`: whether the row carried a share count (scoring).
    row_has_count: Vec<bool>,
    aggregates: Vec<AggregateRow>,
    saw_header: bool,
    /// A single line in the block carried ≥3 as-of date tokens: a
    /// multi-period comparison table whose column mapping is ambiguous
    /// (scoring penalty; another block should win).
    multi_period_header: bool,
}

impl ParsedBlock {
    fn empty(saw_header: bool) -> Self {
        ParsedBlock {
            rows: Vec::new(),
            row_has_count: Vec::new(),
            aggregates: Vec::new(),
            saw_header,
            multi_period_header: false,
        }
    }
}

struct Pending {
    name: String,
    marked: Vec<String>,
    plain: Vec<String>,
    has_count: bool,
    /// The name accumulator overflowed [`PENDING_NAME_CAP`]: this pending is
    /// running prose, not a holder name — discard it on close instead of
    /// emitting a row built from a mid-sentence fragment (real-corpus lesson:
    /// KGH narrative change-of-holdings paragraphs).
    poisoned: bool,
}

impl Pending {
    fn has_values(&self) -> bool {
        !self.marked.is_empty() || !self.plain.is_empty()
    }
}

fn close_pending(pending: &mut Option<Pending>, raw_rows: &mut Vec<RawRow>, two_period: bool) {
    if let Some(p) = pending.take() {
        if p.has_values() && !p.poisoned {
            raw_rows.push(RawRow {
                name: p.name,
                marked: p.marked,
                plain: p.plain,
                has_count: p.has_count,
                two_period,
            });
        }
    }
}

fn parse_block(anchor_line: &str, window: &[String]) -> ParsedBlock {
    let mut raw_rows: Vec<RawRow> = Vec::new();
    let mut pending: Option<Pending> = None;
    let mut saw_header = false;
    // The anchor line itself may be the multi-period column header
    // ("… znaczne pakiety akcji 31.12.2021 31.12.2022 30.06.2023 …").
    let mut multi_period_header = dates_on_line(anchor_line).len() >= 3;
    let mut header_dates: Vec<String> = Vec::new();
    let anchor_deflated = deflate(anchor_line);
    let mut mentions_votes = anchor_deflated.contains("głos") || anchor_deflated.contains("glos");
    let mut stop = false;

    for line in window {
        if stop {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || is_punct_only_line(trimmed) {
            continue;
        }
        let d = deflate(trimmed);
        let two_period = header_dates.len() >= 2;
        if d.contains("głos") || d.contains("glos") {
            mentions_votes = true;
        }

        // A line packing ≥3 as-of date tokens is a multi-period comparison
        // table's column header: the as-of column cannot be mapped
        // deterministically, so flag the block for a scoring penalty
        // (value assembly is left untouched).
        if dates_on_line(trimmed).len() >= 3 {
            multi_period_header = true;
        }

        // Row-terminating furniture first.
        if is_footnote_marker_line(trimmed) {
            continue;
        }
        // A source caption ("Źródło: …") before any row means the anchor
        // heads a chart/figure, not the stakes table — its stray labels must
        // not assemble into rows (real-corpus lesson: PZU pie charts). After
        // rows (or a value-bearing pending row) it ends the table like any
        // footnote text.
        if is_source_caption_line(&d) {
            if raw_rows.is_empty() && pending.as_ref().is_none_or(|p| !p.has_values()) {
                return ParsedBlock::empty(saw_header);
            }
            break;
        }
        if is_footnote_text_line(trimmed) {
            if raw_rows.is_empty() && pending.as_ref().is_none_or(|p| !p.has_values()) {
                continue;
            }
            break;
        }
        // A management/supervisory/registered-shares-only holdings recap is a
        // *different* table. After holder rows it ends the block; before any
        // row it poisons the whole block (the window IS that other table).
        if is_other_table_marker(&d) {
            if raw_rows.is_empty() {
                return ParsedBlock::empty(saw_header);
            }
            break;
        }
        if anchor_strength(trimmed).is_some() {
            // A nested anchor line is furniture, not data.
            continue;
        }
        if is_date_line(trimmed) {
            for date in dates_on_line(trimmed) {
                if !header_dates.contains(&date) {
                    header_dates.push(date);
                }
            }
            continue;
        }
        if is_header_line(trimmed) {
            // Header cells reset the name accumulator. No hard stop here: a
            // junk pre-table pending must not block reaching the real table
            // (the total-row and footnote-text stops guard the tail instead).
            close_pending(&mut pending, &mut raw_rows, two_period);
            saw_header = true;
            for date in dates_on_line(trimmed) {
                if !header_dates.contains(&date) {
                    header_dates.push(date);
                }
            }
            continue;
        }

        let lexed = lex_line(trimmed);

        if !lexed.has_numeric {
            // Text-only line: closes a value-bearing pending row, then starts
            // or extends the holder-name accumulator.
            if pending.as_ref().is_some_and(Pending::has_values) {
                close_pending(&mut pending, &mut raw_rows, two_period);
            }
            let is_bullet =
                trimmed.starts_with('•') || trimmed.starts_with('–') || trimmed.starts_with('—');
            match pending.as_mut() {
                Some(p) if !is_bullet => {
                    if p.name.len() + trimmed.len() + 1 > PENDING_NAME_CAP {
                        // Overflow = running prose; poison the whole pending
                        // (values that later attach must not form a row).
                        p.poisoned = true;
                        p.name = trimmed.to_owned();
                    } else {
                        p.name.push(' ');
                        p.name.push_str(trimmed);
                    }
                }
                _ => {
                    pending = Some(Pending {
                        name: trimmed.to_owned(),
                        marked: Vec::new(),
                        plain: Vec::new(),
                        has_count: false,
                        poisoned: false,
                    });
                }
            }
            continue;
        }

        let prefix_clean = lexed.name_prefix.trim();
        let prefix_is_name = !prefix_clean.is_empty()
            && prefix_clean.chars().filter(|c| c.is_alphabetic()).count() >= 3
            && !name_has_reject_marker(prefix_clean);

        if prefix_is_name {
            // Same-line row (possibly continuing a pending multi-line name).
            let mut name = prefix_clean.to_owned();
            if let Some(p) = pending.take() {
                if p.has_values() && !p.poisoned {
                    raw_rows.push(RawRow {
                        name: p.name,
                        marked: p.marked,
                        plain: p.plain,
                        has_count: p.has_count,
                        two_period,
                    });
                } else if !p.has_values()
                    && !p.poisoned
                    && p.name.len() + name.len() < PENDING_NAME_CAP
                {
                    name = format!("{} {}", p.name, name);
                }
            }
            pending = Some(Pending {
                name,
                marked: lexed.marked,
                plain: lexed.plain,
                has_count: lexed.has_count,
                poisoned: false,
            });
        } else if let Some(p) = pending.as_mut() {
            // Numeric (or connective-prefixed) values/counts attach to the
            // pending holder; with no pending holder they are noise.
            p.marked.extend(lexed.marked);
            p.plain.extend(lexed.plain);
            p.has_count |= lexed.has_count;
        }

        // Total row ("Razem/Ogółem/Suma" at ~100%) ends the table.
        if let Some(p) = pending.as_ref() {
            if p.has_values() && is_total_label(&p.name) {
                let total_hit = p
                    .marked
                    .iter()
                    .chain(p.plain.iter())
                    .any(|v| v.parse::<f64>().is_ok_and(|x| x >= 99.5));
                if total_hit {
                    stop = true;
                }
            }
        }
    }
    let final_two_period = header_dates.len() >= 2;
    close_pending(&mut pending, &mut raw_rows, final_two_period);

    finalize_block(raw_rows, mentions_votes, saw_header, multi_period_header)
}

/// Map raw rows to holder/aggregate rows: %-marked values take priority over
/// plain decimals block-wide, two-period tables keep only the first (as-of)
/// value, and a single-percentage table in a votes-mentioning context is the
/// combined capital==votes disclosure common on the GPW.
fn finalize_block(
    raw_rows: Vec<RawRow>,
    mentions_votes: bool,
    saw_header: bool,
    multi_period_header: bool,
) -> ParsedBlock {
    let block_has_marked = raw_rows.iter().any(|r| !r.marked.is_empty());
    let mut rows: Vec<OwnershipRow> = Vec::new();
    let mut row_has_count: Vec<bool> = Vec::new();
    let mut aggregates = Vec::new();

    for raw in raw_rows {
        let two_period = raw.two_period;
        let values: &[String] = if block_has_marked {
            if raw.marked.is_empty() {
                continue;
            }
            &raw.marked
        } else {
            &raw.plain
        };
        if values.is_empty() {
            continue;
        }
        // Aggregate labels legitimately contain words the holder-name reject
        // list bans ("Pozostali akcjonariusze"), so classify aggregates on the
        // lightly-cleaned label before plausibility filtering.
        let basic = basic_clean_name(&raw.name);
        let aggregate = is_aggregate_label(&basic);
        let name = if aggregate {
            if basic.chars().filter(|c| c.is_alphabetic()).count() < 3 {
                continue;
            }
            basic
        } else {
            match clean_holder_name(&raw.name) {
                Some(name) => name,
                None => continue,
            }
        };

        let (capital, votes) = if two_period {
            let first = values[0].clone();
            if mentions_votes {
                (Some(first.clone()), Some(first))
            } else {
                (Some(first), None)
            }
        } else if values.len() >= 2 {
            (Some(values[0].clone()), Some(values[1].clone()))
        } else {
            let only = values[0].clone();
            if mentions_votes {
                (Some(only.clone()), Some(only))
            } else {
                (Some(only), None)
            }
        };

        if aggregate {
            aggregates.push(AggregateRow {
                label_raw: name,
                capital_pct: capital,
                votes_pct: votes,
            });
        } else {
            let row = OwnershipRow {
                holder_raw: name,
                capital_pct: capital,
                votes_pct: votes,
            };
            // Repeated identical rows are chart-label noise (the same slice
            // rendered on several charts), never a real table.
            if !rows.contains(&row) {
                rows.push(row);
                row_has_count.push(raw.has_count);
            }
        }
    }
    ParsedBlock {
        rows,
        row_has_count,
        aggregates,
        saw_header,
        multi_period_header,
    }
}

// ---------------------------------------------------------------------------
// Holder-name cleanup and plausibility
// ---------------------------------------------------------------------------

/// Sentence fragments that never appear inside a real holder name; a name
/// containing one is prose that leaked through the layout heuristics.
const NAME_REJECT_MARKERS: [&str; 41] = [
    "posiada",
    "stanowi",
    "zapisan",
    "nominaln",
    "wyniós",
    "wynios",
    "wynosi",
    "uprawnia",
    "zgodnie",
    "otrzyma",
    "poinformowa",
    "zawiadomi",
    "akcjonariusz",
    "kapitale zakładow",
    "kapitale zakladow",
    "raport",
    "sprawozdan",
    "co najmniej",
    "na dzień",
    "na dzien",
    "walnym",
    "transakc",
    "sprzeda",
    "w dniu",
    "wykres",
    "udział",
    "udzial",
    "liczbie głosów",
    "liczbie glosow",
    "nabyci",
    "zbyci",
    "kurs",
    "wskaźnik",
    "wynik",
    "przychod",
    "zysk",
    "obligacje",
    "mln pln",
    "inwestor",
    "kapitału zakładow",
    "kapitalu zakladow",
];

fn name_has_reject_marker(name: &str) -> bool {
    let l = name.to_lowercase();
    let squeezed = deflate(name);
    NAME_REJECT_MARKERS
        .iter()
        .any(|m| l.contains(m) || squeezed.contains(&deflate(m)))
}

/// Shared light cleanup: whitespace normalization, bullets, narrative and
/// qualifier tails, trailing footnote markers. Never rejects.
fn basic_clean_name(raw: &str) -> String {
    let mut name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    // Leading bullets / list furniture.
    name = name
        .trim_start_matches(|c: char| "•–—-:;,.".contains(c) || c.is_whitespace())
        .to_owned();
    // Narrative tails: "…, który posiada", "…, które na …".
    if let Some(m) = narrative_cut_regex().find(&name) {
        name.truncate(m.start());
    }
    // Qualifier tails: "… - akcje uprzywilejowane", "… - akcje własne".
    if let Some(m) = akcje_qualifier_regex().find(&name) {
        name.truncate(m.start());
    }
    // Trailing footnote-marker tokens ("1)", "(1)*", "**").
    loop {
        let trimmed = name.trim_end();
        let Some(last) = trimmed.split_whitespace().last() else {
            break;
        };
        let is_marker = !last.is_empty()
            && last
                .chars()
                .all(|c| c.is_ascii_digit() || "()*†".contains(c));
        if is_marker {
            name.truncate(trimmed.len() - last.len());
        } else {
            name = trimmed.to_owned();
            break;
        }
    }
    // Footnote stars glued to the last word ("Fundacja Sky**").
    name.trim_end_matches(['*', '†', ' ']).to_owned()
}

/// Clean a raw accumulated holder name; `None` rejects the row entirely.
fn clean_holder_name(raw: &str) -> Option<String> {
    let name = basic_clean_name(raw);
    let lower = name.to_lowercase();
    if lower.starts_with("w tym") || lower.starts_with("stan na") {
        return None; // sub-holding row / equity-statement noise
    }
    let alpha = name.chars().filter(|c| c.is_alphabetic()).count();
    if alpha < 3 || name.chars().count() > NAME_CAP {
        return None;
    }
    if name_has_reject_marker(&name) {
        return None;
    }
    Some(name)
}

fn is_aggregate_label(name: &str) -> bool {
    // "łącznie/lacznie" is deliberately absent: it appears inside legitimate
    // holder names ("zarządzane fundusze łącznie") on real filings.
    let s = deflate(name);
    const NEEDLES: [&str; 6] = [
        "pozostal",
        "razem",
        "ogółem",
        "ogolem",
        "freefloat",
        "free-float",
    ];
    NEEDLES.iter().any(|n| s.contains(n)) || s.starts_with("suma")
}

fn is_total_label(name: &str) -> bool {
    let s = deflate(name);
    s.contains("razem") || s.contains("ogółem") || s.contains("ogolem") || s.contains("suma")
}

/// A line announcing a different holdings table (management/supervisory-board
/// share recaps, registered-shares-only breakdowns, special control rights) —
/// checked on the deflated line.
fn is_other_table_marker(deflated: &str) -> bool {
    const CONTAINS: [&str; 12] = [
        "zestawieniestanuposiadania",
        "osobyzarządzając",
        "osobyzarzadzajac",
        "osóbzarządzając",
        "osobzarzadzajac",
        "zarządzająceinadzorując",
        "zarzadzajaceinadzorujace",
        "członkówzarządu",
        "czlonkowzarzadu",
        "wyłącznieakcjeimienne",
        "wylacznieakcjeimienne",
        "specjalneuprawnienia",
    ];
    if CONTAINS.iter().any(|m| deflated.contains(m)) {
        return true;
    }
    // Group-header rows inside board-holdings tables.
    matches!(deflated, "zarząd" | "zarzad" | "radanadzorcza")
}

// ---------------------------------------------------------------------------
// Compiled regexes (cached)
// ---------------------------------------------------------------------------

fn date_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\d{1,2}(stycznia|lutego|marca|kwietnia|maja|czerwca|lipca|sierpnia|wrze[śs]nia|pa[źz]dziernika|listopada|grudnia)20\d{2}|\d{1,2}\.\d{1,2}\.20\d{2}",
        )
        .expect("valid regex")
    })
}

fn footnote_text_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(\(\*+\)|\*+\s|\*\p{L}|\d{1,2}\)\s|\)\s|źródło:|zrodlo:)")
            .expect("valid regex")
    })
}

fn plain_value_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{1,3}[.,]\d{1,3}$").expect("valid regex"))
}

fn spaced_pct_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^-?\d{1,3}[.,]\d{1,3}%$").expect("valid regex"))
}

fn narrative_cut_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i),\s*(kt[óo]r|posiada|jak[ao]\b)").expect("valid regex"))
}

fn akcje_qualifier_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\s[-–—]\s*akcje\b").expect("valid regex"))
}
