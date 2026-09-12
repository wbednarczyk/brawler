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
//! (`contract-331-shared.md`, ADR 0112) plus amendment 1
//! (`contract-331-amendment-1.md`, after the astra diff review r1) for every
//! JSON shape and the matching rules this file implements to.
//!
//! Two outcome maps, not one (the shared contract's `keyed-outcomes.json`):
//! a ground-truth **slot** only ever resolves to `MATCH` / `MISSING` (the
//! recall side — `unverified`/`machine_v1` slots never even enter this map,
//! amendment C); a **prediction** (a stored fact) resolves to `MATCH` or one
//! of the mismatch classes / `FALSE_POSITIVE` / `OUT_OF_SCOPE` (the precision
//! side). A "paired but wrong" prediction therefore counts against precision
//! while leaving its GT slot `MISSING` — the two maps are scored
//! independently once pairing has run.
//!
//! **Inert in CI** — [`esef_measurement_v2`] skips loudly unless
//! `BRAWLER_ESEF_V2_DIR` names a readable corpus (`BRAWLER_ESEF_REQUIRED=1`
//! turns that into a panic). The hermetic `#[test]`s below run in CI: most
//! call [`run_measurement`] against the committed synthetic sample corpus
//! (`testdata/esef-v2-sample/`, copied to a temp dir first — never mutated in
//! place), a few call the pure matcher ([`score_event`]) directly with
//! hand-built slots/predictions. Every hermetic test resolves the key map from
//! the crate-local copy (`testdata/esef-v2-sample/gt_key_map.json`) — never
//! `scripts/realdata/esef-v2/gt_key_map.json`, which only the `#[ignore]`d
//! real-data test reads (amendment 18).

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
    /// sha256 over the sorted per-file sha256s of every event's corpus file
    /// (amendment G) — the builder computes it, this harness recomputes it
    /// independently from the actual files and compares (never trusts the
    /// declared value blindly).
    registry_hash: String,
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
    /// The event's PINNED language (`pl` | `en` | `unknown`, amendment S):
    /// within a package event, a GT slot whose OWN `language` disagrees is
    /// the internal twin-diagnostic population, never the floor one.
    language: String,
    /// Filing vintage (a correction/restatement counter) — replay's
    /// chronological tie-break after `labeled_period.period_end` (amendment I).
    vintage: i64,
    labeled_period: LabeledPeriod,
    file: EventFile,
    document: EventDocument,
    /// Present on EVERY event (amendment B): `0` for a non-warmup event,
    /// ascending among warmups. Deserialized and REQUIRED (a missing field is
    /// a typed parse failure) so a generated manifest that omits it on a
    /// non-warmup event is caught immediately (finding 6) — no longer the
    /// sort key itself (replay now orders by `labeled_period.period_end` then
    /// `vintage`, amendment I), so this field is otherwise unread.
    #[allow(dead_code)]
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
    /// The package member this event's OWN labeled content lives at, when
    /// `name` is a package (amendment A/F) — deserialized so a future reader
    /// can trace event->member without re-deriving it; scoring itself keys
    /// off each GT slot's own `package_member` (member evidence can differ
    /// per occurrence within one package).
    #[serde(default)]
    #[allow(dead_code)]
    package_member: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventDocument {
    title: String,
    url: String,
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct GroundTruthV2 {
    gt_version: String,
    normalization_version: i64,
    key_map_version: i64,
    slots: Vec<GtSlot>,
}

#[derive(Debug, Clone, Deserialize)]
struct GtSlot {
    slot_id: String,
    event_id: String,
    /// The package member this occurrence's evidence came from, `None` for a
    /// loose (non-package) instance (amendment A/F) — part of the slot's own
    /// identity (also folded into `slot_id`'s own shape by the labeler), since
    /// one package event can carry conflicting-basis members. Carried through
    /// into [`ResolvedSlot`] as a structured field so the report/console never
    /// need to parse `slot_id` to recover it.
    #[serde(default)]
    package_member: Option<String>,
    concept_local: String,
    /// `total` | `owners_of_parent` | `nci`.
    attribution: String,
    /// `consolidated` | `standalone` | `unknown` — `unknown` is scored as
    /// `UNVERIFIED` regardless of the slot's own `verification` (amendment T).
    basis: String,
    /// `flow` | `point_in_time`.
    window: String,
    variant: String,
    fiscal_year: i64,
    period_type: String,
    period_end: String,
    /// `None` for a pure-number concept (share counts) — such a slot is
    /// labeled-capability only: the app never stores a currency-less fact
    /// on the panel, so it neither matches nor counts as a miss.
    currency: Option<String>,
    value: String,
    /// `machine` | `second_read` | `adjudicated` | `unverified` | `machine_v1`.
    verification: String,
    /// The MEMBER's own language (`pl` | `en` | `unknown`, amendment S) — a
    /// slot whose language disagrees with its event's pinned `language` is
    /// the internal twin-diagnostic population, never the floor one.
    #[serde(default = "unknown_language")]
    language: String,
    /// Whether the labeler's key map found a `(concept, attribution)` entry
    /// for this row (amendment Q) — the harness resolves this INDEPENDENTLY
    /// from its own key map copy (never trusts this flag for scoring), but a
    /// generated corpus that omits it is still a typed parse failure so a
    /// producer/consumer drift is caught immediately.
    #[allow(dead_code)]
    mapped: bool,
    /// `contract_normalized` rule ids (from `gt_key_map.json`) whose
    /// normalization this slot's value/window went through (amendment Q) —
    /// removed from BOTH sides of `sensitivity`. Empty for an
    /// unnormalized slot.
    #[serde(default)]
    normalized_by: Vec<String>,
}

fn unknown_language() -> String {
    "unknown".to_owned()
}

/// One raw occurrence from `occurrences_v2.json` (a bare JSON array —
/// immutable evidence, never a slot). Amendment Q's Layer 1 comparative
/// capture reads this file directly rather than re-deriving occurrence-level
/// evidence from the already-collapsed GT slots.
#[derive(Debug, Deserialize)]
struct Occurrence {
    event_id: String,
    /// The EXPANDED concept identity, `{namespace-uri}LocalName` (amendment
    /// M) — matched against `report_tagged_facts`' own
    /// `concept_namespace_uri`/`concept_local_name` columns, never a bare
    /// local-name suffix.
    concept_qname: String,
    period: OccurrencePeriod,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OccurrencePeriod {
    Instant { instant: String },
    Duration { start: String, end: String },
}

impl OccurrencePeriod {
    fn end(&self) -> &str {
        match self {
            OccurrencePeriod::Instant { instant } => instant,
            OccurrencePeriod::Duration { end, .. } => end,
        }
    }

    /// Whether a raw Layer 1 fact's own period is EXACTLY this occurrence's
    /// full context period — instant-to-instant or start+end-to-start+end,
    /// never a bare end-date/local-name-suffix comparison (amendment Q).
    fn matches_layer1(&self, fact: &Layer1Fact) -> bool {
        match self {
            OccurrencePeriod::Instant { instant } => {
                fact.period_type == "instant" && fact.period_end == *instant
            }
            OccurrencePeriod::Duration { start, end } => {
                fact.period_type != "instant"
                    && fact.period_start.as_deref() == Some(start.as_str())
                    && fact.period_end == *end
            }
        }
    }
}

/// `occurrences_v2.json` is immutable raw evidence (never required for
/// scoring itself, only for amendment Q's comparative Layer 1 capture) — a
/// corpus that omits it entirely (a tiny hand-built test fixture with no
/// comparative-period evidence to carry) measures fine with zero occurrences;
/// a PRESENT-but-malformed file is still a typed failure, never silently
/// swallowed.
fn load_occurrences(corpus_dir: &Path) -> Vec<Occurrence> {
    let path = corpus_dir.join("occurrences_v2.json");
    if !path.is_file() {
        return Vec::new();
    }
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("occurrences_v2.json unreadable at {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("occurrences_v2.json malformed field: {e}"))
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
    /// The SAME attribution the concept's occurrences always carry per this
    /// key-map version — part of the entry's own identity (amendment E): a
    /// GT `(concept, attribution)` pair that doesn't match a listed entry's
    /// exact pair (e.g. `ProfitLoss`/`nci`, which the map lists only under
    /// `total`) resolves to NOTHING, never falls back to the concept's usual
    /// metric_key.
    attribution: String,
}

/// The public key map, resolved to the lookups scoring needs: the semantic
/// `(concept, attribution)` PAIR -> `metric_key` (amendment E — never concept
/// alone, so an unsupported attribution never silently maps to the total's
/// key), the panel set (precision/current-recall scope), and the one
/// attribution value every structured write stamps (ADR 0095) — a prediction
/// whose stored attribution differs from this is `ATTRIBUTION_MISMATCH`
/// BEFORE matching ever runs, independently of the semantic pair lookup.
struct ResolvedKeyMap {
    pair_to_metric_key: BTreeMap<(String, String), String>,
    panel: BTreeSet<String>,
    stored_attribution_on_structured_writes: String,
}

fn load_key_map(path: &Path) -> KeyMap {
    let raw = std::fs::read_to_string(path)
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
        pair_to_metric_key: map
            .entries
            .iter()
            .map(|e| {
                (
                    (e.concept.clone(), e.attribution.clone()),
                    e.metric_key.clone(),
                )
            })
            .collect(),
        panel: map.panel.iter().cloned().collect(),
        stored_attribution_on_structured_writes: map
            .stored_attribution_on_structured_writes
            .clone(),
    }
}

/// The three verification states scoring actually acts on (amendment C).
/// Anything outside this vocabulary — including a currently-unknown string —
/// is a typed failure, never a silent guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerificationClass {
    /// `machine` | `second_read` | `adjudicated` — scorable.
    Eligible,
    /// A labeling-time conflict pinned open — leaves every denominator.
    Unverified,
    /// A v1-imported row awaiting v2 re-verification — leaves every
    /// denominator, counted and printed separately from `unverified`.
    MachineV1,
}

fn verification_class(verification: &str) -> VerificationClass {
    match verification {
        "machine" | "second_read" | "adjudicated" => VerificationClass::Eligible,
        "unverified" => VerificationClass::Unverified,
        "machine_v1" => VerificationClass::MachineV1,
        other => panic!("incomparable: unknown ground-truth verification state {other:?}"),
    }
}

// ===========================================================================
// Outcome vocabulary (shared data contract + amendment D's new class)
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
    /// New class (amendment D): the semantic slot, basis, window, variant,
    /// currency and period END all agree but `period_type` does not — the
    /// specific derivation defect (an H1 filing stored as FY, or vice versa,
    /// under a coincidentally-matching date) that a plain date compare hides.
    PeriodTypeMismatch,
    PeriodDateMismatch,
    AttributionMismatch,
    CurrencyMissing,
    CurrencyMismatch,
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
            Outcome::PeriodTypeMismatch => "PERIOD_TYPE_MISMATCH",
            Outcome::PeriodDateMismatch => "PERIOD_DATE_MISMATCH",
            Outcome::AttributionMismatch => "ATTRIBUTION_MISMATCH",
            Outcome::CurrencyMissing => "CURRENCY_MISSING",
            Outcome::CurrencyMismatch => "CURRENCY_MISMATCH",
            Outcome::OutOfScope => "OUT_OF_SCOPE",
        }
    }
}

// ===========================================================================
// The pure matcher (score_event) — no DB, fully unit-testable
// ===========================================================================

/// A ground-truth slot, resolved to the internal shape the matcher compares —
/// [`GtSlot`] plus its `(concept, attribution)` pair translated to
/// `metric_key` and its value parsed. Only ELIGIBLE-verification, MAPPED
/// slots ever become one of these — see [`ResolvedEvent`].
#[derive(Debug, Clone)]
pub(crate) struct ResolvedSlot {
    pub slot_id: String,
    /// The package member this occurrence's evidence came from, `None` for a
    /// loose (non-package) instance — carried as a structured field (not
    /// re-derived by splitting `slot_id`) so the report/console never parse
    /// the id.
    pub package_member: Option<String>,
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
    /// `contract_normalized` rule ids this slot's value/window went through
    /// (amendment Q) — a non-empty list excludes it from `sensitivity`.
    pub normalized_by: Vec<String>,
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
/// ever `Match`/`Missing`, eligible-mapped slots only) and the prediction
/// outcome map (precision side — `Match`/a mismatch class/`FalsePositive`/
/// `OutOfScope`).
#[derive(Debug, Default)]
pub(crate) struct EventScore {
    pub slot_outcomes: BTreeMap<String, Outcome>,
    pub prediction_outcomes: BTreeMap<String, Outcome>,
}

/// How many of the seven comparable dimensions differ between a GT slot and a
/// candidate prediction (used only to rank mismatch-pairing candidates —
/// "fewest differing dimensions" per the shared contract). Attribution is
/// deliberately NOT one of them: it is a pre-filter against the global
/// `stored_attribution_on_structured_writes` constant (see
/// [`prefilter_predictions`]), never a per-pair comparison — the semantic
/// attribution distinction already lives in `metric_key` via the key map's
/// `(concept, attribution)` pair resolution (amendment E).
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
    if p.period_type != gt.period_type {
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
/// match — `None` if the pair is actually exact (every dimension agrees,
/// including the full fiscal key). Priority order when several dimensions
/// differ at once (a design choice the shared contract leaves open,
/// documented in the PR-A report): currency > basis > window > variant >
/// period type > period date > value. `period_type` is checked BEFORE
/// `period_end` so the amendment-D defect (an H1 filing stored as FY under
/// the SAME date) reports as `PERIOD_TYPE_MISMATCH`, never a plain date
/// mismatch that would hide it.
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
    if p.period_type != gt.period_type {
        return Some(Outcome::PeriodTypeMismatch);
    }
    if p.period_end != gt.period_end {
        return Some(Outcome::PeriodDateMismatch);
    }
    if p.value != gt.value {
        return Some(Outcome::WrongValue);
    }
    None
}

/// Scores one event's ELIGIBLE, MAPPED GT slots against its predictions
/// (shared contract + amendment D matching rules): exact matches first —
/// requiring the FULL fiscal key (`metric_key`, `fiscal_year`, `period_type`)
/// to agree, not just the metric key, so a same-date H1-vs-FY derivation
/// defect can never hide behind a coincidental date match — then
/// deterministic mismatch pairing (same `metric_key` + same fiscal YEAR only,
/// fewest differing dimensions, stable id order breaking ties), one-to-one
/// throughout. `panel` decides whether an unpaired prediction is
/// `FALSE_POSITIVE` (inside) or `OUT_OF_SCOPE` (outside). Predictions whose
/// `data_quality != "final"` or stored attribution disagrees with the global
/// constant must be filtered to `OUT_OF_SCOPE`/`ATTRIBUTION_MISMATCH` by the
/// CALLER before this function ever sees them.
///
/// `gt_slots` here is ALREADY the eligible+mapped subset — ineligible
/// (`unverified`/`machine_v1`) and unmapped slots are handled by the caller
/// ([`resolve_event_slots`] + [`attribute_ineligible_predictions`]) so this
/// function stays a pure, easily-tested matcher.
pub(crate) fn score_event(
    gt_slots: &[ResolvedSlot],
    predictions: &[Prediction],
    panel: &BTreeSet<String>,
) -> EventScore {
    let mut score = EventScore::default();
    let mut remaining_gt: Vec<&ResolvedSlot> = gt_slots.iter().collect();
    remaining_gt.sort_by(|a, b| a.slot_id.cmp(&b.slot_id));

    let mut claimed: BTreeSet<String> = BTreeSet::new();

    // Pass 1 — exact matches: the FULL fiscal key must agree (metric_key,
    // fiscal_year, period_type — amendment D), plus zero differing dimensions.
    let mut still_unmatched: Vec<&ResolvedSlot> = Vec::new();
    for gt in remaining_gt {
        let exact = predictions
            .iter()
            .filter(|p| {
                !claimed.contains(&p.id)
                    && p.metric_key == gt.metric_key
                    && p.fiscal_year == gt.fiscal_year
                    && p.period_type == gt.period_type
            })
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
    // YEAR (period_type is now a scored dimension, not a hard filter, so the
    // H1-vs-FY defect can still be PAIRED and classified), fewest differing
    // dimensions, stable prediction-id order on ties.
    for gt in still_unmatched {
        let best = predictions
            .iter()
            .filter(|p| {
                !claimed.contains(&p.id)
                    && p.metric_key == gt.metric_key
                    && p.fiscal_year == gt.fiscal_year
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

/// Whether `p` and `gt` share the FULL identity (event implicit via the
/// caller's own scoping, fiscal year, period type, concept via `metric_key`,
/// basis, window, variant, currency) — amendment O's exclusion-specificity
/// fix. Deliberately NOT `value`: an `unverified`/`machine_v1` slot's whole
/// point is that its true value is unresolved, so requiring value agreement
/// would be circular. Deliberately NOT the nearest-by-diff-count fuzzy match
/// [`score_event`]'s pass 2 uses for ELIGIBLE mismatch pairing — an
/// ineligible slot may only claim a prediction it is EXACTLY, structurally
/// the same slot as.
fn full_identity_matches(gt: &ResolvedSlot, p: &Prediction) -> bool {
    p.metric_key == gt.metric_key
        && p.fiscal_year == gt.fiscal_year
        && p.period_type == gt.period_type
        && p.basis == gt.basis
        && p.variant == gt.variant
        && p.window == gt.window
        && p.currency.as_deref() == Some(gt.currency.as_str())
}

/// Attributes each STILL-UNMATCHED (after eligible scoring, amendment O)
/// prediction to an INELIGIBLE GT slot it shares the FULL identity with —
/// never a fuzzy nearest-match, and never before eligible matching has had
/// first claim (an unverified H1 Revenue slot must not be able to steal an
/// otherwise-exact eligible FY Revenue match). Marks it `OUT_OF_SCOPE`
/// instead of leaving it `FALSE_POSITIVE`. Mutates `predictions` by removing
/// claimed ones and returns the reason-tagged pairs for the caller's
/// console/diagnostic use.
fn attribute_ineligible_predictions(
    ineligible: &[&ResolvedSlot],
    predictions: &mut Vec<Prediction>,
    reason: &'static str,
) -> Vec<(Prediction, &'static str)> {
    let mut claimed = Vec::new();
    for gt in ineligible {
        let Some(idx) = predictions
            .iter()
            .position(|p| full_identity_matches(gt, p))
        else {
            continue;
        };
        let p = predictions.remove(idx);
        claimed.push((p, reason));
    }
    claimed
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
    /// Strict precision recomputed with convention-affected predictions ALSO
    /// removed (amendment Q) — additive fields, never replacing the three
    /// above (the ratchet schema only requires those to be present).
    pub precision_matched: usize,
    pub precision_denominator: usize,
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
    /// REPLACES `delta_matched` (amendment H): the raw replay matched count
    /// over floor events, non-negative — every informational number in this
    /// struct is finite and >= 0 (amendment J), so a signed delta (which
    /// could print negative) no longer belongs here.
    pub replay_matched: usize,
}

/// The public-shaped aggregate metrics (shared contract + amendment H).
/// Deliberately no ticker/title/id/url/value/filename anywhere — asserted by
/// a hermetic test scanning the serialized JSON text.
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
    /// Current-period, floor-events-only, eligible+mapped GT slots (amendment
    /// H(a)) — comparative rows are EXCLUDED from this and from `matched`.
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

// ===========================================================================
// Structured evidence (finding 2/3, PR-A fix wave 2): the private
// `scoring-report-v2.json` carries every field the console/per-issuer table
// needs as its OWN typed field — never a string the reader has to split.
// ===========================================================================

/// One GT slot's outcome, as structured fields (never a `slot_id` the reader
/// has to parse back apart).
#[derive(Debug, Clone, Serialize)]
struct SlotDetail {
    slot_id: String,
    event_id: String,
    member: Option<String>,
    concept: String,
    attribution: String,
    basis: String,
    window: String,
    variant: String,
    fiscal_year: i64,
    period_type: String,
    currency: String,
    outcome: &'static str,
}

/// One event's cold-start evidence: production's own recorded verdict next
/// to what the matcher concluded — the per-event section finding 2 asks for.
#[derive(Debug, Clone, Serialize)]
struct EventReport {
    event_id: String,
    role: String,
    issuer_id: String,
    labeled_period: String,
    derived_period: String,
    /// Production's own `fundamentals_provenance` verdict for this attempt —
    /// `None` when no period could be derived at all (no attempt was made).
    acceptance: Option<String>,
    reason_code: Option<String>,
    prediction_count: usize,
    matched: usize,
    missing: usize,
    false_positives: usize,
    /// Every OTHER prediction outcome class this event produced, by name
    /// (`WRONG_VALUE`, `BASIS_MISMATCH`, …) -> count.
    mismatch_classes: BTreeMap<String, usize>,
    run_error: Option<String>,
    /// This event's own ineligible-verification GT row count (amendment C) —
    /// carried here so [`build_issuer_counts`] can pool it per issuer without
    /// re-running [`resolve_event_slots`] a second time.
    unverified: usize,
}

/// One issuer's pooled counts across its floor events — the console's
/// per-issuer table (finding 2), built from [`EventReport`]s rather than
/// re-deriving anything from the id strings.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct IssuerCounts {
    events: usize,
    gt_slots: usize,
    matched: usize,
    missing: usize,
    wrong: usize,
    false_positives: usize,
    mismatch_classes: BTreeMap<String, usize>,
    unverified: usize,
}

/// Pools [`EventReport`]s by issuer, floor events only — the console's
/// per-issuer table (finding 2). Pure: independently testable without a DB.
fn build_issuer_counts(event_reports: &[EventReport]) -> BTreeMap<&str, IssuerCounts> {
    let mut by_issuer: BTreeMap<&str, IssuerCounts> = BTreeMap::new();
    for report in event_reports {
        if report.role != "floor" {
            continue;
        }
        let counts = by_issuer.entry(report.issuer_id.as_str()).or_default();
        counts.events += 1;
        counts.gt_slots += report.matched + report.missing;
        counts.matched += report.matched;
        counts.missing += report.missing;
        counts.wrong += report
            .mismatch_classes
            .get("WRONG_VALUE")
            .copied()
            .unwrap_or(0);
        counts.false_positives += report.false_positives;
        counts.unverified += report.unverified;
        for (class, n) in &report.mismatch_classes {
            if class != "WRONG_VALUE" {
                *counts.mismatch_classes.entry(class.clone()).or_default() += n;
            }
        }
    }
    by_issuer
}

/// The shared contract's prediction identity key (finding 1): a fresh
/// cold-start DB mints a new `fact_finper_…` row id on every run, so keying
/// `predictions` by that id makes cross-run comparison (the whole point of
/// `keyed-outcomes.json`) impossible. This key is stable across runs of the
/// SAME corpus because it is built entirely from the slot's own identity
/// dimensions, never a generated id.
fn prediction_identity_key(event_id: &str, p: &Prediction) -> String {
    format!(
        "{event_id}/{}/{}/{}/{}/{}/{}/{}/{}",
        p.metric_key,
        p.fiscal_year,
        p.period_type,
        p.basis,
        p.attribution,
        p.variant,
        p.window,
        p.currency.as_deref().unwrap_or("-")
    )
}

/// `previously_correct_slots_lost` (shared contract): a slot the promoted
/// baseline recorded `MATCH` for that this run no longer matches. Pure —
/// independently testable without any DB.
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
// Keyed baseline (amendment G — full metadata validation, never silently
// disabled)
// ===========================================================================

#[derive(Debug, Deserialize)]
struct KeyedBaseline {
    gt_version: String,
    key_map_version: i64,
    normalization_version: i64,
    registry_hash: String,
    outcomes: BTreeMap<String, String>,
}

/// Loads and validates the promoted keyed baseline (amendment G):
/// - no file at `path` -> `None` (the explicit bootstrap case — `lost = 0`).
/// - a file that exists but is unreadable/malformed JSON -> a typed panic
///   (never silently treated as "no baseline").
/// - a file whose `gt_version`/`key_map_version`/`normalization_version`/
///   `registry_hash` disagree with the CURRENT run's own -> "incomparable":
///   a typed panic in `required` mode, a printed warning (bootstrap
///   semantics: `lost = 0`) in diagnostic mode.
fn load_keyed_baseline(
    path: Option<&Path>,
    current_gt_version: &str,
    current_key_map_version: i64,
    current_normalization_version: i64,
    current_registry_hash: &str,
    required: bool,
) -> BTreeMap<String, String> {
    let Some(path) = path else {
        return BTreeMap::new();
    };
    if !path.is_file() {
        eprintln!(
            "NOTE: no promoted baseline at {} — bootstrap (previously_correct_slots_lost = 0)",
            path.display()
        );
        return BTreeMap::new();
    }
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "keyed baseline present but unreadable at {}: {e}",
            path.display()
        )
    });
    let baseline: KeyedBaseline = serde_json::from_str(&raw).unwrap_or_else(|e| {
        panic!(
            "keyed baseline present but malformed at {}: {e}",
            path.display()
        )
    });

    let mismatch = baseline.gt_version != current_gt_version
        || baseline.key_map_version != current_key_map_version
        || baseline.normalization_version != current_normalization_version
        || baseline.registry_hash != current_registry_hash;
    if mismatch {
        let message = format!(
            "incomparable: keyed baseline metadata does not match this run (baseline gt={}/key_map={}/normalization={}/registry={} vs run gt={current_gt_version}/key_map={current_key_map_version}/normalization={current_normalization_version}/registry={current_registry_hash})",
            baseline.gt_version, baseline.key_map_version, baseline.normalization_version, baseline.registry_hash
        );
        if required {
            panic!("{message}");
        }
        eprintln!("NOTE: {message} — treating as bootstrap (previously_correct_slots_lost = 0)");
        return BTreeMap::new();
    }
    baseline.outcomes
}

// ===========================================================================
// Cold-start integration — the real production pipeline, per event
// ===========================================================================

/// One raw Layer 1 tagged fact, carrying its expanded concept identity and
/// full context period — what amendment Q's comparative capture needs
/// (`report_tagged_facts`' own columns, never a local-name-only shortcut).
#[derive(Debug, Clone)]
struct Layer1Fact {
    concept_namespace_uri: String,
    concept_local_name: String,
    /// `"instant"` | duration otherwise.
    period_type: String,
    period_start: Option<String>,
    period_end: String,
    value_numeric: Option<String>,
}

/// The stored facts a fresh cold-start run produced for one event's company,
/// resolved into [`Prediction`]s the matcher can compare — plus the derived
/// period (for the "derivation passed through" test), whether the run itself
/// errored (a `zero_output_events` case), and the Layer 1 rows (comparative
/// capture, amendment H(c)).
struct ColdStartRun {
    predictions: Vec<Prediction>,
    derived: Option<(i64, String, String)>,
    run_error: Option<String>,
    /// Raw Layer 1 tagged facts (`report_tagged_facts`), enriched enough for
    /// amendment Q's occurrence-level comparative capture: expanded concept
    /// identity (namespace URI + local name) and the FULL context period
    /// (instant, or start+end), never local-name-suffix-plus-end-date alone.
    layer1_facts: Vec<Layer1Fact>,
    /// Production's OWN recorded verdict for this attempt
    /// (`fundamentals_provenance().get_extraction_outcome_for_slot`) —
    /// `(acceptance, reason_code)`. `None` when no period could even be
    /// derived (no extraction was attempted, so no outcome row exists) or the
    /// outcome row itself is missing for some other reason; the caller
    /// synthesizes a `no_period_derived`/`no_outcome_recorded` label for the
    /// zero-output report in that case rather than leaving it unexplained.
    recorded_outcome: Option<(String, String)>,
}

/// Seeds a fresh company + report document for one manifest event and runs
/// the REAL production pipeline against it: [`derive_report_period`] then
/// [`run_structured_extraction`] (never a hand-rolled substitute). `state` is
/// passed in so callers that need the shared per-issuer DB (replay) can reuse
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
    let mut recorded_outcome = None;
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
        // Read back what production itself concluded about this attempt —
        // never re-derive or guess a reason; a run that errored before
        // recording an outcome row leaves this `None`, which is itself
        // informative (paired with `run_error` in the report).
        recorded_outcome = state
            .fundamentals_provenance()
            .get_extraction_outcome_for_slot(&company.id, &document.id, *fy, pt, pe)
            .ok()
            .flatten()
            .map(|o| (o.acceptance, o.reason_code));
    }

    let predictions = read_predictions(state, &company.id);
    let layer1_facts = state
        .report_tagged_facts()
        .facts(&document.id)
        .unwrap_or_default()
        .into_iter()
        .map(|f| Layer1Fact {
            concept_namespace_uri: f.concept_namespace_uri,
            concept_local_name: f.concept_local_name,
            period_type: f.period_type,
            period_start: f.period_start,
            period_end: f.period_end,
            value_numeric: f.value_numeric,
        })
        .collect();

    ColdStartRun {
        predictions,
        derived,
        run_error,
        layer1_facts,
        recorded_outcome,
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
// Ground-truth resolution (amendments C, E, F) — shared by cold start,
// twin-agreement and replay so all three apply IDENTICAL eligibility rules.
// ===========================================================================

/// One event's ground truth, split by scoring eligibility (amendments C/E/O/S/T):
/// `current` = pinned-language, eligible-verification, mapped, THIS event's
/// own labeled period; `comparative` = same but any OTHER period the same
/// document tagged; `internal_twin` = OTHER-language, eligible, mapped,
/// current-period slots living inside the SAME package event (amendment S) —
/// excluded from `current`/`comparative` (never a floor denominator), scored
/// separately and compared to `current` by semantic identity for
/// `twin_agreement`. `unverified`/`machine_v1` = mapped but ineligible
/// verification (or, for `unverified`, an unresolved `basis: unknown`
/// contradiction — amendment T) — their attributable prediction is
/// `OUT_OF_SCOPE`, never scored as an ordinary slot.
/// `unverified_count`/`machine_v1_count` are the GT-file-truth totals
/// (amendment O): incremented for EVERY raw slot in that verification state,
/// independent of whether the `(concept, attribution)` pair is even mapped —
/// `unmapped_eligible_count` is the separate, mapping-only exclusion
/// (amendment E), labeled-capability-only, never scored at all.
#[derive(Default)]
struct ResolvedEvent {
    current: Vec<ResolvedSlot>,
    comparative: Vec<ResolvedSlot>,
    internal_twin: Vec<ResolvedSlot>,
    unverified: Vec<ResolvedSlot>,
    machine_v1: Vec<ResolvedSlot>,
    unmapped_eligible_count: usize,
    unverified_count: usize,
    machine_v1_count: usize,
}

fn resolve_event_slots(
    raw_slots: &[GtSlot],
    labeled_period: &LabeledPeriod,
    event_language: &str,
    resolved_map: &ResolvedKeyMap,
) -> ResolvedEvent {
    let mut out = ResolvedEvent::default();
    for slot in raw_slots {
        let class = verification_class(&slot.verification);
        // Amendment T: an unresolved member-basis contradiction is UNVERIFIED
        // regardless of the slot's own labeled verification state.
        let is_unverified = class == VerificationClass::Unverified || slot.basis == "unknown";
        let is_machine_v1 = !is_unverified && class == VerificationClass::MachineV1;

        // Amendment O: verification-state counts are the GT file's own
        // truth — independent of key-map mapping support.
        if is_unverified {
            out.unverified_count += 1;
        } else if is_machine_v1 {
            out.machine_v1_count += 1;
        }

        let pair = (slot.concept_local.clone(), slot.attribution.clone());
        let Some(metric_key) = resolved_map.pair_to_metric_key.get(&pair) else {
            // Amendment E: an unsupported (concept, attribution) pair (e.g.
            // `ProfitLoss`/`nci`) is labeled-capability only — NEVER falls
            // back to the concept's usual (e.g. `total`) metric_key. Only an
            // otherwise-eligible, otherwise-verified row counts here — an
            // unverified/machine_v1 row was already counted above.
            if !is_unverified && !is_machine_v1 {
                out.unmapped_eligible_count += 1;
            }
            continue;
        };
        let Some(currency) = slot.currency.clone() else {
            // A currency-less (pure-number) slot is labeled capability only.
            if !is_unverified && !is_machine_v1 {
                out.unmapped_eligible_count += 1;
            }
            continue;
        };
        let value = slot.value.parse::<Decimal>().unwrap_or_else(|e| {
            panic!(
                "incomparable: slot {} value '{}': {e}",
                slot.slot_id, slot.value
            )
        });
        let resolved = ResolvedSlot {
            slot_id: slot.slot_id.clone(),
            package_member: slot.package_member.clone(),
            concept_local: slot.concept_local.clone(),
            metric_key: metric_key.clone(),
            fiscal_year: slot.fiscal_year,
            period_type: slot.period_type.clone(),
            period_end: slot.period_end.clone(),
            basis: slot.basis.clone(),
            attribution: slot.attribution.clone(),
            variant: slot.variant.clone(),
            window: slot.window.clone(),
            currency,
            value,
            normalized_by: slot.normalized_by.clone(),
        };
        if is_unverified {
            out.unverified.push(resolved);
        } else if is_machine_v1 {
            out.machine_v1.push(resolved);
        } else {
            let is_current_period = slot.fiscal_year == labeled_period.fiscal_year
                && slot.period_type == labeled_period.period_type
                && slot.period_end == labeled_period.period_end;
            // Amendment S: a slot whose own language disagrees with the
            // event's pinned language is the internal twin-diagnostic
            // population — `unknown` on either side is never treated as a
            // disagreement (an untagged language is not evidence of a twin).
            let is_pinned_language = slot.language == event_language
                || slot.language == "unknown"
                || event_language == "unknown";
            match (is_current_period, is_pinned_language) {
                (true, true) => out.current.push(resolved),
                (false, true) => out.comparative.push(resolved),
                (true, false) => out.internal_twin.push(resolved),
                // A comparative-period, other-language slot fits no scored
                // bucket (amendment S only defines the twin population for
                // the CURRENT period) — evidence only, not tracked further.
                (false, false) => {}
            }
        }
    }
    out
}

// ===========================================================================
// run_measurement — the full orchestration
// ===========================================================================

pub(crate) struct MeasurementConfig<'a> {
    pub corpus_dir: &'a Path,
    /// The public key map to score against — an explicit path so this
    /// function never reaches across the workspace itself (amendment 18):
    /// the `#[ignore]`d real-data test points it at
    /// `scripts/realdata/esef-v2/gt_key_map.json`; every hermetic test points
    /// it at the crate-local `testdata/esef-v2-sample/gt_key_map.json` copy.
    pub key_map_path: &'a Path,
    pub metrics_out: Option<&'a Path>,
    pub keyed_baseline: Option<&'a Path>,
    pub required: bool,
}

/// The full measurement run: loads the manifest/ground-truth/key-map, cold-
/// starts every `floor`/`twin_diagnostic` event, scores it, replays the
/// issuer's warmup+floor events in one shared DB chronologically, writes the
/// private evidence files and the public-shaped metrics, prints the console
/// report, and returns the aggregates. `None` when `corpus_dir` is
/// absent/unreadable and `config.required` is `false` (the CI-safe skip);
/// panics when `required` is `true` (typed panics also fire for version
/// mismatches / malformed JSON / orphan event ids — never a silent skip for
/// those).
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

    let key_map = load_key_map(config.key_map_path);

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

    // Amendment G: recompute the registry hash from the ACTUAL corpus files
    // (never trust the declared value) and compare.
    let recomputed_registry_hash = recompute_registry_hash(config.corpus_dir, &manifest);
    assert_eq!(
        manifest.registry_hash, recomputed_registry_hash,
        "incomparable: MANIFEST_v2.json registry_hash does not match the corpus files on disk \
         (declared {}, recomputed {recomputed_registry_hash})",
        manifest.registry_hash
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

    // Amendment Q: Layer 1 comparative capture reads raw occurrences, never
    // the already-collapsed GT slots.
    let occurrences = load_occurrences(config.corpus_dir);
    let mut occurrences_by_event: BTreeMap<&str, Vec<&Occurrence>> = BTreeMap::new();
    for occurrence in &occurrences {
        occurrences_by_event
            .entry(occurrence.event_id.as_str())
            .or_default()
            .push(occurrence);
    }

    // Amendment F: slot ids must be unique at every boundary.
    let mut seen_slot_ids: BTreeSet<&str> = BTreeSet::new();
    for slot in &ground_truth.slots {
        assert!(
            seen_slot_ids.insert(slot.slot_id.as_str()),
            "incomparable: duplicate ground-truth slot_id {}",
            slot.slot_id
        );
    }

    let event_by_id: BTreeMap<&str, &Event> = manifest
        .events
        .iter()
        .map(|e| (e.event_id.as_str(), e))
        .collect();
    // Amendment C: an event id in the GT with no manifest event is a typed failure.
    let mut gt_by_event: BTreeMap<&str, Vec<GtSlot>> = BTreeMap::new();
    for slot in &ground_truth.slots {
        if !event_by_id.contains_key(slot.event_id.as_str()) {
            panic!(
                "incomparable: ground truth slot {} references unknown event_id {}",
                slot.slot_id, slot.event_id
            );
        }
        gt_by_event
            .entry(&slot.event_id)
            .or_default()
            .push(slot.clone());
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
    let mut unverified_total = 0usize;
    let mut machine_v1_total = 0usize;
    let mut labeled_eligible_total = 0usize; // amendment H: labeled-capability denominator (incl. unmapped)
                                             // amendment H(b): GT slots (all periods) with a stored Layer 2 slot.
    let mut availability_available = 0usize;
    let mut availability_eligible = 0usize;
    // amendment H(c): comparative GT occurrences present in Layer 1, structurally + value-correct.
    let mut layer1_captured = 0usize;
    let mut layer1_eligible = 0usize;
    let mut layer1_value_correct = 0usize;
    let mut derived_by_event: BTreeMap<String, (String, String)> = BTreeMap::new();
    // Amendment Q: sensitivity's convention-resolved exclusion sets — GT
    // side from the labeler's own `normalized_by` field (per slot_id),
    // prediction side from the harness's own metric-key inference (per
    // prediction identity key) — removed from BOTH recall and precision.
    let mut sign_convention_slot_ids: BTreeSet<String> = BTreeSet::new();
    let mut sign_convention_prediction_keys: BTreeSet<String> = BTreeSet::new();
    // Amendment S: within-package language-twin agreement, accumulated
    // across every event that carries one — added to the loose-file twin
    // totals ([`compute_twin_agreement`]) below.
    let mut internal_twin_agree = 0usize;
    let mut internal_twin_compared = 0usize;
    // Finding 2/3: the full per-event/per-slot evidence, structured (never
    // parsed back out of an id) — covers every cold-started event (floor AND
    // twin), while `all_slot_outcomes`/`all_prediction_outcomes` stay
    // floor-only (the primary recall/precision denominators).
    let mut all_slot_details: Vec<SlotDetail> = Vec::new();
    let mut event_reports: Vec<EventReport> = Vec::new();

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
        let raw_slots = gt_by_event.get(event.event_id.as_str()).unwrap_or(&empty);
        let resolved = resolve_event_slots(
            raw_slots,
            &event.labeled_period,
            &event.language,
            &resolved_map,
        );

        if event.role == "floor" {
            unverified_total += resolved.unverified_count;
            machine_v1_total += resolved.machine_v1_count;
            // Amendment Q: labeled-capability's `eligible` denominator counts
            // EVERY labeled IFRS slot with eligible verification — mapped
            // (current/comparative/internal-twin) or not.
            labeled_eligible_total += resolved.current.len()
                + resolved.comparative.len()
                + resolved.internal_twin.len()
                + resolved.unmapped_eligible_count;
            // Amendment Q: sensitivity's convention-resolved population comes
            // from the labeler's OWN `normalized_by` field (every
            // `contract_normalized` rule id a slot's value/window actually
            // went through), never a harness-guessed metric-key list.
            for slot in &resolved.current {
                if !slot.normalized_by.is_empty() {
                    sign_convention_slot_ids.insert(slot.slot_id.clone());
                }
            }

            // Amendment Q: availability = a GT slot (ANY labeled period) with
            // a stored Layer 2 prediction matching its FULL semantic identity
            // — fiscal year and period type INCLUDED, so a current-period
            // prediction can never "make available" a comparative slot it
            // does not actually share a period with (a real bug the r2 diff
            // review caught). Attribution is never compared against the raw
            // stored column — the semantic mapping already lives entirely in
            // `metric_key` via the key map's `(concept, attribution)` pair.
            for slot in resolved.current.iter().chain(resolved.comparative.iter()) {
                availability_eligible += 1;
                let available = run.predictions.iter().any(|p| {
                    p.metric_key == slot.metric_key
                        && p.fiscal_year == slot.fiscal_year
                        && p.period_type == slot.period_type
                        && p.basis == slot.basis
                        && p.variant == slot.variant
                        && p.window == slot.window
                        && p.currency.as_deref() == Some(slot.currency.as_str())
                });
                if available {
                    availability_available += 1;
                }
            }

            // Amendment Q: Layer 1 capture is over comparative-period GT
            // OCCURRENCES (from `occurrences_v2.json`, raw evidence — never
            // the already-collapsed GT slots): captured = a raw tagged fact
            // with the SAME EXPANDED concept (namespace URI + local name) AND
            // the SAME FULL context period (instant, or start+end) — never a
            // local-name-suffix-plus-end-date shortcut. Value-correct is
            // computed separately.
            let empty_occurrences = Vec::new();
            let event_occurrences = occurrences_by_event
                .get(event.event_id.as_str())
                .unwrap_or(&empty_occurrences);
            for occurrence in event_occurrences {
                let is_comparative = occurrence.period.end() != event.labeled_period.period_end;
                if !is_comparative {
                    continue;
                }
                layer1_eligible += 1;
                if let Some(fact) = run.layer1_facts.iter().find(|fact| {
                    occurrence.concept_qname
                        == format!(
                            "{{{}}}{}",
                            fact.concept_namespace_uri, fact.concept_local_name
                        )
                        && occurrence.period.matches_layer1(fact)
                }) {
                    layer1_captured += 1;
                    let occurrence_value = occurrence.value.parse::<Decimal>().ok();
                    if fact
                        .value_numeric
                        .as_deref()
                        .and_then(|v| v.parse::<Decimal>().ok())
                        == occurrence_value
                    {
                        layer1_value_correct += 1;
                    }
                }
            }
        }

        let has_predictions = !run.predictions.is_empty();
        let prediction_count = run.predictions.len();
        let filtered = prefilter_predictions(run.predictions, &resolved_map);
        let eligible_predictions = filtered.eligible;

        // Amendment O: eligible slots get first claim — score them BEFORE any
        // ineligible-slot attribution runs, so an unverified/machine_v1 slot
        // can never steal a prediction an eligible slot would have exact-
        // matched. Amendment H(a): only the event's OWN current-period slots
        // feed primary recall/precision — comparative rows never enter the
        // matcher.
        let score = score_event(
            &resolved.current,
            &eligible_predictions,
            &resolved_map.panel,
        );

        // Amendment S: an internal (within-package) language twin population
        // is scored against the SAME predictions (one document, one
        // extraction run) and compared to the pinned-language `current`
        // slots by semantic identity — no separate DB, unlike a loose-file
        // twin event ([`compute_twin_agreement`] below).
        if !resolved.internal_twin.is_empty() {
            let twin_score = score_event(
                &resolved.internal_twin,
                &eligible_predictions,
                &resolved_map.panel,
            );
            let identity_of = |s: &ResolvedSlot| {
                format!(
                    "{}/{}/{}/{}/{}/{}/{}",
                    s.metric_key,
                    s.attribution,
                    s.basis,
                    s.window,
                    s.variant,
                    s.fiscal_year,
                    s.period_type
                )
            };
            let current_by_identity: BTreeMap<String, Outcome> = resolved
                .current
                .iter()
                .filter_map(|s| {
                    score
                        .slot_outcomes
                        .get(&s.slot_id)
                        .map(|o| (identity_of(s), *o))
                })
                .collect();
            for slot in &resolved.internal_twin {
                let Some(current_outcome) = current_by_identity.get(&identity_of(slot)) else {
                    continue;
                };
                let Some(twin_outcome) = twin_score.slot_outcomes.get(&slot.slot_id) else {
                    continue;
                };
                internal_twin_compared += 1;
                if twin_outcome == current_outcome {
                    internal_twin_agree += 1;
                }
            }
        }

        // ---- structured per-event/per-slot evidence (finding 2/3) ---------
        let eligible_by_id: BTreeMap<&str, &Prediction> = eligible_predictions
            .iter()
            .map(|p| (p.id.as_str(), p))
            .collect();
        let mut event_prediction_outcomes: BTreeMap<String, Outcome> = BTreeMap::new();
        for (fact_id, outcome) in &score.prediction_outcomes {
            let p = eligible_by_id
                .get(fact_id.as_str())
                .expect("a scored prediction id must be in the eligible pool that was scored");
            let key = prediction_identity_key(&event.event_id, p);
            // Amendment Q: the harness infers a PREDICTION's own
            // `contract_normalized` membership from its `metric_key` (the
            // same rule ids the labeler names) — the GT side reads
            // `normalized_by` directly instead (see the main loop above).
            if event.role == "floor" && SIGN_CONVENTION_METRIC_KEYS.contains(&p.metric_key.as_str())
            {
                sign_convention_prediction_keys.insert(key.clone());
            }
            event_prediction_outcomes.insert(key, *outcome);
        }

        // Amendment O: only NOW — after eligible matching has had first
        // claim — do unverified/machine_v1 slots get to attribute the
        // STILL-UNMATCHED (`FALSE_POSITIVE`) predictions, and only by the
        // FULL identity (never metric+year alone, never a fuzzy nearest
        // match).
        let mut still_unmatched: Vec<Prediction> = eligible_predictions
            .iter()
            .filter(|p| {
                event_prediction_outcomes
                    .get(&prediction_identity_key(&event.event_id, p))
                    .copied()
                    == Some(Outcome::FalsePositive)
            })
            .cloned()
            .collect();
        let unverified_refs: Vec<&ResolvedSlot> = resolved.unverified.iter().collect();
        let machine_v1_refs: Vec<&ResolvedSlot> = resolved.machine_v1.iter().collect();
        let ineligible_claims_unverified =
            attribute_ineligible_predictions(&unverified_refs, &mut still_unmatched, "unverified");
        let ineligible_claims_machine_v1 =
            attribute_ineligible_predictions(&machine_v1_refs, &mut still_unmatched, "machine_v1");
        for (p, _reason) in ineligible_claims_unverified
            .iter()
            .chain(&ineligible_claims_machine_v1)
        {
            event_prediction_outcomes.insert(
                prediction_identity_key(&event.event_id, p),
                Outcome::OutOfScope,
            );
        }
        for (p, outcome) in &filtered.excluded {
            event_prediction_outcomes.insert(prediction_identity_key(&event.event_id, p), *outcome);
        }
        let event_matched = score
            .slot_outcomes
            .values()
            .filter(|o| **o == Outcome::Match)
            .count();
        let event_missing = score
            .slot_outcomes
            .values()
            .filter(|o| **o == Outcome::Missing)
            .count();
        let event_false_positives = event_prediction_outcomes
            .values()
            .filter(|o| **o == Outcome::FalsePositive)
            .count();
        let mut event_mismatch_classes: BTreeMap<String, usize> = BTreeMap::new();
        for outcome in event_prediction_outcomes.values() {
            if !matches!(
                outcome,
                Outcome::Match | Outcome::OutOfScope | Outcome::FalsePositive
            ) {
                *event_mismatch_classes
                    .entry(outcome.as_str().to_owned())
                    .or_default() += 1;
            }
        }
        event_reports.push(EventReport {
            event_id: event.event_id.clone(),
            role: event.role.clone(),
            issuer_id: event.issuer_id.clone(),
            labeled_period: format!(
                "{}/{}/{}",
                event.labeled_period.fiscal_year,
                event.labeled_period.period_type,
                event.labeled_period.period_end
            ),
            derived_period: run
                .derived
                .as_ref()
                .map(|(fy, pt, pe)| format!("{fy}/{pt}/{pe}"))
                .unwrap_or_else(|| "none".to_owned()),
            acceptance: run.recorded_outcome.as_ref().map(|(a, _)| a.clone()),
            reason_code: run.recorded_outcome.as_ref().map(|(_, r)| r.clone()),
            prediction_count,
            matched: event_matched,
            missing: event_missing,
            false_positives: event_false_positives,
            mismatch_classes: event_mismatch_classes,
            run_error: run.run_error.clone(),
            unverified: resolved.unverified_count,
        });
        for slot in &resolved.current {
            let outcome = score
                .slot_outcomes
                .get(&slot.slot_id)
                .copied()
                .unwrap_or(Outcome::Missing);
            all_slot_details.push(SlotDetail {
                slot_id: slot.slot_id.clone(),
                event_id: event.event_id.clone(),
                member: slot.package_member.clone(),
                concept: slot.concept_local.clone(),
                attribution: slot.attribution.clone(),
                basis: slot.basis.clone(),
                window: slot.window.clone(),
                variant: slot.variant.clone(),
                fiscal_year: slot.fiscal_year,
                period_type: slot.period_type.clone(),
                currency: slot.currency.clone(),
                outcome: outcome.as_str(),
            });
        }

        if event.role == "floor" {
            floor_events += 1;
            if !has_predictions {
                zero_output_events += 1;
            }
            all_slot_outcomes.extend(score.slot_outcomes.clone());
            // `event_prediction_outcomes` is already the FULLY resolved
            // per-key outcome for this event (eligible scoring, THEN
            // ineligible reattribution overwriting FALSE_POSITIVE with
            // OUT_OF_SCOPE where it claimed one, THEN the pre-filtered
            // exclusions) — merge it in directly rather than redoing the
            // reattribution dance a second time, which would otherwise see
            // the SAME key twice (once FALSE_POSITIVE, once its own
            // legitimate OUT_OF_SCOPE overwrite) and misreport it as a
            // genuine duplicate-identity collision.
            for (key, outcome) in &event_prediction_outcomes {
                if let Some(previous) = all_prediction_outcomes.insert(key.clone(), *outcome) {
                    panic!(
                        "incomparable: duplicate prediction identity key {key} (previous outcome {previous:?}, new outcome {outcome:?}) \
                         — two stored facts resolved to the same identity, which the storage layer's own slot uniqueness should prevent"
                    );
                }
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
    // Loose-file twins (compute_twin_agreement) + within-package language
    // twins (accumulated in the main loop above, amendment S) — one combined
    // `twin_agreement` metric.
    let (loose_twin_agree, loose_twin_compared) = compute_twin_agreement(
        &manifest,
        &gt_by_event,
        config.corpus_dir,
        &issuer_by_id,
        &resolved_map,
    );
    let twin_agree = loose_twin_agree + internal_twin_agree;
    let twin_compared = loose_twin_compared + internal_twin_compared;

    // ---- replay (diagnostic; one shared DB per issuer) ----------------------
    let replay = run_replay(
        &manifest,
        &gt_by_event,
        config.corpus_dir,
        &issuer_by_id,
        &resolved_map,
    );

    let matched = all_slot_outcomes
        .values()
        .filter(|o| **o == Outcome::Match)
        .count();
    let false_positives = all_prediction_outcomes
        .values()
        .filter(|o| **o == Outcome::FalsePositive)
        .count();
    let mismatch_predictions = all_prediction_outcomes
        .values()
        .filter(|o| {
            !matches!(
                o,
                Outcome::Match | Outcome::OutOfScope | Outcome::FalsePositive
            )
        })
        .count();
    let precision_denominator = matched + false_positives + mismatch_predictions;

    // ---- previously_correct_slots_lost ---------------------------------------
    let baseline = load_keyed_baseline(
        config.keyed_baseline,
        &ground_truth.gt_version,
        ground_truth.key_map_version,
        ground_truth.normalization_version,
        &recomputed_registry_hash,
        config.required,
    );
    let lost = previously_correct_slots_lost(&all_slot_outcomes, &baseline);

    // ---- sensitivity (amendment Q): the convention-resolved population —
    // every slot/prediction whose value/window went through ANY
    // `contract_normalized` rule — removed from BOTH recall (GT slots, via
    // the labeler's own `normalized_by`) and STRICT precision (predictions,
    // via the harness's own metric-key inference), recomputed independently.
    let sensitivity_excluded = sign_convention_slot_ids.len();
    let sensitivity_gt_slots = all_slot_outcomes.len().saturating_sub(sensitivity_excluded);
    let sensitivity_matched = all_slot_outcomes
        .iter()
        .filter(|(id, o)| **o == Outcome::Match && !sign_convention_slot_ids.contains(id.as_str()))
        .count();
    let sensitivity_precision_matched = all_prediction_outcomes
        .iter()
        .filter(|(key, o)| {
            **o == Outcome::Match && !sign_convention_prediction_keys.contains(key.as_str())
        })
        .count();
    let sensitivity_precision_denominator = all_prediction_outcomes
        .iter()
        .filter(|(key, o)| {
            **o != Outcome::OutOfScope && !sign_convention_prediction_keys.contains(key.as_str())
        })
        .count();

    let gt_slots_total = all_slot_outcomes.len();

    let aggregates = Aggregates {
        profile: "esef".to_owned(),
        status: "measured".to_owned(),
        measurement_version: 1,
        gt_version: ground_truth.gt_version.clone(),
        key_map_version: key_map.key_map_version,
        normalization_version: ground_truth.normalization_version,
        registry_hash: recomputed_registry_hash.clone(),
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
            eligible: layer1_eligible,
            value_correct: layer1_value_correct,
        },
        labeled_capability: CountPair {
            matched,
            eligible: labeled_eligible_total,
        },
        sensitivity: Sensitivity {
            matched: sensitivity_matched,
            gt_slots: sensitivity_gt_slots,
            excluded: sensitivity_excluded,
            precision_matched: sensitivity_precision_matched,
            precision_denominator: sensitivity_precision_denominator,
        },
        twin_agreement: TwinAgreement {
            agree: twin_agree,
            compared: twin_compared,
        },
        replay,
    };

    // Finding 3: the per-concept MISSING breakdown (floor events' current-
    // period slots only, matching `gt_slots`/`matched`'s own scope) —
    // computed from `all_slot_details`' OWN structured fields, never by
    // splitting `slot_id`.
    let mut missing_by_concept: BTreeMap<(String, String), usize> = BTreeMap::new();
    for detail in &all_slot_details {
        let is_floor = event_by_id
            .get(detail.event_id.as_str())
            .is_some_and(|e| e.role == "floor");
        if is_floor && detail.outcome == "MISSING" {
            *missing_by_concept
                .entry((detail.concept.clone(), detail.attribution.clone()))
                .or_default() += 1;
        }
    }

    // Finding 2: per-issuer counts (floor events only) — printed as a table.
    let by_issuer = build_issuer_counts(&event_reports);

    print_console_report(
        &aggregates,
        &lost,
        machine_v1_total,
        precision_denominator,
        &event_reports,
        &by_issuer,
        &missing_by_concept,
    );

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
            "slots": all_slot_details,
            "events": event_reports,
            "missing_by_concept": missing_by_concept
                .iter()
                .map(|((concept, attribution), count)| serde_json::json!({
                    "concept": concept, "attribution": attribution, "count": count,
                }))
                .collect::<Vec<_>>(),
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
            "gt_version": aggregates.gt_version,
            "key_map_version": key_map.key_map_version,
            "normalization_version": aggregates.normalization_version,
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

/// Amendment H: sensitivity excludes GT slots for a metric_key subject to a
/// `contract_normalized` convention in `gt_key_map.json` — specifically
/// `cash_flow_outflow_sign` (a cash outflow is compared as a negated decimal
/// on both sides). The key map lists conventions as prose rules, not a
/// structured concept list, so the affected metric keys are pinned here,
/// named against that rule id.
const SIGN_CONVENTION_METRIC_KEYS: &[&str] = &["investing_cash_flow", "financing_cash_flow"];

/// Amendment G: sha256 over the sorted per-file sha256 hex digests of every
/// DISTINCT corpus file the manifest's events reference — recomputed from the
/// actual bytes on disk, never trusting `MANIFEST_v2.json`'s own declared
/// `registry_hash`.
fn recompute_registry_hash(corpus_dir: &Path, manifest: &Manifest) -> String {
    let mut file_hashes: BTreeSet<String> = BTreeSet::new();
    for event in &manifest.events {
        let path = corpus_dir.join("corpus").join(&event.file.name);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "corpus file {} unreadable while hashing the registry: {e}",
                path.display()
            )
        });
        file_hashes.insert(content_hash_hex(&bytes));
    }
    content_hash_hex(
        file_hashes
            .into_iter()
            .collect::<Vec<_>>()
            .join("")
            .as_bytes(),
    )
}

fn write_atomic_json<T: Serialize>(path: &Path, value: &T) {
    let json = serde_json::to_string_pretty(value).expect("serialize");
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).unwrap_or_else(|e| panic!("write {}: {e}", tmp.display()));
    std::fs::rename(&tmp, path).unwrap_or_else(|e| panic!("rename into {}: {e}", path.display()));
}

/// Every identity shared by a `twin_diagnostic` event and its paired `floor`
/// event (same issuer, same `labeled_period.period_end`) is scored
/// independently in a throwaway DB pair, then compared. Twins are excluded
/// from replay (amendment I) — this is their only comparison.
fn compute_twin_agreement(
    manifest: &Manifest,
    gt_by_event: &BTreeMap<&str, Vec<GtSlot>>,
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
        let twin_raw = gt_by_event.get(twin.event_id.as_str()).unwrap_or(&empty);
        let floor_raw = gt_by_event.get(floor.event_id.as_str()).unwrap_or(&empty);
        let twin_resolved =
            resolve_event_slots(twin_raw, &twin.labeled_period, &twin.language, resolved_map);
        let floor_resolved = resolve_event_slots(
            floor_raw,
            &floor.labeled_period,
            &floor.language,
            resolved_map,
        );

        let twin_score = score_fresh(
            twin,
            &twin_resolved.current,
            corpus_dir,
            issuer_by_id,
            resolved_map,
        );
        let floor_score = score_fresh(
            floor,
            &floor_resolved.current,
            corpus_dir,
            issuer_by_id,
            resolved_map,
        );

        let identity_of = |s: &ResolvedSlot| {
            format!(
                "{}/{}/{}/{}/{}/{}/{}",
                s.metric_key,
                s.attribution,
                s.basis,
                s.window,
                s.variant,
                s.fiscal_year,
                s.period_type
            )
        };
        let mut floor_by_identity: BTreeMap<String, Outcome> = BTreeMap::new();
        for slot in &floor_resolved.current {
            if let Some(o) = floor_score.slot_outcomes.get(&slot.slot_id) {
                floor_by_identity.insert(identity_of(slot), *o);
            }
        }
        for slot in &twin_resolved.current {
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

/// Cold-starts and scores exactly one event's current-period slots in its own
/// fresh DB — the building block [`compute_twin_agreement`] needs to score a
/// floor and a twin event independently (never sharing state, unlike replay).
fn score_fresh(
    event: &Event,
    current_slots: &[ResolvedSlot],
    corpus_dir: &Path,
    issuer_by_id: &BTreeMap<&str, &Issuer>,
    resolved_map: &ResolvedKeyMap,
) -> EventScore {
    let issuer = issuer_by_id[event.issuer_id.as_str()];
    let connection = open_in_memory_database().expect("in-memory db");
    let state = AppState::with_data_dir(connection, corpus_dir.to_path_buf());
    let run = seed_and_run_event(&state, issuer, event, corpus_dir);
    let filtered = prefilter_predictions(run.predictions, resolved_map);
    score_event(current_slots, &filtered.eligible, &resolved_map.panel)
}

/// Replay (diagnostic, amendment I): one shared DB per issuer, `warmup` +
/// `floor` events (twins EXCLUDED) in chronological order (`labeled_period.
/// period_end`, then `vintage`, then `event_id` as a final stable tie-break),
/// each scored against the DB state immediately after it runs.
/// `exercised_prior_check`/`exercised_quarantine` are read from the ACTUAL
/// run: the former from the real cross-check input
/// (`stored_fact_set_for_cross_check`-equivalent read) taken BEFORE the
/// event's own extraction call, the latter from the real quarantine gate's
/// own input (`metric_histories`) over the event's own current-period metric
/// keys, both inspected before each event runs.
fn run_replay(
    manifest: &Manifest,
    gt_by_event: &BTreeMap<&str, Vec<GtSlot>>,
    corpus_dir: &Path,
    issuer_by_id: &BTreeMap<&str, &Issuer>,
    resolved_map: &ResolvedKeyMap,
) -> Replay {
    let mut by_issuer: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    for event in manifest
        .events
        .iter()
        .filter(|e| e.role != "twin_diagnostic")
    {
        by_issuer
            .entry(event.issuer_id.as_str())
            .or_default()
            .push(event);
    }

    let mut events_replayed = 0usize;
    let mut exercised_prior_check = 0usize;
    let mut exercised_quarantine = 0usize;
    let mut replay_matched = 0usize;

    for (issuer_id, mut events) in by_issuer {
        events.sort_by(|a, b| {
            a.labeled_period
                .period_end
                .cmp(&b.labeled_period.period_end)
                .then_with(|| a.vintage.cmp(&b.vintage))
                .then_with(|| a.event_id.cmp(&b.event_id))
        });
        let issuer = issuer_by_id[issuer_id];
        let connection = open_in_memory_database().expect("in-memory db");
        let state = AppState::with_data_dir(connection, corpus_dir.to_path_buf());

        let empty_raw = Vec::new();
        for event in events {
            events_replayed += 1;
            let company_id = event_company_id(&state, issuer);

            // Amendment R: these are the PRE-run production inputs — only
            // credited as "exercised" once the run below actually reaches a
            // persisted outcome (never on a failed derivation/run error).
            let prior_existed = state
                .financials()
                .stored_fact_set(
                    &company_id,
                    event.labeled_period.fiscal_year - 1,
                    &event.labeled_period.period_type,
                )
                .ok()
                .flatten()
                .is_some();

            let raw_slots = gt_by_event
                .get(event.event_id.as_str())
                .unwrap_or(&empty_raw);
            let resolved = resolve_event_slots(
                raw_slots,
                &event.labeled_period,
                &event.language,
                resolved_map,
            );
            let history_keys: BTreeSet<String> = resolved
                .current
                .iter()
                .map(|s| s.metric_key.clone())
                .collect();
            let sufficient_history = !history_keys.is_empty()
                && state
                    .financials()
                    .metric_histories(
                        &company_id,
                        &history_keys,
                        event.labeled_period.fiscal_year,
                        &event.labeled_period.period_type,
                    )
                    .unwrap_or_default()
                    .values()
                    .any(|h| h.len() >= 2);

            let run = seed_and_run_event(&state, issuer, event, corpus_dir);
            // Amendment R: only an extraction that actually ran to a
            // persisted outcome (acceptance recorded) counts the pre-run
            // inputs above as "exercised".
            if run.recorded_outcome.is_some() {
                if prior_existed {
                    exercised_prior_check += 1;
                }
                if sufficient_history {
                    exercised_quarantine += 1;
                }
            }

            // Amendment O: eligible slots get first claim, same as the main
            // cold-start pass — an ineligible slot may only attribute a
            // prediction eligible scoring left as FALSE_POSITIVE.
            let filtered = prefilter_predictions(run.predictions, resolved_map);
            let score = score_event(&resolved.current, &filtered.eligible, &resolved_map.panel);
            if event.role == "floor" {
                replay_matched += score
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
        replay_matched,
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

#[allow(clippy::too_many_arguments)]
fn print_console_report(
    aggregates: &Aggregates,
    lost: &[String],
    machine_v1_total: usize,
    precision_denominator: usize,
    event_reports: &[EventReport],
    by_issuer: &BTreeMap<&str, IssuerCounts>,
    missing_by_concept: &BTreeMap<(String, String), usize>,
) {
    let pct = |n: usize, d: usize| {
        if d == 0 {
            "not measurable".to_owned()
        } else {
            format!("{:.1}%", 100.0 * n as f64 / d as f64)
        }
    };
    eprintln!("== ESEF measurement v2 ==");
    eprintln!("estimand: exact-normalized-decimal match of one semantic (concept, attribution, basis, window, variant, period) slot against the app's stored fact, current period only");
    eprintln!(
        "versions: measurement={} gt={} key_map={} normalization={}",
        aggregates.measurement_version,
        aggregates.gt_version,
        aggregates.key_map_version,
        aggregates.normalization_version
    );
    eprintln!(
        "events={} floor_events={} issuers={} gt_slots={} unverified={} machine_v1={}",
        aggregates.events,
        aggregates.floor_events,
        aggregates.issuers,
        aggregates.gt_slots,
        aggregates.unverified,
        machine_v1_total
    );
    eprintln!(
        "matched={} ({} of gt_slots)  false_positives={}  zero_output_events={}",
        aggregates.matched,
        pct(aggregates.matched, aggregates.gt_slots),
        aggregates.false_positives,
        aggregates.zero_output_events
    );
    eprintln!(
        "labeled-scope precision: {} ({}/{})",
        pct(aggregates.matched, precision_denominator),
        aggregates.matched,
        precision_denominator
    );
    eprintln!(
        "labeled_capability: {}/{}  sensitivity(recall): {}/{}  sensitivity(strict precision): {}/{} (excluded {})  twin_agreement: {}/{}",
        aggregates.labeled_capability.matched,
        aggregates.labeled_capability.eligible,
        aggregates.sensitivity.matched,
        aggregates.sensitivity.gt_slots,
        aggregates.sensitivity.precision_matched,
        aggregates.sensitivity.precision_denominator,
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
        "replay: events={} prior_check_exercised={} quarantine_exercised={} replay_matched={}",
        aggregates.replay.events,
        aggregates.replay.exercised_prior_check,
        aggregates.replay.exercised_quarantine,
        aggregates.replay.replay_matched
    );
    eprintln!(
        "previously_correct_slots_lost={} {:?}",
        aggregates.previously_correct_slots_lost, lost
    );

    // Finding 2: the per-issuer counts table — the contract's console
    // anatomy, built entirely from structured [`IssuerCounts`], never a
    // string split.
    eprintln!();
    eprintln!("-- per issuer (floor events) --");
    eprintln!(
        "{:<12} {:>6} {:>8} {:>7} {:>7} {:>4} {:>3} {:>11} mismatch_classes",
        "issuer_id", "events", "gt_slots", "matched", "missing", "wrong", "fp", "unverified"
    );
    for (issuer_id, counts) in by_issuer {
        eprintln!(
            "{:<12} {:>6} {:>8} {:>7} {:>7} {:>4} {:>3} {:>11} {:?}",
            issuer_id,
            counts.events,
            counts.gt_slots,
            counts.matched,
            counts.missing,
            counts.wrong,
            counts.false_positives,
            counts.unverified,
            counts.mismatch_classes
        );
    }

    // Finding 3: a zero-output event never gets a mute row — the console
    // names WHY (a run error, or production's own recorded acceptance/reason)
    // so a real-corpus run doesn't have to reopen the JSON to diagnose it.
    let zero_output: Vec<&EventReport> = event_reports
        .iter()
        .filter(|r| r.role == "floor" && r.prediction_count == 0)
        .collect();
    if !zero_output.is_empty() {
        eprintln!();
        eprintln!("-- zero-output floor events (0 predictions) --");
        for report in &zero_output {
            let reason = if let Some(error) = &report.run_error {
                format!("RUN ERROR: {error}")
            } else {
                match (&report.acceptance, &report.reason_code) {
                    (Some(a), Some(r)) => format!("acceptance={a} reason={r}"),
                    _ => "no period derived — no extraction was attempted".to_owned(),
                }
            };
            eprintln!("  {} : {reason}", report.event_id);
        }
    }

    // Finding 3: the per-concept MISSING breakdown (floor events, current
    // period) — so PR-B can target the concept map directly.
    if !missing_by_concept.is_empty() {
        eprintln!();
        eprintln!("-- MISSING by concept (floor events, current period) --");
        for ((concept, attribution), count) in missing_by_concept {
            eprintln!("  {concept}/{attribution}: {count}");
        }
    }
}

// ===========================================================================
// The real-data #[ignore] test — env-driven, inert in CI
// ===========================================================================

/// Real ESEF measurement v2 (#331 PR-A, ADR 0112). **Inert in CI** — skips
/// unless `BRAWLER_ESEF_V2_DIR` names a readable corpus;
/// `BRAWLER_ESEF_REQUIRED=1` turns the skip into a panic (the `make
/// realdata-esef-check` closure gate). The only place this crate reads
/// `scripts/realdata/esef-v2/gt_key_map.json` — every hermetic test below
/// uses the crate-local copy instead (amendment 18), so this `#[ignore]`d
/// test carries the cross-tree read alone (exempt from the source-tree guard
/// by its own `#[ignore]`).
#[test]
#[ignore = "real-data validation; needs BRAWLER_ESEF_V2_DIR (a private labeled corpus)"]
fn esef_measurement_v2() {
    let required = std::env::var("BRAWLER_ESEF_REQUIRED").as_deref() == Ok("1");
    let dir = std::env::var("BRAWLER_ESEF_V2_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../private/realdata/spikes/esef-v2")
        });
    let key_map_path = std::env::var("BRAWLER_ESEF_KEY_MAP")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../scripts/realdata/esef-v2/gt_key_map.json")
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
        key_map_path: &key_map_path,
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

/// The crate-local key map path every hermetic test resolves against —
/// `testdata/esef-v2-sample/gt_key_map.json` lives inside `src-tauri/`, so
/// this is never a cross-tree read (amendment 18).
fn local_key_map_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/esef-v2-sample/gt_key_map.json")
}

fn sample_key_map() -> ResolvedKeyMap {
    let map = load_key_map(&local_key_map_path());
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
) -> ResolvedSlot {
    ResolvedSlot {
        slot_id: slot_id.to_owned(),
        package_member: None,
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
        normalized_by: Vec::new(),
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
/// current-period panel concept the consolidated instances tag MATCHes, the
/// standalone-only concept is scored, comparative FY2024 feeds
/// availability/Layer 1 capture (never primary recall), and the metrics JSON
/// carries no ticker/title/value/filename text.
#[test]
fn sample_corpus_scores_and_metrics_carry_no_content() {
    let dir = materialize_sample_corpus();
    let out = dir.join("metrics.json");
    let key_map_path = local_key_map_path();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: Some(&out),
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    // 16 current-period panel concepts x 2 floor events (floor_pl,
    // floor_package) — the FY2024 comparative rows and the standalone-only
    // CurrentLiabilities row are excluded from primary `gt_slots`/`matched`
    // (amendment H(a)) even though they still exist in ground_truth_v2.json.
    assert_eq!(aggregates.floor_events, 2);
    // floor_pl: 16 current-period consolidated concepts. floor_package: the
    // SAME 16 consolidated-member concepts PLUS the standalone member's
    // CurrentLiabilities row — its period matches the event's own labeled
    // period, so it is current-period too (just a different basis), and
    // stays MISSING (paired as a BASIS_MISMATCH prediction, test 4).
    assert_eq!(aggregates.gt_slots, 16 + 17);
    assert_eq!(aggregates.matched, 16 + 16);
    assert_eq!(aggregates.zero_output_events, 0);
    // Amendment T: the contradicted-basis Liabilities/unknown slot is
    // UNVERIFIED, pinned in its own count — and, amendment O, it never steals
    // the real Liabilities/consolidated slot's exact match above (`matched`
    // is unaffected by its presence).
    assert_eq!(aggregates.unverified, 1);
    // Comparative FY2024 numbers are available in Layer 1 (raw capture) even
    // though production never writes a comparative-period fact.
    assert!(aggregates.layer1_capture.captured >= 3);
    assert!(aggregates.availability_all_periods.eligible >= aggregates.gt_slots);

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

/// Test 1b (amendment P, r2 f20) — a REAL, freshly-measured sample artifact
/// must validate against the ratchet's own JSON schema
/// (`scripts/check/realdata-ratchet.mjs`'s `ESEF_METRICS_SCHEMA`) — the
/// producer/schema field-name drift the r2 diff review caught (`eligible` vs
/// `labeled`) can only be caught by actually running the real validator
/// against real output, never by re-typing the schema's shape by hand in
/// Rust. Skips (never fails) inside the cargo-mutants scratch sandbox, which
/// copies only `src-tauri/` and has no `scripts/`/node runtime to reach.
#[test]
fn sample_corpus_metrics_validate_against_the_ratchet_schema() {
    if crate::source_tree_guards::is_crate_only_sandbox() {
        eprintln!("SKIP sample_corpus_metrics_validate_against_the_ratchet_schema: cargo-mutants sandbox has no scripts/ tree");
        return;
    }

    let dir = materialize_sample_corpus();
    let out = dir.join("metrics.json");
    let key_map_path = local_key_map_path();
    run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: Some(&out),
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    // cross-tree-read-ok: amendment P — validates real output against the cross-language ratchet contract; the mutants sweep is exempted above via `is_crate_only_sandbox`.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let ratchet_script = repo_root.join("scripts/check/realdata-ratchet.mjs");
    let committed_baseline =
        repo_root.join("src-tauri/testdata/esef-v2-sample/baseline/realdata-esef-baseline.json");

    let output = std::process::Command::new("node")
        .arg(&ratchet_script)
        .arg("--profile")
        .arg("esef")
        .arg("--baseline")
        .arg(&committed_baseline)
        .arg("--metrics")
        .arg(&out)
        .output()
        .expect("run node against the real ratchet script");
    assert!(
        output.status.success(),
        "the ratchet must accept a real, freshly-measured sample artifact against the committed baseline:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test 2 — the language twins are two independent events whose per-slot
/// outcomes agree wherever they share a semantic identity: the loose-file
/// twin (`twin_en`, 16 concepts) PLUS the within-package twin (amendment S —
/// two EN-tagged concepts living inside `floor_package` itself, 18 total).
#[test]
fn language_twins_are_two_events_with_agreement() {
    let dir = materialize_sample_corpus();
    let key_map_path = local_key_map_path();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    assert_eq!(aggregates.twin_agreement.compared, 16 + 2);
    assert_eq!(
        aggregates.twin_agreement.agree,
        aggregates.twin_agreement.compared
    );
}

/// Test 2e (amendment S) — the within-package twin population is excluded
/// from floor denominators: `floor_package`'s pinned-language (pl) slots
/// alone drive `gt_slots`/`matched` (17, per test 1) even though the package
/// also carries 2 EN-tagged internal-twin slots — those never widen the
/// denominator, they only feed `twin_agreement` (test 2's `+2`).
#[test]
fn within_package_twin_slots_never_widen_floor_denominators() {
    let dir = materialize_sample_corpus();
    let key_map_path = local_key_map_path();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    // floor_pl (16) + floor_package (16 consolidated + 1 standalone) = 33 —
    // NOT 35, which is what it would be if the 2 EN internal-twin slots had
    // leaked into the floor population.
    assert_eq!(aggregates.gt_slots, 33);
    // But they DO count toward labeled-capability's eligible denominator
    // (amendment Q): 36 (floor, all periods) + 2 (internal twin) = 38.
    assert_eq!(aggregates.labeled_capability.eligible, 38);
}

/// Test 2b (fix wave 2, finding 1) — prediction identity keys are STABLE
/// across two independent runs of the same corpus: a fresh cold-start DB
/// mints a new `fact_finper_…` row id every run, so if the report were still
/// keyed by that id (the bug this fix corrects), the two runs' key SETS would
/// never agree. They must, because the key is built entirely from the slot's
/// own identity dimensions.
#[test]
fn prediction_identity_keys_are_stable_across_two_runs() {
    let key_map_path = local_key_map_path();

    let dir_a = materialize_sample_corpus();
    run_measurement(MeasurementConfig {
        corpus_dir: &dir_a,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("first run must measure");
    let report_a: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir_a.join("scoring-report-v2.json")).unwrap(),
    )
    .unwrap();
    let keys_a: BTreeSet<String> = report_a["prediction_outcomes"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();

    let dir_b = materialize_sample_corpus();
    run_measurement(MeasurementConfig {
        corpus_dir: &dir_b,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("second run must measure");
    let report_b: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir_b.join("scoring-report-v2.json")).unwrap(),
    )
    .unwrap();
    let keys_b: BTreeSet<String> = report_b["prediction_outcomes"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();

    assert!(
        !keys_a.is_empty(),
        "the sample corpus must produce at least one prediction"
    );
    assert_eq!(
        keys_a, keys_b,
        "prediction identity keys must be identical across two fresh-DB runs of the same corpus"
    );
    // Never a DB-generated fact id (the bug this fix corrects) — every key
    // must carry the identity shape instead.
    for key in &keys_a {
        assert!(
            !key.contains("fact_finper"),
            "a prediction key must never be a DB fact id: {key}"
        );
    }
}

/// Test 2c (fix wave 2, finding 2) — `scoring-report-v2.json` carries a
/// per-event section with production's own recorded acceptance/reason,
/// derived-vs-labeled period, and per-event outcome counts.
#[test]
fn per_event_section_is_present_in_the_report() {
    let dir = materialize_sample_corpus();
    let key_map_path = local_key_map_path();
    run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("scoring-report-v2.json")).unwrap())
            .unwrap();
    let events = report["events"]
        .as_array()
        .expect("events must be an array");
    // floor_pl, twin_en, floor_package — every cold-started (non-warmup) event.
    assert_eq!(events.len(), 3);

    let floor_pl = events
        .iter()
        .find(|e| e["event_id"] == "iss_01/FY2025/pl/consolidated/v1")
        .expect("floor_pl event must be present");
    assert_eq!(floor_pl["role"], "floor");
    assert_eq!(floor_pl["issuer_id"], "iss_01");
    assert_eq!(floor_pl["labeled_period"], "2025/FY/2025-12-31");
    assert_eq!(floor_pl["derived_period"], "2025/FY/2025-12-31");
    assert_eq!(floor_pl["acceptance"], "accepted");
    assert_eq!(floor_pl["matched"], 16);
    assert_eq!(floor_pl["missing"], 0);
    assert!(floor_pl["prediction_count"].as_u64().unwrap() > 0);
    assert!(floor_pl["run_error"].is_null());
}

/// Test 2d (fix wave 2, finding 2) — the per-issuer console table's own
/// aggregation ([`build_issuer_counts`]) pools floor events correctly: two
/// events for the same issuer sum their counts, a twin event is excluded, and
/// mismatch classes/unverified/wrong are tallied separately.
#[test]
fn per_issuer_counts_pool_floor_events_only() {
    let mut mismatch_classes_a = BTreeMap::new();
    mismatch_classes_a.insert("WRONG_VALUE".to_owned(), 1);
    mismatch_classes_a.insert("BASIS_MISMATCH".to_owned(), 2);
    let event_a = EventReport {
        event_id: "e1".to_owned(),
        role: "floor".to_owned(),
        issuer_id: "iss_1".to_owned(),
        labeled_period: "2025/FY/2025-12-31".to_owned(),
        derived_period: "2025/FY/2025-12-31".to_owned(),
        acceptance: Some("accepted".to_owned()),
        reason_code: Some("emitted".to_owned()),
        prediction_count: 10,
        matched: 7,
        missing: 3,
        false_positives: 1,
        mismatch_classes: mismatch_classes_a,
        run_error: None,
        unverified: 2,
    };
    let event_twin = EventReport {
        event_id: "e2".to_owned(),
        role: "twin_diagnostic".to_owned(),
        issuer_id: "iss_1".to_owned(),
        ..event_a.clone()
    };
    let event_b = EventReport {
        event_id: "e3".to_owned(),
        role: "floor".to_owned(),
        issuer_id: "iss_1".to_owned(),
        matched: 5,
        missing: 1,
        false_positives: 0,
        mismatch_classes: BTreeMap::new(),
        unverified: 0,
        ..event_a.clone()
    };
    let event_other_issuer = EventReport {
        event_id: "e4".to_owned(),
        issuer_id: "iss_2".to_owned(),
        ..event_b.clone()
    };

    let events = [event_a, event_twin, event_b, event_other_issuer];
    let by_issuer = build_issuer_counts(&events);

    assert_eq!(
        by_issuer.len(),
        2,
        "twin_diagnostic contributes no new issuer row on its own"
    );
    let iss1 = &by_issuer["iss_1"];
    assert_eq!(
        iss1.events, 2,
        "the twin event must be excluded from the floor pool"
    );
    assert_eq!(iss1.gt_slots, (7 + 3) + (5 + 1));
    assert_eq!(iss1.matched, 7 + 5);
    assert_eq!(iss1.missing, 3 + 1);
    assert_eq!(iss1.wrong, 1);
    assert_eq!(iss1.false_positives, 1);
    assert_eq!(iss1.unverified, 2);
    assert_eq!(iss1.mismatch_classes.get("BASIS_MISMATCH"), Some(&2));
    assert_eq!(
        iss1.mismatch_classes.get("WRONG_VALUE"),
        None,
        "WRONG_VALUE is its own `wrong` column, not a mismatch class"
    );

    let iss2 = &by_issuer["iss_2"];
    assert_eq!(iss2.events, 1);
}

/// Test 3 — `owners_of_parent` GT concepts map to their dedicated `wdf_*`
/// metric keys via the semantic `(concept, attribution)` pair, never to the
/// plain total key (pure matcher, no DB).
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
    assert_eq!(score.prediction_outcomes["p_parent"], Outcome::Match);
    assert_eq!(score.prediction_outcomes["p_total"], Outcome::Match);
}

/// Test 3b (finding 2) — a GT `(concept, attribution)` pair with NO key-map
/// entry (`ProfitLoss`/`nci`; the map lists `ProfitLoss` only under `total`)
/// is labeled-capability only: it never enters `gt_slots`/`matched`, and
/// critically never matches the perfectly-good `net_profit`/`total`
/// prediction sitting right next to it for the same concept.
#[test]
fn unsupported_semantic_pair_never_matches_a_total_prediction() {
    let dir = unique_temp_dir("esef-v2-unsupported-pair");
    std::fs::create_dir_all(dir.join("corpus")).expect("dir");
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="d"><xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:ProfitLoss" contextRef="d" unitRef="pln" scale="3">90 000</ix:nonFraction>
    </html>"#;
    std::fs::write(dir.join("corpus/profit.xhtml"), instance).expect("write");
    write_manifest_and_gt(
        &dir,
        "profit.xhtml",
        "application/xhtml+xml",
        2025,
        "FY",
        "2025-12-31",
        &[
            GtSlotSpec {
                concept: "ProfitLoss",
                attribution: "total",
                value: "90000000",
            },
            GtSlotSpec {
                concept: "ProfitLoss",
                attribution: "nci",
                value: "90000000",
            },
        ],
    );

    let key_map_path = local_key_map_path();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("tiny corpus must measure");

    // Only the mapped (ProfitLoss, total) row counts toward gt_slots/matched;
    // the unmapped (ProfitLoss, nci) row is labeled-capability only.
    assert_eq!(aggregates.gt_slots, 1);
    assert_eq!(aggregates.matched, 1);
    assert_eq!(aggregates.labeled_capability.eligible, 2);
    assert_eq!(aggregates.labeled_capability.matched, 1);
}

/// Test 4 — the standalone package member's exclusive concept
/// (`CurrentLiabilities`) is written with `statement_basis='consolidated'`
/// (ADR 0095 default, every structured write) while GT expects `standalone`:
/// a real, honest `BASIS_MISMATCH` on the PREDICTION side, while the GT slot
/// itself resolves to `MISSING` (never a mismatch label — only MATCH/MISSING
/// ever appear in `slot_outcomes`).
#[test]
fn basis_mismatch_counts_against_precision() {
    let dir = materialize_sample_corpus();
    let out = dir.join("metrics.json");
    let key_map_path = local_key_map_path();
    run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: Some(&out),
        keyed_baseline: None,
        required: false,
    })
    .expect("sample corpus must measure");

    let report_raw =
        std::fs::read_to_string(dir.join("scoring-report-v2.json")).expect("scoring report");
    let report: serde_json::Value = serde_json::from_str(&report_raw).expect("scoring report json");
    let predictions = report["prediction_outcomes"]
        .as_object()
        .expect("predictions object");
    assert!(
        predictions.values().any(|v| v == "BASIS_MISMATCH"),
        "the standalone-only prediction (stamped consolidated by production) must be paired as BASIS_MISMATCH: {predictions:?}"
    );
    // The comparative-scope standalone slot never appears in slot_outcomes at
    // all now (it is not current-period for its event's own labeled period —
    // it shares the SAME period_end/fiscal_year/period_type as the event, so
    // it actually IS current-period; assert it resolves to MISSING, never a
    // mismatch label).
    let slot_outcomes = report["slot_outcomes"]
        .as_object()
        .expect("slot outcomes object");
    let standalone_entry = slot_outcomes
        .iter()
        .find(|(id, _)| id.contains("CurrentLiabilities"));
    if let Some((_, outcome)) = standalone_entry {
        assert_eq!(outcome, "MISSING");
    }
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

/// Test 5b (finding 1) — the specific derivation defect this measurement
/// exists to expose: a filing labeled H1 whose facts production derived (and
/// stored) as FY, under the SAME period-end date. A plain date compare would
/// hide this as a MATCH; the full-fiscal-key exact match plus the new
/// `PERIOD_TYPE_MISMATCH` class must surface it instead.
#[test]
fn h1_labeled_slot_never_matches_a_same_date_fy_prediction() {
    let panel: BTreeSet<String> = sample_key_map().panel;
    let gt_h1 = slot(
        "h1",
        "revenue",
        2025,
        "H1",
        "2025-06-30",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "250000000",
    );
    // Same metric, same fiscal YEAR, same period_end, same value — but the
    // WRONG period_type, exactly the defect under test.
    let pred_fy = prediction(
        "p_fy",
        "revenue",
        2025,
        "FY",
        "2025-06-30",
        "consolidated",
        "total",
        "reported",
        "flow",
        Some("PLN"),
        "250000000",
    );

    let score = score_event(&[gt_h1], &[pred_fy], &panel);
    assert_eq!(
        score.slot_outcomes["h1"],
        Outcome::Missing,
        "the full fiscal key must reject this as an exact match"
    );
    assert_eq!(
        score.prediction_outcomes["p_fy"],
        Outcome::PeriodTypeMismatch
    );
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

/// Test 7 (rewritten per finding 3 — the old assertion, `FALSE_POSITIVE`, was
/// the bug) — a GT slot marked `unverified` leaves EVERY denominator: it
/// never appears in `slot_outcomes` at all (not even `MISSING`), and its
/// attributable prediction is `OUT_OF_SCOPE`, never `FALSE_POSITIVE`.
#[test]
fn duplicate_conflict_is_unverified_and_leaves_every_denominator() {
    let dir = unique_temp_dir("esef-v2-unverified");
    std::fs::create_dir_all(dir.join("corpus")).expect("dir");
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="d"><xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Revenue" contextRef="d" unitRef="pln" scale="3">500 000</ix:nonFraction>
    </html>"#;
    std::fs::write(dir.join("corpus/revenue.xhtml"), instance).expect("write");
    write_manifest_and_gt_with_verification(
        &dir,
        "revenue.xhtml",
        "application/xhtml+xml",
        2025,
        "FY",
        "2025-12-31",
        &[GtSlotSpecVerified {
            concept: "Revenue",
            attribution: "total",
            value: "500000000",
            verification: "unverified",
            basis: "consolidated",
        }],
    );

    let key_map_path = local_key_map_path();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("tiny corpus must measure");

    assert_eq!(
        aggregates.gt_slots, 0,
        "an unverified slot leaves the recall denominator entirely"
    );
    assert_eq!(aggregates.matched, 0);
    assert_eq!(aggregates.unverified, 1);
    assert_eq!(
        aggregates.false_positives, 0,
        "the attributable prediction must never be FALSE_POSITIVE"
    );

    let report_raw =
        std::fs::read_to_string(dir.join("scoring-report-v2.json")).expect("scoring report");
    let report: serde_json::Value = serde_json::from_str(&report_raw).expect("scoring report json");
    let predictions = report["prediction_outcomes"]
        .as_object()
        .expect("predictions object");
    assert!(
        predictions.values().any(|v| v == "OUT_OF_SCOPE"),
        "the revenue prediction attributable to the unverified slot must be OUT_OF_SCOPE: {predictions:?}"
    );
}

/// Test 7b (amendment O, r2 f3) — the specific regression the diff review
/// caught: an unverified slot sharing metric+fiscal_year with an ELIGIBLE
/// slot must NEVER steal the eligible slot's exact match, even when a
/// (buggy) implementation would attribute ineligible slots BEFORE eligible
/// scoring runs. Proven at the pure-matcher level: eligible scoring claims
/// the one real prediction first; the ineligible-attribution pass then sees
/// an EMPTY remaining pool and claims nothing.
#[test]
fn unverified_slot_never_steals_an_exact_eligible_match() {
    let panel: BTreeSet<String> = sample_key_map().panel;
    let eligible_fy = slot(
        "eligible",
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
    );
    // The would-be unverified slot: SAME metric_key + fiscal_year (the
    // narrow identity a buggy implementation used to match on), but the
    // production pipeline only ever produced ONE prediction for this event.
    let unverified_h1 = slot(
        "unverified",
        "revenue",
        2025,
        "H1",
        "2025-06-30",
        "consolidated",
        "total",
        "reported",
        "flow",
        "PLN",
        "500000000",
    );
    let only_prediction = prediction(
        "p1",
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

    // The fixed order: eligible slots score first, against ALL predictions.
    let score = score_event(
        &[eligible_fy],
        std::slice::from_ref(&only_prediction),
        &panel,
    );
    assert_eq!(score.slot_outcomes["eligible"], Outcome::Match);
    assert_eq!(score.prediction_outcomes["p1"], Outcome::Match);

    // Only what eligible scoring left as FALSE_POSITIVE may be offered to
    // ineligible attribution — here, nothing (the pool is empty).
    let mut still_unmatched: Vec<Prediction> = vec![only_prediction]
        .into_iter()
        .filter(|p| score.prediction_outcomes.get(&p.id).copied() == Some(Outcome::FalsePositive))
        .collect();
    let claimed =
        attribute_ineligible_predictions(&[&unverified_h1], &mut still_unmatched, "unverified");
    assert!(
        claimed.is_empty(),
        "the unverified slot must not claim a prediction eligible scoring already matched: {claimed:?}"
    );
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
        &[GtSlotSpec {
            concept: "Revenue",
            attribution: "total",
            value: "500000000",
        }],
    );

    let key_map_path = local_key_map_path();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
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
        &[GtSlotSpec {
            concept: "Assets",
            attribution: "total",
            value: "100000000",
        }],
    );

    let key_map_path = local_key_map_path();
    let aggregates = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
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
/// decreases and the altered slot's prediction resolves to `WRONG_VALUE`.
#[test]
fn freshness_altered_bytes_change_the_outcome() {
    let dir = materialize_sample_corpus();
    let key_map_path = local_key_map_path();
    let baseline = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
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
    // The manifest's registry_hash pins the ORIGINAL corpus file bytes
    // (amendment G) — altering a file without re-pinning it must now be a
    // typed "incomparable" failure, which is itself proof the guard works.
    // This test intentionally exercises the OTHER path: assert the panic.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_measurement(MeasurementConfig {
            corpus_dir: &dir,
            key_map_path: &key_map_path,
            metrics_out: None,
            keyed_baseline: None,
            required: false,
        })
    }));
    assert!(
        result.is_err(),
        "amendment G: altering a corpus file without updating MANIFEST_v2.json's registry_hash must be a typed failure, not a silent re-measure"
    );

    // Re-pin the registry hash to the altered bytes (as a real relabeling
    // pass would) so the run can proceed and the freshness assertion (matched
    // decreases, WRONG_VALUE appears) still gets exercised end to end.
    repin_registry_hash(&dir);
    let after = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    })
    .expect("altered-and-repinned corpus must still measure");

    assert!(
        after.matched < baseline.matched,
        "altering a value through the real job must lower matched: before={} after={}",
        baseline.matched,
        after.matched
    );

    let report_raw =
        std::fs::read_to_string(dir.join("scoring-report-v2.json")).expect("scoring report");
    let report: serde_json::Value = serde_json::from_str(&report_raw).expect("scoring report json");
    let predictions = report["prediction_outcomes"]
        .as_object()
        .expect("predictions object");
    assert!(
        predictions.values().any(|v| v == "WRONG_VALUE"),
        "the altered revenue value must surface as a WRONG_VALUE prediction: {predictions:?}"
    );
}

/// Recomputes and rewrites `MANIFEST_v2.json`'s `registry_hash` from the
/// corpus files currently on disk at `dir` — test-only, mirrors what the
/// (python-side) builder does after a genuine relabeling pass.
fn repin_registry_hash(dir: &Path) {
    let manifest_path = dir.join("MANIFEST_v2.json");
    let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let manifest: Manifest = serde_json::from_str(&raw).expect("parse manifest");
    let fresh_hash = recompute_registry_hash(dir, &manifest);
    let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse manifest as json");
    value["registry_hash"] = serde_json::Value::String(fresh_hash);
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .expect("rewrite manifest");
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

/// Test 11b (finding 4) — a present-but-malformed keyed baseline is a typed
/// failure, never silently treated as "no baseline".
#[test]
#[should_panic(expected = "malformed")]
fn malformed_keyed_baseline_is_a_typed_failure() {
    let dir = unique_temp_dir("esef-v2-malformed-baseline");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("keyed-baseline.json");
    std::fs::write(&path, "{ not json").expect("write malformed baseline");
    let _ = load_keyed_baseline(Some(&path), "1", 1, 1, "irrelevant", false);
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
    let map = load_key_map(&local_key_map_path());
    let _ = resolve_key_map(&map, map.key_map_version + 1);
}

/// Test 12b — a ground-truth slot missing a required field is a typed panic
/// naming the field (serde's own message), never a silent skip.
#[test]
#[should_panic(expected = "missing field")]
fn malformed_ground_truth_field_is_a_typed_panic() {
    serde_json::from_str::<GtSlot>(r#"{"slot_id":"s1"}"#).expect("must fail: missing fields");
}

/// Test 12c (finding 3) — a ground-truth slot whose `event_id` names no
/// manifest event is a typed "incomparable" failure, never silently unscored.
#[test]
#[should_panic(expected = "incomparable")]
fn orphan_ground_truth_event_id_is_a_typed_failure() {
    let dir = unique_temp_dir("esef-v2-orphan-event");
    std::fs::create_dir_all(dir.join("corpus")).expect("dir");
    std::fs::write(dir.join("corpus/x.xhtml"), b"<html></html>").expect("write");
    write_manifest_and_gt(
        &dir,
        "x.xhtml",
        "application/xhtml+xml",
        2025,
        "FY",
        "2025-12-31",
        &[],
    );
    // Append a slot referencing an event_id that does not exist.
    let gt_path = dir.join("ground_truth_v2.json");
    let mut gt: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&gt_path).unwrap()).unwrap();
    gt["slots"].as_array_mut().unwrap().push(serde_json::json!({
        "slot_id": "orphan/-/Revenue/total/consolidated/flow/reported/2025/FY/PLN",
        "event_id": "no/such/event",
        "package_member": null,
        "concept_local": "Revenue",
        "attribution": "total",
        "basis": "consolidated",
        "window": "flow",
        "variant": "reported",
        "fiscal_year": 2025,
        "period_type": "FY",
        "period_end": "2025-12-31",
        "currency": "PLN",
        "value": "1",
        "duration_months": null,
        "verification": "machine",
        "language": "pl",
        "mapped": true,
        "normalized_by": [],
        "contributing_occurrence_ids": [],
        "resolution_ref": null
    }));
    std::fs::write(&gt_path, serde_json::to_string_pretty(&gt).unwrap()).unwrap();

    let key_map_path = local_key_map_path();
    let _ = run_measurement(MeasurementConfig {
        corpus_dir: &dir,
        key_map_path: &key_map_path,
        metrics_out: None,
        keyed_baseline: None,
        required: false,
    });
}

/// Test 13 — required mode panics without a corpus (through
/// [`MeasurementConfig`], never process env — this test stays env-free).
#[test]
fn required_mode_panics_without_corpus() {
    let missing = unique_temp_dir("esef-v2-missing").join("does-not-exist");
    let key_map_path = local_key_map_path();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_measurement(MeasurementConfig {
            corpus_dir: &missing,
            key_map_path: &key_map_path,
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

/// The window a concept's stored fact actually carries
/// (`storage::financials::resolve_measure_window`'s `period_nature` ->
/// `measure_window` mapping — instant -> point_in_time, else flow), so the
/// tiny test fixtures below build a GT slot the real prediction can actually
/// exact-match on this dimension instead of hardcoding one value for every
/// concept regardless of nature.
fn concept_window(concept: &str) -> &'static str {
    match concept {
        "Assets"
        | "CurrentAssets"
        | "Liabilities"
        | "CurrentLiabilities"
        | "Equity"
        | "EquityAttributableToOwnersOfParent"
        | "CashAndCashEquivalents" => "point_in_time",
        _ => "flow",
    }
}

struct GtSlotSpec {
    concept: &'static str,
    attribution: &'static str,
    value: &'static str,
}

struct GtSlotSpecVerified {
    concept: &'static str,
    attribution: &'static str,
    value: &'static str,
    verification: &'static str,
    basis: &'static str,
}

/// Builds a minimal one-event manifest + ground truth in `dir` (tests 3b, 8,
/// 9, 12c) — the small, single-purpose sibling of [`materialize_sample_corpus`]
/// for scenarios that need a corpus shape the committed sample does not
/// carry. `registry_hash` is computed for real (amendment G) so these tiny
/// fixtures pass the same integrity check the sample corpus does.
fn write_manifest_and_gt(
    dir: &Path,
    file_name: &str,
    content_type: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    slots: &[GtSlotSpec],
) {
    let verified: Vec<GtSlotSpecVerified> = slots
        .iter()
        .map(|s| GtSlotSpecVerified {
            concept: s.concept,
            attribution: s.attribution,
            value: s.value,
            verification: "machine",
            basis: "consolidated",
        })
        .collect();
    write_manifest_and_gt_with_verification(
        dir,
        file_name,
        content_type,
        fiscal_year,
        period_type,
        period_end,
        &verified,
    );
}

#[allow(clippy::too_many_arguments)]
fn write_manifest_and_gt_with_verification(
    dir: &Path,
    file_name: &str,
    content_type: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    slots: &[GtSlotSpecVerified],
) {
    let bytes = std::fs::read(dir.join("corpus").join(file_name))
        .expect("read the just-written corpus file");
    let file_hash = content_hash_hex(&bytes);
    let registry_hash = content_hash_hex(file_hash.as_bytes());

    let event_id = "iss_t/FY/ev/v1";
    let manifest = serde_json::json!({
        "manifest_version": 1,
        "snapshot": { "source": "test", "taken_at": "2026-09-12" },
        "registry_hash": registry_hash,
        "issuers": [
            { "issuer_id": "iss_t", "ticker": "TST", "exchange": "GPW", "display_name": "Test S.A." }
        ],
        "events": [
            {
                "event_id": event_id,
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
                "file": { "name": file_name, "sha256": file_hash, "bytes": bytes.len(), "package_member": null },
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
        .map(|s| {
            let window = concept_window(s.concept);
            serde_json::json!({
                "slot_id": format!("{event_id}/-/{}/{}/{}/{window}/reported/{fiscal_year}/{period_type}/PLN", s.concept, s.attribution, s.basis),
                "event_id": event_id,
                "package_member": null,
                "concept_local": s.concept,
                "attribution": s.attribution,
                "basis": s.basis,
                "window": window,
                "variant": "reported",
                "fiscal_year": fiscal_year,
                "period_type": period_type,
                "period_end": period_end,
                "period_start": null,
                "currency": "PLN",
                "value": s.value,
                "duration_months": null,
                "verification": s.verification,
                "language": "pl",
                "mapped": true,
                "normalized_by": [],
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
