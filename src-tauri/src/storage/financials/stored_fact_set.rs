//! The stored fact set for one `(company, fiscal_year, period_type)` — the
//! comparative prior [`super::stored_fact_set`]/[`super::stored_fact_set_for_cross_check`]
//! read (ADR 0061 dec. 4b; veto semantics ADR 0086 dec. 3/4; basis filter ADR 0100
//! decision 4, #508). Extracted from `financials.rs` to stay under its file-size
//! ratchet pin (ADR 0103).

use super::*;

/// The stored fact set for one `(company, fiscal_year, period_type)`, bridged to
/// `metric_key`s — the ONE general body [`stored_fact_set`] and
/// [`stored_fact_set_for_cross_check`] share. `veto_filter` is the only knob:
/// - `None`: every stored fact (the plain comparative prior — [`stored_fact_set`]).
/// - `Some(incoming_tier)`: only facts an `incoming_tier` extraction may be VETOED
///   by (ADR 0086 dec. 3/4) — facts with no provenance row (manual, top of the
///   ladder) and facts whose provenance tier the incoming tier does NOT outrank. A
///   strictly-LOWER-tier prior (e.g. the daily BiznesRadar pull) is excluded so a
///   third-party number never fails an issuer filing's comparative cross-check and
///   discards the whole set.
///
/// The tier lookup is ONE provenance query for the whole period (not a per-fact
/// SELECT — the N+1 this consolidation removed, hot in the rebuild's pass-2 over
/// ~250 docs).
///
/// `basis` (#508): `None` = every basis; `Some` = that basis only, no fallback.
fn stored_fact_set_filtered(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    veto_filter: Option<crate::fundamentals::extraction::SourceTier>,
    basis: Option<&str>,
) -> StorageResult<Option<FactSet>> {
    use crate::fundamentals::extraction::SourceTier;

    let periods = list_financial_periods(
        connection,
        ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: Some(fiscal_year),
        },
    )?;
    let Some(period) = periods
        .into_iter()
        .find(|p| p.period_type.eq_ignore_ascii_case(period_type))
    else {
        return Ok(None);
    };

    let mut facts = list_financial_facts(
        connection,
        ListFinancialFactsInput {
            company_id: None,
            period_id: Some(period.id.clone()),
            definition_id: None,
        },
    )?;
    if facts.is_empty() {
        return Ok(None);
    }

    // ADR 0093 dec. 2: `final` first; the stable sort keeps the list's own
    // canonical order (period date, then CANONICAL_FACT_PREFERENCE_ORDER).
    facts.sort_by_key(|f| u8::from(f.data_quality != "final"));

    // The map is read out of the CATALOG (not derived from ids), so it stays
    // correct now that non-canonical ids carry a scope discriminator
    // (`kpi_definition_id`): listing with no scope filter returns every row,
    // whichever scope produced the definition a fact references.
    let definitions = list_kpi_definitions(
        connection,
        ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        },
    )?;
    let metric_key_by_definition: HashMap<String, String> = definitions
        .into_iter()
        .map(|d| (d.id, d.metric_key))
        .collect();

    // One provenance query for the whole period, only when a veto filter is
    // active — a fact absent from this map has no provenance row (a manual entry).
    let tier_by_fact: HashMap<String, String> = if veto_filter.is_some() {
        fact_tiers_for_period(connection, &period.id)?
    } else {
        HashMap::new()
    };

    let mut set = FactSet::new();
    for fact in facts {
        if let Some(incoming_tier) = veto_filter {
            // A fact with no provenance row is a manual entry — always
            // veto-capable. A provenanced fact is veto-capable only when the
            // incoming tier does not outrank it; an unparsable stored tier is
            // treated as veto-capable (never silently discounted).
            let veto_capable = match tier_by_fact.get(&fact.id) {
                None => true,
                Some(stored) => match SourceTier::parse(stored) {
                    Some(stored_tier) => !incoming_tier.outranks(stored_tier),
                    None => true,
                },
            };
            if !veto_capable {
                continue;
            }
        }
        // #508: like-for-like only, no fallback.
        if basis.is_some_and(|b| fact.statement_basis != b) {
            continue;
        }
        let Some(metric_key) = metric_key_by_definition.get(&fact.definition_id) else {
            continue;
        };
        let Ok(value) = Decimal::from_str(fact.value_numeric.trim()) else {
            continue;
        };
        // Slot-once: facts are pre-sorted final-first, so the first value
        // seen per metric_key is kept — a later (non-final, or same-quality
        // but older) sibling never overwrites a final value already in the
        // set. This is what closes the THE REAL HAZARD (ADR 0093 T4): a
        // preliminary row can no longer silently shadow its final sibling in
        // the cross-check prior.
        set.entry(metric_key.clone()).or_insert(value);
    }

    if set.is_empty() {
        Ok(None)
    } else {
        Ok(Some(set))
    }
}

/// `fact_id → source_tier` for every provenanced fact in one period, in a single
/// JOIN — the batched replacement for the per-fact `fact_source_tier` SELECT.
fn fact_tiers_for_period(
    connection: &Connection,
    period_id: &str,
) -> StorageResult<HashMap<String, String>> {
    let mut statement = connection.prepare(
        "SELECT p.fact_id, p.source_tier
         FROM financial_fact_provenance p
         JOIN financial_facts f ON f.id = p.fact_id
         WHERE f.period_id = ?1",
    )?;
    let rows = statement.query_map([period_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(StorageError::from)
}

/// The plain comparative prior — every stored fact for the period, bridged to
/// `metric_key`s (ADR 0061 dec. 4b). This is the cross-check-UNAWARE variant: a
/// future author wanting the reversed-witnessing veto semantics wants
/// [`stored_fact_set_for_cross_check`] instead. Only test harnesses read this
/// unfiltered form today (production callers go through the cross-check variant).
pub(in crate::storage) fn stored_fact_set(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
) -> StorageResult<Option<FactSet>> {
    stored_fact_set_filtered(connection, company_id, fiscal_year, period_type, None, None)
}

/// [`stored_fact_set`] restricted to facts an `incoming_tier` extraction may be
/// VETOED by (ADR 0086 decisions 3/4). See [`stored_fact_set_filtered`] for the
/// veto semantics and the `basis` filter (ADR 0100 decision 4, #508).
pub(in crate::storage) fn stored_fact_set_for_cross_check(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    incoming_tier: crate::fundamentals::extraction::SourceTier,
    basis: Option<&str>,
) -> StorageResult<Option<FactSet>> {
    stored_fact_set_filtered(
        connection,
        company_id,
        fiscal_year,
        period_type,
        Some(incoming_tier),
        basis,
    )
}
