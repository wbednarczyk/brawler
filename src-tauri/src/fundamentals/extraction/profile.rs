//! Per-company PDF extraction profile + drift detection (ADR 0061, S3).
//!
//! A profile is the confirmed, versioned record of *how a given company lays
//! out its statements*: which normalized labels map to which `metric_key`, and
//! the unit scale its figures are printed in. It turns the tier-2 PDF parser
//! from a best-effort heuristic into a deterministic, self-checking reader:
//!
//! - On each new report, the parser's observed labels are compared to the
//!   profile. If they diverge — a renamed line, a new subtotal, a unit change —
//!   [`detect_drift`] reports a **clean label diff** (no raw-text noise). The
//!   pipeline distrusts the parse, falls to the HTML witness, and surfaces a
//!   "structure changed" notification built from that diff.
//! - When the fallback confirms the corrected reading, [`ExtractionProfile::
//!   merge_confirmed`] folds the new labels back in and bumps the version, so
//!   the next period parses cleanly with no human touch. Bootstrap once per
//!   company/template; converge to zero-touch thereafter.
//!
//! Pure and serializable: the profile is persisted as JSON by the storage layer
//! (wired in S5); this module owns only the model, the hash, and the diff.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::pdf::{PdfParse, UnitScale};

/// The confirmed extraction layout for one company.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionProfile {
    pub company_id: String,
    /// Stable fingerprint of the confirmed label set — identifies "same
    /// template" across periods.
    pub template_hash: String,
    pub unit_scale: UnitScale,
    /// Normalized label → `metric_key`.
    pub label_map: BTreeMap<String, String>,
    /// Monotonic version, bumped on every confirmed merge.
    pub version: i64,
}

impl ExtractionProfile {
    /// Builds the first profile for a company from a confirmed parse.
    pub fn bootstrap(company_id: &str, parse: &PdfParse) -> Self {
        let label_map = parse.label_metric_map();
        Self {
            company_id: company_id.to_string(),
            template_hash: template_hash(&label_map.keys().cloned().collect()),
            unit_scale: parse.unit_scale,
            label_map,
            version: 1,
        }
    }

    /// Folds a confirmed newer parse into the profile: observed labels win,
    /// prior labels are retained, the unit scale is refreshed, and the version
    /// is bumped. Used by the learning loop after a drift is resolved.
    pub fn merge_confirmed(&self, parse: &PdfParse) -> Self {
        let mut label_map = self.label_map.clone();
        for (label, metric) in parse.label_metric_map() {
            label_map.insert(label, metric);
        }
        Self {
            company_id: self.company_id.clone(),
            template_hash: template_hash(&label_map.keys().cloned().collect()),
            unit_scale: parse.unit_scale,
            label_map,
            version: self.version + 1,
        }
    }
}

/// A deterministic fingerprint of a label set (order-independent).
pub fn template_hash(labels: &BTreeSet<String>) -> String {
    let mut hasher = Sha256::new();
    for label in labels {
        hasher.update(label.as_bytes());
        hasher.update([0u8]); // separator so ["ab","c"] != ["a","bc"]
    }
    let digest = hasher.finalize();
    // First 16 hex chars are ample identity for a per-company template.
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The outcome of comparing an observed parse to a company profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Drift {
    /// The observed layout matches the profile — parse is trusted.
    None,
    /// The layout diverged; carries a clean, human-readable diff.
    Detected(DriftReport),
}

impl Drift {
    pub fn is_drift(&self) -> bool {
        matches!(self, Drift::Detected(_))
    }
}

/// A clean, presentable description of how a report's structure changed versus
/// the confirmed profile — the payload of the "structure changed" notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftReport {
    /// Labels present now but not in the profile (new/renamed lines).
    pub added_labels: Vec<String>,
    /// Profile labels no longer seen (removed/renamed lines).
    pub removed_labels: Vec<String>,
    /// `(profile_scale, observed_scale)` when the reporting unit changed.
    pub unit_changed: Option<(UnitScale, UnitScale)>,
}

/// Compares an observed parse against a company profile and reports any drift.
/// Only the *matched* labels are compared: unmapped prose lines never trigger
/// drift, so the signal stays clean and low-noise.
pub fn detect_drift(profile: &ExtractionProfile, parse: &PdfParse) -> Drift {
    let profile_labels: BTreeSet<&String> = profile.label_map.keys().collect();
    let observed_labels: BTreeSet<&String> = parse.matched_labels.iter().collect();

    let added_labels: Vec<String> = observed_labels
        .difference(&profile_labels)
        .map(|s| (*s).clone())
        .collect();
    let removed_labels: Vec<String> = profile_labels
        .difference(&observed_labels)
        .map(|s| (*s).clone())
        .collect();
    let unit_changed = if profile.unit_scale != parse.unit_scale {
        Some((profile.unit_scale, parse.unit_scale))
    } else {
        None
    };

    if added_labels.is_empty() && removed_labels.is_empty() && unit_changed.is_none() {
        Drift::None
    } else {
        Drift::Detected(DriftReport {
            added_labels,
            removed_labels,
            unit_changed,
        })
    }
}

#[cfg(test)]
mod tests;
