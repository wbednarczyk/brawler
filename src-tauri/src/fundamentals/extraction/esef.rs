//! ESEF / inline-XBRL parser — tier 1 of the deterministic pipeline (ADR 0061).
//!
//! ESEF (European Single Electronic Format) mandates that annual financial
//! reports of issuers on EU regulated markets — GPW main market included since
//! FY2020 — be published as Inline XBRL: an xHTML document whose figures are
//! machine-tagged with IFRS taxonomy concepts. Each numeric fact is an
//! `ix:nonFraction` element carrying a `contextRef` (which period), a `unitRef`
//! (which currency), a `scale` (power-of-ten multiplier) and an optional `sign`.
//! Because the value is *tagged* rather than laid out for a human eye, ESEF is
//! the one tier where extraction is provably deterministic — the source of
//! truth the rest of the pipeline validates against.
//!
//! This parser streams the iXBRL instance (via `quick-xml`), reconstructs the
//! `xbrli:context`/`xbrli:unit` tables, reads every `ix:nonFraction` fact,
//! applies its scale/sign, and maps the IFRS concept to our `metric_key`
//! catalog. Untracked concepts (company extension taxonomy, disclosure notes)
//! are ignored. It is pure over `&[u8]` — no IO — so it is fully unit-testable.
//!
//! Scope: it reads the inline-XBRL **instance document**. Acquiring it from an
//! ESEF report *package* (a ZIP) is a fetch-path concern handled where the
//! pipeline is wired; this module parses the xHTML bytes it is handed.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use super::{ExtractedFact, FactPeriod, SourceTier, StatementBasis};

/// Maps an IFRS taxonomy concept (matched on its local name, prefix-agnostic)
/// to our canonical `metric_key`. Kept deliberately small and explicit: only
/// the concepts that back a seeded `kpi_definitions` metric. Company extension
/// concepts and note-level disclosures are intentionally absent.
fn concept_to_metric_key(local_name: &str) -> Option<&'static str> {
    Some(match local_name {
        // Balance sheet (instant).
        "Assets" => "total_assets",
        "Liabilities" => "total_liabilities",
        "Equity" => "total_equity",
        "CashAndCashEquivalents" => "cash",
        // Income statement (duration).
        "Revenue" | "RevenueFromContractsWithCustomers" => "revenue",
        "GrossProfit" => "gross_profit",
        "ProfitLossFromOperatingActivities" => "operating_profit",
        "ProfitLoss" => "net_profit",
        "BasicEarningsLossPerShare" => "eps_basic",
        "DilutedEarningsLossPerShare" => "eps_diluted",
        // Cash-flow statement (duration).
        "CashFlowsFromUsedInOperatingActivities" => "operating_cash_flow",
        "CashFlowsFromUsedInInvestingActivities" => "investing_cash_flow",
        "CashFlowsFromUsedInFinancingActivities" => "financing_cash_flow",
        _ => return None,
    })
}

/// Parse error surfaced to the pipeline. Deliberately coarse: the pipeline
/// treats any parse failure as "tier 1 unavailable" and falls to the next tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EsefError {
    /// The XML was malformed.
    Xml(String),
    /// No `ix:nonFraction` facts mapped to a tracked metric.
    NoTrackedFacts,
}

impl std::fmt::Display for EsefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EsefError::Xml(e) => write!(f, "iXBRL parse error: {e}"),
            EsefError::NoTrackedFacts => write!(f, "no tracked iXBRL facts found"),
        }
    }
}

impl std::error::Error for EsefError {}

// ---------------------------------------------------------------------------
// Intermediate builders held during the streaming parse
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ContextPeriod {
    instant: Option<String>,
    start: Option<String>,
    end: Option<String>,
}

impl ContextPeriod {
    fn into_fact_period(self) -> Option<FactPeriod> {
        if let Some(instant) = self.instant {
            Some(FactPeriod::Instant(instant))
        } else if let (Some(start), Some(end)) = (self.start, self.end) {
            Some(FactPeriod::Duration { start, end })
        } else {
            None
        }
    }
}

struct PendingFact {
    concept_local: String,
    context_ref: String,
    unit_ref: Option<String>,
    scale: i32,
    negate: bool,
    format: Option<String>,
    text: String,
    open_depth: usize,
}

/// Returns the local part of a possibly-prefixed name (`ix:nonFraction` →
/// `nonFraction`, `Assets` → `Assets`).
fn local_of(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

fn local_str(name: &[u8]) -> String {
    String::from_utf8_lossy(local_of(name)).into_owned()
}

// ---------------------------------------------------------------------------
// Number parsing
// ---------------------------------------------------------------------------

/// Parses the human-formatted number inside an `ix:nonFraction`, honouring the
/// inline-XBRL transform named in `format` (e.g. `ixt:num-dot-decimal`,
/// `ixt:num-comma-decimal`). Handles thousands separators (space, NBSP, narrow
/// NBSP, `.`/`,`), a decimal comma or dot, and accounting parentheses for
/// negatives. Returns the unscaled magnitude with its own sign.
fn parse_ixbrl_number(raw: &str, format: Option<&str>) -> Option<Decimal> {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return None;
    }

    // Accounting parentheses → negative.
    let mut negative = false;
    if s.starts_with('(') && s.ends_with(')') {
        negative = true;
        s = s[1..s.len() - 1].to_string();
    }

    // Decide the decimal separator from the transform, else infer.
    let fmt = format.map(|f| local_of(f.as_bytes()).to_ascii_lowercase());
    let fmt = fmt.as_deref();
    let decimal_comma = match fmt {
        Some(f)
            if f.windows(b"comma-decimal".len())
                .any(|w| w == b"comma-decimal") =>
        {
            true
        }
        Some(f) if f.windows(b"dot-decimal".len()).any(|w| w == b"dot-decimal") => false,
        // Legacy transform names without hyphens (numcommadecimal / numdotdecimal).
        Some(f)
            if f.windows(b"commadecimal".len())
                .any(|w| w == b"commadecimal") =>
        {
            true
        }
        Some(f) if f.windows(b"dotdecimal".len()).any(|w| w == b"dotdecimal") => false,
        _ => infer_decimal_comma(&s),
    };

    // Strip all whitespace (incl. NBSP variants) used as thousands separators.
    s.retain(|c| !c.is_whitespace() && c != '\u{00A0}' && c != '\u{202F}' && c != '\u{2009}');

    let normalized = if decimal_comma {
        // Comma is the decimal mark; dots are thousands separators.
        s.replace('.', "").replace(',', ".")
    } else {
        // Dot is the decimal mark; commas are thousands separators.
        s.replace(',', "")
    };

    let mut value = Decimal::from_str(&normalized)
        .or_else(|_| Decimal::from_str_exact(&normalized))
        .ok()?;
    if negative {
        value.set_sign_negative(true);
    }
    Some(value)
}

/// When no transform is declared, guess whether the decimal mark is a comma:
/// if the last comma appears after the last dot (or there is only a comma with
/// 1–2 trailing digits), treat comma as decimal — the Polish/European default.
fn infer_decimal_comma(s: &str) -> bool {
    let last_comma = s.rfind(',');
    let last_dot = s.rfind('.');
    match (last_comma, last_dot) {
        (Some(c), Some(d)) => c > d,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Applies a power-of-ten `scale` to a magnitude. Returns `None` on overflow so
/// a hostile/garbled scale can never panic the parser — the fact is dropped.
fn apply_scale(value: Decimal, scale: i32) -> Option<Decimal> {
    if scale == 0 {
        return Some(value);
    }
    // 10^28 fits i128 and covers any real financial scale; cap defensively.
    let n = scale.unsigned_abs().min(28);
    let factor = Decimal::from(10i128.pow(n));
    if scale > 0 {
        value.checked_mul(factor)
    } else {
        Some(value / factor)
    }
}

// ---------------------------------------------------------------------------
// Top-level parse
// ---------------------------------------------------------------------------

/// Parses an inline-XBRL instance document into the tracked [`ExtractedFact`]s
/// it contains, across all reported periods (current and comparative). Callers
/// group by period end with [`super::fact_set_for_period`]. Every returned fact
/// carries [`SourceTier::Esef`].
pub fn parse_esef(bytes: &[u8]) -> Result<Vec<ExtractedFact>, EsefError> {
    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);

    let mut contexts: HashMap<String, ContextPeriod> = HashMap::new();
    let mut units: HashMap<String, String> = HashMap::new();

    // Streaming state.
    let mut depth: usize = 0;
    let mut ctx_id: Option<String> = None;
    let mut ctx_period = ContextPeriod::default();
    let mut period_field: Option<PeriodField> = None;
    let mut unit_id: Option<String> = None;
    let mut in_measure = false;
    let mut pending: Option<PendingFact> = None;
    let mut raw_facts: Vec<RawFact> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                let local = local_of(e.name().as_ref()).to_vec();
                match local.as_slice() {
                    b"context" => {
                        ctx_id = attr(&e, b"id");
                        ctx_period = ContextPeriod::default();
                    }
                    b"instant" => period_field = Some(PeriodField::Instant),
                    b"startDate" => period_field = Some(PeriodField::Start),
                    b"endDate" => period_field = Some(PeriodField::End),
                    b"unit" => unit_id = attr(&e, b"id"),
                    b"measure" => in_measure = true,
                    b"nonFraction" if pending.is_none() => {
                        if let Some(context_ref) = attr(&e, b"contextRef") {
                            let concept = attr(&e, b"name").unwrap_or_default();
                            pending = Some(PendingFact {
                                concept_local: local_str(concept.as_bytes()),
                                context_ref,
                                unit_ref: attr(&e, b"unitRef"),
                                scale: attr(&e, b"scale").and_then(|s| s.parse().ok()).unwrap_or(0),
                                negate: attr(&e, b"sign").as_deref() == Some("-"),
                                format: attr(&e, b"format"),
                                text: String::new(),
                                open_depth: depth,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let value = crate::source_adapters::parsing::decode_xml_text_value(t)
                    .map_err(EsefError::Xml)?;
                if let Some(field) = period_field {
                    match field {
                        PeriodField::Instant => ctx_period.instant = Some(value.trim().to_string()),
                        PeriodField::Start => ctx_period.start = Some(value.trim().to_string()),
                        PeriodField::End => ctx_period.end = Some(value.trim().to_string()),
                    }
                } else if in_measure {
                    if let Some(id) = &unit_id {
                        // e.g. "iso4217:PLN" → "PLN".
                        units.insert(id.clone(), local_str(value.trim().as_bytes()));
                    }
                } else if let Some(p) = pending.as_mut() {
                    p.text.push_str(&value);
                }
            }
            Ok(Event::End(e)) => {
                let local = local_of(e.name().as_ref()).to_vec();
                // Finalize a nonFraction whose own End we've reached.
                if let Some(p) = pending.as_ref() {
                    if local.as_slice() == b"nonFraction" && depth == p.open_depth {
                        let p = pending.take().unwrap();
                        if let Some(metric_key) = concept_to_metric_key(&p.concept_local) {
                            if let Some(mut value) =
                                parse_ixbrl_number(&p.text, p.format.as_deref())
                                    .and_then(|v| apply_scale(v, p.scale))
                            {
                                if p.negate {
                                    value.set_sign_negative(!value.is_sign_negative());
                                }
                                raw_facts.push(RawFact {
                                    metric_key,
                                    value,
                                    context_ref: p.context_ref,
                                    unit_ref: p.unit_ref,
                                    concept_local: p.concept_local,
                                });
                            }
                        }
                    }
                }
                match local.as_slice() {
                    b"context" => {
                        if let Some(id) = ctx_id.take() {
                            contexts.insert(id, std::mem::take(&mut ctx_period));
                        }
                    }
                    b"instant" | b"startDate" | b"endDate" => period_field = None,
                    b"unit" => unit_id = None,
                    b"measure" => in_measure = false,
                    _ => {}
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(EsefError::Xml(e.to_string())),
            _ => {}
        }
    }

    // Resolve contexts/units into ExtractedFacts.
    let mut facts: Vec<ExtractedFact> = Vec::new();
    for rf in raw_facts {
        let Some(period) = contexts.get(&rf.context_ref).and_then(|c| {
            ContextPeriod {
                instant: c.instant.clone(),
                start: c.start.clone(),
                end: c.end.clone(),
            }
            .into_fact_period()
        }) else {
            continue;
        };
        let currency = rf.unit_ref.and_then(|u| units.get(&u).cloned());
        facts.push(ExtractedFact {
            metric_key: rf.metric_key.to_string(),
            value: rf.value,
            period,
            // ESEF is a consolidated-IFRS mandate.
            basis: Some(StatementBasis::Consolidated),
            currency,
            tier: SourceTier::Esef,
            citation: rf.concept_local,
        });
    }

    let facts = dedup_longest_duration(facts);
    if facts.is_empty() {
        return Err(EsefError::NoTrackedFacts);
    }
    Ok(facts)
}

/// When a duration concept is reported for several spans sharing a period end
/// (e.g. a 3-month and a year-to-date figure both ending on the report date),
/// keep the longest span — the cumulative YTD value the fundamentals store
/// tracks. Instants and single-span durations pass through unchanged.
fn dedup_longest_duration(facts: Vec<ExtractedFact>) -> Vec<ExtractedFact> {
    use std::collections::HashMap;
    // Key on (metric_key, end_date); keep the fact with the earliest start.
    let mut best: HashMap<(String, String), ExtractedFact> = HashMap::new();
    let mut out_instants: Vec<ExtractedFact> = Vec::new();
    for f in facts {
        match &f.period {
            FactPeriod::Instant(_) => out_instants.push(f),
            FactPeriod::Duration { start, end } => {
                let key = (f.metric_key.clone(), end.clone());
                match best.get(&key) {
                    Some(existing) => {
                        if let FactPeriod::Duration {
                            start: existing_start,
                            ..
                        } = &existing.period
                        {
                            if start < existing_start {
                                best.insert(key, f);
                            }
                        }
                    }
                    None => {
                        best.insert(key, f);
                    }
                }
            }
        }
    }
    let mut out: Vec<ExtractedFact> = out_instants;
    out.extend(best.into_values());
    // Deterministic order for snapshots/tests.
    out.sort_by(|a, b| {
        a.metric_key
            .cmp(&b.metric_key)
            .then(a.period.end_date().cmp(b.period.end_date()))
    });
    out
}

struct RawFact {
    metric_key: &'static str,
    value: Decimal,
    context_ref: String,
    unit_ref: Option<String>,
    concept_local: String,
}

#[derive(Clone, Copy)]
enum PeriodField {
    Instant,
    Start,
    End,
}

/// Reads an attribute value by local key from a start element.
fn attr(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_of(a.key.as_ref()) == key {
            a.unescape_value().ok().map(|v| v.into_owned())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests;
