use super::*;

#[test]
fn stores_only_derived_license_metadata() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));

    let stored = state
        .upsert_license_metadata(LicenseMetadataUpdate {
            status: "valid".to_owned(),
            reason: None,
            license_id: Some("lic_friend".to_owned()),
            holder: Some("Friend Tester".to_owned()),
            channel: Some("friend_test".to_owned()),
            edition: Some("friend".to_owned()),
            features: vec!["core".to_owned(), "sync_preview".to_owned()],
            issued_at: Some("2026-06-01T00:00:00Z".to_owned()),
            expires_at: Some("2027-01-01T00:00:00Z".to_owned()),
            app_version_range: Some("*".to_owned()),
            key_id: Some("owner_friend_test_2026_06".to_owned()),
        })
        .expect("metadata should save");

    assert_eq!(stored.status, "valid");
    assert_eq!(stored.license_id.as_deref(), Some("lic_friend"));
    assert_eq!(stored.features, vec!["core", "sync_preview"]);

    let loaded = state
        .get_license_metadata()
        .expect("metadata should load")
        .expect("metadata should exist");

    assert_eq!(loaded.holder.as_deref(), Some("Friend Tester"));
    assert_eq!(loaded.key_id.as_deref(), Some("owner_friend_test_2026_06"));
}

#[test]
fn clears_license_metadata_without_touching_other_settings() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));

    state
        .upsert_license_metadata(LicenseMetadataUpdate {
            status: "expired".to_owned(),
            reason: Some("This license has expired.".to_owned()),
            license_id: Some("lic_friend".to_owned()),
            holder: Some("Friend Tester".to_owned()),
            channel: Some("friend_test".to_owned()),
            edition: Some("friend".to_owned()),
            features: vec!["core".to_owned()],
            issued_at: Some("2026-01-01T00:00:00Z".to_owned()),
            expires_at: Some("2026-02-01T00:00:00Z".to_owned()),
            app_version_range: Some("*".to_owned()),
            key_id: Some("owner_friend_test_2026_06".to_owned()),
        })
        .expect("metadata should save");

    state
        .clear_license_metadata()
        .expect("metadata should clear");

    assert!(state
        .get_license_metadata()
        .expect("metadata lookup should work")
        .is_none());
    assert_eq!(
        state
            .get_settings()
            .expect("settings should remain")
            .settings_source,
        "sqlite"
    );
}
