//! Tests for per-company extraction profiles + drift detection (ADR 0061 S3).

use super::*;
use crate::fundamentals::extraction::pdf::parse_pdf_text;

const BASE: &str = "\
dane w tys. zł
Aktywa razem  45 000  40 000
Zobowiązania razem  20 000  18 000
Kapitał własny  25 000  22 000
";

fn parse(text: &str) -> PdfParse {
    parse_pdf_text(text, "2026-03-31", None)
}

#[test]
fn bootstrap_captures_label_map_and_scale() {
    let profile = ExtractionProfile::bootstrap("cmp1", &parse(BASE));
    assert_eq!(profile.version, 1);
    assert_eq!(profile.unit_scale, UnitScale::Thousands);
    assert_eq!(
        profile.label_map.get("aktywa razem").map(String::as_str),
        Some("total_assets")
    );
    assert_eq!(profile.label_map.len(), 3);
}

#[test]
fn no_drift_on_identical_layout() {
    let profile = ExtractionProfile::bootstrap("cmp1", &parse(BASE));
    assert_eq!(detect_drift(&profile, &parse(BASE)), Drift::None);
}

#[test]
fn drift_reports_added_label() {
    let profile = ExtractionProfile::bootstrap("cmp1", &parse(BASE));
    let changed = format!("{BASE}Środki pieniężne i ich ekwiwalenty  8 000  6 000\n");
    match detect_drift(&profile, &parse(&changed)) {
        Drift::Detected(r) => {
            assert_eq!(
                r.added_labels,
                vec!["środki pieniężne i ich ekwiwalenty".to_string()]
            );
            assert!(r.removed_labels.is_empty());
        }
        Drift::None => panic!("expected drift"),
    }
}

#[test]
fn drift_reports_removed_label() {
    let profile = ExtractionProfile::bootstrap("cmp1", &parse(BASE));
    // A report missing the equity line.
    let changed =
        "dane w tys. zł\nAktywa razem  45 000  40 000\nZobowiązania razem  20 000  18 000\n";
    match detect_drift(&profile, &parse(changed)) {
        Drift::Detected(r) => {
            assert_eq!(r.removed_labels, vec!["kapitał własny".to_string()]);
            assert!(r.added_labels.is_empty());
        }
        Drift::None => panic!("expected drift"),
    }
}

#[test]
fn drift_reports_unit_change() {
    let profile = ExtractionProfile::bootstrap("cmp1", &parse(BASE));
    let millions = BASE.replace("w tys. zł", "w mln zł");
    match detect_drift(&profile, &parse(&millions)) {
        Drift::Detected(r) => {
            assert_eq!(
                r.unit_changed,
                Some((UnitScale::Thousands, UnitScale::Millions))
            );
        }
        Drift::None => panic!("expected drift"),
    }
}

#[test]
fn merge_confirmed_learns_and_bumps_version() {
    let profile = ExtractionProfile::bootstrap("cmp1", &parse(BASE));
    let with_cash = format!("{BASE}Środki pieniężne i ich ekwiwalenty  8 000  6 000\n");
    let merged = profile.merge_confirmed(&parse(&with_cash));
    assert_eq!(merged.version, 2);
    assert!(merged
        .label_map
        .contains_key("środki pieniężne i ich ekwiwalenty"));
    // Old labels retained.
    assert!(merged.label_map.contains_key("aktywa razem"));
    // The template fingerprint changed with the layout.
    assert_ne!(merged.template_hash, profile.template_hash);
    // After learning, the once-drifting layout no longer drifts.
    assert_eq!(detect_drift(&merged, &parse(&with_cash)), Drift::None);
}

#[test]
fn template_hash_is_order_independent_and_stable() {
    let a: BTreeSet<String> = ["aktywa razem", "kapitał własny"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let b: BTreeSet<String> = ["kapitał własny", "aktywa razem"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(template_hash(&a), template_hash(&b));
    // Distinct sets differ.
    let c: BTreeSet<String> = ["aktywa razem"].iter().map(|s| s.to_string()).collect();
    assert_ne!(template_hash(&a), template_hash(&c));
}
