//! Shared numeric + label helpers for the deterministic extraction tiers.
//!
//! These primitives were extracted from the retired PDF fact-arm (ADR 0086
//! decision 1) because they serve the HTML aggregator
//! ([`super::html`], BiznesRadar-primary), reusing the Polish/English
//! unit-scale detector, the Polish number grammar (space/NBSP/dot thousands,
//! comma decimal, accounting parentheses, note-ref rejection) and the Polish
//! label dictionary. The tier-3b positional xHTML reader that used to also
//! stand on this module is retired too (ADR 0095).
//!
//! Pure over `&str` (text, not bytes), so every rule is unit-testable. No tier
//! reads financial FACTS out of PDF statements anymore (ADR 0086 dec. 1); this
//! module is the number/label substrate the html tier stands on.

use std::sync::OnceLock;

use regex::Regex;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// The default Polish label → `metric_key` dictionary. Each entry is a
/// normalized label (lowercase, whitespace-collapsed) and the metric it maps
/// to. Longer labels are matched first so `zysk z działalności operacyjnej`
/// wins over `zysk`.
fn default_dictionary() -> &'static [(&'static str, &'static str)] {
    // Ordered longest-first at match time; declaration order is not relied on.
    &[
        // Balance sheet.
        ("aktywa razem", "total_assets"),
        ("suma aktywów", "total_assets"),
        ("aktywa ogółem", "total_assets"),
        ("zobowiązania razem", "total_liabilities"),
        ("zobowiązania ogółem", "total_liabilities"),
        (
            "zobowiązania i rezerwy na zobowiązania",
            "total_liabilities",
        ),
        ("kapitał własny", "total_equity"),
        ("kapitał własny razem", "total_equity"),
        ("razem kapitał własny", "total_equity"),
        // BiznesRadar's BANK balance-sheet schema (real PEO live pages,
        // 2026-07-31; epic #277 T1) uses a PLURAL equity pair, distinct
        // strings from the singular "kapitał własny" above so neither
        // prefix-matches the other: "Kapitały własne" is the
        // parent-attributable figure, "Kapitały razem" is the group total
        // (parent + non-controlling). Verified on the live page: Kapitały
        // własne + Udziały niekontrolujące = Kapitały razem (e.g.
        // 8 407 290 + 15 436 = 8 422 726; 14 666 788 + 80 507 = 14 747 295).
        ("kapitały własne", "wdf_equity_parent"),
        ("kapitały razem", "total_equity"),
        ("środki pieniężne i ich ekwiwalenty", "cash"),
        ("środki pieniężne i ekwiwalenty środków pieniężnych", "cash"),
        // SFP cash-position label variant (asb / Asseco Business Solutions and
        // others): the balance sheet lists "środki pieniężne w banku i gotówka"
        // rather than "…i ich ekwiwalenty" (card 40281b3).
        ("środki pieniężne w banku i gotówka", "cash"),
        // Balance-sheet health-score inputs (ADR 0083 Decision 5). `zyski
        // zatrzymane` is the standard retained-earnings line; `kredyty i pożyczki
        // długoterminowe` is the borrowings-specific long-term-debt line — never
        // `zobowiązania długoterminowe`, which is *total* non-current liabilities.
        ("aktywa obrotowe", "current_assets"),
        // Inventory line, the aggregator's balance-sheet `Zapasy` row (ADR 0086
        // — BiznesRadar-primary core KPI; `inventories` catalog metric seeded by
        // migration 0110). Working-capital detail the issuer PDF long tail rarely
        // spells in a machine-consumable form.
        ("zapasy", "inventories"),
        ("zobowiązania krótkoterminowe", "current_liabilities"),
        ("zyski zatrzymane", "retained_earnings"),
        // BiznesRadar's BANK balance-sheet schema (real PEO live pages,
        // 2026-07-31; epic #277 T1): customer-facing loans/deposits, distinct
        // from the page's separate INTERBANK rows ("Należności/Zobowiązania
        // wobec banków"), which stay deliberately unmapped — interbank
        // exposure is not the retail/corporate book these KPIs track.
        ("należności od klientów", "total_loans"),
        ("zobowiązania wobec klientów", "total_deposits"),
        ("kredyty i pożyczki długoterminowe", "long_term_debt"),
        ("długoterminowe kredyty i pożyczki", "long_term_debt"),
        // Income statement.
        ("przychody netto ze sprzedaży", "revenue"),
        ("przychody ze sprzedaży", "revenue"),
        ("przychody z umów z klientami", "revenue"),
        ("zysk brutto ze sprzedaży", "gross_profit"),
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
        // Parent-only net-profit REFINEMENT (ADR 0086 dec. 2, card 8f82ad5).
        // The aggregator's "Zysk netto akcjonariuszy jednostki dominującej" is the
        // profit attributable to the parent's shareholders — a DISTINCT metric from
        // the group `net_profit`, mapped to `wdf_net_profit_parent` for consistency
        // with the WDF cover-note tier. Being longer than "zysk netto" it wins the
        // longest-label-first match, so the parent row never overwrites group net
        // profit (and the generic "Zysk (strata) netto" stays `net_profit`).
        (
            "zysk netto akcjonariuszy jednostki dominującej",
            "wdf_net_profit_parent",
        ),
        // Discontinued-operations result — G2 vocabulary-contract catch
        // (2026-07-22): the "zysk (strata) netto" prefix swallowed this row and,
        // being listed BEFORE the true net-profit row on BR's income page, its
        // value deduped the real group net profit away for any company with
        // discontinued operations. Longer keys win, so it now routes to its own
        // metric.
        (
            "zysk (strata) netto z działalności zaniechanej",
            "net_profit_discontinued",
        ),
        (
            "zysk netto z działalności zaniechanej",
            "net_profit_discontinued",
        ),
        (
            "zysk (strata) z działalności zaniechanej",
            "net_profit_discontinued",
        ),
        // The BR balance page publishes PARENT-attributable equity as its only
        // equity row (review 2026-07-22, CBF live evidence: BR 323.8M parent vs
        // the filing's 543.3M group total). Longer keys win over the bare
        // "kapitał własny" prefix, so these route to the parent metric instead
        // of silently understating the group `total_equity`.
        (
            "kapitał własny akcjonariuszy jednostki dominującej",
            "wdf_equity_parent",
        ),
        (
            "kapitał własny przypadający akcjonariuszom jednostki dominującej",
            "wdf_equity_parent",
        ),
        // Cash-flow statement. The aggregator's own labels OMIT "netto" ("Przepływy
        // pieniężne z działalności operacyjnej"), so the issuer-PDF `…netto…` forms
        // above do not prefix-match them — both spellings are kept (ADR 0086).
        (
            "przepływy pieniężne netto z działalności operacyjnej",
            "operating_cash_flow",
        ),
        (
            "przepływy środków pieniężnych z działalności operacyjnej",
            "operating_cash_flow",
        ),
        (
            "przepływy pieniężne z działalności operacyjnej",
            "operating_cash_flow",
        ),
        (
            "przepływy pieniężne netto z działalności inwestycyjnej",
            "investing_cash_flow",
        ),
        (
            "przepływy pieniężne z działalności inwestycyjnej",
            "investing_cash_flow",
        ),
        (
            "przepływy pieniężne netto z działalności finansowej",
            "financing_cash_flow",
        ),
        (
            "przepływy pieniężne z działalności finansowej",
            "financing_cash_flow",
        ),
        // BiznesRadar's BANK income-statement schema (real PEO live pages,
        // 2026-07-31; epic #277 T1): the bank-specific KPI-pack metrics
        // (migration 0034/0126/0127) that only a bank's dedicated report
        // structure carries — no non-bank row ever uses these labels.
        ("wynik z tytułu odsetek", "net_interest_income"),
        ("wynik z tytułu prowizji", "net_fee_commission_income"),
        // Per-share (never scaled by the document unit).
        ("zysk na jedną akcję", "eps_basic"),
        ("zysk na akcję", "eps_basic"),
        ("rozwodniony zysk na akcję", "eps_diluted"),
    ]
}

/// Normalizes a label for dictionary matching: lowercased, whitespace
/// collapsed, surrounding punctuation and note-reference digits trimmed.
pub fn normalize_label(raw: &str) -> String {
    let lowered = raw.to_lowercase();
    let collapsed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_matches(|c: char| c == ':' || c == '.' || c == '-' || c.is_whitespace())
        .to_string()
}

/// The unit scale a document reports its monetary figures in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnitScale {
    Ones,
    Thousands,
    Millions,
}

impl UnitScale {
    pub fn factor(self) -> i64 {
        match self {
            UnitScale::Ones => 1,
            UnitScale::Thousands => 1_000,
            UnitScale::Millions => 1_000_000,
        }
    }
    pub fn as_i64(self) -> i64 {
        self.factor()
    }
}

/// Detects the document-wide monetary unit from an explicit scale
/// **declaration** — Polish (`w tys. zł`, `dane w tysiącach`, `w mln zł`) or
/// English (`in thousands`, `PLN '000`, `in millions`) — falling back to a bare
/// scale caption (`(tys. zł)` / `mln zł`). When the document declares **no**
/// unit anywhere, the scale is [`UnitScale::Ones`] — raw złoty, no multiplier.
///
/// **Owner rule (2026-07-21, dogfooding on real data).** The unit is READ from
/// the document, never guessed: "in every report, for every company, somewhere
/// it is written what unit the report is in; if nothing is written, assume no
/// multiplier; needing to guess a multiplier means the reader failed to read the
/// declaration." So the silent default is Ones (raw złoty) — NOT Thousands.
/// Groszy on aggregate lines (2-digit comma decimals) corroborate the raw
/// denomination; precedence is **declaration > bare caption > silent-default
/// Ones**, and a declaration always wins over a groszy signal. (ADR 0061.)
///
/// Dominance-weighted (cards e6ebda3 + 610bcae). A scale **declaration** is how
/// a statement names its own unit, repeated across headers, captions and
/// footnotes; a stray narrative `w mln`/`mln zł` inside a thousands statement
/// must lose to the actual declaration, so declarations are COUNTED per side and
/// the majority wins. English declarations join the DECLARATION tier as
/// first-class votes (Benefit Systems H1 2025: an English translation filing
/// whose "expressed in thousands of Polish złoty" was invisible to the
/// Polish-only detector fell to the bare tie, where a stray "95 mln zł" prose
/// token flipped it to Millions → every value ×1000 too big). On a declaration
/// tie (typically both zero) the bare currency-scale captions ("(tys. zł)" /
/// "mln zł", no `w`) break it SYMMETRICALLY (majority wins, thousands on a
/// further tie); a caption-declared thousands statement with a stray narrative
/// "mln zł" therefore stays Thousands. With no declaration AND no caption at all,
/// the owner rule applies: Ones.
pub fn detect_unit_scale(text: &str) -> UnitScale {
    let t = text.to_lowercase();
    // "w tys" is a prefix of every Polish thousands declaration form
    // ("w tys.", "w tysiącach", "w tysięcy"); "w mln"/"w milionach" are the two
    // millions roots. English declaration forms join the same tier: "in
    // thousands" (covers "expressed/amounts in thousands"), "thousands of", the
    // "PLN '000" / "PLN thousand(s)" annotations, and the millions counterparts.
    let thousands_decl = t.matches("w tys").count()
        + t.matches("in thousands").count()
        + t.matches("thousands of").count()
        + t.matches("pln '000").count()
        + t.matches("pln thousand").count();
    let millions_decl = t.matches("w mln").count()
        + t.matches("w milionach").count()
        + t.matches("in millions").count()
        + t.matches("millions of").count()
        + t.matches("pln million").count();
    if millions_decl > thousands_decl {
        return UnitScale::Millions;
    }
    if thousands_decl > millions_decl {
        return UnitScale::Thousands;
    }
    // Declarations tie (typically both zero): fall back to bare currency-scale
    // captions — the "(tys. zł)"/"mln zł" column annotations without the "w"
    // preposition. These are weaker than a declaration but still name the unit,
    // so they break the tie SYMMETRICALLY: count both sides, majority wins. A
    // caption-declared thousands statement ("(tys. zł)") with a stray narrative
    // "mln zł" therefore stays Thousands — the CDR-regression class that
    // over-scaled 46 real facts ×1000 when only the millions token counted.
    let thousands_bare = t.matches("tys. zł").count()
        + t.matches("tys. zl").count()
        + t.matches("tys zł").count()
        + t.matches("tys zl").count()
        + t.matches("tys. pln").count()
        + t.matches("tys pln").count();
    let millions_bare =
        t.matches("mln zł").count() + t.matches("mln zl").count() + t.matches("mln pln").count();
    if millions_bare > thousands_bare {
        return UnitScale::Millions;
    }
    if thousands_bare > 0 {
        // A scale caption is present and thousands >= millions: Thousands.
        return UnitScale::Thousands;
    }
    // No declaration and no scale caption anywhere. Owner rule 2026-07-21: the
    // scale is read, never guessed — with nothing declared, there is no
    // multiplier. Groszy on aggregate lines corroborates this raw denomination.
    UnitScale::Ones
}

/// Matches a single value column: an optional sign/paren, then a grouped or
/// plain digit run with an optional decimal comma. Within a column, a single
/// space (or NBSP) is a thousands separator.
fn column_number_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // A `.` joins the thousands-separator class alongside space/NBSP/narrow:
        // some issuers group thousands with a dot ("106.208"). It only groups
        // when followed by exactly three digits, so a two-digit "2.45" never
        // matches here (Polish decimals use a comma, handled by `,\d+`).
        Regex::new(r"^\(?-?(?:\d{1,3}(?:[ \u{00A0}\u{202F}.]\d{3})+|\d+)(?:,\d+)?\)?$").unwrap()
    })
}

/// Strips a trailing per-cell currency word (`"1 234 zł"` → `"1 234"`). Scale
/// words (`tys.`/`mln`) and `%` are intentionally *not* stripped: `%` marks a
/// non-monetary ratio that must be rejected, and cell-level scale words are a
/// rare edge left to the document-level unit.
fn strip_currency_suffix(s: &str) -> String {
    let mut out = s.trim().to_string();
    loop {
        let lower = out.to_lowercase();
        let mut stripped = false;
        for suf in [" zł", " zl", " pln", " eur", "zł", "zl", "pln", "eur"] {
            if lower.ends_with(suf) {
                out.truncate(out.len() - suf.len());
                out = out.trim_end().to_string();
                stripped = true;
                break;
            }
        }
        if !stripped {
            break;
        }
    }
    out
}

/// Parses a single value column into its base magnitude (no unit scaling), or
/// `None` if the column is not a clean number. Handles accounting parentheses,
/// ASCII and Unicode minus/dash (`-`, `–`, `—`, `−`), space/NBSP/dot thousands
/// grouping and a trailing currency word; a bare dash (`–`, "no data") yields
/// `None`. Shared with the HTML aggregator tier (the tier-3b positional reader that used to also share this is retired, ADR 0095).
pub(super) fn parse_amount(tok: &str) -> Option<Decimal> {
    // Normalize a leading Unicode minus/dash to ASCII '-'.
    let mut s = tok.trim().to_string();
    if let Some(rest) = s
        .strip_prefix('\u{2013}')
        .or_else(|| s.strip_prefix('\u{2014}'))
        .or_else(|| s.strip_prefix('\u{2212}'))
    {
        s = format!("-{}", rest.trim_start());
    }
    s = strip_currency_suffix(&s);
    let s = s.trim();
    if !column_number_regex().is_match(s) {
        return None;
    }
    let mut negative = false;
    let mut body = s.to_string();
    if body.starts_with('(') && body.ends_with(')') {
        negative = true;
        body = body[1..body.len() - 1].to_string();
    }
    if let Some(stripped) = body.strip_prefix('-') {
        negative = true;
        body = stripped.to_string();
    }
    // An integer part of "0" can NEVER be a thousands grouping — "0.456" is the
    // decimal 0.456, not 456 (dot-stripped). Detect the leading-zero-dot form and
    // keep the dot as a decimal point, so every EPS/ratio with three decimal
    // places stops silently over-scaling ×1000. Comma handling is untouched.
    let leading_zero_decimal =
        body.starts_with("0.") && body.len() > 2 && body[2..].bytes().all(|b| b.is_ascii_digit());
    let normalized = if leading_zero_decimal {
        // Already a clean decimal string ("0.456"); no separator stripping.
        body.clone()
    } else {
        // Strip whitespace AND dot thousands separators before the comma→dot
        // decimal rewrite, so "106.208" becomes 106208 (grouped), never 106.208.
        body.retain(|c| !c.is_whitespace() && c != '.');
        body.replace(',', ".")
    };
    let mut value = Decimal::from_str(&normalized).ok()?;
    if negative {
        value.set_sign_negative(true);
    }
    Some(value)
}

pub(super) fn is_per_share(metric_key: &str) -> bool {
    matches!(
        metric_key,
        "eps_basic" | "eps_diluted" | "dividend_per_share"
    )
}

/// Matches a normalized label against the default Polish dictionary
/// (longest-label-first). Shared with the HTML aggregator tier (the tier-3b positional reader that used to also share this is retired, ADR 0095).
pub(super) fn match_dictionary_label(label: &str) -> Option<&'static str> {
    let mut entries: Vec<&(&str, &str)> = default_dictionary().iter().collect();
    entries.sort_by_key(|(l, _)| std::cmp::Reverse(l.len()));
    for (pattern, metric) in entries {
        if label == *pattern || label.starts_with(pattern) {
            return Some(metric);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Unit scale — the dominance rule (Polish + English declarations, silent =
    // raw złoty). These regressions MUST survive the PDF-fact-arm retirement
    // (ADR 0086 dec. 1): the html tier stands on this detector (the tier-3b
    // positional reader that used to also stand on it is retired, ADR 0095).
    // -----------------------------------------------------------------------

    #[test]
    fn detects_thousands_and_millions() {
        assert_eq!(detect_unit_scale("dane w tys. zł"), UnitScale::Thousands);
        assert_eq!(detect_unit_scale("wartości w mln zł"), UnitScale::Millions);
        // Owner rule 2026-07-21: nothing declared ⇒ NO multiplier (raw złoty), not
        // the historical Thousands. Scale must be READ, never guessed.
        assert_eq!(detect_unit_scale("bez jednostki"), UnitScale::Ones);
    }

    /// Regression (Benefit Systems H1 2025 SSF, English translation filing): the
    /// unit declaration is written in ENGLISH — "All amounts are expressed in
    /// thousands of Polish złoty unless indicated otherwise." — and the old
    /// detector counted ONLY Polish forms ("w tys"/"tys. zł"). Invisible to the
    /// declaration tier, the filing fell to the bare-caption tie, where a stray
    /// narrative "mln zł" (e.g. "revenue grew to 95 mln zł") won → Millions →
    /// every value stored ×1000 too big. English declarations must be first-class
    /// DECLARATION-tier votes, so the thousands declaration wins.
    #[test]
    fn english_thousands_declaration_beats_stray_prose_millions() {
        let doc = "INTERIM CONDENSED CONSOLIDATED FINANCIAL STATEMENTS OF THE BENEFIT \
                   SYSTEMS GROUP FOR THE SIX MONTHS ENDED 30 JUNE 2025 [TRANSLATION ONLY] \
                   All amounts are expressed in thousands of Polish złoty unless indicated \
                   otherwise. Group revenue grew to 95 mln zł in the half.";
        assert_eq!(detect_unit_scale(doc), UnitScale::Thousands);
        // The other recognized English forms are equally declaration-tier votes.
        assert_eq!(
            detect_unit_scale("Figures in PLN '000."),
            UnitScale::Thousands
        );
        assert_eq!(
            detect_unit_scale("Amounts in millions of PLN. Note: 95 tys. zł award."),
            UnitScale::Millions
        );
    }

    /// Regression (Digital Network Q1 2025): a cover table with NO unit
    /// declaration anywhere (bare "PLN PLN EUR EUR" header) and values carrying
    /// groszy. Owner rule 2026-07-21: nothing declared ⇒ NO multiplier — the
    /// values are raw złoty (not the old ×1000 silent default).
    #[test]
    fn no_declaration_groszy_values_are_raw_zloty() {
        let text = "WYBRANE SKONSOLIDOWANE DANE FINANSOWE\n\
                    PLN PLN EUR EUR\n\
                    Przychody netto ze sprzedaży  15 395 950,73  12 346 878,97\n";
        assert_eq!(detect_unit_scale(text), UnitScale::Ones);
    }

    /// Owner rule 2026-07-21, the pure default: a document with NO declaration and
    /// NO scale caption anywhere — and no groszy either — is still raw units. This
    /// flips the historical silent default (was Thousands).
    #[test]
    fn no_declaration_no_groszy_defaults_to_units() {
        let text = "Sprawozdanie finansowe\nAktywa razem  4 500 000  4 000 000\n";
        assert_eq!(detect_unit_scale(text), UnitScale::Ones);
    }

    /// Regression (card e6ebda3, CDR Q3 2023): a statement DECLARED in thousands
    /// ("wszystkie kwoty … w tys. zł") that also mentions a millions amount in
    /// NARRATIVE prose ("około 95 mln zł") must stay Thousands — the prose token
    /// must NOT override the statement's scale declaration.
    #[test]
    fn prose_millions_token_does_not_override_a_thousands_declaration() {
        let doc = "Skonsolidowane sprawozdanie finansowe. Wszystkie kwoty są w tys. \
                   złotych o ile nie podano inaczej. Koszty kampanii to około 95 mln zł. \
                   Zasądzona szkoda wyniosła co najmniej 16 mln zł.";
        assert_eq!(detect_unit_scale(doc), UnitScale::Thousands);
        // A real millions declaration still resolves to millions even with a stray
        // "w tys. sztuk" share-count note present.
        let millions = "Dane w mln zł. Liczba akcji w tysiącach sztuk 100 388.";
        assert_eq!(detect_unit_scale(millions), UnitScale::Millions);
        // A bare "mln zł" with NO thousands declaration keeps signalling millions.
        assert_eq!(detect_unit_scale("Aktywa (mln zł)"), UnitScale::Millions);
    }

    /// Regression (card 610bcae): a document dominated by thousands declarations
    /// with a couple of stray "w mln" prose mentions must stay Thousands.
    /// Dominance-weighted detection lets the thousands declarations win.
    #[test]
    fn stray_millions_token_does_not_flip_a_thousands_dominant_document() {
        let mut doc = String::from("Skonsolidowane sprawozdanie finansowe\n");
        for _ in 0..13 {
            doc.push_str("dane w tys. ");
        }
        for _ in 0..10 {
            doc.push_str("kwoty w tys. zł ");
        }
        doc.push_str("nabycie za 12 w mln zł oraz inwestycja 8 w mln zł");
        assert_eq!(detect_unit_scale(&doc), UnitScale::Thousands);
    }

    /// Regression (A6, CDR-regression class): a statement that declares thousands
    /// ONLY via the column caption "(tys. zł)" — no preposition "w" — and also
    /// mentions a millions amount in narrative prose must resolve Thousands. The
    /// caption is a real scale declaration; it must count as a thousands vote.
    #[test]
    fn caption_thousands_without_w_beats_prose_millions() {
        let doc = "Skonsolidowane sprawozdanie finansowe (tys. zł)\n\
                   Wartość transakcji wyniosła 95 mln zł.";
        assert_eq!(detect_unit_scale(doc), UnitScale::Thousands);
        // The bare "tys. PLN" caption variant is equally a thousands vote.
        let pln = "Sprawozdanie (tys. PLN)\nNagroda 12 mln zł w konkursie.";
        assert_eq!(detect_unit_scale(pln), UnitScale::Thousands);
        // A genuine millions statement (bare "mln zł" caption, no thousands token)
        // still resolves Millions on the tie-break.
        assert_eq!(detect_unit_scale("Aktywa (mln zł)"), UnitScale::Millions);
    }

    // -----------------------------------------------------------------------
    // Number grammar (parse_amount) — the leading-zero decimal guard + dot
    // thousands rule the html tier relies on (the tier-3b positional reader
    // that used to also rely on it is retired, ADR 0095).
    // -----------------------------------------------------------------------

    #[test]
    fn dot_two_digit_group_is_not_treated_as_thousands() {
        // A dot followed by fewer than three digits is NOT a thousands separator
        // (Polish statements use a comma decimal). Such a token is rejected.
        assert!(parse_amount("2.45").is_none());
        assert!(parse_amount("18.1").is_none());
        // The genuine 3-digit grouping parses.
        assert_eq!(parse_amount("106.208"), Some(Decimal::from(106_208)));
        assert_eq!(parse_amount("1.234.567"), Some(Decimal::from(1_234_567)));
    }

    #[test]
    fn leading_zero_dot_is_decimal_not_thousands() {
        // Regression (A2): "0.456" is the ratio/EPS 0.456, never dot-grouped
        // thousands. An integer part of "0" can never be a thousands grouping.
        assert_eq!(
            parse_amount("0.456"),
            Some(Decimal::from_str_exact("0.456").unwrap())
        );
        // Genuine dot-grouped thousands (non-zero integer part) still parse whole.
        assert_eq!(parse_amount("1.234.567"), Some(Decimal::from(1_234_567)));
        assert_eq!(parse_amount("106.208"), Some(Decimal::from(106_208)));
        // Comma decimals are untouched by the leading-zero rule.
        assert_eq!(
            parse_amount("0,456"),
            Some(Decimal::from_str_exact("0.456").unwrap())
        );
    }

    #[test]
    fn parentheses_make_negative() {
        assert_eq!(parse_amount("(1 100)"), Some(Decimal::from(-1_100)));
    }

    #[test]
    fn normalize_label_lowercases_and_trims() {
        assert_eq!(normalize_label("  Aktywa Razem: "), "aktywa razem");
    }

    #[test]
    fn dictionary_matches_longest_label_first() {
        assert_eq!(match_dictionary_label("aktywa razem"), Some("total_assets"));
        assert_eq!(
            match_dictionary_label("zysk z działalności operacyjnej"),
            Some("operating_profit")
        );
        // Parent-only net profit beats the generic "zysk netto" prefix.
        assert_eq!(
            match_dictionary_label("zysk netto akcjonariuszy jednostki dominującej"),
            Some("wdf_net_profit_parent")
        );
        assert_eq!(match_dictionary_label("zysk netto"), Some("net_profit"));
    }

    /// Review finding (2026-07-22, CBF live evidence): the BR balance page's
    /// only equity row is the PARENT-attributable one — it must map to the
    /// parent metric, never collapse into the group `total_equity` (which BR
    /// simply does not publish there).
    #[test]
    fn parent_equity_row_maps_to_the_parent_metric_not_total_equity() {
        assert_eq!(
            match_dictionary_label("kapitał własny akcjonariuszy jednostki dominującej"),
            Some("wdf_equity_parent")
        );
        assert_eq!(
            match_dictionary_label(
                "kapitał własny przypadający akcjonariuszom jednostki dominującej"
            ),
            Some("wdf_equity_parent")
        );
        // The group totals keep resolving to total_equity.
        assert_eq!(
            match_dictionary_label("kapitał własny"),
            Some("total_equity")
        );
        assert_eq!(
            match_dictionary_label("kapitał własny razem"),
            Some("total_equity")
        );
    }
}
