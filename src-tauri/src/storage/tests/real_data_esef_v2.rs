//! ESEF measurement v2 — cold-start harness (#331 PR-A, ADR 0112).
//!
//! Unlike [`super::real_data_extraction::esef_positional_ground_truth_scores`]
//! (a stored-state audit over an existing DB snapshot), this harness RUNS the
//! real deterministic pipeline from scratch for every labeled event: a fresh
//! in-memory database, a real [`crate::storage::AppState`], the real
//! [`crate::jobs::structured_extraction::derive_report_period`] +
//! [`crate::jobs::structured_extraction::run_structured_extraction`] — then
//! reads back whatever those functions actually committed and scores it
//! against hand-labeled ground truth. See the shared data contract
//! (`contract-331-shared.md`, ADR 0112) for every JSON shape and the matching
//! rules this file implements to.
//!
//! Two outcome maps, not one (the shared contract's `keyed-outcomes.json`):
//! a ground-truth **slot** only ever resolves to `MATCH` / `MISSING` /
//! `UNVERIFIED` (the recall side); a **prediction** (a stored fact) resolves
//! to `MATCH` or one of the mismatch classes / `FALSE_POSITIVE` /
//! `OUT_OF_SCOPE` (the precision side). A "paired but wrong" prediction
//! therefore counts against precision while leaving its GT slot `MISSING` —
//! the two maps are scored independently once pairing has run.
//!
//! **Inert in CI** — [`esef_measurement_v2`] skips loudly unless
//! `BRAWLER_ESEF_V2_DIR` names a readable corpus (`BRAWLER_ESEF_REQUIRED=1`
//! turns that into a panic). The hermetic `#[test]`s below run in CI: most
//! call [`run_measurement`] against the committed synthetic sample corpus
//! (`testdata/esef-v2-sample/`, copied to a temp dir first — never mutated in
//! place), a few call the pure matcher ([`score_event`]) directly with
//! hand-built slots/predictions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::jobs::structured_extraction::{derive_report_period, run_structured_extraction};
use crate::report_documents_capture::content_hash_hex;
use crate::storage::{
    open_in_memory_database, CaptureReportDocumentInput, ListFinancialFactsInput,
    ListFinancialPeriodsInput, NewCompany, MODE_AUTOPILOT,
};
use crate::test_support::unique_temp_dir;

// ===========================================================================
// JSON domain shapes (shared data contract — implement to it, never invent)
// ===========================================================================

#[derive(Debug, Deserialize)]
struct Manifest {
    manifest_version: i64,
    issuers: Vec<Issuer>,
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct Issuer {
    issuer_id: String,
    ticker: String,
    exchange: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct Event {
    event_id: String,
    issuer_id: String,
    /// `floor` | `twin_diagnostic` | `warmup`.
    role: String,
    labeled_period: LabeledPeriod,
    file: EventFile,
    document: EventDocument,
    warmup_order: i64,
}

#[derive(Debug, Deserialize)]
struct LabeledPeriod {
    fiscal_year: i64,
    period_type: String,
    period_end: String,
}

#[derive(Debug, Deserialize)]
struct EventFile {
    name: String,
}

#[derive(Debug, Deserialize)]
struct EventDocument {
    title: String,
    url: String,
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct GroundTruthV2 {
    key_map_version: i64,
    slots: Vec<GtSlot>,
}

#[derive(Debug, Clone, Deserialize)]
struct GtSlot {
    slot_id: String,
    event_id: String,
    concept_local: String,
    /// `total` | `owners_of_parent` | `nci`.
    attribution: String,
    /// `consolidated` | `standalone` | `unknown`.
    basis: String,
    /// `flow` | `point_in_time`.
    window: String,
    variant: String,
    fiscal_year: i64,
    period_type: String,
    period_end: String,
    currency: String,
    value: String,
    /// `machine` | `second_read` | `adjudicated` | `unverified` | `machine_v1`.
    verification: String,
}

#[derive(Debug, Deserialize)]
struct KeyMap {
    key_map_version: i64,
    entries: Vec<KeyMapEntry>,
    panel: Vec<String>,
    stored_attribution_on_structured_writes: String,
}

#[derive(Debug, Deserialize)]
struct KeyMapEntry {
    concept: String,
    metric_key: String,
}

/// The public key map, resolved to the lookups scoring needs: concept ->
/// metric_key (GT side), the panel set (precision/current-recall scope), and
/// the one attribution value every structured write stamps (ADR 0095) — a
/// prediction whose stored attribution differs from this is `ATTRIBUTION_MISMATCH`
/// BEFORE matching ever runs (shared contract), never compared per-pair
/// against a GT slot's own semantic attribution (that semantic distinction is
/// already carried by `metric_key` — e.g. `wdf_equity_parent` vs `total_equity`).
struct ResolvedKeyMap {
    concept_to_metric_key: BTreeMap<String, String>,
    panel: BTreeSet<String>,
    stored_attribution_on_structured_writes: String,
}

fn load_key_map(repo_root: &Path) -> KeyMap {
    let path = repo_root.join("scripts/realdata/esef-v2/gt_key_map.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("gt_key_map.json unreadable at {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("gt_key_map.json malformed field: {e}"))
}

fn resolve_key_map(map: &KeyMap, gt_key_map_version: i64) -> ResolvedKeyMap {
    assert_eq!(
        map.key_map_version, gt_key_map_version,
        "incomparable: ground_truth_v2.json key_map_version {} != gt_key_map.json key_map_version {}",
        gt_key_map_version, map.key_map_version
    );
    ResolvedKeyMap {
        concept_to_metric_key: map
            .entries
            .iter()
            .map(|e| (e.concept.clone(), e.metric_key.clone()))
            .collect(),
        panel: map.panel.iter().cloned().collect(),
        stored_attribution_on_structured_writes: map
            .stored_attribution_on_structured_writes
            .clone(),
    }
}

// ===========================================================================
// Outcome vocabulary (shared data contract)
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Outcome {
    Match,
    WrongValue,
    Missing,
    FalsePositive,
    BasisMismatch,
    WindowMismatch,
    VariantMismatch,
    PeriodDateMismatch,
    AttributionMismatch,
    CurrencyMissing,
    CurrencyMismatch,
    Unverified,
    OutOfScope,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Match => "MATCH",
            Outcome::WrongValue => "WRONG_VALUE",
            Outcome::Missing => "MISSING",
            Outcome::FalsePositive => "FALSE_POSITIVE",
            Outcome::BasisMismatch => "BASIS_MISMATCH",
            Outcome::WindowMismatch => "WINDOW_MISMATCH",
            Outcome::VariantMismatch => "VARIANT_MISMATCH",
            Outcome::PeriodDateMismatch => "PERIOD_DATE_MISMATCH",
            Outcome::AttributionMismatch => "ATTRIBUTION_MISMATCH",
            Outcome::CurrencyMissing => "CURRENCY_MISSING",
            Outcome::CurrencyMismatch => "CURRENCY_MISMATCH",
            Outcome::Unverified => "UNVERIFIED",
            Outcome::OutOfScope => "OUT_OF_SCOPE",
        }
    }
}

// ===========================================================================
// The pure matcher (score_event) — no DB, fully unit-testable
// ===========================================================================

/// A ground-truth slot, resolved to the internal shape the matcher compares —
/// [`GtSlot`] plus its concept translated to `metric_key` and its value parsed.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedSlot {
    pub slot_id: String,
    pub concept_local: String,
    pub metric_key: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end: String,
    pub basis: String,
    pub attribution: String,
    pub variant: String,
    pub window: String,
    pub currency: String,
    pub value: Decimal,
    pub verification: String,
}

/// A stored fact, resolved to the internal shape the matcher compares — one
/// `financial_facts` row (+ its joined period), read back after a real
/// extraction run.
#[derive(Debug, Clone)]
pub(crate) struct Prediction {
    pub id: String,
    pub metric_key: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end: String,
    pub basis: String,
    pub attribution: String,
    pub variant: String,
    pub window: String,
    pub currency: Option<String>,
    pub value: Decimal,
    pub data_quality: String,
}

/// One event's scoring result: the GT-slot outcome map (recall side — only
/// ever `Match`/`Missing`/`Unverified`) and the prediction outcome map
/// (precision side — `Match`/a mismatch class/`FalsePositive`/`OutOfScope`).
#[derive(Debug, Default)]
pub(crate) struct EventScore {
    pub slot_outcomes: BTreeMap<String, Outcome>,
    pub prediction_outcomes: BTreeMap<String, Outcome>,
}

/// How many of the six comparable dimensions differ between a GT slot and a
/// candidate prediction (used only to rank mismatch-pairing candidates —
/// "fewest differing dimensions" per the shared contract). Attribution is
/// deliberately NOT one of them: it is a pre-filter against the global
/// `stored_attribution_on_structured_writes` constant (see
/// [`partition_by_attribution`]), never a per-pair comparison against a GT
/// slot's own semantic attribution — that semantic distinction already lives
/// in `metric_key` (`wdf_equity_parent` vs `total_equity`), and production
/// stamps the SAME attribution (`total`) on every structured write regardless
/// of concept.
fn diff_count(gt: &ResolvedSlot, p: &Prediction) -> usize {
    let mut n = 0;
    if p.currency.as_deref() != Some(gt.currency.as_str()) {
        n += 1;
    }
    if p.basis != gt.basis {
        n += 1;
    }
    if p.window != gt.window {
        n += 1;
    }
    if p.variant != gt.variant {
        n += 1;
    }
    if p.period_end != gt.period_end {
        n += 1;
    }
    if p.value != gt.value {
        n += 1;
    }
    n
}

/// The single reported class for a (gt, prediction) pair that is not an exact
/// match — `None` if the pair is actually exact (every dimension agrees).
/// Priority order when several dimensions differ at once (a design choice the
/// shared contract leaves open, documented in the PR-A report): currency >
/// basis > window > variant > period date > value.
fn classify_pair(gt: &ResolvedSlot, p: &Prediction) -> Option<Outcome> {
    match p.currency.as_deref() {
        None => return Some(Outcome::CurrencyMissing),
        Some(c) if c != gt.currency => return Some(Outcome::CurrencyMismatch),
        _ => {}
    }
    if p.basis != gt.basis {
        return Some(Outcome::BasisMismatch);
    }
    if p.window != gt.window {
        return Some(Outcome::WindowMismatch);
    }
    if p.variant != gt.variant {
        return Some(Outcome::VariantMismatch);
    }
    if p.period_end != gt.period_end {
        return Some(Outcome::PeriodDateMismatch);
    }
    if p.value != gt.value {
        return Some(Outcome::WrongValue);
    }
    None
}

/// Scores one event's GT slots against its predictions (shared contract
/// matching rules): exact matches first, then deterministic mismatch pairing
/// (same `metric_key` + same fiscal period, fewest differing dimensions,
/// stable id order breaking ties), one-to-one throughout. `panel` decides
/// whether an unpaired prediction is `FALSE_POSITIVE` (inside) or
/// `OUT_OF_SCOPE` (outside). Predictions whose `data_quality != "final"` must
/// be filtered to `OUT_OF_SCOPE` by the CALLER before this function ever sees
/// them (this function has no data-quality opinion).
pub(crate) fn score_event(
    gt_slots: &[ResolvedSlot],
    predictions: &[Prediction],
    panel: &BTreeSet<String>,
) -> EventScore {
    let mut score = EventScore::default();
    let mut remaining_gt: Vec<&ResolvedSlot> = Vec::new();

    for gt in gt_slots {
        if gt.verification == "unverified" {
            score
                .slot_outcomes
                .insert(gt.slot_id.clone(), Outcome::Unverified);
        } else {
            remaining_gt.push(gt);
        }
    }
    remaining_gt.sort_by(|a, b| a.slot_id.cmp(&b.slot_id));

    let mut claimed: BTreeSet<String> = BTreeSet::new();

    // Pass 1 — exact matches (same metric_key, zero differing dimensions).
    let mut still_unmatched: Vec<&ResolvedSlot> = Vec::new();
    for gt in remaining_gt {
        let exact = predictions
            .iter()
            .filter(|p| !claimed.contains(&p.id) && p.metric_key == gt.metric_key)
            .find(|p| classify_pair(gt, p).is_none());
        match exact {
            Some(p) => {
                claimed.insert(p.id.clone());
                score
                    .slot_outcomes
                    .insert(gt.slot_id.clone(), Outcome::Match);
                score
                    .prediction_outcomes
                    .insert(p.id.clone(), Outcome::Match);
            }
            None => still_unmatched.push(gt),
        }
    }

    // Pass 2 — deterministic mismatch pairing: same metric_key + same fiscal
    // period, fewest differing dimensions, stable prediction-id order on ties.
    for gt in still_unmatched {
        let best = predictions
            .iter()
            .filter(|p| {
                !claimed.contains(&p.id)
                    && p.metric_key == gt.metric_key
                    && p.fiscal_year == gt.fiscal_year
                    && p.period_type == gt.period_type
            })
            .min_by_key(|p| (diff_count(gt, p), p.id.clone()));
        score
            .slot_outcomes
            .insert(gt.slot_id.clone(), Outcome::Missing);
        if let Some(p) = best {
            claimed.insert(p.id.clone());
            let outcome = classify_pair(gt, p).unwrap_or(Outcome::WrongValue);
            score.prediction_outcomes.insert(p.id.clone(), outcome);
        }
    }

    // Every prediction never claimed above is unpaired.
    for p in predictions {
        if !claimed.contains(&p.id) {
            let outcome = if panel.contains(&p.metric_key) {
                Outcome::FalsePositive
            } else {
                Outcome::OutOfScope
            };
            score.prediction_outcomes.insert(p.id.clone(), outcome);
        }
    }

    score
}

// ===========================================================================
// Aggregates (the public-shaped metrics file)
// ===========================================================================

#[derive(Debug, Default, Serialize)]
pub(crate) struct CountPair {
    pub matched: usize,
    pub eligible: usize,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct AvailabilityPair {
    pub available: usize,
    pub eligible: usize,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Layer1Capture {
    pub captured: usize,
    pub eligible: usize,
    pub value_correct: usize,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Sensitivity {
    pub matched: usize,
    pub gt_slots: usize,
    pub excluded: usize,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct TwinAgreement {
    pub agree: usize,
    pub compared: usize,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Replay {
    pub events: usize,
    pub exercised_prior_check: usize,
    pub exercised_quarantine: usize,
    pub delta_matched: i64,
}

/// The public-shaped aggregate metrics (shared contract). Deliberately no
/// ticker/title/id/url/value/filename anywhere — asserted by a hermetic test
/// scanning the serialized JSON text.
#[derive(Debug, Default, Serialize)]
pub(crate) struct Aggregates {
    pub profile: String,
    pub status: String,
    pub measurement_version: i64,
    pub gt_version: String,
    pub key_map_version: i64,
    pub normalization_version: i64,
    pub registry_hash: String,
    pub events: usize,
    pub floor_events: usize,
    pub issuers: usize,
    pub gt_slots: usize,
    pub unverified: usize,
    pub matched: usize,
    pub previously_correct_slots_lost: usize,
    pub false_positives: usize,
    pub zero_output_events: usize,
    pub availability_all_periods: AvailabilityPair,
    pub layer1_capture: Layer1Capture,
    pub labeled_capability: CountPair,
    pub sensitivity: Sensitivity,
    pub twin_agreement: TwinAgreement,
    pub replay: Replay,
}

/// `previously_correct_slots_lost` (shared contract): a slot the promoted
/// baseline recorded `MATCH` for that this run no longer matches. Pure —
/// independently testable without any DB (test 11).
pub(crate) fn previously_correct_slots_lost(
    current: &BTreeMap<String, Outcome>,
    baseline: &BTreeMap<String, String>,
) -> Vec<String> {
    baseline
        .iter()
        .filter(|(slot_id, outcome)| {
            outcome.as_str() == "MATCH"
                && current.get(slot_id.as_str()).copied() != Some(Outcome::Match)
        })
        .map(|(slot_id, _)| slot_id.clone())
        .collect()
}

// ===========================================================================
// Cold-start integration — the real production pipeline, per event
// ===========================================================================

/// The stored facts a fresh cold-start run produced for one event's company,
/// resolved into [`Prediction`]s the matcher can compare — plus the derived
/// period (for the "derivation passed through" test) and whether the run
/// itself errored (a `zero_output_events` case).
struct ColdStartRun {
    predictions: Vec<Prediction>,
    derived: Option<(i64, String, String)>,
    run_error: Option<String>,
    layer1_facts: Vec<(String, String, Option<String>)>, // (concept_local, period_end, value_numeric)
}

/// Seeds a fresh company + report document for one manifest event and runs
/// the REAL production pipeline against it: [`derive_report_period`] then
/// [`run_structured_extraction`] (never a hand-rolled substitute). `state` is
/// returned so callers that need the shared per-issuer DB (replay) can reuse
/// it across events instead of opening a fresh one.
fn seed_and_run_event(
    state: &AppState,
    issuer: &Issuer,
    event: &Event,
    corpus_dir: &Path,
) -> ColdStartRun {
    let company = state
        .create_company(NewCompany {
            exchange: issuer.exchange.clone(),
            ticker: issuer.ticker.clone(),
            display_name: issuer.display_name.clone(),
            isin: None,
            cik: None,
            lei: None,
        })
        .or_else(|_| {
            // Replay reuses one company across events for the same issuer —
            // `create_company` on an existing ticker fails; fetch it instead.
            state.list_companies().map(|companies| {
                companies
                    .into_iter()
                    .find(|c| c.ticker == issuer.ticker)
                    .expect("company just failed to create must already exist")
            })
        })
        .expect("company");

    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: event.document.url.clone(),
            period_id: None,
            origin_ref: None,
            title: Some(event.document.title.clone()),
            attribution: None,
        })
        .expect("document");

    // The shared data contract's file layout: the corpus dir holds
    // `MANIFEST_v2.json`/`ground_truth_v2.json` at its top level and the
    // actual report files under `corpus/` — `file.name` is relative to that
    // subdirectory, never to the corpus dir itself.
    let relative_path = format!("corpus/{}", event.file.name);
    let bytes = std::fs::read(corpus_dir.join(&relative_path)).unwrap_or_else(|e| {
        panic!(
            "corpus file {} unreadable: {e}",
            corpus_dir.join(&relative_path).display()
        )
    });
    let content_hash = content_hash_hex(&bytes);
    // `local_path` is relative to `state.data_dir()`, which this harness
    // points at the corpus dir itself (see `with_data_dir(_, corpus_dir)`
    // below) — so it must carry the same `corpus/` prefix as the read above.
    state
        .mark_report_document_fetched(
            &document.id,
            Some(&relative_path),
            Some(&event.document.content_type),
            Some(&content_hash),
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");

    let document = state.get_report_document(&document.id).expect("document");
    let derived =
        derive_report_period(state, &document).map(|(fy, pt, pe)| (fy, pt.to_owned(), pe));

    let mut run_error = None;
    if let Some((fy, pt, pe)) = &derived {
        if let Err(err) = run_structured_extraction(
            state,
            &company.id,
            &document.id,
            *fy,
            pt,
            pe,
            MODE_AUTOPILOT,
        ) {
            run_error = Some(err);
        }
    }

    let predictions = read_predictions(state, &company.id);
    let layer1_facts = state
        .report_tagged_facts()
        .facts(&document.id)
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.concept_local_name, f.period_end, f.value_numeric))
        .collect();

    ColdStartRun {
        predictions,
        derived,
        run_error,
        layer1_facts,
    }
}

/// Reads back every stored fact for `company_id`, joined to its period, as
/// [`Prediction`]s — the harness's only read of the app's own state, through
/// the real read models (`FinancialsStore::list_financial_facts` +
/// `list_financial_periods`), never a private/shortcut query.
fn read_predictions(state: &AppState, company_id: &str) -> Vec<Prediction> {
    let periods = state
        .financials()
        .list_financial_periods(ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: None,
        })
        .expect("list financial periods");
    let period_by_id: BTreeMap<&str, &crate::storage::FinancialPeriod> =
        periods.iter().map(|p| (p.id.as_str(), p)).collect();

    let facts = state
        .financials()
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        })
        .expect("list financial facts");

    facts
        .into_iter()
        .filter_map(|f| {
            let period = period_by_id.get(f.period_id.as_str())?;
            let value = f.value_numeric.parse::<Decimal>().ok()?;
            Some(Prediction {
                id: f.id,
                metric_key: f.metric_key,
                fiscal_year: period.fiscal_year,
                period_type: period.period_type.clone(),
                period_end: period.period_end_date.clone().unwrap_or_default(),
                basis: f.statement_basis,
                attribution: f.attribution,
                variant: f.variant,
                window: f.measure_window,
                currency: f.currency,
                value,
                data_quality: f.data_quality,
            })
        })
        .collect()
}

/// A prediction never even offered to the matcher, plus WHY (shared
/// contract: `data_quality != "final"` is `OUT_OF_SCOPE`; a stored attribution
/// other than the global `stored_attribution_on_structured_writes` constant is
/// `ATTRIBUTION_MISMATCH` — both decided before pairing ever runs).
struct PreFiltered {
    eligible: Vec<Prediction>,
    excluded: Vec<(Prediction, Outcome)>,
}

fn prefilter_predictions(
    predictions: Vec<Prediction>,
    resolved_map: &ResolvedKeyMap,
) -> PreFiltered {
    let mut eligible = Vec::new();
    let mut excluded = Vec::new();
    for p in predictions {
        if p.data_quality != "final" {
            excluded.push((p, Outcome::OutOfScope));
        } else if p.attribution != resolved_map.stored_attribution_on_structured_writes {
            excluded.push((p, Outcome::AttributionMismatch));
        } else {
            eligible.push(p);
        }
    }
    PreFiltered { eligible, excluded }
}

// ===========================================================================
// run_measurement — the full orchestration
// ===========================================================================

pub(crate) struct MeasurementConfig<'a> {
    pub corpus_dir: &'a Path,
    pub metrics_out: Option<&'a Path>,
    pub keyed_baseline: Option<&'a Path>,
    pub required: bool,
}

/// The full measurement run: loads the manifest/ground-truth/key-map, cold-
/// starts every `floor`/`twin_diagnostic` event, scores it, replays the
/// issuer's events in one shared DB, writes the private evidence files and the
/// public-shaped metrics, prints the console report, and returns the
/// aggregates. `None` when `corpus_dir` is absent/unreadable and
/// `config.required` is `false` (the CI-safe skip); panics when `required` is
/// `true` (typed panics also fire for version mismatches / malformed JSON —
/// never a silent skip for those).
pub(crate) fn run_measurement(config: MeasurementConfig<'_>) -> Option<Aggregates> {
    if !config.corpus_dir.is_dir() {
        if config.required {
            panic!(
                "BRAWLER_ESEF_REQUIRED=1 but the corpus dir does not exist: {}",
                config.corpus_dir.display()
            );
        }
        eprintln!(
            "SKIP esef_measurement_v2: no corpus at {}",
            config.corpus_dir.display()
        );
        return None;
    }

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let key_map = load_key_map(&repo_root);

    let manifest_path = config.corpus_dir.join("MANIFEST_v2.json");
    let manifest_raw = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!(
            "MANIFEST_v2.json unreadable at {}: {e}",
            manifest_path.display()
        )
    });
    let manifest: Manifest = serde_json::from_str(&manifest_raw)
        .unwrap_or_else(|e| panic!("MANIFEST_v2.json malformed field: {e}"));
    assert_eq!(
        manifest.manifest_version, 1,
        "incomparable: unsupported manifest_version {}",
        manifest.manifest_version
    );

    let gt_path = config.corpus_dir.join("ground_truth_v2.json");
    let gt_raw = std::fs::read_to_string(&gt_path).unwrap_or_else(|e| {
        panic!(
            "ground_truth_v2.json unreadable at {}: {e}",
            gt_path.display()
        )
    });
    let ground_truth: GroundTruthV2 = serde_json::from_str(&gt_raw)
        .unwrap_or_else(|e| panic!("ground_truth_v2.json malformed field: {e}"));
    let resolved_map = resolve_key_map(&key_map, ground_truth.key_map_version);

    let mut gt_by_event: BTreeMap<&str, Vec<ResolvedSlot>> = BTreeMap::new();
    let mut unverified_total = 0usize;
    for slot in &ground_truth.slots {
        let Some(metric_key) = resolved_map.concept_to_metric_key.get(&slot.concept_local) else {
            panic!(
                "incomparable: ground truth concept '{}' (slot {}) has no gt_key_map entry",
                slot.concept_local, slot.slot_id
            );
        };
        if slot.verification == "unverified" {
            unverified_total += 1;
        }
        let value = slot.value.parse::<Decimal>().unwrap_or_else(|e| {
            panic!(
                "incomparable: slot {} value '{}': {e}",
                slot.slot_id, slot.value
            )
        });
        gt_by_event
            .entry(&slot.event_id)
            .or_default()
            .push(ResolvedSlot {
                slot_id: slot.slot_id.clone(),
                concept_local: slot.concept_local.clone(),
                metric_key: metric_key.clone(),
                fiscal_year: slot.fiscal_year,
                period_type: slot.period_type.clone(),
                period_end: slot.period_end.clone(),
                basis: slot.basis.clone(),
                attribution: slot.attribution.clone(),
                variant: slot.variant.clone(),
                window: slot.window.clone(),
                currency: slot.currency.clone(),
                value,
                verification: slot.verification.clone(),
            });
    }

    let issuer_by_id: BTreeMap<&str, &Issuer> = manifest
        .issuers
        .iter()
        .map(|i| (i.issuer_id.as_str(), i))
        .collect();

    // ---- cold start (one fresh DB per event) — the floor/precision numbers --
    let mut all_slot_outcomes: BTreeMap<String, Outcome> = BTreeMap::new();
    let mut all_prediction_outcomes: BTreeMap<String, Outcome> = BTreeMap::new();
    let mut floor_events = 0usize;
    let mut zero_output_events = 0usize;
    let mut availability_available = 0usize;
    let mut availability_eligible = 0usize;
    let mut layer1_captured = 0usize;
    let mut layer1_value_correct = 0usize;
    // event_id -> (derived "fy/pt/period_end" or "none", labeled "fy/pt/period_end") —
    // the evidence table's derived-vs-labeled column (deliverable 2).
    let mut derived_by_event: BTreeMap<String, (String, String)> = BTreeMap::new();

    for event in &manifest.events {
        if event.role == "warmup" {
            continue;
        }
        let issuer = issuer_by_id
            .get(event.issuer_id.as_str())
            .unwrap_or_else(|| {
                panic!(
                    "incomparable: event {} names unknown issuer {}",
                    event.event_id, event.issuer_id
                )
            });
        let connection = open_in_memory_database().expect("in-memory db");
        let state = AppState::with_data_dir(connection, config.corpus_dir.to_path_buf());
        let run = seed_and_run_event(&state, issuer, event, config.corpus_dir);

        let empty = Vec::new();
        let gt_slots = gt_by_event.get(event.event_id.as_str()).unwrap_or(&empty);
        for slot in gt_slots {
            availability_eligible += 1;
            if let Some((concept, _, value)) =
                run.layer1_facts.iter().find(|(concept, period_end, _)| {
                    concept == &slot.concept_local && period_end == &slot.period_end
                })
            {
                availability_available += 1;
                layer1_captured += 1;
                let _ = concept;
                if value.as_deref().and_then(|v| v.parse::<Decimal>().ok()) == Some(slot.value) {
                    layer1_value_correct += 1;
                }
            }
        }

        let has_predictions = !run.predictions.is_empty();
        let filtered = prefilter_predictions(run.predictions, &resolved_map);
        let score = score_event(gt_slots, &filtered.eligible, &resolved_map.panel);

        if event.role == "floor" {
            floor_events += 1;
            if !has_predictions {
                zero_output_events += 1;
            }
            all_slot_outcomes.extend(score.slot_outcomes.clone());
            all_prediction_outcomes.extend(score.prediction_outcomes.clone());
            for (p, outcome) in &filtered.excluded {
                all_prediction_outcomes.insert(p.id.clone(), *outcome);
            }
        }

        if let Some(error) = &run.run_error {
            eprintln!("  RUN ERROR event={} : {error}", event.event_id);
        }
        let derived_str = run
            .derived
            .as_ref()
            .map(|(fy, pt, pe)| format!("{fy}/{pt}/{pe}"))
            .unwrap_or_else(|| "none".to_owned());
        let labeled_str = format!(
            "{}/{}/{}",
            event.labeled_period.fiscal_year,
            event.labeled_period.period_type,
            event.labeled_period.period_end
        );
        if derived_str != labeled_str {
            eprintln!(
                "  NOTE event={} derived period ({derived_str}) != labeled period ({labeled_str})",
                event.event_id
            );
        }
        derived_by_event.insert(event.event_id.clone(), (derived_str, labeled_str));
    }

    // ---- twin agreement -----------------------------------------------------
    // Compare each twin_diagnostic event's per-identity outcome against its
    // paired floor event's (same issuer + period_end) — computed above by
    // accumulating floor identities first is not guaranteed by iteration
    // order, so recompute cleanly below instead of relying on map merge order.
    let (twin_agree, twin_compared) = compute_twin_agreement(
        &manifest,
        &gt_by_event,
        config.corpus_dir,
        &issuer_by_id,
        &resolved_map,
    );

    // ---- replay (diagnostic; one shared DB per issuer) ----------------------
    let replay = run_replay(
        &manifest,
        &gt_by_event,
        config.corpus_dir,
        &issuer_by_id,
        &resolved_map,
        &all_slot_outcomes,
        floor_events,
    );

    let matched = all_slot_outcomes
        .values()
        .filter(|o| **o == Outcome::Match)
        .count();
    let false_positives = all_prediction_outcomes
        .values()
        .filter(|o| **o == Outcome::FalsePositive)
        .count();

    // ---- previously_correct_slots_lost ---------------------------------------
    let baseline: BTreeMap<String, String> = config
        .keyed_baseline
        .filter(|p| p.is_file())
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("outcomes").cloned())
        .and_then(|v| serde_json::from_value::<BTreeMap<String, String>>(v).ok())
        .unwrap_or_default();
    if config.keyed_baseline.is_some_and(|p| !p.is_file()) {
        eprintln!(
            "NOTE: no promoted baseline at the configured path — previously_correct_slots_lost = 0"
        );
    }
    let lost = previously_correct_slots_lost(&all_slot_outcomes, &baseline);

    // ---- sensitivity: excludes comparative-period rows (never capturable by
    // a single-period extraction call by construction) ----------------------
    let comparative_slot_ids: BTreeSet<&str> = ground_truth
        .slots
        .iter()
        .filter(|s| {
            manifest
                .events
                .iter()
                .find(|e| e.event_id == s.event_id)
                .is_some_and(|e| e.labeled_period.period_end != s.period_end)
        })
        .map(|s| s.slot_id.as_str())
        .collect();
    let sensitivity_excluded = comparative_slot_ids.len();
    let sensitivity_gt_slots = all_slot_outcomes.len().saturating_sub(sensitivity_excluded);
    let sensitivity_matched = all_slot_outcomes
        .iter()
        .filter(|(id, o)| **o == Outcome::Match && !comparative_slot_ids.contains(id.as_str()))
        .count();

    let gt_slots_total = all_slot_outcomes.len();
    let labeled_total = gt_slots_total; // every entry in gt_key_map.json's panel version 1 is also labeled here.

    let aggregates = Aggregates {
        profile: "esef".to_owned(),
        status: "measured".to_owned(),
        measurement_version: 1,
        gt_version: "1".to_owned(),
        key_map_version: key_map.key_map_version,
        normalization_version: 1,
        registry_hash: manifest_registry_hash(&manifest_raw),
        events: manifest
            .events
            .iter()
            .filter(|e| e.role != "warmup")
            .count(),
        floor_events,
        issuers: manifest.issuers.len(),
        gt_slots: gt_slots_total,
        unverified: unverified_total,
        matched,
        previously_correct_slots_lost: lost.len(),
        false_positives,
        zero_output_events,
        availability_all_periods: AvailabilityPair {
            available: availability_available,
            eligible: availability_eligible,
        },
        layer1_capture: Layer1Capture {
            captured: layer1_captured,
            eligible: availability_eligible,
            value_correct: layer1_value_correct,
        },
        labeled_capability: CountPair {
            matched,
            eligible: labeled_total,
        },
        sensitivity: Sensitivity {
            matched: sensitivity_matched,
            gt_slots: sensitivity_gt_slots,
            excluded: sensitivity_excluded,
        },
        twin_agreement: TwinAgreement {
            agree: twin_agree,
            compared: twin_compared,
        },
        replay,
    };

    print_console_report(&aggregates, &all_slot_outcomes, &lost);

    if let Some(out) = config.metrics_out {
        write_atomic_json(out, &aggregates);
    }
    // Private evidence (scoring report + keyed outcomes) lives in the corpus
    // dir — never outside it (ADR 0091 dec. 4: aggregates only leave the
    // private dir; `metrics_out` above is the one file allowed to).
    write_atomic_json(
        &config.corpus_dir.join("scoring-report-v2.json"),
        &serde_json::json!({
            "slot_outcomes": all_slot_outcomes.iter().map(|(k, v)| (k.clone(), v.as_str())).collect::<BTreeMap<_,_>>(),
            "prediction_outcomes": all_prediction_outcomes.iter().map(|(k, v)| (k.clone(), v.as_str())).collect::<BTreeMap<_,_>>(),
            "derived_vs_labeled_period": derived_by_event,
        }),
    );
    // The run's own nonce (never handed in — `make realdata-esef-check`'s
    // metrics-file nonce is a SEPARATE value the harness never sees): printed
    // below so the owner can copy it straight into
    // `make realdata-esef-promote RUN=<nonce>`.
    let run_nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| std::process::id().to_string());
    let keyed_outcomes_path = config
        .corpus_dir
        .join(format!("keyed-outcomes.{run_nonce}.json"));
    write_atomic_json(
        &keyed_outcomes_path,
        &serde_json::json!({
            "measurement_version": 1,
            "gt_version": "1",
            "key_map_version": key_map.key_map_version,
            "normalization_version": 1,
            "registry_hash": aggregates.registry_hash,
            "outcomes": all_slot_outcomes.iter().map(|(k, v)| (k.clone(), v.as_str())).collect::<BTreeMap<_,_>>(),
            "predictions": all_prediction_outcomes.iter().map(|(k, v)| (k.clone(), v.as_str())).collect::<BTreeMap<_,_>>(),
        }),
    );
    eprintln!(
        "keyed outcomes written to {} — to promote: make realdata-esef-promote RUN={run_nonce}",
        keyed_outcomes_path.display()
    );

    Some(aggregates)
}

/// A stable content hash of the loaded manifest text — the harness's own
/// `registry_hash` echo (equality field, per the shared contract); a real
/// deployment computes it over sorted event-file sha256s (python-side
/// tooling), this harness only needs it to round-trip identically between a
/// run and its promoted baseline.
fn manifest_registry_hash(manifest_raw: &str) -> String {
    content_hash_hex(manifest_raw.as_bytes())
}

fn write_atomic_json<T: Serialize>(path: &Path, value: &T) {
    let json = serde_json::to_string_pretty(value).expect("serialize");
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).unwrap_or_else(|e| panic!("write {}: {e}", tmp.display()));
    std::fs::rename(&tmp, path).unwrap_or_else(|e| panic!("rename into {}: {e}", path.display()));
}

/// Every identity shared by a `twin_diagnostic` event and its paired `floor`
/// event (same issuer, same `labeled_period.period_end`) is scored
/// independently in a throwaway DB pair, then compared.
fn compute_twin_agreement(
    manifest: &Manifest,
    gt_by_event: &BTreeMap<&str, Vec<ResolvedSlot>>,
    corpus_dir: &Path,
    issuer_by_id: &BTreeMap<&str, &Issuer>,
    resolved_map: &ResolvedKeyMap,
) -> (usize, usize) {
    let mut agree = 0usize;
    let mut compared = 0usize;

    for twin in manifest
        .events
        .iter()
        .filter(|e| e.role == "twin_diagnostic")
    {
        let Some(floor) = manifest.events.iter().find(|e| {
            e.role == "floor"
                && e.issuer_id == twin.issuer_id
                && e.labeled_period.period_end == twin.labeled_period.period_end
        }) else {
            continue;
        };

        let empty = Vec::new();
        let twin_gt = gt_by_event.get(twin.event_id.as_str()).unwrap_or(&empty);
        let floor_gt = gt_by_event.get(floor.event_id.as_str()).unwrap_or(&empty);

        let twin_score = score_fresh(twin, twin_gt, corpus_dir, issuer_by_id, resolved_map);
        let floor_score = score_fresh(floor, floor_gt, corpus_dir, issuer_by_id, resolved_map);

        let identity_of = |s: &ResolvedSlot| {
            format!(
                "{}/{}/{}/{}/{}/{}/{}",
                s.concept_local,
                s.attribution,
                s.basis,
                s.window,
                s.variant,
                s.fiscal_year,
                s.period_type
            )
        };
        let mut floor_by_identity: BTreeMap<String, Outcome> = BTreeMap::new();
        for slot in floor_gt {
            if let Some(o) = floor_score.slot_outcomes.get(&slot.slot_id) {
                floor_by_identity.insert(identity_of(slot), *o);
            }
        }
        for slot in twin_gt {
            let identity = identity_of(slot);
            if let Some(floor_outcome) = floor_by_identity.get(&identity) {
                if let Some(twin_outcome) = twin_score.slot_outcomes.get(&slot.slot_id) {
                    compared += 1;
                    if twin_outcome == floor_outcome {
                        agree += 1;
                    }
                }
            }
        }
    }

    (agree, compared)
}

/// Cold-starts and scores exactly one event in its own fresh DB — the
/// building block [`compute_twin_agreement`] needs to score a floor and a
/// twin event independently (never sharing state, unlike replay).
fn score_fresh(
    event: &Event,
    gt_slots: &[ResolvedSlot],
    corpus_dir: &Path,
    issuer_by_id: &BTreeMap<&str, &Issuer>,
    resolved_map: &ResolvedKeyMap,
) -> EventScore {
    let issuer = issuer_by_id[event.issuer_id.as_str()];
    let connection = open_in_memory_database().expect("in-memory db");
    let state = AppState::with_data_dir(connection, corpus_dir.to_path_buf());
    let run = seed_and_run_event(&state, issuer, event, corpus_dir);
    let filtered = prefilter_predictions(run.predictions, resolved_map);
    score_event(gt_slots, &filtered.eligible, &resolved_map.panel)
}

/// Replay (diagnostic): one shared DB per issuer, events in `warmup_order`
/// then `labeled_period.period_end` order, each scored against the DB state
/// immediately after it runs. `floor_events_cold_start_matched` is the cold-
/// start floor-events matched count already computed by the caller, so
/// `delta_matched` never re-derives it differently.
fn run_replay(
    manifest: &Manifest,
    gt_by_event: &BTreeMap<&str, Vec<ResolvedSlot>>,
    corpus_dir: &Path,
    issuer_by_id: &BTreeMap<&str, &Issuer>,
    resolved_map: &ResolvedKeyMap,
    cold_start_slot_outcomes: &BTreeMap<String, Outcome>,
    floor_events: usize,
) -> Replay {
    let cold_start_floor_matched = {
        let _ = floor_events;
        cold_start_slot_outcomes
            .values()
            .filter(|o| **o == Outcome::Match)
            .count()
    };

    let mut by_issuer: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    for event in manifest.events.iter().filter(|e| e.role != "warmup") {
        by_issuer
            .entry(event.issuer_id.as_str())
            .or_default()
            .push(event);
    }

    let mut events_replayed = 0usize;
    let mut exercised_prior_check = 0usize;
    let mut exercised_quarantine = 0usize;
    let mut replay_floor_matched = 0usize;

    for (issuer_id, mut events) in by_issuer {
        events.sort_by(|a, b| {
            a.warmup_order.cmp(&b.warmup_order).then_with(|| {
                a.labeled_period
                    .period_end
                    .cmp(&b.labeled_period.period_end)
            })
        });
        let issuer = issuer_by_id[issuer_id];
        let connection = open_in_memory_database().expect("in-memory db");
        let state = AppState::with_data_dir(connection, corpus_dir.to_path_buf());

        for event in events {
            events_replayed += 1;
            let prior = state
                .financials()
                .stored_fact_set(
                    &event_company_id(&state, issuer),
                    event.labeled_period.fiscal_year - 1,
                    &event.labeled_period.period_type,
                )
                .ok()
                .flatten();
            if prior.is_some() {
                exercised_prior_check += 1;
            }
            let history_periods = state
                .financials()
                .list_financial_periods(ListFinancialPeriodsInput {
                    company_id: event_company_id(&state, issuer),
                    fiscal_year: None,
                })
                .map(|p| p.len())
                .unwrap_or(0);
            if history_periods >= 2 {
                exercised_quarantine += 1;
            }

            let run = seed_and_run_event(&state, issuer, event, corpus_dir);
            let filtered = prefilter_predictions(run.predictions, resolved_map);
            let empty = Vec::new();
            let gt_slots = gt_by_event.get(event.event_id.as_str()).unwrap_or(&empty);
            let score = score_event(gt_slots, &filtered.eligible, &resolved_map.panel);
            if event.role == "floor" {
                replay_floor_matched += score
                    .slot_outcomes
                    .values()
                    .filter(|o| **o == Outcome::Match)
                    .count();
            }
        }
    }

    Replay {
        events: events_replayed,
        exercised_prior_check,
        exercised_quarantine,
        delta_matched: replay_floor_matched as i64 - cold_start_floor_matched as i64,
    }
}

/// The company id for an issuer already seeded into `state` (replay reuses one
/// company per issuer across its events) — looked up by ticker since replay
/// never threads the id explicitly between loop iterations.
fn event_company_id(state: &AppState, issuer: &Issuer) -> String {
    state
        .list_companies()
        .expect("list companies")
        .into_iter()
        .find(|c| c.ticker == issuer.ticker)
        .map(|c| c.id)
        .unwrap_or_default()
}

fn print_console_report(
    aggregates: &Aggregates,
    slot_outcomes: &BTreeMap<String, Outcome>,
    lost: &[String],
) {
    let pct = |n: usize, d: usize| {
        if d == 0 {
            "not measurable".to_owned()
        } else {
            format!("{:.1}%", 100.0 * n as f64 / d as f64)
        }
    };
    eprintln!("== ESEF measurement v2 ==");
    eprintln!(
        "estimand: exact-normalized-decimal match of one semantic (concept, attribution, basis, window, variant, period) slot against the app's stored fact"
    );
    eprintln!(
        "versions: measurement={} gt={} key_map={} normalization={}",
        aggregates.measurement_version,
        aggregates.gt_version,
        aggregates.key_map_version,
        aggregates.normalization_version
    );
    eprintln!(
        "events={} floor_events={} issuers={} gt_slots={} unverified={}",
        aggregates.events,
        aggregates.floor_events,
        aggregates.issuers,
        aggregates.gt_slots,
        aggregates.unverified
    );
    eprintln!(
        "matched={} ({} of gt_slots)  false_positives={}  zero_output_events={}",
        aggregates.matched,
        pct(aggregates.matched, aggregates.gt_slots),
        aggregates.false_positives,
        aggregates.zero_output_events
    );
    eprintln!(
        "labeled_capability: {}/{}  sensitivity: {}/{} (excluded {})  twin_agreement: {}/{}",
        aggregates.labeled_capability.matched,
        aggregates.labeled_capability.eligible,
        aggregates.sensitivity.matched,
        aggregates.sensitivity.gt_slots,
        aggregates.sensitivity.excluded,
        aggregates.twin_agreement.agree,
        aggregates.twin_agreement.compared
    );
    eprintln!(
        "availability_all_periods: {}/{}  layer1_capture: {}/{} (value_correct {})",
        aggregates.availability_all_periods.available,
        aggregates.availability_all_periods.eligible,
        aggregates.layer1_capture.captured,
        aggregates.layer1_capture.eligible,
        aggregates.layer1_capture.value_correct
    );
    eprintln!(
        "replay: events={} prior_check_exercised={} quarantine_exercised={} delta_matched={}",
        aggregates.replay.events,
        aggregates.replay.exercised_prior_check,
        aggregates.replay.exercised_quarantine,
        aggregates.replay.delta_matched
    );
    eprintln!(
        "previously_correct_slots_lost={} {:?}",
        aggregates.previously_correct_slots_lost, lost
    );
    let _ = slot_outcomes;
}

// ===========================================================================
// The real-data #[ignore] test — env-driven, inert in CI
// ===========================================================================

/// Real ESEF measurement v2 (#331 PR-A, ADR 0112). **Inert in CI** — skips
/// unless `BRAWLER_ESEF_V2_DIR` names a readable corpus;
/// `BRAWLER_ESEF_REQUIRED=1` turns the skip into a panic (the `make
/// realdata-esef-check` closure gate).
#[test]
#[ignore = "real-data validation; needs BRAWLER_ESEF_V2_DIR (a private labeled corpus)"]
fn esef_measurement_v2() {
    let required = std::env::var("BRAWLER_ESEF_REQUIRED").as_deref() == Ok("1");
    let dir = std::env::var("BRAWLER_ESEF_V2_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../private/realdata/spikes/esef-v2")
        });
    let metrics_out = std::env::var("BRAWLER_ESEF_METRICS_OUT")
        .map(PathBuf::from)
        .ok();
    if required {
        assert!(
            metrics_out.is_some(),
            "BRAWLER_ESEF_REQUIRED=1 requires BRAWLER_ESEF_METRICS_OUT"
        );
    }
    let keyed_baseline = std::env::var("BRAWLER_ESEF_KEYED_BASELINE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dir.join("baseline/keyed-baseline.json"));

    let result = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: metrics_out.as_deref(),
        keyed_baseline: Some(&keyed_baseline),
        required,
    });
    if required {
        assert!(result.is_some(), "required mode must produce aggregates");
    }
}

// ===========================================================================
// Hermetic tests (no env; run in CI)
// ===========================================================================

/// Copies the committed synthetic sample corpus into a fresh temp dir —
/// hermetic tests never mutate `testdata/` in place.
fn materialize_sample_corpus() -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/esef-v2-sample");
    let dst = unique_temp_dir("esef-v2-sample");
    copy_dir(&src, &dst);
    dst
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create dir");
    for entry in std::fs::read_dir(src).expect("read dir") {
        let entry = entry.expect("dir entry");
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &dst_path);
        } else {
            std::fs::copy(entry.path(), &dst_path).expect("copy file");
        }
    }
}

fn sample_key_map() -> ResolvedKeyMap {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let map = load_key_map(&repo_root);
    resolve_key_map(&map, 1)
}

#[allow(clippy::too_many_arguments)]
fn slot(
    slot_id: &str,
    metric_key: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    basis: &str,
    attribution: &str,
    variant: &str,
    window: &str,
    currency: &str,
    value: &str,
    verification: &str,
) -> ResolvedSlot {
    ResolvedSlot {
        slot_id: slot_id.to_owned(),
        // The pure-matcher tests below never assert on `concept_local` (only
        // `metric_key` drives matching) — reusing it here avoids an unused
        // parameter without widening every call site.
        concept_local: metric_key.to_owned(),
        metric_key: metric_key.to_owned(),
        fiscal_year,
        period_type: period_type.to_owned(),
        period_end: period_end.to_owned(),
        basis: basis.to_owned(),
        attribution: attribution.to_owned(),
        variant: variant.to_owned(),
        window: window.to_owned(),
        currency: currency.to_owned(),
        value: value.parse().unwrap(),
        verification: verification.to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
fn prediction(
    id: &str,
    metric_key: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    basis: &str,
    attribution: &str,
    variant: &str,
    window: &str,
    currency: Option<&str>,
    value: &str,
) -> Prediction {
    Prediction {
        id: id.to_owned(),
        metric_key: metric_key.to_owned(),
        fiscal_year,
        period_type: period_type.to_owned(),
        period_end: period_end.to_owned(),
        basis: basis.to_owned(),
        attribution: attribution.to_owned(),
        variant: variant.to_owned(),
        window: window.to_owned(),
        currency: currency.map(str::to_owned),
        value: value.parse().unwrap(),
        data_quality: "final".to_owned(),
    }
}

/// Test 1 — the full harness path on the committed synthetic sample: every
/// panel concept the consolidated instances tag MATCHes, the standalone-only
/// concept is scored, comparative FY2024 feeds availability/Layer 1 capture,
/// and the metrics JSON carries no ticker/title/value/filename text.
#[test]
fn sample_corpus_scores_and_metrics_carry_no_content() {
    let dir = materialize_sample_corpus();
    let out = dir.join("metrics.json");
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: Some(&out),
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    // 16 panel concepts x 3 floor events (floor_pl, floor_package) + the 3
    // FY2024 comparative rows on floor_pl + the 1 standalone-only row on
    // floor_package. floor events only (floor_pl, floor_package) — twin_en is
    // twin_diagnostic and excluded from the floor numbers.
    assert_eq!(aggregates.floor_events, 2);
    assert_eq!(aggregates.gt_slots, 16 + 3 + 16 + 1);
    // Every consolidated panel concept MATCHes in both floor events.
    assert_eq!(aggregates.matched, 16 + 16);
    assert_eq!(aggregates.zero_output_events, 0);
    // Comparative FY2024 numbers are available in Layer 1 (raw capture) even
    // though production never writes a comparative-period fact.
    assert!(aggregates.availability_all_periods.available >= 3);
    assert!(aggregates.layer1_capture.captured >= 3);

    let metrics_json = std::fs::read_to_string(&out).expect("metrics file");
    for needle in [
        "SMPL",
        "Sample S.A.",
        "500000000",
        "900 000",
        "sample_pl.xhtml",
    ] {
        assert!(
            !metrics_json.contains(needle),
            "metrics JSON must carry no ticker/title/value/filename, found {needle:?}"
        );
    }
}

/// Test 2 — the language twins are two independent events whose per-slot
/// outcomes agree wherever they share a semantic identity.
#[test]
fn language_twins_are_two_events_with_agreement() {
    let dir = materialize_sample_corpus();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    assert_eq!(aggregates.twin_agreement.compared, 16);
    assert_eq!(
        aggregates.twin_agreement.agree,
        aggregates.twin_agreement.compared
    );
}

/// Test 3 — `owners_of_parent` GT concepts map to their dedicated `wdf_*`
/// metric keys, never to the plain total key (pure matcher, no DB).
#[test]
fn owners_of_parent_maps_to_wdf_parent_keys() {
    let panel: BTreeSet<String> = sample_key_map().panel;

    let gt = vec![
        slot(
            "s1",
            "wdf_equity_parent",
            2025,
            "FY",
            "2025-12-31",
            "consolidated",
            "owners_of_parent",
            "reported",
            "point_in_time",
            "PLN",
            "380000000",
            "machine",
        ),
        slot(
            "s2",
            "total_equity",
            2025,
            "FY",
            "2025-12-31",
            "consolidated",
            "total",
            "reported",
            "point_in_time",
            "PLN",
            "400000000",
            "machine",
        ),
    ];
    let predictions = vec![
        prediction(
            "p_parent",
            "wdf_equity_parent",
            2025,
            "FY",
            "2025-12-31",
            "consolidated",
            "total",
            "reported",
            "point_in_time",
            Some("PLN"),
            "380000000",
        ),
        prediction(
            "p_total",
            "total_equity",
            2025,
            "FY",
            "2025-12-31",
            "consolidated",
            "total",
            "reported",
            "point_in_time",
            Some("PLN"),
            "400000000",
        ),
    ];

    let score = score_event(&gt, &predictions, &panel);
    assert_eq!(score.slot_outcomes["s1"], Outcome::Match);
    assert_eq!(score.slot_outcomes["s2"], Outcome::Match);
    // A GT `Equity` (total_equity) row never matches the wdf_equity_parent
    // prediction — proven by both resolving to MATCH against their OWN
    // metric-key-correct counterpart, never cross-wired.
    assert_eq!(score.prediction_outcomes["p_parent"], Outcome::Match);
    assert_eq!(score.prediction_outcomes["p_total"], Outcome::Match);
}

/// Test 4 — the standalone package member's exclusive concept
/// (`CurrentLiabilities`) is written with `statement_basis='consolidated'`
/// (ADR 0095 default, every structured write) while GT expects `standalone`:
/// a real, honest `BASIS_MISMATCH`, not a silently-dropped duplicate.
#[test]
fn basis_mismatch_counts_against_precision() {
    let dir = materialize_sample_corpus();
    let out = dir.join("metrics.json");
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: Some(&out),
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");
    let _ = aggregates;

    let report_raw =
        std::fs::read_to_string(dir.join("scoring-report-v2.json")).expect("scoring report");
    let report: serde_json::Value = serde_json::from_str(&report_raw).expect("scoring report json");
    let standalone_slot_id = "iss_01/FY2025/pl/consolidated_and_standalone/v1/CurrentLiabilities/total/standalone/point_in_time/reported/2025/FY";
    assert_eq!(
        report["slot_outcomes"][standalone_slot_id],
        serde_json::Value::String("MISSING".to_owned()),
        "the GT slot itself never resolves to a mismatch label — only MATCH/MISSING/UNVERIFIED"
    );
    let predictions = report["prediction_outcomes"]
        .as_object()
        .expect("predictions object");
    assert!(
        predictions.values().any(|v| v == "BASIS_MISMATCH"),
        "the standalone-only prediction (stamped consolidated by production) must be paired as BASIS_MISMATCH: {predictions:?}"
    );
}

/// Test 5 — window/variant/period-date mismatches are scored as their own
/// distinct classes, never silently normalized away (pure matcher).
#[test]
fn window_and_variant_and_period_date_mismatches_are_scored() {
    let panel: BTreeSet<String> = sample_key_map().panel;

    let gt_window = slot(
        "w",
        "revenue",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "500000000",
        "machine",
    );
    let pred_window = prediction(
        "pw",
        "revenue",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "point_in_time",
        Some("PLN"),
        "500000000",
    );
    let score = score_event(&[gt_window], &[pred_window], &panel);
    assert_eq!(score.prediction_outcomes["pw"], Outcome::WindowMismatch);

    let gt_variant = slot(
        "v",
        "revenue",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "500000000",
        "machine",
    );
    let pred_variant = prediction(
        "pv",
        "revenue",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "restated",
        "flow",
        Some("PLN"),
        "500000000",
    );
    let score = score_event(&[gt_variant], &[pred_variant], &panel);
    assert_eq!(score.prediction_outcomes["pv"], Outcome::VariantMismatch);

    let gt_period = slot(
        "d",
        "revenue",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "500000000",
        "machine",
    );
    let pred_period = prediction(
        "pd",
        "revenue",
        2025,
        "FY",
        "2025-06-30",
        "consolidated",
        "total",
        "reported",
        "flow",
        Some("PLN"),
        "500000000",
    );
    let score = score_event(&[gt_period], &[pred_period], &panel);
    assert_eq!(score.prediction_outcomes["pd"], Outcome::PeriodDateMismatch);
}

/// Test 6 — value equality is the exact normalized decimal: `1.0` and `1.00`
/// agree, `1.00` and `0.10` do not (never the production `Tolerance`).
#[test]
fn eps_equality_is_exact() {
    let panel: BTreeSet<String> = sample_key_map().panel;

    let gt = slot(
        "e1",
        "eps_basic",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "1.00",
        "machine",
    );
    let wrong = prediction(
        "pe1",
        "eps_basic",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        Some("PLN"),
        "0.10",
    );
    let score = score_event(&[gt], &[wrong], &panel);
    assert_eq!(score.prediction_outcomes["pe1"], Outcome::WrongValue);

    let gt2 = slot(
        "e2",
        "eps_basic",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "1.0",
        "machine",
    );
    let right = prediction(
        "pe2",
        "eps_basic",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        Some("PLN"),
        "1.00",
    );
    let score = score_event(&[gt2], &[right], &panel);
    assert_eq!(score.prediction_outcomes["pe2"], Outcome::Match);
}

/// Test 7 — a GT slot marked `unverified` (a labeling-time conflict) leaves
/// both denominators: it never counts as `MISSING`, and any prediction it
/// might otherwise have paired with is scored as unpaired instead.
#[test]
fn duplicate_conflict_is_unverified_and_pinned() {
    let panel: BTreeSet<String> = sample_key_map().panel;

    let gt = slot(
        "u1",
        "revenue",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "500000000",
        "unverified",
    );
    let pred = prediction(
        "pu1",
        "revenue",
        2025,
        "FY",
        "2025-12-31",
        "consolidated",
        "total",
        "reported",
        "flow",
        Some("PLN"),
        "500000000",
    );
    let score = score_event(&[gt], &[pred], &panel);

    assert_eq!(score.slot_outcomes["u1"], Outcome::Unverified);
    // The prediction was never claimed by the unverified slot — it is scored
    // as an ordinary unpaired prediction (in-panel -> FALSE_POSITIVE).
    assert_eq!(score.prediction_outcomes["pu1"], Outcome::FalsePositive);
}

/// Test 8 — an event whose stored file is not iXBRL at all yields zero
/// predictions: every GT slot for it is MISSING and it counts once toward
/// `zero_output_events`.
#[test]
fn zero_output_event_counts_in_recall() {
    let dir = unique_temp_dir("esef-v2-zero-output");
    std::fs::create_dir_all(dir.join("corpus")).expect("dir");
    std::fs::write(
        dir.join("corpus/not_ixbrl.txt"),
        b"just some plain text, no XBRL here at all",
    )
    .expect("write");
    write_manifest_and_gt(
        &dir,
        "not_ixbrl.txt",
        "application/octet-stream",
        2025,
        "FY",
        "2025-12-31",
        &[("revenue", "Revenue", "500000000")],
    );

    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("tiny corpus must measure");

    assert_eq!(aggregates.zero_output_events, 1);
    assert_eq!(aggregates.matched, 0);
    assert_eq!(aggregates.gt_slots, 1);
}

/// Test 9 — the production-derived period is recorded even when it disagrees
/// with the labeled period: the event's slots go MISSING (nothing was
/// extracted for the LABELED period) and the derivation is visibly `Some`,
/// not silently coerced to match the label.
#[test]
fn derivation_result_is_passed_through_and_recorded() {
    let dir = unique_temp_dir("esef-v2-derivation");
    std::fs::create_dir_all(dir.join("corpus")).expect("dir");
    // A genuinely different period than the manifest claims: the labeled
    // period below says 2025-12-31, but this instance's own iXBRL context
    // says 2030-12-31 — derive_report_period reads the CONTEXT, not the label.
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="i"><xbrli:period><xbrli:instant>2030-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="i" unitRef="pln" scale="3">100 000</ix:nonFraction>
    </html>"#;
    std::fs::write(dir.join("corpus/drift.xhtml"), instance).expect("write");
    write_manifest_and_gt(
        &dir,
        "drift.xhtml",
        "application/xhtml+xml",
        2025,
        "FY",
        "2025-12-31",
        &[("total_assets", "Assets", "100000000")],
    );

    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("tiny corpus must measure");

    // The GT slot is labeled 2025-12-31; production derives (and writes under)
    // 2030-12-31 — a genuine period mismatch, not a false MATCH.
    assert_eq!(aggregates.matched, 0);
    assert_eq!(aggregates.gt_slots, 1);
}

/// Test 10 — altering one value on a COPY of the sample corpus (never the
/// committed testdata) through the real job changes the outcome: matched
/// decreases and the altered slot becomes WRONG_VALUE.
#[test]
fn freshness_altered_bytes_change_the_outcome() {
    let dir = materialize_sample_corpus();
    let baseline = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    let path = dir.join("corpus/sample_pl.xhtml");
    let original = std::fs::read_to_string(&path).expect("read fixture");
    let altered = original.replacen(
        r#"<ix:nonFraction name="ifrs-full:Revenue" contextRef="d2025" unitRef="pln" scale="3">500 000</ix:nonFraction>"#,
        r#"<ix:nonFraction name="ifrs-full:Revenue" contextRef="d2025" unitRef="pln" scale="3">1 000</ix:nonFraction>"#,
        1,
    );
    assert_ne!(
        altered, original,
        "the replacement must actually hit the fixture text"
    );
    std::fs::write(&path, altered).expect("write altered fixture");

    let after = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("altered corpus must still measure");

    assert!(
        after.matched < baseline.matched,
        "altering a value through the real job must lower matched: before={} after={}",
        baseline.matched,
        after.matched
    );
}

/// Test 11 — `previously_correct_slots_lost` compares against the PROMOTED
/// keyed baseline (pure, no DB): a slot the baseline recorded MATCH for that
/// the current run no longer matches is lost; a within-issuer offset (one
/// lost, one newly gained) still reports exactly the lost one.
#[test]
fn previously_correct_slots_lost_uses_the_promoted_keyed_baseline() {
    let mut current = BTreeMap::new();
    current.insert("s1".to_owned(), Outcome::Match);
    current.insert("s2".to_owned(), Outcome::Missing);
    current.insert("s3".to_owned(), Outcome::Match);

    let mut baseline = BTreeMap::new();
    baseline.insert("s1".to_owned(), "MATCH".to_owned());
    baseline.insert("s2".to_owned(), "MATCH".to_owned()); // lost
    baseline.insert("s3".to_owned(), "MISSING".to_owned()); // newly gained — not a loss

    let lost = previously_correct_slots_lost(&current, &baseline);
    assert_eq!(lost, vec!["s2".to_owned()]);
}

/// Test 12a — a `key_map_version` mismatch between the GT file and the public
/// key map is a typed panic naming "incomparable", never a silent skip or a
/// best-effort guess. `#[should_panic(expected = ..)]` is the idiomatic way
/// to pin a panic MESSAGE in Rust — more robust than `catch_unwind` +
/// downcasting the payload, which this toolchain does not box as a plain
/// `String`/`&str` (verified empirically while writing this test).
#[test]
#[should_panic(expected = "incomparable")]
fn unmeasured_or_mismatched_versions_are_typed_failures() {
    // cross-tree-read-ok: reads the small, deterministic, committed public key map (same as run_measurement's own read; must run in default CI, so not #[ignore]-able).
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let map = load_key_map(&repo_root);
    let _ = resolve_key_map(&map, map.key_map_version + 1);
}

/// Test 12b — a ground-truth slot missing a required field is a typed panic
/// naming the field (serde's own message), never a silent skip.
#[test]
#[should_panic(expected = "missing field")]
fn malformed_ground_truth_field_is_a_typed_panic() {
    serde_json::from_str::<GtSlot>(r#"{"slot_id":"s1"}"#).expect("must fail: missing fields");
}

/// Test 13 — required mode panics without a corpus (through
/// [`MeasurementConfig`], never process env — this test stays env-free).
#[test]
fn required_mode_panics_without_corpus() {
    let missing = unique_temp_dir("esef-v2-missing").join("does-not-exist");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_measurement(MeasurementConfig {
            corpus_dir: &missing,
            metrics_out: None,
            keyed_baseline: None,
            required: true,
        })
    }));
    assert!(
        result.is_err(),
        "required mode must panic when the corpus dir is absent"
    );
}

/// Builds a minimal one-event manifest + one-slot ground truth in `dir`
/// (tests 8/9) — the small, single-purpose sibling of
/// [`materialize_sample_corpus`] for scenarios that need a corpus shape the
/// committed sample does not carry.
fn write_manifest_and_gt(
    dir: &Path,
    file_name: &str,
    content_type: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    slots: &[(&str, &str, &str)],
) {
    let manifest = serde_json::json!({
        "manifest_version": 1,
        "snapshot": { "source": "test", "taken_at": "2026-09-12" },
        "registry_hash": "test",
        "issuers": [
            { "issuer_id": "iss_t", "ticker": "TST", "exchange": "GPW", "display_name": "Test S.A." }
        ],
        "events": [
            {
                "event_id": "iss_t/FY/ev/v1",
                "issuer_id": "iss_t",
                "role": "floor",
                "language": "pl",
                "basis_scope": "consolidated",
                "vintage": 1,
                "labeled_period": {
                    "fiscal_year": fiscal_year,
                    "period_type": period_type,
                    "period_end": period_end,
                    "period_start": null
                },
                "file": { "name": file_name, "sha256": "test", "bytes": 0, "package_member": null },
                "document": {
                    "id": "doc_t",
                    "title": "Test document",
                    "url": "https://example.com/test",
                    "content_type": content_type,
                    "content_hash": null
                },
                "warmup_order": 0
            }
        ],
        "honest_limitations": []
    });
    std::fs::write(
        dir.join("MANIFEST_v2.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .expect("write manifest");

    let gt_slots: Vec<serde_json::Value> = slots
        .iter()
        .map(|(metric_key, concept, value)| {
            let _ = metric_key;
            serde_json::json!({
                "slot_id": format!("iss_t/FY/ev/v1/{concept}/total/consolidated/point_in_time/reported/{fiscal_year}/{period_type}"),
                "event_id": "iss_t/FY/ev/v1",
                "concept_local": concept,
                "attribution": "total",
                "basis": "consolidated",
                "window": "point_in_time",
                "variant": "reported",
                "fiscal_year": fiscal_year,
                "period_type": period_type,
                "period_end": period_end,
                "period_start": null,
                "currency": "PLN",
                "value": value,
                "duration_months": null,
                "verification": "machine",
                "contributing_occurrence_ids": [],
                "resolution_ref": null
            })
        })
        .collect();
    let gt = serde_json::json!({
        "gt_version": "1",
        "normalization_version": 1,
        "key_map_version": 1,
        "slots": gt_slots
    });
    std::fs::write(
        dir.join("ground_truth_v2.json"),
        serde_json::to_string_pretty(&gt).unwrap(),
    )
    .expect("write gt");
}
