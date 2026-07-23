//! Typed severity taxonomy for the attention/stream surfaces (ADR 0087 dec. 2).
//!
//! THE single place severity is decided. Severity is **derived** from a
//! `trigger_type` + signal category (attention events) or a run status (autopilot
//! runs) and shipped as a typed value on the read models — never stored, never
//! re-inferred from strings by the frontend. Three levels:
//!
//! - [`AttentionSeverity::Urgent`] — leads the stream; the only level that may
//!   raise a persistent toast (insider transaction, profit warning, auditor
//!   opinion, a missed-report reconciliation).
//! - [`AttentionSeverity::Notable`] — stream + a transient toast at most (an
//!   autopilot failure, a fired price alert, most disclosures).
//! - [`AttentionSeverity::Routine`] — stream only, dimmed (a successful autopilot
//!   run, an `other`-category filing).
//!
//! The authoritative table lives in product-spec §Attention Routing; the code
//! below mirrors it exactly. **Adding a trigger type or a signal category without
//! classifying its severity here is a gate failure** (`storage::tests::severity`,
//! same posture as the MCP registry classification gate, ADR 0088): the gate
//! derives the real trigger/category inventory from the source of truth
//! (`attention::TRIGGER_TYPES` + the system trigger, and the seeded
//! `signal_categories` registry) and reddens on any unclassified member.

use serde::{Deserialize, Serialize};

use super::attention::{
    TRIGGER_AUTOPILOT_RUN_COMPLETED, TRIGGER_PRICE_ENTERS_RANGE, TRIGGER_PRICE_WEEK52_LOW,
    TRIGGER_SIGNAL_CATEGORY, TRIGGER_SOURCE_RECONCILIATION,
};

/// The typed severity of an attention/stream item (ADR 0087 dec. 2). Serialized
/// camelCase (`"urgent"` | `"notable"` | `"routine"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub enum AttentionSeverity {
    /// Leads the stream; the only level that may raise a persistent toast.
    Urgent,
    /// Stream + a transient toast at most.
    Notable,
    /// Stream only, dimmed.
    Routine,
}

/// Severity of an attention event, from its `trigger_type` and (for a
/// `signal_category` event) the signal's category. The one mapping the read
/// models call. An unrecognized trigger is treated as [`AttentionSeverity::Notable`]
/// (never silently routine) — the gate keeps that branch unreachable for a real
/// trigger.
pub fn severity_for_attention_event(
    trigger_type: &str,
    signal_category: Option<&str>,
) -> AttentionSeverity {
    if trigger_type == TRIGGER_SIGNAL_CATEGORY {
        return severity_for_signal_category(signal_category);
    }
    severity_for_trigger(trigger_type).unwrap_or(AttentionSeverity::Notable)
}

/// An `urgent` attention event that has gone UNACTED past this age stops shouting.
/// 72h (= 3 days) after `fired_at`. Named per the [ADR 0087] amendment (2026-07-23
/// live-checkpoint finding): a week-old missed-report reconciliation must not keep
/// leading the stream as a fresh alarm.
///
/// [ADR 0087]: ../../../docs/adr/0087-today-attention-home-v2.md
pub const URGENT_AGING_THRESHOLD: time::Duration = time::Duration::hours(72);

/// Age-demote a severity at read time (ADR 0087 dec. 2 amendment): an `urgent`
/// event whose `fired_at` is **strictly older** than [`URGENT_AGING_THRESHOLD`]
/// before `now` demotes to [`AttentionSeverity::Notable`] — nothing is hidden or
/// auto-dismissed, it just stops leading the stream. The rule is **purely
/// age-based**: seen/dismissed state never enters it. A non-urgent severity passes
/// through unchanged, as does an `urgent` whose `fired_at` cannot be parsed (we
/// never demote what we cannot prove is old). At **exactly** 72h the event stays
/// urgent — the boundary belongs to urgent. This is the ONE place the aging rule
/// lives; read models call it right after the base [`severity_for_attention_event`]
/// mapping.
pub fn aged_attention_severity(
    base: AttentionSeverity,
    fired_at: &str,
    now: time::OffsetDateTime,
) -> AttentionSeverity {
    if base != AttentionSeverity::Urgent {
        return base;
    }
    let Ok(fired) = time::OffsetDateTime::parse(
        fired_at.trim(),
        &time::format_description::well_known::Rfc3339,
    ) else {
        return base;
    };
    if now - fired > URGENT_AGING_THRESHOLD {
        AttentionSeverity::Notable
    } else {
        base
    }
}

/// Severity of a `signal_category` attention event. An unclassified or absent
/// category falls back to [`AttentionSeverity::Notable`] (never silently routine);
/// the gate forbids a *known* seeded category from reaching that fallback.
pub fn severity_for_signal_category(category: Option<&str>) -> AttentionSeverity {
    category
        .and_then(classify_signal_category)
        .unwrap_or(AttentionSeverity::Notable)
}

/// Explicit classification of a single signal category, or `None` when the
/// category is not in the mapping. The severity gate asserts this is `Some` for
/// every seeded `signal_categories.key`, so a new category reddens until it is
/// classified here.
pub(crate) fn classify_signal_category(category: &str) -> Option<AttentionSeverity> {
    Some(match category {
        // Act-now disclosures — the only categories that may raise a persistent
        // toast (ADR 0087 dec. 2): a management/insider trade, an earnings
        // warning, or a qualified/going-concern audit opinion.
        "insider_transaction" | "profit_warning" | "auditor_opinion" => AttentionSeverity::Urgent,
        // Plainly informational: a filing the classifier placed in the catch-all
        // `other` bucket carries no specific signal — stream-only, dimmed.
        "other" => AttentionSeverity::Routine,
        // Worth-a-look disclosures — stream + a transient toast at most. Corporate
        // actions (dividend, own-shares/buyback, general meeting), material events
        // (significant contract, guidance change), ownership/short-interest moves
        // (major-holdings change, short-position change, fund exit), analyst moves
        // (recommendation change), and derived red flags (report delay, score
        // deterioration).
        "dividend"
        | "own_shares"
        | "general_meeting"
        | "significant_contract"
        | "guidance_change"
        | "major_holdings_change"
        | "short_position_change"
        | "fund_exit"
        | "recommendation_change"
        | "report_delay"
        | "score_deterioration" => AttentionSeverity::Notable,
        _ => return None,
    })
}

/// Explicit classification of a non-`signal_category` trigger, or `None` for an
/// unrecognized trigger. The gate asserts this is `Some` for every real trigger
/// other than `signal_category` (whose severity is category-dependent), so a new
/// trigger reddens until it has an arm here.
pub(crate) fn severity_for_trigger(trigger_type: &str) -> Option<AttentionSeverity> {
    Some(match trigger_type {
        // A missed-report reconciliation: the primary channel missed an official
        // report the witness saw — act now.
        TRIGGER_SOURCE_RECONCILIATION => AttentionSeverity::Urgent,
        // A fired price alert (entered a set band / new 52-week low): worth a look.
        TRIGGER_PRICE_ENTERS_RANGE | TRIGGER_PRICE_WEEK52_LOW => AttentionSeverity::Notable,
        // An autopilot run completed: notable at the event level (the run's own
        // outcome severity is `severity_for_autopilot_run`).
        TRIGGER_AUTOPILOT_RUN_COMPLETED => AttentionSeverity::Notable,
        _ => return None,
    })
}

/// Severity of an autopilot RUN from its status (ADR 0087 dec. 2). A run that did
/// not fully succeed (`failed` | `partial`) is [`AttentionSeverity::Notable`]; a
/// `succeeded`/`pending`/`running` run is [`AttentionSeverity::Routine`].
pub fn severity_for_autopilot_run(status: &str) -> AttentionSeverity {
    match status {
        // Did not fully succeed — worth surfacing (a transient toast at most).
        "failed" | "partial" => AttentionSeverity::Notable,
        // In-flight or clean success — stream-only.
        _ => AttentionSeverity::Routine,
    }
}
