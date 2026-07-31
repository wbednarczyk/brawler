//! ESPI cover-note "WYBRANE DANE FINANSOWE" (WDF) parser — tier 2a
//! ([ADR 0061](../../../../docs/adr/0061-deterministic-fundamentals-data-gathering.md)
//! decision 1).
//!
//! Parses the mandated "WYBRANE DANE FINANSOWE" cover table that GPW issuers
//! embed as **plain text** in the body of a periodic-report komunikat. The
//! carrier text is already ingested by the Bankier primary, so this tier is
//! zero-fetch and available on publication day; it slots below
//! [`SourceTier::StructuredXhtml`] (cover-note figures are untagged) and above
//! [`SourceTier::Pdf`].
//!
//! The parser is written against the **form's structure**, never against
//! document names: unit header (`w tys.` / `w mln`), roman-numeral or
//! custom-label row grammar, Polish number formatting (space thousands
//! separator, comma decimal, negatives via leading `-` or parentheses), scope
//! markers, and a Polish label → `metric_key` dictionary.
//!
//! **The PLN↔EUR cross-check is the emit gate.** The four columns
//! (current-PLN, prior-PLN, current-EUR, prior-EUR) are packed either fully
//! concatenated or space-separated; in both a space is ambiguous between an
//! intra-number thousands separator and a column gap. Every grammar-valid split
//! into exactly four numbers is enumerated, and a row is emitted **only** when
//! the splits surviving the FX cross-check (`PLN/EUR` inside the band from the
//! document's own FX footnote, else 4.0–4.6 ±18%) agree on one value. Otherwise
//! the row **abstains** — a missing number is always better than a wrong one.
//!
//! Known limits (ADR 0061 tier 2a): headline lines only, no full-statement
//! depth; `Liczba akcji` scale depends on a per-row `(w tys.)` annotation and is
//! read conservatively (the literal, scaled only when the label says so).
//!
//! 1:1 Rust port of the measured spike parser, pinned to the same 15-document
//! hand-labeled ground truth (347 facts, recall and precision 347/347, zero
//! false values, 33 rows resolved by the FX cross-check).

use std::collections::BTreeSet;
use std::str::FromStr;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::{ExtractedFact, FactPeriod, SourceTier, StatementBasis};

/// Why a parse produced no facts. Distinguishes a legitimately empty cover note
/// from a parse failure, so the caller can flag the latter (ADR 0084: a
/// document no deterministic tier parses is flagged, never guessed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WdfEmptyReason {
    /// The issuer deferred the table to an attachment
    /// (`przedstawione w załączeniu`).
    DeferredToAttachment,
    /// A column header with no parseable data rows (e.g. USD/EUR-reporting
    /// issuers whose ESPI body carries no PLN table).
    HeaderOnly,
}

/// Outcome of one cover-note parse, including the counters the real-data
/// harness grades against.
#[derive(Debug, Clone)]
pub struct WdfParseResult {
    pub facts: Vec<ExtractedFact>,
    /// Monetary rows the PLN↔EUR cross-check refused to emit.
    pub abstained: usize,
    /// Rows that had more than one candidate split, where the cross-check
    /// picked exactly one.
    pub fx_resolved: usize,
    pub empty_reason: Option<WdfEmptyReason>,
    /// Unit multiplier from the table header declaration: 1_000 (`w tys.` /
    /// `in thousands`) or 1_000_000 (`w mln` / `in millions`); **1 (raw złoty)
    /// when the header declares no unit** — owner rule 2026-07-21 (ADR 0061):
    /// scale is read from the declaration, never guessed.
    pub unit_scale: i64,
    /// Whether the report's title block marks it consolidated (the base scope
    /// for rows before any standalone sub-section marker).
    pub base_consolidated: bool,
}

impl WdfParseResult {
    fn empty(reason: WdfEmptyReason, unit_scale: i64, base_consolidated: bool) -> Self {
        Self {
            facts: Vec::new(),
            abstained: 0,
            fx_resolved: 0,
            empty_reason: Some(reason),
            unit_scale,
            base_consolidated,
        }
    }
}

// ---------------------------------------------------------------------------
// character-slice helpers
//
// The parser works on `&[char]` throughout: the source is Polish text with
// multi-byte characters, and every index in the ported grammar is a *character*
// index. Slicing chars sidesteps UTF-8 boundary hazards entirely.
// ---------------------------------------------------------------------------

fn find_from(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (from.min(hay.len())..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn find_str(hay: &[char], needle: &str, from: usize) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    find_from(hay, &n, from)
}

fn as_string(s: &[char]) -> String {
    s.iter().collect()
}

/// Uppercase letters the roman-numeral lookbehind/lookahead treats as letters:
/// ASCII `A-Z` plus the Polish diacritics, matching the spike's character class.
fn is_upper_pl(c: char) -> bool {
    c.is_ascii_uppercase() || matches!(c, 'Ą' | 'Ć' | 'Ę' | 'Ł' | 'Ń' | 'Ó' | 'Ś' | 'Ż' | 'Ź')
}

// ---------------------------------------------------------------------------
// number grammar
// ---------------------------------------------------------------------------

/// Every grammar-valid grouped magnitude that is a **prefix** of `s`, as
/// `(value, end_index)`. The first group is 1–3 digits; each further group is a
/// space plus exactly three digits; an optional two-digit comma decimal
/// terminates the number.
///
/// Note the returned end index can be `s.len() + 1` for the trailing
/// single-decimal-digit form (a number at the very end of the run written with
/// one decimal digit) — callers must index defensively.
fn grouped(s: &[char]) -> Vec<(Decimal, usize)> {
    let n = s.len();
    let mut out: Vec<(Decimal, usize)> = Vec::new();

    fn emit(int_digits: &str, pos: usize, s: &[char], out: &mut Vec<(Decimal, usize)>) {
        let n = s.len();
        if let Ok(v) = Decimal::from_str(int_digits) {
            out.push((v, pos));
        }
        let two_digit = pos + 2 < n
            && s[pos] == ','
            && s[pos + 1].is_ascii_digit()
            && s[pos + 2].is_ascii_digit();
        if two_digit {
            let text = format!("{int_digits}.{}{}", s[pos + 1], s[pos + 2]);
            if let Ok(v) = Decimal::from_str(&text) {
                out.push((v, pos + 3));
            }
        } else if pos + 2 == n && s[pos] == ',' && s[pos + 1].is_ascii_digit() {
            // A trailing number written with a single decimal digit.
            let text = format!("{int_digits}.{}", s[pos + 1]);
            if let Ok(v) = Decimal::from_str(&text) {
                out.push((v, pos + 3));
            }
        }
    }

    fn extend(digits: &str, pos: usize, s: &[char], out: &mut Vec<(Decimal, usize)>) {
        emit(digits, pos, s, out);
        let n = s.len();
        if pos + 4 <= n
            && s[pos] == ' '
            && s[pos + 1].is_ascii_digit()
            && s[pos + 2].is_ascii_digit()
            && s[pos + 3].is_ascii_digit()
        {
            let mut next = String::with_capacity(digits.len() + 3);
            next.push_str(digits);
            next.extend(&s[pos + 1..pos + 4]);
            extend(&next, pos + 4, s, out);
        }
    }

    for l in 1..=3usize {
        if l <= n && s[..l].iter().all(|c| c.is_ascii_digit()) {
            extend(&as_string(&s[..l]), l, s, &mut out);
        }
    }
    out
}

/// One number read at the start of `s` (leading spaces already stripped), as
/// `(value, offset_of_rest)`. Handles parenthesised and leading-minus
/// negatives.
fn one_number(s: &[char]) -> Vec<(Decimal, usize)> {
    let mut out = Vec::new();
    if s.is_empty() {
        return out;
    }
    if s[0] == '(' {
        if let Some(j) = s.iter().position(|&c| c == ')') {
            let inner_raw = &s[1..j];
            let start = inner_raw
                .iter()
                .position(|c| !c.is_whitespace())
                .unwrap_or(inner_raw.len());
            let end = inner_raw
                .iter()
                .rposition(|c| !c.is_whitespace())
                .map_or(start, |p| p + 1);
            let inner: Vec<char> = inner_raw[start..end].to_vec();
            for (v, pos) in grouped(&inner) {
                if pos == inner.len() {
                    out.push((-v, j + 1));
                }
            }
        }
        return out;
    }
    let neg = s[0] == '-';
    let base = usize::from(neg);
    for (v, pos) in grouped(&s[base..]) {
        out.push((if neg { -v } else { v }, base + pos));
    }
    out
}

/// Cap on enumerated splits — the spike's guard against a pathological run.
const SPLIT_CAP: usize = 4000;

/// Every way to read `s` as exactly `k` numbers (whitespace between numbers
/// allowed, and numbers may be directly adjacent — which is exactly what makes
/// concatenated runs ambiguous).
fn split_numbers(s: &[char], k: usize) -> Vec<Vec<Decimal>> {
    fn rec(rem: &[char], acc: &mut Vec<Decimal>, k: usize, results: &mut Vec<Vec<Decimal>>) {
        if results.len() > SPLIT_CAP {
            return;
        }
        let start = rem.iter().position(|&c| c != ' ').unwrap_or(rem.len());
        let rem = &rem[start..];
        if acc.len() == k {
            if rem.is_empty() {
                results.push(acc.clone());
            }
            return;
        }
        for (v, off) in one_number(rem) {
            acc.push(v);
            rec(rem.get(off..).unwrap_or(&[]), acc, k, results);
            acc.pop();
        }
    }
    let mut results = Vec::new();
    rec(s, &mut Vec::new(), k, &mut results);
    results
}

// ---------------------------------------------------------------------------
// label → metric_key
// ---------------------------------------------------------------------------

/// Per-share metrics: never scaled by the document unit, always printed with
/// two decimals.
const PER_SHARE_KEYS: &[&str] = &[
    "eps_basic",
    "eps_diluted",
    "dividend_per_share",
    "wdf_book_value_per_share",
    "wdf_rozwodniona_wartosc_ksiegowa_na_jedna_akcje",
];

fn is_per_share(key: &str) -> bool {
    PER_SHARE_KEYS.contains(&key)
}

/// Strips a leading run of roman-numeral row markers (`^\s*(?:[IVXLC]{1,6}\.\s*)+`).
fn strip_roman(label: &str) -> String {
    let ch: Vec<char> = label.chars().collect();
    let n = ch.len();
    let mut i = 0;
    while i < n && ch[i].is_whitespace() {
        i += 1;
    }
    let mut consumed_any = false;
    loop {
        let mut j = i;
        while j < n && matches!(ch[j], 'I' | 'V' | 'X' | 'L' | 'C') {
            j += 1;
        }
        // `{1,6}` cannot backtrack onto a roman char, so the maximal run must be
        // 1..=6 long and immediately followed by the dot.
        if j == i || j - i > 6 || j >= n || ch[j] != '.' {
            break;
        }
        i = j + 1;
        while i < n && ch[i].is_whitespace() {
            i += 1;
        }
        consumed_any = true;
    }
    // The `+` requires at least one repetition; without it the pattern does not
    // match and nothing (not even leading whitespace) is stripped.
    if consumed_any {
        as_string(&ch[i..])
    } else {
        label.to_string()
    }
}

/// Maps a row label to a canonical `metric_key`, or `None` for an unmapped row
/// or a sub-header.
///
/// **Duplication note for integration (A1 → orchestrator):** this is a
/// stem-rule classifier over the WDF cover-note wording; [`super::text_numbers`] carries
/// `default_dictionary`, an *exact normalized-label lookup* over full-statement
/// line names. The two overlap on a handful of canonical keys (`revenue`,
/// `operating_profit`, `net_profit`, `total_assets`, `total_equity`,
/// `current_assets`, `current_liabilities`, `eps_basic`, `eps_diluted`) but are
/// structurally different mechanisms with different inputs, so neither can be
/// expressed in terms of the other today. `pdf.rs` was deliberately left
/// untouched by this slice; unifying them behind one Polish-label dictionary is
/// an integration-time decision, not a parser change.
fn classify(label: &str) -> Option<&'static str> {
    let ll = strip_roman(label);
    let ll = ll.trim();
    let low = ll.to_lowercase();
    if low.is_empty() || low.starts_with("pozycje") {
        return None;
    }

    let per_share =
        low.contains("na jedną akcję") || low.contains("na akcję") || low.contains("na 1 akcję");

    if per_share {
        if low.contains("dywidend") {
            return Some("dividend_per_share");
        }
        if low.contains("wartość księgowa") || low.contains("wartosc ksiegowa") {
            return Some(if low.starts_with("rozwodnion") {
                "wdf_rozwodniona_wartosc_ksiegowa_na_jedna_akcje"
            } else {
                "wdf_book_value_per_share"
            });
        }
        if low.contains("zysk") || low.contains("strata") || low.contains("podstawowy") {
            return Some(if low.starts_with("rozwodnion") {
                "eps_diluted"
            } else {
                "eps_basic"
            });
        }
    }

    if low.contains("liczba akcji") {
        return Some("shares_outstanding");
    }

    // Cash flow — totals are tested before the three activity buckets.
    if low.contains("środki pieniężne")
        || low.contains("srodki pieniezne")
        || low.contains("przepływy pieniężne")
        || low.contains("przeplywy pieniezne")
        || low.contains("zwiększenie")
        || low.contains("zwiekszenie")
    {
        if low.contains("zwiększenie")
            || low.contains("zwiekszenie")
            || low.contains("zmniejszenie")
            || low.contains("stanu środków")
            || low.contains("stanu srodkow")
            || (low.contains("razem")
                && !low.contains("działalności")
                && !low.contains("dzialalnosci"))
        {
            return Some("wdf_net_cash_change");
        }
        if low.contains("operacyjn") {
            return Some("operating_cash_flow");
        }
        if low.contains("inwestycyjn") {
            return Some("investing_cash_flow");
        }
        if low.contains("finansow") {
            return Some("financing_cash_flow");
        }
    }

    if low.contains("ebitda")
        || low.contains("powiększony o amortyzację")
        || low.contains("powiekszony o amortyzacje")
    {
        return Some(if low.contains("przed odpisami") {
            "wdf_ebitda_przed_odpisami_aktualizujacymi_netto"
        } else {
            "ebitda"
        });
    }

    // Bank income-statement rows (epic #277 T2, card #279, evidenced by the
    // real Pekao PSr H1/2026 note): these are already-net figures with no
    // "przychody"/"zysk"/"strata"/"kapitał" stem to collide with, so the
    // branch is safe ahead of every check below it. Checked against the real
    // note: it carries no "Przychody z tytułu odsetek"/"prowizji" row, so the
    // `przychody` branch immediately below is left untouched — no guard
    // needed today, but a bank note that DID carry such a row would need one.
    if low.contains("wynik z tytułu odsetek") {
        return Some("net_interest_income");
    }
    if low.contains("wynik z tytułu prowizji") {
        return Some("net_fee_commission_income");
    }

    if low.contains("przychody") {
        return Some("revenue");
    }

    if low.contains("całkowit") || low.contains("calkowit") {
        if low.starts_with("łączne") || low.starts_with("laczne") {
            return Some("wdf_laczne_calkowite_dochody");
        }
        let dom = low.contains("dominując") || low.contains("dominujac");
        let strata = low.contains("(strata)") || low.contains("strata");
        if low.contains("przypad") || dom {
            if strata {
                return Some(
                    "wdf_calkowity_dochod_strata_netto_akcjonariuszy_jednostki_dominujacej",
                );
            }
            if low.contains("dochody") {
                return Some(
                    "wdf_calkowite_dochody_netto_przypadajace_na_akcjonariuszy_jednostki_dominujacej",
                );
            }
            return Some("wdf_calkowity_dochod_przypadajacy_akcjonariuszom_jednostki_dominujacej");
        }
        if low.contains("dochody") {
            return Some("wdf_calkowite_dochody_netto");
        }
        if strata {
            return Some("wdf_calkowity_dochod_strata_netto");
        }
        if low.trim_end().ends_with("netto")
            || low.contains("dochód netto")
            || low.contains("dochod netto")
        {
            return Some("wdf_calkowity_dochod_netto");
        }
        return Some("wdf_calkowity_dochod");
    }

    if low.contains("zysk") || low.contains("strata") {
        if low.contains("operacyjn") {
            return Some("operating_profit");
        }
        if low.contains("brutto ze sprzedaży") || low.contains("brutto ze sprzedazy") {
            return Some("wdf_zysk_strata_brutto_ze_sprzedazy");
        }
        if low.contains("przed odpisami") {
            return Some("wdf_zysk_netto_przed_odpisami_aktualizujacymi_netto");
        }
        if low.contains("przed opodatkowaniem") || low.contains("brutto") {
            return Some("wdf_pretax_profit");
        }
        // "niedając(ym) kontroli" (Pekao's own wording) is a synonym of
        // "niekontrolując" — both name non-controlling interests, checked
        // ahead of the generic "przypad" arm so an NCI row never lands on
        // `wdf_net_profit_parent`.
        if low.contains("niekontrolując")
            || low.contains("niekontrolujac")
            || low.contains("niedając")
            || low.contains("niedajac")
        {
            return Some("wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace");
        }
        if low.contains("przypad") || low.contains("dominując") || low.contains("dominujac") {
            return Some("wdf_net_profit_parent");
        }
        if low.contains("netto") {
            return Some("net_profit");
        }
        return None;
    }

    if low.contains("aktyw") {
        if low.contains("trwałe") || low.contains("trwale") {
            return Some("wdf_noncurrent_assets");
        }
        if low.contains("obrotowe") {
            return Some("current_assets");
        }
        if low.contains("razem") || low.contains("suma") || low.contains("pasywa") {
            return Some("total_assets");
        }
        return None;
    }

    if low.contains("zobowiązania") || low.contains("zobowiazania") {
        if low.contains("długoterminow") || low.contains("dlugoterminow") {
            return Some("wdf_noncurrent_liabilities");
        }
        if low.contains("krótkoterminow") || low.contains("krotkoterminow") {
            return Some("current_liabilities");
        }
        // Bank-table honesty (epic #277 T2, card #279, evidenced by the real
        // Pekao PSr H1/2026 note): a bank WDF table often carries NO
        // "Zobowiązania razem" row at all, only qualified sub-lines
        // ("wobec innych banków", "wobec klientów", "z tytułu...",
        // "podporządkowane", "finansowe"...). Greedily falling through those
        // to `total_liabilities` produced a 40x-understated fact from the
        // interbank-liabilities row (7 899 000 000 vs the real ~316.9bn).
        // `total_liabilities` is now returned ONLY for an explicit total
        // ("razem"/"ogółem"/"łącznie", or the industrial "zobowiązania i
        // rezerwy na zobowiązania" form) or a bare, unqualified row; customer
        // deposits map to the same `total_deposits` key the T1 aggregator
        // dictionary (`text_numbers.rs`) already uses; every other qualified
        // form — including interbank/central-bank liabilities — abstains
        // (`None`) as a reviewed abstention rather than a guessed total.
        if low.contains("wobec klientów") || low.contains("wobec klientow") {
            return Some("total_deposits");
        }
        let bare = low.trim() == "zobowiązania" || low.trim() == "zobowiazania";
        if bare
            || low.contains("razem")
            || low.contains("ogółem")
            || low.contains("ogolem")
            || low.contains("łącznie")
            || low.contains("lacznie")
            || low.contains("i rezerwy")
        {
            return Some("total_liabilities");
        }
        return None;
    }

    if low.contains("kapitał") || low.contains("kapital") {
        if low.contains("własny") || low.contains("wlasny") {
            // Bank-table honesty (epic #277 T2, card #279, evidenced by the
            // real Pekao PSr H1/2026 note): checked BEFORE the parent-equity
            // arm below, whose "przypisan" trigger otherwise also matches
            // "Kapitał własny przypisany udziałom niedającym kontroli" — the
            // non-controlling-interests row, the exact opposite of parent
            // equity (12 000 000 emitted as `wdf_equity_parent` in place of
            // the real ~32.9bn parent-equity row that follows it in the
            // table). No seeded NCI-equity catalog key exists today (checked
            // against migrations 0111/0112, which seed the parent-equity key
            // and the NCI *profit* key
            // `wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace`
            // respectively, but no NCI-equity counterpart) — inventing one
            // here would need its own catalog seed + definition, out of this
            // card's scope, so this abstains (`None`), a deliberate asymmetry
            // with the profit branch above.
            if low.contains("niedając")
                || low.contains("niedajac")
                || low.contains("niekontrolując")
                || low.contains("niekontrolujac")
            {
                return None;
            }
            if low.contains("przypad")
                || low.contains("przypisan")
                || low.contains("dominując")
                || low.contains("dominujac")
                || low.contains("akcjonariuszy jednostki")
            {
                return Some("wdf_equity_parent");
            }
            return Some("total_equity");
        }
        if low.contains("podstawow")
            || low.contains("zakładow")
            || low.contains("zakladow")
            || low.contains("akcyjn")
        {
            return Some("wdf_share_capital");
        }
    }

    None
}

// ---------------------------------------------------------------------------
// document decomposition
// ---------------------------------------------------------------------------

/// Scope adjectives may sit between "WYBRANE" and "DANE FINANSOWE" (Digital
/// Network Q1 2025 class: "WYBRANE **SKONSOLIDOWANE** DANE FINANSOWE"). Cap the
/// intervening run at a few uppercase words so the anchor stays a header match,
/// not an open-ended scan.
const MAX_HEADER_ADJ_WORDS: usize = 3;

/// Finds the next WDF section header at or after `from`, tolerating a scope
/// adjective (`SKONSOLIDOWANE` / `JEDNOSTKOWE` / `SKRÓCONE` …) between the
/// bracketing phrases: `WYBRANE` + up to [`MAX_HEADER_ADJ_WORDS`] intervening
/// uppercase words + `DANE FINANSOWE`. Returns `(start_of_WYBRANE, index_after_
/// FINANSOWE)`.
///
/// Case-sensitive uppercase, matching how the table-of-contents and section
/// headers render (the lower/title-case "Wybrane dane finansowe" caption is a
/// different thing and deliberately not matched). `DANE FINANSOWE` is tested at
/// every word gap *before* a word is consumed, so the plain form still matches
/// at its shortest — the generalized anchor is byte-for-byte the old behavior on
/// a plain header.
fn find_wdf_header(body: &[char], from: usize) -> Option<(usize, usize)> {
    let wybrane: Vec<char> = "WYBRANE".chars().collect();
    let tail: Vec<char> = "DANE FINANSOWE".chars().collect();
    let mut p = from;
    while let Some(w) = find_from(body, &wybrane, p) {
        let mut q = w + wybrane.len();
        for _ in 0..=MAX_HEADER_ADJ_WORDS {
            // A single run of separating spaces is mandatory before the tail or
            // the next adjective word.
            let r = q + body[q..].iter().take_while(|&&c| c == ' ').count();
            if r == q {
                break;
            }
            if body.len() - r >= tail.len() && body[r..r + tail.len()] == tail[..] {
                return Some((w, r + tail.len()));
            }
            // Otherwise consume one uppercase adjective word and look again.
            let s = r + body[r..].iter().take_while(|&&c| is_upper_pl(c)).count();
            if s == r {
                break;
            }
            q = s;
        }
        p = w + 1;
    }
    None
}

/// Whether a matched header carries the standalone (`JEDNOSTKOWE`) scope — such a
/// header is a sub-section *splitter* (see [`STANDALONE_MARKERS`]), never the
/// main section start when a consolidated/base header exists.
fn header_is_standalone(header: &[char]) -> bool {
    find_str(header, "JEDNOSTKOW", 0).is_some()
}

/// Boilerplate that follows the table — the section ends at the earliest of
/// these.
const END_MARKERS: &[&str] = &[
    "W przypadku prezentowania wybranych danych",
    "INFORMACJA O KOREKCIE",
    "Raport powinien zostać przekazany",
    "Powyższe dane finansowe",
    "Dane prezentowane za",
    "Zastosowane do przeliczeń",
    "Zastosowane kursy",
    "Porównywalne dane dotyczące",
    "Wybrane dane finansowe zostały przeliczone",
    "Zasady przyjęte do przeliczenia",
    "Zaprezentowana kwota",
    "*Dane dla pozycji",
];

/// Markers that begin the standalone (jednostkowe) sub-section.
const STANDALONE_MARKERS: &[&str] = &[
    "WYBRANE JEDNOSTKOWE DANE FINANSOWE",
    "dane dotyczące jednostkowego",
    "Dane dotyczące jednostkowego",
    "Dane dotyczące kwartalnej informacji finansowej",
    "dane dotyczące skróconego sprawozdania",
    "dane dotyczące kwartalnej informacji",
];

const DEFER_MARKERS: &[&str] = &["przedstawione w załączeniu", "przedstawione w zalaczeniu"];

/// Row-label anchors for forms that carry no roman numerals. Order matters:
/// tried in sequence at each position, longest wording first, exactly like the
/// spike's regex alternation.
const LABEL_LEADS: &[&str] = &[
    "Przychody netto ze sprzedaży",
    "Przychody ze sprzedaży",
    "Zysk z działalności operacyjnej powiększony o amortyzację (EBITDA)",
    "EBITDA przed odpisami aktualizującymi netto",
    "Zysk z działalności operacyjnej (EBIT)",
    "Zysk (strata) z działalności operacyjnej",
    "Zysk/(Strata) przed opodatkowaniem",
    "Zysk przed opodatkowaniem",
    "Zysk netto przed odpisami aktualizującymi netto",
    "Zysk netto przypadający na akcjonariuszy jednostki dominującej",
    "Zysk/(Strata) netto i rozwodniony zysk/(strata) netto na jedną akcję",
    "Zysk netto i rozwodniony zysk netto na jedną akcję",
    "Zysk/(Strata) netto",
    "Zysk (strata) netto",
    "Zysk netto",
    "Całkowite dochody netto przypadające na akcjonariuszy jednostki dominującej",
    "Całkowite dochody netto",
    "Środki pieniężne netto z działalności operacyjnej",
    "Środki pieniężne netto (wykorzystane) w działalności inwestycyjnej",
    "Środki pieniężne netto z/(wykorzystane w) działalności inwestycyjnej",
    "Środki pieniężne netto z/(wykorzystane w) działalności finansowej",
    "Środki pieniężne netto z działalności finansowej",
    "Środki pieniężne netto (wykorzystane) w działalności finansowej",
    "Zwiększenie/(Zmniejszenie) netto stanu środków pieniężnych",
    "Zwiększenie netto stanu środków pieniężnych",
    "Aktywa trwałe",
    "Aktywa obrotowe",
    "Aktywa razem",
    "Kapitał podstawowy",
    "Kapitał własny przypadający na akcjonariuszy jednostki dominującej",
    "Kapitał własny razem",
    "Zobowiązania długoterminowe",
    "Zobowiązania krótkoterminowe",
    "Liczba akcji",
    "Wartość księgowa i rozwodniona wartość księgowa na jedną akcję",
];

/// Normalizes the body: non-breaking spaces become spaces, zero-width and
/// left-to-right marks are dropped, and line breaks collapse to spaces (the
/// table is one logical run of text).
fn normalize(body: &str) -> Vec<char> {
    body.chars()
        .filter(|&c| c != '\u{200e}' && c != '\u{200b}')
        .map(|c| match c {
            '\u{a0}' | '\r' | '\n' => ' ',
            other => other,
        })
        .collect()
}

fn roman_value(token: &[char]) -> u32 {
    let mut total = 0u32;
    let mut prev = 0u32;
    for &ch in token.iter().rev() {
        let v = match ch {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            _ => 0,
        };
        if v < prev {
            total = total.saturating_sub(v);
        } else {
            total += v;
            prev = v;
        }
    }
    total
}

fn int_to_roman(mut n: u32) -> String {
    let table = [
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (val, sym) in table {
        while n >= val {
            out.push_str(sym);
            n -= val;
        }
    }
    out
}

/// Canonical round-trip validation: accepts subtractive forms (`IV`, `IX`,
/// `XL`, `XLI`…) and rejects malformed ones (`IIII`, `VV`, `IL`). Getting this
/// wrong silently dropped every row whose numeral contained a subtractive pair
/// during the spike.
fn is_roman(token: &[char]) -> bool {
    let v = roman_value(token);
    (1..=199).contains(&v) && int_to_roman(v) == as_string(token)
}

/// Roman row markers in `section`, as `(start, end_after_dot)`.
///
/// Known ALL-CAPS WDF sub-section headers a bank-format note glues directly
/// onto the following row's roman marker with NO separator — evidenced by
/// the real Bank Pekao PSr H1/2026 note (epic #277 T2b, card #279):
/// `"...ZYSKÓW I STRATI. Wynik..."`, `"...PIENIĘŻNEX. Przepływy..."`,
/// `"...FINANSOWEJXIV. Aktywa..."`, `"...KAPITAŁOWAXXIII. Łączny..."`. A
/// **closed, evidenced vocabulary** — deliberately NOT a general relaxation
/// of the uppercase lookbehind below, which stays blocked for everything
/// else (the ported spike's own rationale: an uppercase-preceded run is
/// otherwise an acronym or series-letter tail, e.g. "...obligacje serii XI."
/// must never become row XI).
const GLUED_SECTION_HEADERS: &[&str] = &[
    "RACHUNEK ZYSKÓW I STRAT",
    "PRZEPŁYWY PIENIĘŻNE",
    "SPRAWOZDANIE Z SYTUACJI FINANSOWEJ",
    "ADEKWATNOŚĆ KAPITAŁOWA",
];

/// Ports `(?<![A-ZĄĆĘŁŃÓŚŻŹ])([IVXLC]{1,8})\.(?=\s?[A-ZĄĆĘŁŃÓŚŻŹ0-9(])`. Roman
/// tokens are themselves uppercase, so only a preceding **uppercase** letter is
/// disqualifying — sub-headers and scope markers run straight into the next
/// numeral with no separator (`…sprawozdania finansowegoI. Przychody`) and must
/// still anchor a row. The blanket uppercase block is itself relaxed ONLY when
/// the marker is glued to one of [`GLUED_SECTION_HEADERS`] (T2b, card #279) —
/// every other uppercase-preceded run (an acronym/series-letter tail) stays
/// blocked, exactly as the ported spike measured it.
fn roman_markers(section: &[char]) -> Vec<(usize, usize)> {
    let n = section.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if !matches!(section[i], 'I' | 'V' | 'X' | 'L' | 'C') {
            i += 1;
            continue;
        }
        // Maximal run of roman characters starting here.
        let mut j = i;
        while j < n && matches!(section[j], 'I' | 'V' | 'X' | 'L' | 'C') {
            j += 1;
        }
        let run = j - i;
        // Lookbehind: the char before the run must not be an uppercase letter
        // — UNLESS the run is glued directly to one of the known
        // GLUED_SECTION_HEADERS (T2b), in which case the header itself
        // disambiguates the boundary and the marker is allowed through.
        let glued_to_known_header = GLUED_SECTION_HEADERS.iter().any(|header| {
            let header: Vec<char> = header.chars().collect();
            i >= header.len() && section[i - header.len()..i] == header[..]
        });
        let blocked = i > 0 && is_upper_pl(section[i - 1]) && !glued_to_known_header;
        // `{1,8}` cannot backtrack onto another roman char, so the run must be
        // 1..=8 long and immediately followed by the dot.
        let ok_shape = run <= 8 && j < n && section[j] == '.';
        let ok_ahead = if ok_shape {
            let after = j + 1;
            let direct = section
                .get(after)
                .is_some_and(|&c| is_upper_pl(c) || c.is_ascii_digit() || c == '(');
            let skipped = section.get(after).is_some_and(|c| c.is_whitespace())
                && section
                    .get(after + 1)
                    .is_some_and(|&c| is_upper_pl(c) || c.is_ascii_digit() || c == '(');
            direct || skipped
        } else {
            false
        };
        if !blocked && ok_shape && ok_ahead && is_roman(&section[i..j]) {
            out.push((i, j + 1));
        }
        // Any start inside the run is blocked by the lookbehind, so skip it.
        i = j.max(i + 1);
    }
    out
}

/// FX plausibility band from the document's own rate footnote (`4,3212`-style
/// tokens), else the default 4.0–4.6 band, both widened by ±18%.
fn fx_band(body: &[char]) -> (f64, f64) {
    let n = body.len();
    let mut rates: Vec<f64> = Vec::new();
    for i in 0..n {
        if !matches!(body[i], '3' | '4' | '5') {
            continue;
        }
        if i > 0 && body[i - 1].is_ascii_digit() {
            continue;
        }
        if i + 5 >= n || body[i + 1] != ',' {
            continue;
        }
        if !body[i + 2..i + 6].iter().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if body.get(i + 6).is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        let text: String = std::iter::once(body[i])
            .chain(std::iter::once('.'))
            .chain(body[i + 2..i + 6].iter().copied())
            .collect();
        if let Ok(v) = text.parse::<f64>() {
            if (3.5..=5.5).contains(&v) {
                rates.push(v);
            }
        }
    }
    if rates.is_empty() {
        return (4.0 * 0.82, 4.6 * 1.18);
    }
    let lo = rates.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = rates.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (lo * 0.82, hi * 1.18)
}

/// Splits a row into its label and its numbers run. Numbers start at the first
/// digit, `-<digit>` or `(<digit>`, and run to the first alphabetic character
/// after — which strips trailing note references and dot leaders.
fn numbers_region(row: &[char]) -> (usize, usize) {
    let n = row.len();
    let mut start = None;
    for i in 0..n {
        let c = row[i];
        let minus = c == '-'
            && row.get(i + 1).is_some_and(|d| d.is_ascii_digit())
            && !(i > 0 && (row[i - 1].is_ascii_digit() || row[i - 1] == ','));
        let paren = c == '(' && {
            let mut k = i + 1;
            while k < n && row[k].is_whitespace() {
                k += 1;
            }
            row.get(k).is_some_and(|d| d.is_ascii_digit())
        };
        if minus || paren || c.is_ascii_digit() {
            start = Some(i);
            break;
        }
    }
    let Some(i) = start else {
        return (n, n);
    };
    let end = row[i..]
        .iter()
        .position(|c| c.is_alphabetic())
        .map_or(n, |p| i + p);
    (i, end)
}

/// Finds the label anchors for a roman-free form, reproducing the spike's
/// non-overlapping leftmost-first regex alternation.
fn label_anchors(section: &[char]) -> Vec<usize> {
    let leads: Vec<Vec<char>> = LABEL_LEADS.iter().map(|s| s.chars().collect()).collect();
    let mut out = BTreeSet::new();
    let mut p = 0;
    while p < section.len() {
        let mut matched = 0;
        for lead in &leads {
            if section.len() - p >= lead.len() && &section[p..p + lead.len()] == lead.as_slice() {
                matched = lead.len();
                break;
            }
        }
        if matched > 0 {
            out.insert(p);
            p += matched;
        } else {
            p += 1;
        }
    }
    out.into_iter().collect()
}

// ---------------------------------------------------------------------------
// value extraction
// ---------------------------------------------------------------------------

/// Reads the leading `\(?-?\d{1,3}(?: \d{3})*,\d{2}\)?` per-share token, with
/// the backtracking the greedy group repetition needs.
fn per_share_value(nums: &[char]) -> Option<Decimal> {
    let n = nums.len();
    let mut i = 0;
    let paren = nums.first() == Some(&'(');
    if paren {
        i += 1;
    }
    let minus = nums.get(i) == Some(&'-');
    if minus {
        i += 1;
    }
    // `\d{1,3}` greedy: longest first.
    for lead in (1..=3usize).rev() {
        if i + lead > n || !nums[i..i + lead].iter().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // Collect every prefix of the ` \d{3}` repetition, then try them
        // longest-first (greedy with backtracking).
        let mut ends = vec![i + lead];
        let mut cur = i + lead;
        while cur + 4 <= n
            && nums[cur] == ' '
            && nums[cur + 1..cur + 4].iter().all(|c| c.is_ascii_digit())
        {
            cur += 4;
            ends.push(cur);
        }
        for &end in ends.iter().rev() {
            if nums.get(end) == Some(&',')
                && nums.get(end + 1).is_some_and(|c| c.is_ascii_digit())
                && nums.get(end + 2).is_some_and(|c| c.is_ascii_digit())
            {
                let digits: String = nums[i..end]
                    .iter()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>();
                let text = format!("{}.{}{}", digits, nums[end + 1], nums[end + 2]);
                let v = Decimal::from_str(&text).ok()?;
                return Some(if paren || minus { -v } else { v });
            }
        }
    }
    None
}

/// Reads the current-period PLN value from a row's numbers run, or `None` to
/// abstain.
fn extract_value(
    key: &str,
    label: &str,
    nums: &[char],
    scale: i64,
    fx: (f64, f64),
    fx_resolved: &mut usize,
) -> Option<Decimal> {
    if is_per_share(key) {
        return per_share_value(nums);
    }

    if key == "shares_outstanding" {
        for k in [4usize, 2, 1] {
            let segs = split_numbers(nums, k);
            let mut firsts: Vec<Decimal> = Vec::new();
            for s in &segs {
                let v = s[0].round();
                if !firsts.contains(&v) {
                    firsts.push(v);
                }
            }
            if firsts.len() == 1 {
                let mut v = firsts[0];
                // The one line the form leaves under-specified: scaled only
                // when the label itself says `(w tys.)`.
                if strip_roman(label).to_lowercase().contains("w tys") {
                    v *= Decimal::from(1000);
                }
                return Some(v);
            }
        }
        return None;
    }

    // Monetary: read as four columns, disambiguated by the PLN↔EUR cross-check.
    let segs = split_numbers(nums, 4);
    if segs.is_empty() {
        return None;
    }
    let mut firsts: Vec<Decimal> = Vec::new();
    for s in &segs {
        if !firsts.contains(&s[0]) {
            firsts.push(s[0]);
        }
    }
    if firsts.len() == 1 {
        return Some(firsts[0] * Decimal::from(scale));
    }

    let (lo, hi) = fx;
    let ok_pair = |pln: Decimal, eur: Decimal| -> bool {
        let (pln, eur) = (pln.abs(), eur.abs());
        if eur.is_zero() {
            // A small PLN value whose EUR column legitimately rounds to zero.
            return pln <= Decimal::from(2);
        }
        match (pln.to_f64(), eur.to_f64()) {
            (Some(p), Some(e)) if e != 0.0 => (lo..=hi).contains(&(p / e)),
            _ => false,
        }
    };

    let mut survivors: Vec<Decimal> = Vec::new();
    for s in &segs {
        if ok_pair(s[0], s[2]) && ok_pair(s[1], s[3]) && !survivors.contains(&s[0]) {
            survivors.push(s[0]);
        }
    }
    if survivors.len() == 1 {
        *fx_resolved += 1;
        return Some(survivors[0] * Decimal::from(scale));
    }
    // Several splits survive with different values — abstain rather than guess.
    None
}

fn format_value(key: &str, value: Decimal) -> Decimal {
    if is_per_share(key) {
        value.round_dp(2)
    } else {
        value.round()
    }
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Parses the "WYBRANE DANE FINANSOWE" cover table out of an ESPI
/// periodic-report komunikat body.
///
/// `period_end` is the reporting period end (ISO `YYYY-MM-DD`) the caller
/// already knows from the komunikat metadata — the cover table itself carries no
/// machine-readable dates. Every emitted fact is stamped with it, following the
/// [`super::html_positional`] precedent, so [`super::fact_set_for_period`]
/// groups cover-note facts alongside the other tiers.
pub fn parse_espi_cover_note(body_text: &str, period_end: &str) -> WdfParseResult {
    let body = normalize(body_text);

    // The WDF section is the *second* occurrence of the header — the first is
    // the report's table of contents. The header may carry a scope adjective
    // (Digital Network class: "WYBRANE SKONSOLIDOWANE DANE FINANSOWE"). The
    // standalone "WYBRANE JEDNOSTKOWE DANE FINANSOWE" is a sub-section splitter,
    // not a section start, so it anchors the section only when it is the report's
    // *only* WDF header (a standalone-only report) — never swallowing its
    // splitter role when a consolidated/base section exists (PKN class).
    let mut primary: Vec<usize> = Vec::new();
    let mut standalone_hdr: Vec<usize> = Vec::new();
    let mut scan = 0;
    while let Some((s, e)) = find_wdf_header(&body, scan) {
        if header_is_standalone(&body[s..e]) {
            standalone_hdr.push(s);
        } else {
            primary.push(s);
        }
        scan = s + 1;
    }
    let start = if primary.len() >= 2 {
        primary[1]
    } else if primary.is_empty() && standalone_hdr.len() >= 2 {
        standalone_hdr[1]
    } else {
        return WdfParseResult::empty(WdfEmptyReason::HeaderOnly, 1000, true);
    };

    let base_consolidated = as_string(&body[..start])
        .to_lowercase()
        .contains("skonsolidowany raport");

    let mut end = body.len();
    for marker in END_MARKERS {
        if let Some(p) = find_str(&body, marker, start + 20) {
            end = end.min(p);
        }
    }
    let section = &body[start..end];

    // Unit scale from the header declaration: whichever of millions / thousands
    // is declared first (Polish `w mln`/`w tys`, or the English `in millions`/
    // `in thousands` forms an English-translation cover note carries). Owner rule
    // 2026-07-21 (ADR 0061): the scale is READ from the declaration — with NO
    // unit declared in the header at all, there is NO multiplier (raw złoty,
    // scale 1), not the historical 1000. Digital Network Q1 2025 class: a
    // "WYBRANE ... DANE FINANSOWE PLN PLN EUR EUR" header declaring nothing, whose
    // groszy-bearing values are raw złoty.
    let head_str = as_string(&section[..section.len().min(160)]).to_lowercase();
    let head_low: Vec<char> = head_str.chars().collect();
    let mln = find_str(&head_low, "w mln", 0).or_else(|| find_str(&head_low, "in millions", 0));
    let tys = find_str(&head_low, "w tys", 0).or_else(|| find_str(&head_low, "in thousands", 0));
    let unit_scale = match (mln, tys) {
        (Some(m), Some(t)) if m < t => 1_000_000,
        (Some(_), None) => 1_000_000,
        (None, None) => 1,
        _ => 1000,
    };

    if DEFER_MARKERS
        .iter()
        .any(|m| find_str(section, m, 0).is_some())
    {
        return WdfParseResult::empty(
            WdfEmptyReason::DeferredToAttachment,
            unit_scale,
            base_consolidated,
        );
    }

    let fx = fx_band(&body);
    let standalone_pos = STANDALONE_MARKERS
        .iter()
        .filter_map(|m| find_str(section, m, 0))
        .min();

    // Row anchors: roman numerals when the form uses them, else known labels.
    let romans = roman_markers(section);
    let anchors: Vec<usize> = if romans.len() >= 3 {
        // Collapse doubled roman prefixes (`I. I.` / `XVII. XV.`): the outer of
        // a pair has an empty label and is dropped.
        let mut collapsed = Vec::new();
        for (i, &(s, e)) in romans.iter().enumerate() {
            if let Some(&(next_start, _)) = romans.get(i + 1) {
                let between = as_string(&section[e..next_start]);
                if between.trim_matches(|c| c == ' ' || c == '.').is_empty() {
                    continue;
                }
            }
            collapsed.push(s);
        }
        collapsed
    } else {
        label_anchors(section)
    };

    if anchors.is_empty() {
        return WdfParseResult::empty(WdfEmptyReason::HeaderOnly, unit_scale, base_consolidated);
    }

    let mut res = WdfParseResult {
        facts: Vec::new(),
        abstained: 0,
        fx_resolved: 0,
        empty_reason: None,
        unit_scale,
        base_consolidated,
    };
    let mut seen: BTreeSet<(String, &'static str)> = BTreeSet::new();

    for (i, &a) in anchors.iter().enumerate() {
        let b = anchors.get(i + 1).copied().unwrap_or(section.len());
        let row = &section[a..b];
        let (num_start, num_end) = numbers_region(row);
        let label = as_string(&row[..num_start]);
        let nums: Vec<char> = row[num_start..num_end]
            .iter()
            .copied()
            .filter(|&c| c != '*')
            .collect();
        let nums: Vec<char> = {
            let s = as_string(&nums);
            s.trim().chars().collect()
        };
        if nums.is_empty() {
            continue;
        }
        let Some(key) = classify(&label) else {
            continue;
        };

        // Standalone when the whole report is standalone (RR/Q forms), or once
        // the row sits past the standalone sub-section marker.
        let standalone = !base_consolidated || standalone_pos.is_some_and(|p| a >= p);
        let basis = if standalone {
            StatementBasis::Standalone
        } else {
            StatementBasis::Consolidated
        };
        if seen.contains(&(key.to_string(), basis.as_str())) {
            continue;
        }

        let Some(value) = extract_value(key, &label, &nums, unit_scale, fx, &mut res.fx_resolved)
        else {
            // Per-share and share-count rows have their own grammar; only a
            // monetary row that failed the cross-check is an abstention.
            if !is_per_share(key) && key != "shares_outstanding" {
                res.abstained += 1;
            }
            continue;
        };

        seen.insert((key.to_string(), basis.as_str()));
        res.facts.push(ExtractedFact {
            metric_key: key.to_string(),
            value: format_value(key, value),
            period: FactPeriod::Instant(period_end.to_string()),
            basis: Some(basis),
            currency: Some("PLN".to_string()),
            tier: SourceTier::EspiCoverNote,
            citation: strip_roman(&label).trim().to_string(),
        });
    }

    if res.facts.is_empty() && res.empty_reason.is_none() {
        res.empty_reason = Some(WdfEmptyReason::HeaderOnly);
    }
    res
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::str::FromStr;

    const PERIOD_END: &str = "2026-03-31";

    /// Golden synthetic body (authored here — never owner private data). Covers
    /// the full row grammar: TOC-vs-section header, `w tys.` unit, roman rows,
    /// parenthesised and leading-minus negatives, an FX-disambiguated split,
    /// share count and a per-share row, plus the FX footnote end marker.
    const GOLDEN_BODY: &str = "RAPORT OKRESOWY QSr 1/2026\n\
Spis: WYBRANE DANE FINANSOWE\n\
Skonsolidowany raport kwartalny QSr 1/2026\n\
WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
I. Przychody ze sprzedaży 100 000 90 000 23 256 20 930\n\
II. Zysk z działalności operacyjnej 10 000 9 000 2 326 2 093\n\
III. Zysk netto 8 000 (1 000) 1 860 (233)\n\
IV. Aktywa razem 500 000 480 000 116 279 111 628\n\
V. Zobowiązania krótkoterminowe -2 000 3 000 -465 698\n\
VI. Liczba akcji 12 500 12 500 12 500 12 500\n\
VII. Zysk na jedną akcję 8,00 7,00 1,86 1,63\n\
Zastosowane kursy: 4,3000\n";

    /// Same form, but `Zysk netto` carries a concatenated digit run (`1801500`)
    /// that the document's own 4,3000 rate does NOT resolve to a single split
    /// (both `18` and `180` survive the PLN↔EUR cross-check).
    const AMBIGUOUS_BODY: &str = "RAPORT OKRESOWY\n\
Spis: WYBRANE DANE FINANSOWE\n\
Skonsolidowany raport kwartalny\n\
WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
I. Przychody ze sprzedaży 100 000 90 000 23 256 20 930\n\
II. Zysk netto 1801500\n\
III. Aktywa razem 500 000 480 000 116 279 111 628\n\
Zastosowane kursy: 4,3000\n";

    /// Verbatim real Bank Pekao S.A. PSr H1/2026 "WYBRANE DANE FINANSOWE"
    /// cover note (feed item `feed_bankier_company_komunikatyarticle9175261`,
    /// a public ESPI periodic report — no owner-private content), used by
    /// the bank-table honesty tests below (epic #277 T2, card #279).
    const PEO_BANK_BODY: &str = r#"Spis treści:1. STRONA TYTUŁOWA2. METADANE ESAP3. WYBRANE DANE FINANSOWE4. KOREKTA RAPORTU5. ZAWARTOŚĆ RAPORTU6. PODPISY OSÓB REPREZENTUJĄCYCH SPÓŁKĘSpis załączników:Wybrane_dane_finansowe_Bank_Pekao_30.06.2026.pdfWybrane_dane_finansowe_Grupa_Banku_Pekao_30.06.2026.pdfSzDZ_Grupy_Banku_Pekao_I_polrocze_2026.pdfJSF_Banku_Pekao_30.06.2026.pdfSSF_Grupy_Banku_Pekao_30.06.2026.pdfRaport_audytora_z_przegladu_JSF_Banku_Pekao_S.A._za_I_polrocze_2026.pdfRaport_audytora_z_przegladu_SSF_Banku_Pekao_S.A._za_I_polrocze_2026.pdfSTRONA TYTUŁOWA>>>skorygowanyKOMISJA NADZRO FINANSOWEGOSkonsolidowany raport półroczny PSr2026(rok)zgodnie z § 61 ust. 2 i § 63 ust. 3 Rozporządzenia w sprawie informacji bieżących i okresowychdlabanków(rodzaj emitenta)za półrocze roku obrotowego2026obejmujące okres2026-01-01do2026-06-30zawierający skonsolidowane sprawozdanie finansowe wedługMSSFw waluciePLNoraz skrócone sprawozdanie finansowe wedługMSSFw waluciePLNdata przekazania:2026-07-30BANK POLSKA KASA OPIEKI SPÓŁKA AKCYJNA(pełna nazwa emitenta)BANK PEKAO S.A.Banki (ban)(skrócona nazwa emitenta)(sektor wg klasyfikacji GPW w Warszawie / branża)01-066WARSZAWA(kod pocztowy)(miejscowość)ŻUBRA1(ulica)(numer)656 00 000000014843(telefon)(fax)bri@pekao.com.pl(e-mail)(www)526-000-68-41000010205(NIP)(REGON)PricewaterhouseCoopers Polska spółka z ograniczoną odpowiedzialnością Audyt sp.k.(firma audytorska)Ramy prawneRodzaj informacji przekazanych przez podmiotTRANSDPółroczne sprawozdanie finansoweTRANSDDodatkowe informacje regulowane, których ujawnienie jest wymagane na mocy przepisów państwa członkowskiegoRegulatoryDataOrgan zbierający dane odpowiedzialny za zbieranie informacjiPLKNFUnikalny identyfikator danychRodzaj przedkładanych informacjinowe (do zastosowania w przypadku nowych informacji)Dobrowolny lub obowiązkowy charakter przedłożonych informacjiData lub początek okresu, której lub którego dotyczą informacje2026-01-01Data lub koniec okresu, której lub którego dotyczą informacje2026-06-30Znacznik danych osobowychMacierzyste państwo członkowskie, w stosownych przypadkachPLDocumentReferenceJęzyk, w którym przekazano informacjeOryginał (ORIG) czy tłumaczenie (TRAN)Numer referencyjny pliku danychPLORIGSubmittingEntityIdentyfikator podmiotu prawnego (LEI) dotyczący podmiotu przekazującego informacje - wypełnij w przypadku osób prawnychlubImię i nazwisko, osoby która przekazała informacje - wypełnij w przypadku osób fizycznychRelatedEntity/LegalPersonIdentyfikator podmiotu prawnego (LEI) przypisany do osoby prawnej, której dotyczą informacjeWielkość osoby prawnej, której dotyczą informacjeSektor(y) przemysłu, w których osoba fizyczna lub prawna, której te informacje dotyczą, prowadzi działalność gospodarczą5493000LKS7B3UTF7H35Duża jednostka(wg regulacji) Instytucja kredytowa5493000LKS7B3UTF7H35Duża grupa(wg regulacji) Instytucja kredytowa WYBRANE DANE FINANSOWEw mln.PLNw tys.EURpółrocze /2026półrocze /2025półrocze /2026półrocze /2025Wybrane skonsolidowane dane finansowe Grupy Kapitałowej Banku Pekao S.A.RACHUNEK ZYSKÓW I STRATI. Wynik z tytułu odsetek6 614,006 860,001 555,001 625,00II. Wynik z tytułu prowizji i opłat1 661,001 497,00391,00355,00III. Zysk brutto4 259,004 286,001 002,001 015,00IV. Zysk netto2 750,003 288,00646,00779,00V. Zysk netto przypadający na akcjonariuszy Banku2 748,003 286,00646,00779,00VI. Zysk netto przypadający na udziały niedające kontroli2,002,00VII. Zysk na akcję zwykłą (w PLN/EUR)10,4712,522,462,97VIII. Rozwodniony zysk na akcję zwykłą (w PLN/EUR)10,4712,522,462,97IX. Wypłacona dywidenda na akcję zwykłą (w PLN/EUR)19,7718,364,654,35PRZEPŁYWY PIENIĘŻNEX. Przepływy pieniężne netto z działalności operacyjnej16 617,005 971,003 908,001 415,00XI. Przepływy pieniężne netto z działalności inwestycyjnej-3 419,002 050,00-804,00486,00XII. Przepływy pieniężne netto z działalności finansowej-4 004,00-4 358,00-942,00-1 033,00XIII. Przepływy pieniężne netto razem9 194,003 663,002 162,00868,00SPRAWOZDANIE Z SYTUACJI FINANSOWEJXIV. Aktywa razem367 789,00352 233,0085 606,0083 335,00XV. Zobowiązania wobec innych banków7 899,005 748,001 839,001 360,00XVI. Zobowiązania wobec klientów284 801,00269 552,0066 290,0063 774,00XVII. Kapitał własny przypisany udziałom niedającym kontroli12,0014,003,003,00XVIII. Kapitał własny przypisany akcjonariuszom Banku32 881,0035 348,007 653,008 363,00XIX. Kapitał zakładowy262,00262,0061,0062,00XX. Liczba akcji (w szt.)262 470 034,00262 470 034,00262 470 034,00262 470 034,00XXI. Wartość księgowa na jedną akcję (w PLN/EUR)125,28134,6729,1631,86XXII. Rozwodniona wartość księgowa na jedną akcję (w PLN/EUR)125,28134,6729,1631,86ADEKWATNOŚĆ KAPITAŁOWAXXIII. Łączny współczynnik kapitałowy (%)16,8017,1016,8017,10XXIV. Aktywa ważone ryzykiem186 468,00175 935,0043 402,0041 625,00XXV. Kapitał Tier I27 623,0027 533,006 429,006 514,00XXVI. Kapitał Tier II3 683,002 464,00857,00583,00Dane finansowe w powyższej tabeli są prezentowane w mln EUR, a nie w tys. EUR(*) Dane za 31 grudnia 2025 roku zostały przeliczone z uwzględnieniem retrospektywnego zaliczenia części zysku za 2025 rok (po podziale wyniku przez Walne Zgromadzenie Akcjonariuszy), zgodnie ze stanowiskiem EBA wyrażonym w Q&A 2018_3822 oraz Q&A 2018_4085."Do przeliczenia wybranych pozycji ze złotych na EUR zastosowano następujące kursy:• do przeliczenia pozycji bilansowych średni kurs ogłoszony przez NBP na 30.06.2026 - 1 EUR = 4,2963 PLN oraz na 31.12.2025 - 1 EUR =4,2267 PLN,• do przeliczenia pozycji rachunku zysków i strat, pozycji przepływów pieniężnych oraz dywidendy - średnie arytmetyczne średnich kursów ogłoszonych przez NBPna ostatni dzień każdego miesiąca odpowiednio za I półrocze 2026 roku oraz za I półrocze 2025 roku - 1 EUR = 4,2522 PLN oraz 1 EUR = 4,2208 PLN.W przypadku prezentowania wybranych danych finansowych z półrocznego skróconego sprawozdania finansowego dane te należy odpowiednio opisać.Wybrane dane finansowe ze skonsolidowanego bilansu (skonsolidowanego sprawozdania z sytuacji finansowej) lub odpowiednio z bilansu (sprawozdania z sytuacji finansowej) prezentuje się na koniec półrocza bieżącego roku obrotowego i na koniec poprzedniego roku obrotowego, co należy odpowiednio opisać.Raport powinien zostać przekazany Komisji Nadzoru Finansowego, spółce prowadzącej rynek regulowany oraz do publicznej wiadomości za pośrednictwem agencji informacyjnej zgodnie z przepisami prawa.INFORMACJA O KOREKCIE RAPORTUSkorygowany raportem bieżącym nrz dniao treści:PlikOpisZAWARTOŚĆ RAPORTURozszerzony skonsolidowany raport półroczny powinien zawierać składniki i informacje zgodnie z przepisami Rozporządzenia w sprawie informacji bieżących i okresowych lub odpowiednio zgodnie z art. 56 ust. 1 pkt 2 lit. b i art. 61 Ustawy o ofercie lub odpowiednio zgodnie z art. 56 ust. 1 pkt 2 i ust. 6 tej UstawyPlikOpisWybrane_dane_finansowe_Bank_Pekao_30.06.2026.pdfWybrane dane finansowe_Bank Pekao_30.06.2026Wybrane_dane_finansowe_Grupa_Banku_Pekao_30.06.2026.pdfWybrane dane finansowe_Grupa Banku Pekao_30.06.2026SzDZ_Grupy_Banku_Pekao_I_polrocze_2026.pdfSzDZ_Grupy_Banku_Pekao_I_pólrocze_2026JSF_Banku_Pekao_30.06.2026.pdfJSF_Banku_Pekao_30.06.2026SSF_Grupy_Banku_Pekao_30.06.2026.pdfSSF_Grupy_Banku_Pekao_30.06.2026Raport_audytora_z_przegladu_JSF_Banku_Pekao_S.A._za_I_polrocze_2026.pdfRaport_audytora_z_przeglądu_JSF_Banku_Pekao_S.A._za_I_półrocze_2026Raport_audytora_z_przegladu_SSF_Banku_Pekao_S.A._za_I_polrocze_2026.pdfRaport_audytora_z_przeglądu_SSF_Banku_Pekao_S.A._za_I_półrocze_2026PODPISY OSÓB REPREZENTUJĄCYCH SPÓŁKĘDataImię i NazwiskoStanowisko/FunkcjaPodpis2026-07-29Cezary StypułkowskiPrezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym2026-07-29Marcin GadomskiWiceprezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym2026-07-29Łukasz JanuszewskiWiceprezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym2026-07-29Michał PanowiczWiceprezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym2026-07-29Robert SochackiWiceprezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym2026-07-29Błażej SzczeckiWiceprezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym2026-07-29Dagmara WojnarWiceprezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym2026-07-29Marcin ZygmanowskiWiceprezes Zarządu BankuPodpisano kwalifikowanym podpisem elektronicznym"#;

    fn keys(res: &WdfParseResult) -> Vec<String> {
        res.facts.iter().map(|f| f.metric_key.clone()).collect()
    }

    // -- Test 2: the abstain gate (no private data) --------------------------

    #[test]
    fn abstains_when_fx_does_not_resolve_a_unique_split() {
        let res = parse_espi_cover_note(AMBIGUOUS_BODY, PERIOD_END);
        assert!(
            !keys(&res).contains(&"net_profit".to_string()),
            "ambiguous concatenated run must emit NOTHING, got {:?}",
            keys(&res)
        );
        assert_eq!(res.abstained, 1, "the refused row must be counted");
        // The unambiguous rows in the same document still emit.
        assert!(keys(&res).contains(&"revenue".to_string()));
        assert!(keys(&res).contains(&"total_assets".to_string()));
    }

    #[test]
    fn emits_every_grammar_form_from_the_golden_body() {
        let res = parse_espi_cover_note(GOLDEN_BODY, PERIOD_END);
        let got: BTreeMap<String, Decimal> = res
            .facts
            .iter()
            .map(|f| (f.metric_key.clone(), f.value))
            .collect();
        let expect = [
            ("revenue", "100000000"),
            ("operating_profit", "10000000"),
            ("net_profit", "8000000"),
            ("total_assets", "500000000"),
            ("current_liabilities", "-2000000"),
            ("shares_outstanding", "12500"),
            ("eps_basic", "8.00"),
        ];
        for (k, v) in expect {
            assert_eq!(
                got.get(k),
                Some(&Decimal::from_str(v).unwrap()),
                "metric {k}"
            );
        }
        assert_eq!(res.abstained, 0);
        assert_eq!(res.unit_scale, 1000);
        assert!(res.base_consolidated);
        assert!(res
            .facts
            .iter()
            .all(|f| f.tier == SourceTier::EspiCoverNote));
        assert!(res
            .facts
            .iter()
            .all(|f| f.basis == Some(StatementBasis::Consolidated)));
        assert!(res.facts.iter().all(|f| !f.citation.is_empty()));
    }

    /// Owner rule 2026-07-21 (Digital Network class): a WDF cover table whose
    /// header declares NO unit — a bare "PLN PLN EUR EUR" header, no `w tys.` /
    /// `w mln` — resolves to scale 1 (raw złoty), NOT the historical 1000.
    /// Scale is read from the declaration; with nothing declared there is no
    /// multiplier. Values carry groszy, corroborating the raw denomination.
    #[test]
    fn cover_note_without_unit_declaration_is_raw_units() {
        let body = "Spis: WYBRANE DANE FINANSOWE\n\
                    Skonsolidowany raport kwartalny\n\
                    WYBRANE DANE FINANSOWE PLN PLN EUR EUR\n\
                    I. Przychody ze sprzedaży 15 395 950 12 346 878 3 679 017 2 857 346\n\
                    Zastosowane kursy: 4,3000\n";
        let res = parse_espi_cover_note(body, PERIOD_END);
        assert_eq!(
            res.unit_scale, 1,
            "no unit declaration ⇒ raw złoty (scale 1)"
        );
        let revenue = res.facts.iter().find(|f| f.metric_key == "revenue");
        assert_eq!(
            revenue.map(|f| f.value),
            Some(Decimal::from_str("15395950").unwrap()),
            "raw value must not be scaled ×1000"
        );
    }

    /// Owner dogfooding 2026-07-21 (Digital Network Q1 2025 class): the mandated
    /// cover table is headed "WYBRANE SKONSOLIDOWANE DANE FINANSOWE" — a scope
    /// adjective sits BETWEEN the anchor words, so the plain-substring anchor
    /// never found the section and the whole issuer class abstained. The header
    /// must be recognized as "WYBRANE" + scope adjective(s) + "DANE FINANSOWE".
    /// Values are RAW złoty (no unit declaration ⇒ scale 1, owner rule) and carry
    /// groszy; the row labels use the "Przychody netto ze sprzedaży" wording.
    #[test]
    fn skonsolidowane_adjective_header_is_found_and_emits_raw_revenue() {
        let body = "GRUPA KAPITAŁOWA DIGITAL NETWORK S.A.\n\
                    Skrócony skonsolidowany raport za I kwartał 2025 roku\n\
                    Spis: WYBRANE SKONSOLIDOWANE DANE FINANSOWE\n\
                    1. WYBRANE SKONSOLIDOWANE DANE FINANSOWE\n\
                    PLN PLN EUR EUR\n\
                    01.01.2025 31.03 2025 01.01.2024 31.03 2024\n\
                    Skonsolidowane sprawozdanie z zysków lub strat oraz innych całkowitych dochodów\n\
                    Przychody netto ze sprzedaży 15 395 950,73 12 346 878,97 3 679 017,09 2 857 346,27\n\
                    Zysk (strata) z działalności operacyjnej 5 508 539,09 3 166 713,58 1 316 320,75 732 848,95\n\
                    Zysk (strata) netto 5 018 228,99 3 296 239,15 1 199 156,23 762 824,08\n\
                    Zastosowane kursy: 4,3212\n";
        let res = parse_espi_cover_note(body, PERIOD_END);
        assert_eq!(
            res.unit_scale, 1,
            "no unit declaration ⇒ raw złoty (scale 1)"
        );
        let got: BTreeMap<String, Decimal> = res
            .facts
            .iter()
            .map(|f| (f.metric_key.clone(), f.value))
            .collect();
        // Raw values (round of the raw złoty figure) — NOT ×1000: revenue ×1000
        // would be 15_395_950_730. The SKONSOLIDOWANE-adjective header must be
        // FOUND for any of these to appear at all.
        for (k, v) in [
            ("revenue", "15395951"),
            ("operating_profit", "5508539"),
            ("net_profit", "5018229"),
        ] {
            assert_eq!(
                got.get(k),
                Some(&Decimal::from_str(v).unwrap()),
                "DIG metric {k} missing/wrong; got {:?}",
                keys(&res)
            );
        }
        assert!(res
            .facts
            .iter()
            .all(|f| f.basis == Some(StatementBasis::Consolidated)));
    }

    /// Test (c): a report whose FIRST header carries the SKONSOLIDOWANE adjective
    /// and which LATER carries the "WYBRANE JEDNOSTKOWE DANE FINANSOWE" standalone
    /// splitter must still split consolidated vs standalone exactly as a
    /// plain-header document does — the generalized main anchor must NOT swallow
    /// the standalone marker's role.
    #[test]
    fn skonsolidowane_header_with_jednostkowe_splitter_still_splits_scopes() {
        let body = "Skonsolidowany raport kwartalny QSr 1/2026\n\
                    Spis: WYBRANE SKONSOLIDOWANE DANE FINANSOWE\n\
                    WYBRANE SKONSOLIDOWANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
                    I. Przychody ze sprzedaży 100 000 90 000 23 256 20 930\n\
                    WYBRANE JEDNOSTKOWE DANE FINANSOWE w tys. PLN w tys. EUR\n\
                    I. Przychody ze sprzedaży 50 000 45 000 11 628 10 465\n\
                    Zastosowane kursy: 4,3000\n";
        let res = parse_espi_cover_note(body, PERIOD_END);
        let cons = res
            .facts
            .iter()
            .find(|f| f.metric_key == "revenue" && f.basis == Some(StatementBasis::Consolidated));
        let stand = res
            .facts
            .iter()
            .find(|f| f.metric_key == "revenue" && f.basis == Some(StatementBasis::Standalone));
        assert_eq!(
            cons.map(|f| f.value),
            Some(Decimal::from_str("100000000").unwrap()),
            "consolidated revenue (before the jednostkowe splitter) missing; got {:?}",
            keys(&res)
        );
        assert_eq!(
            stand.map(|f| f.value),
            Some(Decimal::from_str("50000000").unwrap()),
            "standalone revenue (after the jednostkowe splitter) missing; got {:?}",
            keys(&res)
        );
        assert!(res.base_consolidated);
    }

    /// Test (b): a report whose ONLY WDF header is "WYBRANE JEDNOSTKOWE DANE
    /// FINANSOWE" (no consolidated section at all) parses as a standalone report.
    #[test]
    fn jednostkowe_only_header_parses_as_standalone() {
        let body = "Skrócony raport kwartalny za I kwartał 2026\n\
                    Spis: WYBRANE JEDNOSTKOWE DANE FINANSOWE\n\
                    WYBRANE JEDNOSTKOWE DANE FINANSOWE w tys. PLN w tys. EUR\n\
                    I. Przychody ze sprzedaży 100 000 90 000 23 256 20 930\n\
                    Zastosowane kursy: 4,3000\n";
        let res = parse_espi_cover_note(body, PERIOD_END);
        assert!(
            !res.base_consolidated,
            "no consolidated title ⇒ standalone base"
        );
        let revenue = res.facts.iter().find(|f| f.metric_key == "revenue");
        assert_eq!(
            revenue.map(|f| f.value),
            Some(Decimal::from_str("100000000").unwrap()),
            "jednostkowe-only header must be FOUND; got {:?}",
            keys(&res)
        );
        assert_eq!(
            revenue.and_then(|f| f.basis),
            Some(StatementBasis::Standalone)
        );
    }

    // -- Test 4: golden insta snapshot ---------------------------------------

    #[test]
    fn golden_snapshot() {
        let res = parse_espi_cover_note(GOLDEN_BODY, PERIOD_END);
        insta::assert_debug_snapshot!("espi_cover_note_golden", res);
    }

    // -- Test 5: bank-table honesty (epic #277 T2, card #279) ---------------
    //
    // `PEO_BANK_BODY` is the REAL "WYBRANE DANE FINANSOWE" cover note Bank
    // Pekao S.A. published for PSr H1/2026 (feed item
    // `feed_bankier_company_komunikatyarticle9175261`, a public ESPI periodic
    // report — no owner-private content), verbatim as the Bankier ingest seam
    // stores it (body_text, no HTML). It is the note that produced the two
    // LIVE WRONG facts this card fixes: a bare `zobowiązania` stem greedily
    // mapped the interbank-liabilities row ("Zobowiązania wobec innych
    // banków", row XV) to `total_liabilities` (40x understated — the real
    // total, ~316.9bn, has no "razem"/"ogółem" row in this bank-format note
    // at all), and the equity branch's "przypisan" trigger mapped the
    // non-controlling-interests row ("Kapitał własny przypisany udziałom
    // niedającym kontroli", row XVII) onto `wdf_equity_parent`, shadowing the
    // real parent-equity row (row XVIII) that follows it and shares the same
    // slot.
    const PERIOD_END_PEO: &str = "2026-06-30";

    #[test]
    fn classify_bank_liabilities_rows_by_evidence() {
        // Interbank / central-bank liabilities: qualified, not a total — abstain.
        assert_eq!(classify("Zobowiązania wobec innych banków"), None);
        assert_eq!(classify("Zobowiązania wobec Banku Centralnego"), None);
        // Customer deposits: the same mapping T1 added to the aggregator
        // dictionary (`text_numbers.rs`) — kept consistent here.
        assert_eq!(
            classify("Zobowiązania wobec klientów"),
            Some("total_deposits")
        );
        // A genuine total: "razem"/"ogółem"/"łącznie", the industrial "i
        // rezerwy" form, or a bare unqualified row.
        assert_eq!(classify("Zobowiązania razem"), Some("total_liabilities"));
        assert_eq!(classify("Zobowiązania ogółem"), Some("total_liabilities"));
        assert_eq!(
            classify("Zobowiązania i rezerwy na zobowiązania"),
            Some("total_liabilities")
        );
        assert_eq!(classify("Zobowiązania"), Some("total_liabilities"));
        // Every other qualified form is a reviewed abstention, never a guess.
        assert_eq!(classify("Zobowiązania z tytułu dostaw i usług"), None);
        assert_eq!(classify("Zobowiązania podporządkowane"), None);
        assert_eq!(classify("Zobowiązania finansowe"), None);
        // The existing długo/krótkoterminowe arms are untouched.
        assert_eq!(
            classify("Zobowiązania długoterminowe"),
            Some("wdf_noncurrent_liabilities")
        );
        assert_eq!(
            classify("Zobowiązania krótkoterminowe"),
            Some("current_liabilities")
        );
    }

    #[test]
    fn classify_never_maps_non_controlling_equity_to_parent_equity() {
        // No seeded NCI-equity catalog key exists today (checked against the
        // 0111/0112 migrations) — the deliberate asymmetry with
        // `wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace` noted
        // at the mapping site. An abstention here is honest; the wrong parent
        // key is not.
        assert_eq!(
            classify("Kapitał własny przypisany udziałom niedającym kontroli"),
            None
        );
        assert_eq!(
            classify("Kapitał własny przypadający udziałom niekontrolującym"),
            None
        );
        // The real parent-equity row still maps correctly.
        assert_eq!(
            classify("Kapitał własny przypisany akcjonariuszom Banku"),
            Some("wdf_equity_parent")
        );
    }

    #[test]
    fn classify_non_controlling_net_profit_never_becomes_parent_profit() {
        // PEO's own wording ("udziały niedające kontroli") is a synonym of the
        // "niekontrolując" stem the profit branch already recognized elsewhere
        // in the catalog (0112's seeded
        // `wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace`) but
        // did not match — so this row fell through to the "przypad" arm and
        // silently collided with the real parent-profit row's slot.
        assert_eq!(
            classify("Zysk netto przypadający na udziały niedające kontroli"),
            Some("wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace")
        );
        assert_eq!(
            classify("Zysk netto przypadający na akcjonariuszy Banku"),
            Some("wdf_net_profit_parent")
        );
    }

    #[test]
    fn classify_bank_income_rows_map_to_bank_keys() {
        assert_eq!(
            classify("Wynik z tytułu odsetek"),
            Some("net_interest_income")
        );
        assert_eq!(
            classify("Wynik z tytułu prowizji i opłat"),
            Some("net_fee_commission_income")
        );
    }

    /// End-to-end proof that the `net_interest_income` mapping itself
    /// extracts correctly on a minimal synthetic body (kept alongside the
    /// full real-note golden test
    /// [`peo_bank_note_never_emits_wrong_total_liabilities_or_nci_as_parent_equity`]
    /// below, which since T2b also covers the row-glued-to-a-known-header
    /// case with the real Pekao text).
    #[test]
    fn bank_income_row_extracts_end_to_end_when_its_anchor_is_reachable() {
        let body = "RAPORT PÓŁROCZNY PSr\n\
Spis: WYBRANE DANE FINANSOWE\n\
Skonsolidowany raport półroczny\n\
WYBRANE DANE FINANSOWE w mln. PLN w tys. EUR\n\
I. Wynik z tytułu odsetek 6 614,00 6 860,00 1 555,00 1 625,00\n\
II. Wynik z tytułu prowizji i opłat 1 661,00 1 497,00 391,00 355,00\n\
III. Zysk netto 2 750,00 3 288,00 646,00 779,00\n\
Do przeliczenia zastosowano kursy: 1 EUR = 4,2522 PLN oraz 1 EUR = 4,2208 PLN.\n";
        let res = parse_espi_cover_note(body, PERIOD_END_PEO);
        let got: BTreeMap<String, Decimal> = res
            .facts
            .iter()
            .map(|f| (f.metric_key.clone(), f.value))
            .collect();
        assert_eq!(
            got.get("net_interest_income"),
            Some(&Decimal::from_str("6614000000").unwrap())
        );
        assert_eq!(
            got.get("net_fee_commission_income"),
            Some(&Decimal::from_str("1661000000").unwrap())
        );
    }

    /// The full real Pekao PSr H1/2026 note through the extractor: neither
    /// misclassification the two live-wrong facts trace to may reappear, the
    /// bank-specific rows the note actually carries must emit under their own
    /// keys, and every previously-correct row (net profit, parent profit,
    /// EPS, dividend, cash flows, share data) must emit identically to
    /// before this change.
    ///
    /// **T2b (card #279 follow-up):** rows I ("Wynik z tytułu odsetek" —
    /// `net_interest_income`), X ("Przepływy pieniężne netto z działalności
    /// operacyjnej" — `operating_cash_flow`) and XIV ("Aktywa razem" —
    /// `total_assets`) each immediately follow an ALL-CAPS sub-heading with
    /// NO separating space ("...ZYSKÓW I STRATI.", "...PIENIĘŻNEX.",
    /// "...FINANSOWEJXIV."). `roman_markers`'s lookbehind now recognizes a
    /// marker glued to one of the closed set of known WDF sub-section
    /// headers (`GLUED_SECTION_HEADERS`) while keeping every other
    /// uppercase-preceded run blocked (the acronym/series-letter-tail risk
    /// the ported spike's guard exists for) — so these three rows anchor and
    /// emit like every other row.
    #[test]
    fn peo_bank_note_never_emits_wrong_total_liabilities_or_nci_as_parent_equity() {
        let res = parse_espi_cover_note(PEO_BANK_BODY, PERIOD_END_PEO);
        let got: BTreeMap<String, Decimal> = res
            .facts
            .iter()
            .map(|f| (f.metric_key.clone(), f.value))
            .collect();

        // The two live-wrong facts: total_liabilities must never emit from
        // this note (no unqualified/total liabilities row exists in it), and
        // wdf_equity_parent must be the REAL parent-equity value, never the
        // NCI row's.
        assert!(
            !got.contains_key("total_liabilities"),
            "no unqualified/total liabilities row exists in this bank note, \
             got {:?}",
            got.get("total_liabilities")
        );
        assert_eq!(
            got.get("wdf_equity_parent"),
            Some(&Decimal::from_str("32881000000").unwrap()),
            "wdf_equity_parent must be row XVIII (parent equity), never row \
             XVII (NCI equity)"
        );

        // Row VI ("Zysk netto przypadający na udziały niedające kontroli",
        // the NCI-profit row) carries only 2 numbers in the real filing (its
        // EUR columns are blank) where the grammar needs 4 — it legitimately
        // ABSTAINS (a pre-existing, unrelated column-count limitation), but
        // critically it must never land on `wdf_net_profit_parent` (its
        // pre-fix behavior, silently shadowing row V's slot).
        assert!(
            !got.contains_key("wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace"),
            "row VI has only 2 of the expected 4 columns and must abstain, \
             not emit"
        );
        assert_eq!(res.abstained, 1, "row VI is the sole abstention");

        // Bank-specific rows the note actually carries, including the two
        // (row I, row X) whose marker is glued directly to a known WDF
        // sub-section header with no separator (T2b).
        let expect_bank = [
            ("total_deposits", "284801000000"),
            ("net_fee_commission_income", "1661000000"),
            ("net_interest_income", "6614000000"),
            ("operating_cash_flow", "16617000000"),
        ];
        for (k, v) in expect_bank {
            assert_eq!(
                got.get(k),
                Some(&Decimal::from_str(v).unwrap()),
                "bank metric {k}"
            );
        }

        // Previously-correct rows still emit identically, including
        // total_assets (row XIV — also glued to a known header, T2b).
        let expect_unchanged = [
            ("net_profit", "2750000000"),
            ("wdf_net_profit_parent", "2748000000"),
            ("wdf_pretax_profit", "4259000000"),
            ("eps_basic", "10.47"),
            ("eps_diluted", "10.47"),
            ("dividend_per_share", "19.77"),
            ("investing_cash_flow", "-3419000000"),
            ("financing_cash_flow", "-4004000000"),
            ("wdf_net_cash_change", "9194000000"),
            ("shares_outstanding", "262470034"),
            ("wdf_share_capital", "262000000"),
            ("total_assets", "367789000000"),
        ];
        for (k, v) in expect_unchanged {
            assert_eq!(
                got.get(k),
                Some(&Decimal::from_str(v).unwrap()),
                "unaffected metric {k}"
            );
        }
    }

    // -- R3: cross-authority dictionary-parity guard -------------------------

    /// Two authorities map financial-statement line labels → `metric_key`: this
    /// cover-note `classify` (stem rules) and `super::text_numbers::default_dictionary`
    /// (exact normalized-label lookup). They are structurally different and
    /// deliberately NOT unified (documented known-debt, see `classify`'s doc
    /// comment). This guard pins the drift CLASS without unifying: for every pdf
    /// dictionary label whose `metric_key` is one of the shared canonical keys,
    /// the cover-note classifier must either ABSTAIN (`None`) or return the SAME
    /// key — never a DIFFERENT one — save for the explicitly-documented known
    /// divergence below. A NEW divergence (either mapping drifting) reddens here.
    ///
    /// `default_dictionary` is module-private to `text_numbers` is out of this
    /// slice's scope, so its shared-key entries are hand-mirrored here; if text_numbers
    /// gains a new shared-key label, mirror it into `PDF_SHARED_LABELS` too.
    #[test]
    fn cover_note_classifier_never_contradicts_pdf_dictionary_on_shared_keys() {
        // The 9 canonical keys both authorities map (classify's doc comment).
        const SHARED_KEYS: &[&str] = &[
            "revenue",
            "operating_profit",
            "net_profit",
            "total_assets",
            "total_equity",
            "current_assets",
            "current_liabilities",
            "eps_basic",
            "eps_diluted",
        ];
        // Mirror of `super::text_numbers::default_dictionary` entries whose metric_key is
        // in SHARED_KEYS (normalized lowercase labels, as pdf stores them).
        const PDF_SHARED_LABELS: &[(&str, &str)] = &[
            ("aktywa razem", "total_assets"),
            ("suma aktywów", "total_assets"),
            ("aktywa ogółem", "total_assets"),
            ("kapitał własny", "total_equity"),
            ("kapitał własny razem", "total_equity"),
            ("razem kapitał własny", "total_equity"),
            ("aktywa obrotowe", "current_assets"),
            ("zobowiązania krótkoterminowe", "current_liabilities"),
            ("przychody netto ze sprzedaży", "revenue"),
            ("przychody ze sprzedaży", "revenue"),
            ("przychody z umów z klientami", "revenue"),
            ("zysk z działalności operacyjnej", "operating_profit"),
            (
                "zysk (strata) z działalności operacyjnej",
                "operating_profit",
            ),
            ("zysk operacyjny", "operating_profit"),
            ("zysk netto", "net_profit"),
            ("zysk (strata) netto", "net_profit"),
            (
                "zysk netto przypisany akcjonariuszom jednostki dominującej",
                "net_profit",
            ),
            ("zysk na jedną akcję", "eps_basic"),
            ("zysk na akcję", "eps_basic"),
            ("rozwodniony zysk na akcję", "eps_diluted"),
        ];

        // The one divergence that exists today and is intentionally left in place
        // (reported, not silently adjusted): pdf maps the *parent-attributable*
        // net-profit line onto the generic `net_profit`, while the WDF classifier
        // refines it to the distinct `wdf_net_profit_parent`. Arguably pdf's
        // mapping is the coarse one; changing either mapping is a separate,
        // flagged decision — not this guard's job.
        const KNOWN_DIVERGENCES: &[(&str, &str, &str)] = &[(
            "zysk netto przypisany akcjonariuszom jednostki dominującej",
            "net_profit",
            "wdf_net_profit_parent",
        )];

        let mut divergences: Vec<(&str, &str, &str)> = Vec::new();
        for &(label, pdf_key) in PDF_SHARED_LABELS {
            assert!(
                SHARED_KEYS.contains(&pdf_key),
                "fixture label {label:?} maps to non-shared key {pdf_key:?}"
            );
            match classify(label) {
                None => {}                    // abstain — allowed
                Some(k) if k == pdf_key => {} // same key — agreement
                Some(k) => divergences.push((label, pdf_key, k)),
            }
        }

        assert_eq!(
            divergences,
            KNOWN_DIVERGENCES.to_vec(),
            "cover-note classifier diverged from text_numbers::default_dictionary on a \
             shared key outside the documented known set — investigate before \
             adjusting either mapping"
        );
    }

    // -- Test 3: proptest invariants (ADR 0049) ------------------------------

    mod properties {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// No panic on arbitrary row text, and the parse is deterministic.
            #[test]
            fn never_panics_and_is_deterministic(row in "\\PC{0,240}") {
                let body = format!(
                    "Spis: WYBRANE DANE FINANSOWE\n\
                     Skonsolidowany raport\n\
                     WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
                     I. {row}\n\
                     II. {row}\n\
                     III. {row}\n\
                     Zastosowane kursy: 4,3000\n"
                );
                let a = parse_espi_cover_note(&body, PERIOD_END);
                let b = parse_espi_cover_note(&body, PERIOD_END);
                prop_assert_eq!(a.facts, b.facts);
            }

            /// Idempotence of the emitted values: re-parsing a body whose rows
            /// were re-rendered from the parser's own citation+raw view is
            /// stable, and a metric never appears twice per basis.
            #[test]
            fn metric_basis_identity_is_unique(
                rev in 1i64..900,
                prof in 1i64..900,
            ) {
                let body = format!(
                    "Spis: WYBRANE DANE FINANSOWE\n\
                     Skonsolidowany raport\n\
                     WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
                     I. Przychody ze sprzedaży {rev} {rev} 1 1\n\
                     II. Zysk netto {prof} {prof} 1 1\n\
                     III. Aktywa razem {rev} {rev} 1 1\n\
                     Zastosowane kursy: 4,3000\n"
                );
                let res = parse_espi_cover_note(&body, PERIOD_END);
                let mut seen = std::collections::BTreeSet::new();
                for f in &res.facts {
                    prop_assert!(
                        seen.insert((f.metric_key.clone(), f.basis.map(|b| b.as_str()))),
                        "duplicate metric/basis {}", f.metric_key
                    );
                    prop_assert_eq!(f.tier, SourceTier::EspiCoverNote);
                    prop_assert_eq!(f.period.end_date(), PERIOD_END);
                }
            }

            /// Whitespace-only and marker-free input never yields facts.
            #[test]
            fn junk_yields_no_facts(s in "\\PC{0,200}") {
                let res = parse_espi_cover_note(&s, PERIOD_END);
                if !s.contains("WYBRANE DANE FINANSOWE") {
                    prop_assert!(res.facts.is_empty());
                    prop_assert_eq!(res.empty_reason, Some(WdfEmptyReason::HeaderOnly));
                }
            }
        }
    }

    // -- Test 1: the acceptance bar — hand-labeled ground-truth corpus --------

    #[derive(Debug, serde::Deserialize)]
    struct GtFact {
        metric_key: String,
        scope: String,
        value: String,
    }

    #[derive(Debug, serde::Deserialize)]
    struct GtDocument {
        name: String,
        period: GtPeriod,
        facts: Vec<GtFact>,
    }

    #[derive(Debug, serde::Deserialize)]
    struct GtPeriod {
        to: String,
    }

    #[derive(Debug, serde::Deserialize)]
    struct GroundTruth {
        documents: Vec<GtDocument>,
    }

    fn spike_dir() -> PathBuf {
        if let Ok(d) = std::env::var("BRAWLER_WDF_SPIKE_DIR") {
            return PathBuf::from(d);
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("private/realdata/spikes/espi-wdf")
    }

    fn num_eq(a: Decimal, b: Decimal) -> bool {
        (a - b).abs() < Decimal::from_str("0.005").unwrap()
    }

    /// Pinned acceptance bar (ADR 0061 tier 2a / plan A1): the Rust port must
    /// reproduce the Python spike's measured 347/347 recall and precision with
    /// zero false values and zero abstentions on the labeled set.
    ///
    /// `#[ignore]` + env-gated like the other real-data harnesses — `private/`
    /// is gitignored owner data. Run it from `src-tauri/`:
    /// ```text
    /// cargo nextest run -p brawler espi_cover_note --run-ignored all --no-capture
    /// ```
    #[test]
    #[ignore = "requires the owner's private espi-wdf spike corpus"]
    fn ground_truth_corpus_347_of_347() {
        let dir = spike_dir();
        let gt_path = dir.join("ground_truth.json");
        if !gt_path.is_file() {
            eprintln!(
                "SKIP: espi-wdf ground truth not found at {} \
                 (set BRAWLER_WDF_SPIKE_DIR to the spike directory)",
                gt_path.display()
            );
            return;
        }
        let gt: GroundTruth =
            serde_json::from_str(&std::fs::read_to_string(&gt_path).unwrap()).unwrap();

        let (mut total_gt, mut total_emit, mut total_match) = (0usize, 0usize, 0usize);
        let (mut total_abstain, mut total_fx) = (0usize, 0usize);
        let mut false_values: Vec<String> = Vec::new();
        let mut spurious: Vec<String> = Vec::new();
        let mut missed: Vec<String> = Vec::new();

        for doc in &gt.documents {
            let body_path = dir.join("corpus").join(format!("{}.txt", doc.name));
            let body = std::fs::read_to_string(&body_path)
                .unwrap_or_else(|e| panic!("corpus {} : {e}", body_path.display()));
            let res = parse_espi_cover_note(&body, &doc.period.to);

            let labeled: BTreeMap<(String, String), Decimal> = doc
                .facts
                .iter()
                .map(|f| {
                    (
                        (f.metric_key.clone(), f.scope.clone()),
                        Decimal::from_str(&f.value).unwrap(),
                    )
                })
                .collect();

            // First emission per (metric_key, scope) wins, as the spike does.
            let mut emitted: BTreeMap<(String, String), Decimal> = BTreeMap::new();
            for f in &res.facts {
                let scope = match f.basis {
                    Some(StatementBasis::Standalone) => "standalone",
                    _ => "consolidated",
                };
                emitted
                    .entry((f.metric_key.clone(), scope.to_string()))
                    .or_insert(f.value);
            }

            for (k, want) in &labeled {
                match emitted.get(k) {
                    Some(got) if num_eq(*got, *want) => total_match += 1,
                    Some(got) => {
                        false_values.push(format!("{} {k:?} got={got} want={want}", doc.name))
                    }
                    None => missed.push(format!("{} {k:?} want={want}", doc.name)),
                }
            }
            for (k, got) in &emitted {
                if !labeled.contains_key(k) {
                    spurious.push(format!("{} {k:?} got={got}", doc.name));
                }
            }

            total_gt += labeled.len();
            total_emit += emitted.len();
            total_abstain += res.abstained;
            total_fx += res.fx_resolved;
        }

        eprintln!(
            "WDF corpus: recall {total_match}/{total_gt}  precision {total_match}/{total_emit}  \
             false={}  spurious={}  abstained={total_abstain}  fx_resolved={total_fx}",
            false_values.len(),
            spurious.len()
        );
        for line in false_values.iter().chain(&spurious).chain(&missed) {
            eprintln!("  {line}");
        }

        assert!(false_values.is_empty(), "false values: {false_values:?}");
        assert!(spurious.is_empty(), "spurious facts: {spurious:?}");
        assert!(missed.is_empty(), "missed facts: {missed:?}");
        assert_eq!(total_gt, 347, "ground truth must hold 347 labeled facts");
        assert_eq!(total_match, 347, "recall must be 347/347");
        assert_eq!(total_emit, 347, "precision must be 347/347");
        assert_eq!(total_abstain, 0, "no abstentions on the labeled set");
        assert_eq!(total_fx, 33, "33 rows must need the FX cross-check");
    }
}
