//! Per-company OCR-markdown extraction profile (ADR 0077 §4, tier-4).
//!
//! The tier-4 last-resort layer reads a report through Mistral OCR, which emits
//! per-page markdown (tables stitched in by the provider). A confirmed,
//! versioned [`OcrExtractionProfile`] records *how that company lays its OCR
//! tables out*: the normalized label → `metric_key` map, the reporting scale,
//! which value column carries the requested period, which header columns to
//! skip (e.g. a `Nota` reference column), and whether row labels carry a
//! leading enumerator.
//!
//! It is deliberately **separate** from the tier-2
//! [`super::super::profile::ExtractionProfile`]: OCR markdown has a different
//! template fingerprint and different drift semantics (a `Nota` column, a
//! value-column layout, an enumerator convention) than the tier-2 PDF-text
//! parser, so it gets its own table (migration 0063) and its own model rather
//! than muddying the tier-2 profile's `profile_json` versioning.
//!
//! Pure and serializable: the storage layer persists it as JSON; this module
//! owns only the model and its constructors. The G0 spike's verdict (ADR 0077
//! Evidence) is that the text-LLM *bootstraps* this profile once per company;
//! the deterministic [`super::parser::parse_ocr_markdown`] then reads every
//! subsequent document through it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::super::pdf::UnitScale;
use super::super::profile::template_hash;

/// Which column of an OCR-emitted statement table carries the value for the
/// requested reporting period. The four layouts the G0 spike observed across
/// issuers (ADR 0077 Evidence).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueColumnLayout {
    /// The current period is the first value column (the dominant Polish
    /// convention — current period printed leftmost, comparatives to its right).
    CurrentPeriodFirst,
    /// The current period is the last value column.
    CurrentPeriodLast,
    /// Columns are labeled by period; pick the value column whose header matches
    /// the requested period end (year or full date). No confident match → the
    /// table yields nothing (never a guessed column).
    LabeledByPeriodHeader,
    /// A single value column (no comparatives).
    SingleValue,
}

/// The confirmed, versioned OCR-markdown extraction layout for one company.
///
/// A company with no persisted profile has never been bootstrapped; tier-4
/// cannot yet parse deterministically for it (the bootstrap run must confirm a
/// profile first — ADR 0077 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrExtractionProfile {
    pub company_id: String,
    /// Monotonic version, bumped on every [`confirm`](Self::confirm).
    pub version: i64,
    /// The reporting scale figures are printed in (mln / tys. / full PLN). A
    /// wrong scale mis-states values 1000× silently, so it is a first-class
    /// profile field (ADR 0077 Evidence).
    pub scale: UnitScale,
    /// Normalized label → `metric_key`. The only labels the parser may emit.
    pub label_map: BTreeMap<String, String>,
    /// Which value column carries the requested period.
    pub value_column: ValueColumnLayout,
    /// Header labels of columns to skip when locating the value column (e.g.
    /// `"Nota"`), matched case-insensitively.
    pub skip_columns: Vec<String>,
    /// Strip a leading enumerator (`"1."`, `"I."`, `"a)"`) from each row label
    /// before normalization.
    pub strip_enumerators: bool,
    /// Stable fingerprint of the confirmed label set — identifies "same
    /// template" across periods (shares the tier-2 `template_hash` idiom).
    pub template_hash: String,
}

impl OcrExtractionProfile {
    /// Builds the first profile for a company from a confirmed bootstrap (the
    /// text-LLM's proposed layout, once confirmed). Version starts at 1.
    pub fn bootstrap(
        company_id: &str,
        scale: UnitScale,
        label_map: BTreeMap<String, String>,
        value_column: ValueColumnLayout,
        skip_columns: Vec<String>,
        strip_enumerators: bool,
    ) -> Self {
        let template_hash = template_hash(&label_map.keys().cloned().collect());
        Self {
            company_id: company_id.to_string(),
            version: 1,
            scale,
            label_map,
            value_column,
            skip_columns,
            strip_enumerators,
            template_hash,
        }
    }

    /// Parses a text-LLM bootstrap response (ADR 0077 §4 kickoff decision 3) into
    /// an unconfirmed **version 1** profile for `company_id`. Defensive: tolerates
    /// markdown fences / surrounding prose (reads the outermost JSON object),
    /// unknown scale/value-column words, and a partial label map. Returns `None`
    /// when no JSON object is present or the proposed `labelMap` is empty (no
    /// usable layout — the caller degrades rather than storing an empty profile).
    pub fn from_bootstrap_json(company_id: &str, response: &str) -> Option<Self> {
        let json = outermost_json_object(response)?;
        let parsed: BootstrapJson = serde_json::from_str(json).ok()?;
        let label_map: BTreeMap<String, String> = parsed
            .label_map
            .into_iter()
            .filter_map(|(label, metric)| {
                let label = label.trim().to_lowercase();
                let metric = metric.trim().to_owned();
                (!label.is_empty() && !metric.is_empty()).then_some((label, metric))
            })
            .collect();
        if label_map.is_empty() {
            return None;
        }
        let skip_columns = parsed
            .skip_columns
            .into_iter()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        Some(Self::bootstrap(
            company_id,
            scale_from_word(parsed.scale.as_deref()),
            label_map,
            value_column_from_word(parsed.value_column.as_deref()),
            skip_columns,
            parsed.strip_enumerators.unwrap_or(false),
        ))
    }

    /// Confirms an updated layout, bumping the version — mirrors the tier-2
    /// [`super::super::profile::ExtractionProfile::merge_confirmed`] versioning
    /// idiom. Used when a later proposal is confirmed for the same company.
    pub fn confirm(
        &self,
        scale: UnitScale,
        label_map: BTreeMap<String, String>,
        value_column: ValueColumnLayout,
        skip_columns: Vec<String>,
        strip_enumerators: bool,
    ) -> Self {
        let template_hash = template_hash(&label_map.keys().cloned().collect());
        Self {
            company_id: self.company_id.clone(),
            version: self.version + 1,
            scale,
            label_map,
            value_column,
            skip_columns,
            strip_enumerators,
            template_hash,
        }
    }
}

/// The text-LLM's proposed bootstrap layout (ADR 0077 §4). All fields optional /
/// defaulted so a partial or slightly-off response still parses to a usable
/// profile rather than being rejected wholesale.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapJson {
    #[serde(default)]
    scale: Option<String>,
    #[serde(default)]
    value_column: Option<String>,
    #[serde(default)]
    skip_columns: Vec<String>,
    #[serde(default)]
    strip_enumerators: Option<bool>,
    #[serde(default)]
    label_map: BTreeMap<String, String>,
}

/// The outermost `{ … }` slice of a model response, tolerating markdown fences
/// or prose around it. `None` when there is no balanced brace pair.
fn outermost_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

/// Maps a bootstrap `scale` word to a [`UnitScale`], defaulting to `Thousands`
/// (the dominant Polish interim-statement convention) for an unknown/absent word.
fn scale_from_word(word: Option<&str>) -> UnitScale {
    match word.map(str::trim).map(str::to_lowercase).as_deref() {
        Some("ones") | Some("units") | Some("full") => UnitScale::Ones,
        Some("millions") | Some("mln") | Some("million") => UnitScale::Millions,
        _ => UnitScale::Thousands,
    }
}

/// Maps a bootstrap `valueColumn` word to a [`ValueColumnLayout`], defaulting to
/// `CurrentPeriodFirst` (the dominant Polish convention) for an unknown word.
fn value_column_from_word(word: Option<&str>) -> ValueColumnLayout {
    match word.map(str::trim).map(str::to_lowercase).as_deref() {
        Some("current_period_last") => ValueColumnLayout::CurrentPeriodLast,
        Some("labeled_by_period_header") => ValueColumnLayout::LabeledByPeriodHeader,
        Some("single_value") => ValueColumnLayout::SingleValue,
        _ => ValueColumnLayout::CurrentPeriodFirst,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_label_map() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("przychody ze sprzedaży".to_string(), "revenue".to_string()),
            ("aktywa razem".to_string(), "total_assets".to_string()),
        ])
    }

    #[test]
    fn bootstrap_starts_at_version_one_and_hashes_labels() {
        let profile = OcrExtractionProfile::bootstrap(
            "GPW:LPP",
            UnitScale::Millions,
            sample_label_map(),
            ValueColumnLayout::CurrentPeriodFirst,
            vec!["Nota".to_string()],
            true,
        );
        assert_eq!(profile.version, 1);
        assert_eq!(profile.scale, UnitScale::Millions);
        assert!(!profile.template_hash.is_empty());
        assert_eq!(profile.value_column, ValueColumnLayout::CurrentPeriodFirst);
    }

    #[test]
    fn confirm_bumps_version_and_rehashes() {
        let base = OcrExtractionProfile::bootstrap(
            "GPW:LPP",
            UnitScale::Thousands,
            sample_label_map(),
            ValueColumnLayout::CurrentPeriodFirst,
            Vec::new(),
            false,
        );
        let mut extended = sample_label_map();
        extended.insert("zysk netto".to_string(), "net_profit".to_string());
        let confirmed = base.confirm(
            UnitScale::Millions,
            extended,
            ValueColumnLayout::CurrentPeriodLast,
            vec!["Nota".to_string()],
            true,
        );
        assert_eq!(confirmed.version, 2);
        assert_eq!(confirmed.scale, UnitScale::Millions);
        assert_ne!(confirmed.template_hash, base.template_hash);
        assert_eq!(confirmed.company_id, "GPW:LPP");
    }

    #[test]
    fn from_bootstrap_json_parses_a_clean_response() {
        let response = r#"{"scale":"millions","valueColumn":"current_period_last","skipColumns":["Nota"],"stripEnumerators":true,"labelMap":{"Przychody ze sprzedaży":"revenue","Aktywa razem":"total_assets"}}"#;
        let profile =
            OcrExtractionProfile::from_bootstrap_json("GPW:LPP", response).expect("parses");
        assert_eq!(profile.version, 1);
        assert_eq!(profile.scale, UnitScale::Millions);
        assert_eq!(profile.value_column, ValueColumnLayout::CurrentPeriodLast);
        assert_eq!(profile.skip_columns, vec!["Nota".to_string()]);
        assert!(profile.strip_enumerators);
        // Labels are normalized to lower-case (the parser's contract).
        assert_eq!(
            profile
                .label_map
                .get("przychody ze sprzedaży")
                .map(String::as_str),
            Some("revenue")
        );
    }

    #[test]
    fn from_bootstrap_json_tolerates_fences_and_unknown_words() {
        let response = "```json\n{\"scale\":\"weird\",\"valueColumn\":\"nope\",\"labelMap\":{\"zysk netto\":\"net_profit\"}}\n```";
        let profile =
            OcrExtractionProfile::from_bootstrap_json("GPW:CBF", response).expect("parses");
        // Unknown scale/value-column words fall back to the Polish defaults.
        assert_eq!(profile.scale, UnitScale::Thousands);
        assert_eq!(profile.value_column, ValueColumnLayout::CurrentPeriodFirst);
        assert!(!profile.strip_enumerators);
    }

    #[test]
    fn from_bootstrap_json_rejects_empty_or_absent_label_map() {
        assert!(OcrExtractionProfile::from_bootstrap_json("GPW:X", "not json").is_none());
        assert!(OcrExtractionProfile::from_bootstrap_json(
            "GPW:X",
            r#"{"scale":"ones","labelMap":{}}"#
        )
        .is_none());
    }

    #[test]
    fn profile_round_trips_through_json() {
        let profile = OcrExtractionProfile::bootstrap(
            "GPW:CBF",
            UnitScale::Thousands,
            sample_label_map(),
            ValueColumnLayout::LabeledByPeriodHeader,
            vec!["Nota".to_string()],
            true,
        );
        let json = serde_json::to_string(&profile).expect("serialize");
        let restored: OcrExtractionProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, profile);
    }
}
