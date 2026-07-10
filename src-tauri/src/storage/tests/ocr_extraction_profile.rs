use super::*;

use crate::fundamentals::extraction::ocr::{OcrExtractionProfile, ValueColumnLayout};
use crate::fundamentals::extraction::pdf::UnitScale;
use std::collections::BTreeMap;

fn sample_profile(company_id: &str, version_seed: UnitScale) -> OcrExtractionProfile {
    OcrExtractionProfile::bootstrap(
        company_id,
        version_seed,
        BTreeMap::from([
            ("przychody ze sprzedaży".to_string(), "revenue".to_string()),
            ("aktywa razem".to_string(), "total_assets".to_string()),
        ]),
        ValueColumnLayout::CurrentPeriodFirst,
        vec!["Nota".to_string()],
        true,
    )
}

#[test]
fn ocr_profile_round_trips_through_storage() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.fundamentals_provenance();

    let profile = sample_profile("GPW:LPP", UnitScale::Millions);
    store
        .upsert_ocr_profile(&profile)
        .expect("upsert ocr profile");

    let restored = store
        .get_ocr_profile("GPW:LPP")
        .expect("read ocr profile")
        .expect("profile is present");
    assert_eq!(restored, profile);
    assert_eq!(restored.scale, UnitScale::Millions);
}

#[test]
fn ocr_profile_missing_row_reads_none() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.fundamentals_provenance();

    assert!(store
        .get_ocr_profile("GPW:UNKNOWN")
        .expect("read is ok")
        .is_none());
}

#[test]
fn ocr_profile_upsert_persists_confirmed_version_bump() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.fundamentals_provenance();

    let base = sample_profile("GPW:CBF", UnitScale::Thousands);
    store.upsert_ocr_profile(&base).expect("insert v1");

    let mut extended = base.label_map.clone();
    extended.insert("zysk netto".to_string(), "net_profit".to_string());
    let confirmed = base.confirm(
        UnitScale::Millions,
        extended,
        ValueColumnLayout::LabeledByPeriodHeader,
        vec!["Nota".to_string()],
        true,
    );
    store
        .upsert_ocr_profile(&confirmed)
        .expect("upsert bumped version");

    let restored = store
        .get_ocr_profile("GPW:CBF")
        .expect("read")
        .expect("present");
    assert_eq!(restored.version, 2);
    assert_eq!(restored.scale, UnitScale::Millions);
    assert_eq!(
        restored.value_column,
        ValueColumnLayout::LabeledByPeriodHeader
    );
    assert!(restored.label_map.contains_key("zysk netto"));
}
