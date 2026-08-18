//! Tauri commands for the Layer 1 raw-tagged-fact read model and the
//! owner-promotion action (ADR 0100 decisions 1, 2, 10; epic #398 final
//! slice).
//!
//! Two reads (`get_report_tagged_fact_coverage`, `list_uncrosswalked_concepts`)
//! surface what `storage::report_tagged_facts` already computes; the one act
//! (`promote_uncrosswalked_concept`) is the sole reachable path decision 10
//! opens — company-scoped only, the name always sourced from the report
//! itself (never invented), and it materializes the definition's already-
//! captured Layer 1 rows into that company's Fundamentals rather than
//! leaving an empty definition behind.

use serde::Serialize;

use crate::app_state::{self, AppState};
use crate::fundamentals::extraction::esef_package::extract_label_linkbase;
use crate::storage::{
    CompanyHarvestedConcept, ListKpiDefinitionsInput, NewKpiDefinition, StoredTaggedFact,
    StructuredFactCommit, StructuredFactInput, TaggedFactCoverageCounts,
};

/// The Coverage panel's compact read model — see
/// `storage::report_tagged_facts::TaggedFactCoverageCounts` for the bucket
/// semantics; this is a thin passthrough DTO so the Rust type re-exports
/// cleanly to `src/api/generated`.
pub type TaggedFactCoverage = TaggedFactCoverageCounts;

/// One row of "positions the program doesn't know yet" (ADR 0100 decision
/// 10) — a captured concept with no crosswalk entry, at THIS company.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct UncrosswalkedConceptRow {
    pub concept_local_name: String,
    pub concept_namespace_uri: String,
    /// How many companies across the WHOLE corpus tag this concept — the
    /// ranking signal (ADR 0100 decision 2's harvest), not scoped to this
    /// company.
    pub company_count: i64,
    /// How many times THIS company tagged it.
    pub occurrence_count: i64,
    /// `balance | income | cash_flow | other`.
    pub statement_group: String,
    /// `instant | duration`.
    pub period_nature: String,
    /// The name to show. Never invented (decision 10): the issuer's own
    /// published label when `labelSource == "issuer"`, the raw taxonomy
    /// concept name otherwise.
    pub human_label: String,
    /// `issuer` (the package's own label linkbase) | `technical` (no curated
    /// name — the frontend renders the "no translation yet" hint through
    /// `text()`; this is a typed code, never backend prose).
    pub label_source: String,
    pub already_promoted: bool,
    pub promoted_definition_id: Option<String>,
}

/// The result of a promotion — echoes the definition it ensured plus how many
/// NEW facts this call actually wrote (0 on a pure re-run of an
/// already-promoted concept: idempotent, never a duplicate).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PromotedConcept {
    pub definition_id: String,
    pub metric_key: String,
    pub label: String,
    pub label_source: String,
    pub facts_projected: i64,
}

/// The Coverage panel's compact "what did the program read from the report"
/// line (ADR 0100, epic #398). Offloaded (`spawn_blocking`) — it re-runs the
/// deterministic projection rule over the company's Layer 1 rows.
#[tauri::command]
pub async fn get_report_tagged_fact_coverage(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<TaggedFactCoverage, String> {
    let state = state.inner().clone();
    crate::jobs::scheduler::run_blocking_task(move || {
        compute_tagged_fact_coverage(&state, &company_id)
    })
    .await
}

/// "Positions the program doesn't know yet" for one company (ADR 0100
/// decision 10). Offloaded.
#[tauri::command]
pub async fn list_uncrosswalked_concepts(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<UncrosswalkedConceptRow>, String> {
    let state = state.inner().clone();
    crate::jobs::scheduler::run_blocking_task(move || {
        compute_uncrosswalked_concepts(&state, &company_id)
    })
    .await
}

/// The "Show in Fundamentals" action (ADR 0100 decision 10) — the ONLY
/// reachable path onto a captured-but-uncurated concept, and it is
/// company-scoped only: never a canonical crosswalk entry, never machine-
/// initiated. Offloaded.
#[tauri::command]
pub async fn promote_uncrosswalked_concept(
    company_id: String,
    concept_namespace_uri: String,
    concept_local_name: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<PromotedConcept, String> {
    let state = state.inner().clone();
    crate::jobs::scheduler::run_blocking_task(move || {
        promote_uncrosswalked_concept_core(
            &state,
            &company_id,
            &concept_namespace_uri,
            &concept_local_name,
        )
    })
    .await
}

fn company_exists(state: &AppState, company_id: &str) -> Result<bool, String> {
    Ok(state
        .list_companies()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|company| company.id == company_id))
}

pub(crate) fn compute_tagged_fact_coverage(
    state: &AppState,
    company_id: &str,
) -> Result<TaggedFactCoverage, String> {
    if !company_exists(state, company_id)? {
        return Err("company_not_found".to_owned());
    }
    state
        .report_tagged_facts()
        .coverage_counts(company_id)
        .map_err(|error| error.to_string())
}

/// Standard IFRS taxonomy namespaces carry no company-specific label — the
/// package's label linkbase only ever supplies labels for an issuer's OWN
/// extension concepts (ADR 0100 decision 10). One namespace rule for the
/// whole epic: the crosswalk's own predicate (sol round 3).
fn is_standard_taxonomy_concept(namespace_uri: &str) -> bool {
    crate::fundamentals::extraction::ifrs_crosswalk::is_standard_ifrs_namespace(namespace_uri)
}

/// Whether a stored fact belongs to the concept identified by
/// `(namespace, local name)`. Standard concepts match ACROSS taxonomy-year
/// namespace versions (the annual IFRS releases are one vocabulary — the
/// same normalization the harvest counts use); an extension matches only
/// its exact namespace (sol round 3, finding 1).
fn concept_matches(fact_ns: &str, fact_local: &str, ns: &str, local: &str) -> bool {
    if fact_local != local {
        return false;
    }
    if is_standard_taxonomy_concept(ns) {
        is_standard_taxonomy_concept(fact_ns)
    } else {
        fact_ns == ns
    }
}

/// The company-scoped `metric_key` a promoted concept mints (sol rounds
/// 3/4): a standard concept keeps its local name (local names are unique
/// within the IFRS taxonomy by construction); an extension gets a
/// fixed-length key from the FULL sha256 of `(namespace, local)` — ADR 0100
/// decision 2's "namespace-URI digest" at full strength, because a
/// truncated prefix is birthday-collidable by a crafted namespace, and a
/// collision would silently merge two concepts into one definition. The
/// human-readable name lives in `label`; this key is identity, not prose.
fn promoted_metric_key(namespace_uri: &str, local: &str) -> String {
    if is_standard_taxonomy_concept(namespace_uri) {
        return local.to_owned();
    }
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{namespace_uri}\u{1f}{local}").as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest.iter() {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("xconcept_{hex}")
}

/// Reads the ONE report document's stored bytes and parses its label
/// linkbase — `None` on any failure (no stored file, unreadable ZIP, no
/// `*-lab-pl.xml`), which the caller folds into the honest "technical name,
/// no translation yet" fallback, never a guess.
fn resolve_issuer_label(
    state: &AppState,
    report_document_id: &str,
    concept_local_name: &str,
) -> Option<String> {
    let document = state
        .report_documents()
        .get_report_document(report_document_id)
        .ok()?;
    let local_path = document.local_path?;
    let bytes = std::fs::read(state.data_dir().join(&local_path)).ok()?;
    extract_label_linkbase(&bytes).remove(concept_local_name)
}

/// `(human_label, label_source)` per ADR 0100 decision 10: an issuer
/// extension resolves through the package's own label linkbase; a standard
/// concept — or an extension whose linkbase carries no Polish label — falls
/// back to the raw technical name, explicitly marked untranslated.
fn resolve_label(state: &AppState, rows: &[StoredTaggedFact]) -> (String, String) {
    let concept = &rows[0].concept_local_name;
    let namespace = &rows[0].concept_namespace_uri;
    if !is_standard_taxonomy_concept(namespace) {
        if let Some(issuer_label) =
            resolve_issuer_label(state, &rows[0].report_document_id, concept)
        {
            return (issuer_label, "issuer".to_owned());
        }
    }
    (concept.clone(), "technical".to_owned())
}

/// The `scope='company'` definition id this concept would resolve to, if one
/// already exists — `FinancialsStore` exposes no direct "get by metric key"
/// method, so this filters the company's own catalog slice (small — a
/// handful of custom rows per company).
fn find_company_definition(
    state: &AppState,
    company_id: &str,
    metric_key: &str,
) -> Result<Option<crate::storage::KpiDefinition>, String> {
    let defs = state
        .financials()
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("company".to_owned()),
            sector: None,
            company_id: Some(company_id.to_owned()),
        })
        .map_err(|error| error.to_string())?;
    Ok(defs.into_iter().find(|d| d.metric_key == metric_key))
}

pub(crate) fn compute_uncrosswalked_concepts(
    state: &AppState,
    company_id: &str,
) -> Result<Vec<UncrosswalkedConceptRow>, String> {
    if !company_exists(state, company_id)? {
        return Err("company_not_found".to_owned());
    }
    let harvested: Vec<CompanyHarvestedConcept> = state
        .report_tagged_facts()
        .harvest_uncrosswalked_concepts_for_company(company_id)
        .map_err(|error| error.to_string())?;
    let all_facts = state
        .report_tagged_facts()
        .facts_for_company(company_id)
        .map_err(|error| error.to_string())?;

    let mut rows = Vec::with_capacity(harvested.len());
    for concept in harvested {
        let matching: Vec<StoredTaggedFact> = all_facts
            .iter()
            .filter(|f| {
                concept_matches(
                    &f.concept_namespace_uri,
                    &f.concept_local_name,
                    &concept.concept_namespace_uri,
                    &concept.concept_local_name,
                )
            })
            .cloned()
            .collect();
        let (human_label, label_source) = if matching.is_empty() {
            (concept.concept_local_name.clone(), "technical".to_owned())
        } else {
            resolve_label(state, &matching)
        };
        let existing = find_company_definition(
            state,
            company_id,
            &promoted_metric_key(&concept.concept_namespace_uri, &concept.concept_local_name),
        )?;
        rows.push(UncrosswalkedConceptRow {
            concept_local_name: concept.concept_local_name,
            concept_namespace_uri: concept.concept_namespace_uri,
            company_count: concept.company_count,
            occurrence_count: concept.occurrence_count,
            statement_group: concept.statement_group,
            period_nature: concept.period_nature,
            human_label,
            label_source,
            already_promoted: existing.is_some(),
            promoted_definition_id: existing.map(|d| d.id),
        });
    }
    Ok(rows)
}

pub(crate) fn promote_uncrosswalked_concept_core(
    state: &AppState,
    company_id: &str,
    concept_namespace_uri: &str,
    concept_local_name: &str,
) -> Result<PromotedConcept, String> {
    if !company_exists(state, company_id)? {
        return Err("company_not_found".to_owned());
    }

    // The concept's identity is (namespace, local name) — sol round 2: a
    // standard `ifrs-full:Revenue` and an issuer `issuer:Revenue` are
    // different concepts, and promoting one must never sweep in the other's
    // observations.
    let rows: Vec<StoredTaggedFact> = state
        .report_tagged_facts()
        .facts_for_company(company_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|f| {
            concept_matches(
                &f.concept_namespace_uri,
                &f.concept_local_name,
                concept_namespace_uri,
                concept_local_name,
            )
        })
        .collect();
    if rows.is_empty() {
        return Err("concept_not_captured".to_owned());
    }

    // Statement group / period nature: reuse the SAME derivation the list
    // uses (the harvest still carries this concept after promotion — ADR
    // 0100 decision 10's third bullet — so this lookup never goes stale).
    let harvested = state
        .report_tagged_facts()
        .harvest_uncrosswalked_concepts_for_company(company_id)
        .map_err(|error| error.to_string())?;
    let (statement_group, period_nature) = harvested
        .iter()
        .find(|c| {
            c.concept_local_name == concept_local_name
                && c.concept_namespace_uri == concept_namespace_uri
        })
        .map(|c| (c.statement_group.clone(), c.period_nature.clone()))
        .unwrap_or_else(|| ("other".to_owned(), "duration".to_owned()));

    let (label, label_source) = resolve_label(state, &rows);

    let metric_key = promoted_metric_key(concept_namespace_uri, concept_local_name);
    let definition = match find_company_definition(state, company_id, &metric_key)? {
        Some(existing) => existing,
        None => state
            .financials()
            .create_kpi_definition(NewKpiDefinition {
                scope: "company".to_owned(),
                company_id: Some(company_id.to_owned()),
                sector: None,
                metric_key: metric_key.clone(),
                label: label.clone(),
                value_kind: "monetary".to_owned(),
                unit: None,
                computation: "reported".to_owned(),
                formula: None,
                display_format: None,
                origin: Some("user".to_owned()),
                statement_group: Some(statement_group),
                period_nature: Some(period_nature),
            })
            .map_err(|error| error.to_string())?,
    };

    // Project: group the company's dimensionless, valued occurrences of this
    // concept by period_end, resolving repeat-vs-conflict the same way ADR
    // 0100 decision 4 does — a genuine value disagreement is never written,
    // never resolved by document order.
    use std::collections::BTreeMap;
    let mut by_period: BTreeMap<&str, Vec<&StoredTaggedFact>> = BTreeMap::new();
    for row in &rows {
        if row.is_dimensional || row.value_numeric.is_none() {
            continue;
        }
        by_period
            .entry(row.period_end.as_str())
            .or_default()
            .push(row);
    }

    let mut facts_projected = 0i64;
    for (period_end, occurrences) in by_period {
        let first = occurrences[0];
        let first_value = first.value_numeric.as_deref().expect("filtered above");
        let all_agree = occurrences.iter().all(|o| {
            o.value_numeric.as_deref() == Some(first_value) && o.unit_measure == first.unit_measure
        });
        if !all_agree {
            continue; // a typed conflict — never written, decision 4
        }
        let fiscal_year: i64 = period_end
            .get(0..4)
            .and_then(|y| y.parse().ok())
            .unwrap_or(0);
        // GPW ESEF filings are annual-only (ADR 0100 context: the annual
        // report is what carries an iXBRL package), so every promoted period
        // is a fiscal year — matches the `annual` convention `financial_
        // periods.period_type` uses elsewhere (renders "FY").
        let commit = state
            .kpi_extraction()
            .record_structured_fact(StructuredFactInput {
                company_id,
                fiscal_year,
                period_type: "annual",
                period_end: Some(period_end),
                report_document_id: &first.report_document_id,
                metric_key: &definition.metric_key,
                value_numeric: first_value,
                currency: first.unit_measure.as_deref(),
                confirmation_state: "confirmed",
                source_tier: "esef",
                extraction_method: "api",
                // No `validate_tier` gate runs over a promoted position — it
                // is a single company-scoped custom metric, not a canonical
                // statement total the balance/cross-check identities apply
                // to (ADR 0100 decision 10 carves this out as the owner's
                // OWN authority, distinct from decision 5's automated-tier
                // gate). Honestly labeled, the same way the MCP single-fact
                // write path is (`stamp_agent_fact_provenance`).
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some(concept_local_name),
                attribution: None,
                measure_window: None,
                data_quality: None,
            })
            .map_err(|error| error.to_string())?;
        if matches!(commit, StructuredFactCommit::Created(_)) {
            facts_projected += 1;
        }
    }

    Ok(PromotedConcept {
        definition_id: definition.id,
        metric_key: definition.metric_key,
        label,
        label_source,
        facts_projected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, NewCompany, NewTaggedFact, NewTaggedFactRole,
        TaggedFactExtraction,
    };

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    fn seed_document(state: &AppState, company_id: &str, doc_id: &str) {
        // Direct insert — no source fetch machinery needed for these tests,
        // mirroring the storage-layer test seeding idiom.
        state
            .checkout_for_tests()
            .expect("checkout")
            .execute(
                "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
                 VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
                rusqlite::params![doc_id, company_id, format!("https://x/{doc_id}.zip")],
            )
            .expect("document");
    }

    fn balance_fact(identity: &str, namespace: &str, concept: &str, value: &str) -> NewTaggedFact {
        NewTaggedFact {
            package_entry_path: "reports/instance.xhtml".to_owned(),
            fact_identity: identity.to_owned(),
            identity_kind: "xml_id".to_owned(),
            concept_namespace_uri: namespace.to_owned(),
            concept_local_name: concept.to_owned(),
            context_ref: "c1".to_owned(),
            period_type: "instant".to_owned(),
            period_start: None,
            period_end: "2025-12-31".to_owned(),
            unit_ref: Some("u1".to_owned()),
            unit_measure: Some("PLN".to_owned()),
            value_raw: value.to_owned(),
            value_numeric: Some(value.to_owned()),
            scale: Some(0),
            sign: None,
            decimals: Some("0".to_owned()),
            is_dimensional: false,
            dimensions_json: None,
            parse_status: "ok".to_owned(),
            parse_error: None,
            roles: vec![NewTaggedFactRole {
                role_uri: "http://x/role/balance".to_owned(),
                role_kind: "balance".to_owned(),
            }],
        }
    }

    fn seed_uncrosswalked_fact(
        state: &AppState,
        company_id: &str,
        doc_id: &str,
        namespace: &str,
        concept: &str,
        identity: &str,
        value: &str,
    ) {
        seed_document(state, company_id, doc_id);
        state
            .report_tagged_facts()
            .replace_tagged_facts(
                doc_id,
                company_id,
                &TaggedFactExtraction {
                    source_content_hash: Some(format!("hash-{doc_id}")),
                    extractor_version: 1,
                    state: "extracted".to_owned(),
                    encountered_count: 1,
                    stored_count: 1,
                    dimensional_count: 0,
                    no_linkbase_fallback_count: 0,
                    facts: vec![balance_fact(identity, namespace, concept, value)],
                },
            )
            .expect("replace_tagged_facts");
    }

    #[test]
    fn get_report_tagged_fact_coverage_rejects_an_unknown_company() {
        let s = state();
        let error = compute_tagged_fact_coverage(&s, "missing").expect_err("must be rejected");
        assert_eq!(error, "company_not_found");
    }

    /// The digest must bind BOTH identity halves (sol round 5 guardrail):
    /// a hash that quietly dropped either the namespace or the local name
    /// would stay green in the wiring tests while re-enabling the very
    /// merge it exists to prevent. Standard taxonomy-year namespaces are
    /// one vocabulary and share the local-name key.
    #[test]
    fn promoted_metric_key_binds_namespace_and_local_name() {
        let a = promoted_metric_key("http://issuer-a.example.com/2025", "Foo");
        let b = promoted_metric_key("http://issuer-b.example.com/2025", "Foo");
        assert_ne!(
            a, b,
            "same local name, different namespaces — different keys"
        );

        let c = promoted_metric_key("http://issuer-a.example.com/2025", "Bar");
        assert_ne!(
            a, c,
            "same namespace, different local names — different keys"
        );

        let std_2023 = promoted_metric_key(
            "http://xbrl.ifrs.org/taxonomy/2023-03-23/ifrs-full",
            "Assets",
        );
        let std_2024 = promoted_metric_key(
            "https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full",
            "Assets",
        );
        assert_eq!(std_2023, "Assets");
        assert_eq!(
            std_2023, std_2024,
            "standard taxonomy-year namespaces are one vocabulary"
        );
    }

    #[test]
    fn list_uncrosswalked_concepts_marks_an_already_promoted_row() {
        let s = state();
        let company_id = company(&s);
        seed_uncrosswalked_fact(
            &s,
            &company_id,
            "doc1",
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeStandardConceptNotYetCurated",
            "f1",
            "1000",
        );

        let before = compute_uncrosswalked_concepts(&s, &company_id).expect("before");
        assert_eq!(before.len(), 1);
        assert!(!before[0].already_promoted);
        assert_eq!(before[0].label_source, "technical");
        assert_eq!(before[0].human_label, "SomeStandardConceptNotYetCurated");

        promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeStandardConceptNotYetCurated",
        )
        .expect("promote");

        let after = compute_uncrosswalked_concepts(&s, &company_id).expect("after");
        assert_eq!(
            after.len(),
            1,
            "a promoted concept must still appear in the harvest output"
        );
        assert!(after[0].already_promoted);
        assert!(after[0].promoted_definition_id.is_some());
    }

    #[test]
    fn promote_creates_a_company_scoped_definition_with_the_issuer_label() {
        let s = state();
        let company_id = company(&s);
        // An issuer extension namespace (not xbrl.ifrs.org) with no matching
        // label linkbase in this test's document (no stored file) — the
        // honest fallback still applies: technical name, marked untranslated.
        seed_uncrosswalked_fact(
            &s,
            &company_id,
            "doc1",
            "http://issuer.example.com/2025-12-31",
            "PozostaleUslugiObce",
            "f1",
            "500",
        );

        let promoted = promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://issuer.example.com/2025-12-31",
            "PozostaleUslugiObce",
        )
        .expect("promote");

        // An issuer extension's key is the full-strength namespace+name
        // digest (sol rounds 3/4): `issuer:Foo` and `ifrs-full:Foo` promoted
        // at one company can never share a definition, and a crafted
        // namespace cannot birthday-collide a truncated prefix.
        let expected_key = promoted_metric_key(
            "http://issuer.example.com/2025-12-31",
            "PozostaleUslugiObce",
        );
        assert!(expected_key.starts_with("xconcept_") && expected_key.len() == 73);
        assert_eq!(promoted.metric_key, expected_key);
        assert_eq!(
            promoted.label_source, "technical",
            "no stored document bytes to resolve a label from — must fall back honestly"
        );
        assert_eq!(promoted.facts_projected, 1);

        let definitions = s
            .financials()
            .list_kpi_definitions(ListKpiDefinitionsInput {
                scope: Some("company".to_owned()),
                sector: None,
                company_id: Some(company_id.clone()),
            })
            .expect("list definitions");
        let definition = definitions
            .iter()
            .find(|d| d.metric_key == expected_key)
            .expect("company-scoped definition exists");
        assert_eq!(definition.scope, "company");
        assert_eq!(definition.company_id.as_deref(), Some(company_id.as_str()));
    }

    #[test]
    fn a_standard_concept_with_no_curated_name_promotes_with_the_technical_name() {
        let s = state();
        let company_id = company(&s);
        seed_uncrosswalked_fact(
            &s,
            &company_id,
            "doc1",
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeStandardConceptNotYetCurated",
            "f1",
            "1234",
        );

        let promoted = promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeStandardConceptNotYetCurated",
        )
        .expect("promote");

        assert_eq!(promoted.label, "SomeStandardConceptNotYetCurated");
        assert_eq!(promoted.label_source, "technical");
    }

    #[test]
    fn promotion_projects_the_existing_layer1_rows_into_facts() {
        let s = state();
        let company_id = company(&s);
        seed_uncrosswalked_fact(
            &s,
            &company_id,
            "doc1",
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeConcept",
            "f1",
            "777",
        );

        let promoted = promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeConcept",
        )
        .expect("promote");
        assert_eq!(promoted.facts_projected, 1);

        let facts = s
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts");
        let projected = facts
            .iter()
            .find(|f| f.definition_id == promoted.definition_id)
            .expect("the promoted fact exists");
        assert_eq!(projected.value_numeric, "777");
    }

    #[test]
    fn promoting_twice_is_idempotent() {
        let s = state();
        let company_id = company(&s);
        seed_uncrosswalked_fact(
            &s,
            &company_id,
            "doc1",
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeConcept",
            "f1",
            "777",
        );

        let first = promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeConcept",
        )
        .expect("first");
        let second = promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeConcept",
        )
        .expect("second");

        assert_eq!(first.definition_id, second.definition_id);
        assert_eq!(
            second.facts_projected, 0,
            "the second promotion re-observes the same slot, it never creates a duplicate"
        );

        let facts = s
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: Some(first.definition_id.clone()),
            })
            .expect("list facts");
        assert_eq!(facts.len(), 1, "no duplicate fact row");
    }

    #[test]
    fn promotion_never_writes_a_genuine_value_disagreement() {
        let s = state();
        let company_id = company(&s);
        seed_document(&s, &company_id, "doc1");
        s.report_tagged_facts()
            .replace_tagged_facts(
                "doc1",
                &company_id,
                &TaggedFactExtraction {
                    source_content_hash: Some("hash1".to_owned()),
                    extractor_version: 1,
                    state: "extracted".to_owned(),
                    encountered_count: 2,
                    stored_count: 2,
                    dimensional_count: 0,
                    no_linkbase_fallback_count: 0,
                    facts: vec![
                        balance_fact(
                            "f1",
                            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
                            "SomeConcept",
                            "1000",
                        ),
                        balance_fact(
                            "f2",
                            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
                            "SomeConcept",
                            "1500",
                        ),
                    ],
                },
            )
            .expect("replace");

        let promoted = promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "SomeConcept",
        )
        .expect("promote");
        assert_eq!(
            promoted.facts_projected, 0,
            "a genuine disagreement must never resolve by document order"
        );
    }

    #[test]
    fn promote_rejects_a_concept_this_company_never_captured() {
        let s = state();
        let company_id = company(&s);
        let error = promote_uncrosswalked_concept_core(
            &s,
            &company_id,
            "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
            "NeverSeen",
        )
        .expect_err("must be rejected");
        assert_eq!(error, "concept_not_captured");
    }
}
