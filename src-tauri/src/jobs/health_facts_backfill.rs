//! Health-facts re-extraction backfill (ADR 0083 Decision 4 amendment (d),
//! 2026-07-17 — T3 gate finding).
//!
//! Stored `financial_facts` predate the v0.57 concept-map extension (T1), so the
//! four new health concepts (`current_assets`, `current_liabilities`,
//! `retained_earnings`, `long_term_debt`) were never persisted for reports
//! extracted before the extension landed. The history sweep does **not** fill the
//! gap — it only re-attacks periods with *zero* accepted facts, and these periods
//! already carry the older facts, so they are never candidates. This backfill
//! **force-re-runs** the deterministic structured extraction over every stored
//! ESEF-route periodic document, so the new concepts land as confirmed facts.
//!
//! **Additive + idempotent by construction** (the [`record_structured_fact`] slot
//! model, [`crate::storage::StructuredFactCommit`]): an existing slot re-observes
//! (`Reobserved`, a no-op — `created_at`/value untouched) or diverges
//! (`Divergent`, surfaced, **never** silently overwritten); only a genuinely new
//! concept slot is `Created`. So a re-run adds nothing and alters no confirmed
//! fact.
//!
//! **Deterministic-only.** The whole pipeline is deterministic (ADR 0084), so
//! the backfill never guesses; only the ESEF (and positional-xHTML)
//! deterministic tier runs.
//! PDF-only reports are skipped — the new concepts are ESEF/structured-xHTML
//! mapped (ADR 0083 Decision 5).
//!
//! **Trigger idiom.** Modeled on [`crate::jobs::ownership_extraction::
//! backfill_company_ownership_extraction`]: a headless, per-company force-pass. No
//! fundamentals structured-extraction *startup* catch-up exists (only ownership +
//! management-holdings have one), so — per the "do not invent a new startup path"
//! rule — this is not wired into startup; it is invoked headless (the
//! `backfill_company_health_facts` command, and the T3 gate harness).

use serde::Serialize;

use crate::app_state::AppState;
use crate::jobs::structured_extraction::{
    derive_report_period, is_esef_route, is_fetched_periodic, run_structured_extraction,
};
use crate::storage::MODE_AUTOPILOT;

/// The counted outcome of a health-facts backfill over one company.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthFactsBackfillResult {
    /// ESEF-route periodic documents actually re-extracted.
    pub documents_processed: usize,
    /// Documents skipped because their period could not be derived (never a
    /// silent guess — an undatable ESEF file is left alone).
    pub documents_skipped_no_period: usize,
    /// Genuinely new facts written this pass (the missing health concepts on the
    /// first run; `0` on a re-run).
    pub facts_created: usize,
    /// Existing slots re-observed with the same value (idempotent no-ops).
    pub facts_reobserved: usize,
    /// Re-observed slots whose freshly-extracted value disagrees with the stored
    /// (possibly confirmed) fact — surfaced for ratification, never overwritten.
    pub divergences: usize,
}

/// Force-re-extract the deterministic structured facts for every stored ESEF-route
/// periodic document of one company, so the v0.57 health concepts persist. Additive
/// and idempotent (see the module doc). Returns the counted outcome.
pub fn backfill_company_health_facts(
    state: &AppState,
    company_id: &str,
) -> Result<HealthFactsBackfillResult, String> {
    let documents = state
        .list_report_documents_by_company(company_id)
        .map_err(|error| error.to_string())?;

    let mut result = HealthFactsBackfillResult::default();
    for document in documents {
        // ESEF/structured-xHTML deterministic route only — the concept extension is
        // ESEF-mapped; PDF-only reports (which need an OCR profile) are out of scope.
        if !is_fetched_periodic(&document) {
            continue;
        }
        if document.local_path.is_none() {
            continue;
        }
        if !is_esef_route(&document) {
            continue;
        }

        let Some((fiscal_year, period_type, period_end)) = derive_report_period(state, &document)
        else {
            result.documents_skipped_no_period += 1;
            continue;
        };

        let run = run_structured_extraction(
            state,
            company_id,
            &document.id,
            fiscal_year,
            period_type,
            &period_end,
            MODE_AUTOPILOT,
        )?;

        result.documents_processed += 1;
        result.facts_created += run.produced_fact_ids.len();
        result.facts_reobserved += run.skipped_fact_ids.len();
        result.divergences += run.divergences.len();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, ListFinancialFactsInput,
        NewCompany,
    };

    /// A balanced iXBRL statement carrying ONLY the core balance-sheet identity
    /// (Assets = Liabilities + Equity) — the "old" stored facts, extracted before
    /// the v0.57 concept-map extension.
    const CORE_ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
    </html>"#;

    /// The SAME period package after the concept-map extension: the core identity
    /// PLUS the four v0.57 health concepts. Values are internally consistent
    /// (current ⊂ total, retained ⊂ equity, long-term ⊂ liabilities).
    const FULL_ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:CurrentAssets" contextRef="c" unitRef="pln" scale="3">18 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:CurrentLiabilities" contextRef="c" unitRef="pln" scale="3">9 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:RetainedEarnings" contextRef="c" unitRef="pln" scale="3">14 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:LongtermBorrowings" contextRef="c" unitRef="pln" scale="3">6 000</ix:nonFraction>
    </html>"#;

    /// Every concept either fixture tags — all balance-sheet instants (ADR
    /// 0100 decision 3, epic #398). A role entry for a concept absent from a
    /// given instance is simply unused.
    const ROLES: &[(&str, &str)] = &[
        ("Assets", crate::test_support::BALANCE_SHEET_ROLE_SUFFIX),
        (
            "Liabilities",
            crate::test_support::BALANCE_SHEET_ROLE_SUFFIX,
        ),
        ("Equity", crate::test_support::BALANCE_SHEET_ROLE_SUFFIX),
        (
            "CurrentAssets",
            crate::test_support::BALANCE_SHEET_ROLE_SUFFIX,
        ),
        (
            "CurrentLiabilities",
            crate::test_support::BALANCE_SHEET_ROLE_SUFFIX,
        ),
        (
            "RetainedEarnings",
            crate::test_support::BALANCE_SHEET_ROLE_SUFFIX,
        ),
        (
            "LongtermBorrowings",
            crate::test_support::BALANCE_SHEET_ROLE_SUFFIX,
        ),
    ];

    fn unique_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "brawler-health-backfill-{}-{n}",
            std::process::id()
        ))
    }

    /// Seed a company + a fetched ESEF periodic document backed by `content`.
    fn seed(content: &str) -> (AppState, String, String, std::path::PathBuf) {
        let dir = unique_dir();
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = AppState::with_data_dir(open_in_memory_database().expect("db"), dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/skonsolidowany-raport-roczny-2025-ssf.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Skonsolidowany raport roczny 2025 SSF".to_owned()),
                attribution: None,
            })
            .expect("document");
        // A package, not a bare instance (ADR 0100 decision 3, epic #398): a
        // bare iXBRL instance has no presentation linkbase to read, so its
        // facts get no role rows and never survive Layer 2 projection.
        let bytes = crate::test_support::esef_package_zip(content, ROLES);
        std::fs::write(dir.join("report.xhtml"), &bytes).expect("write esef");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.xhtml"),
                Some("application/octet-stream"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id, dir)
    }

    fn facts(state: &AppState, company_id: &str) -> Vec<crate::storage::FinancialFact> {
        state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company_id.to_owned()),
                period_id: None,
                definition_id: None,
            })
            .expect("facts")
    }

    fn metric_keys(state: &AppState, company_id: &str) -> std::collections::BTreeSet<String> {
        let defs = state
            .financials()
            .list_kpi_definitions(crate::storage::ListKpiDefinitionsInput {
                scope: None,
                sector: None,
                company_id: None,
            })
            .expect("definitions");
        let by_id: std::collections::HashMap<String, String> =
            defs.into_iter().map(|d| (d.id, d.metric_key)).collect();
        facts(state, company_id)
            .into_iter()
            .filter_map(|f| by_id.get(&f.definition_id).cloned())
            .collect()
    }

    /// The T3 scenario end-to-end: the stored package predates the concept-map
    /// extension (only the core facts landed), then the backfill re-extracts the
    /// full package — adding exactly the four new concepts, leaving the old facts
    /// byte-identical, and adding zero on a re-run.
    #[test]
    fn backfill_adds_new_concepts_idempotently_without_touching_old_facts() {
        // (1) The "old" extraction: only the core identity landed.
        let (state, company_id, _doc, dir) = seed(CORE_ESEF);
        let first = backfill_company_health_facts(&state, &company_id).expect("first backfill");
        assert_eq!(first.documents_processed, 1);
        assert_eq!(
            first.facts_created, 3,
            "core: total_assets/liabilities/equity"
        );
        let old = facts(&state, &company_id);
        assert_eq!(old.len(), 3);
        // Snapshot the old facts' identity for the untouched assertion.
        let old_snapshot: std::collections::HashMap<String, (String, String)> = old
            .iter()
            .map(|f| {
                (
                    f.id.clone(),
                    (f.value_numeric.clone(), f.created_at.clone()),
                )
            })
            .collect();

        // (2) The concept-map extension: the SAME package now yields the four
        // health concepts too. Overwrite the stored file in place.
        let full_bytes = crate::test_support::esef_package_zip(FULL_ESEF, ROLES);
        std::fs::write(dir.join("report.xhtml"), &full_bytes).expect("rewrite esef");
        let second = backfill_company_health_facts(&state, &company_id).expect("second backfill");
        assert_eq!(second.documents_processed, 1);
        assert_eq!(
            second.facts_created, 4,
            "exactly the four new health concepts are added"
        );
        assert_eq!(
            second.facts_reobserved, 3,
            "the core facts re-observe, never dupe"
        );
        assert_eq!(second.divergences, 0, "no value disagreement");

        let keys = metric_keys(&state, &company_id);
        for concept in [
            "current_assets",
            "current_liabilities",
            "retained_earnings",
            "long_term_debt",
        ] {
            assert!(keys.contains(concept), "backfill added {concept}: {keys:?}");
        }
        // The old facts are byte-identical (idempotent slot re-observation never
        // rewrites a stored fact's value or created_at).
        for (id, (value, created_at)) in &old_snapshot {
            let now = facts(&state, &company_id)
                .into_iter()
                .find(|f| &f.id == id)
                .expect("old fact still present");
            assert_eq!(&now.value_numeric, value, "old fact value untouched");
            assert_eq!(&now.created_at, created_at, "old fact created_at untouched");
        }

        // (3) A re-run adds nothing — purely idempotent.
        let count_before = facts(&state, &company_id).len();
        let third = backfill_company_health_facts(&state, &company_id).expect("third backfill");
        assert_eq!(third.facts_created, 0, "re-run creates zero facts");
        assert_eq!(third.facts_reobserved, 7, "all seven slots re-observe");
        assert_eq!(
            facts(&state, &company_id).len(),
            count_before,
            "no facts added on a re-run"
        );
    }
}
