use super::*;

// Research cockpit layout persistence behaviour (ADR 0053, decision 3A).

fn input(name: &str) -> NewCockpitLayout {
    NewCockpitLayout {
        name: name.to_owned(),
        panels_json: "{\"panels\":[\"feed\"]}".to_owned(),
        layout_json: Some("{\"grid\":{}}".to_owned()),
        dockview_version: Some("6.6.1".to_owned()),
    }
}

#[test]
fn save_inserts_and_lists_a_layout() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();

    let saved = store
        .save_cockpit_layout(input("Earnings season"))
        .expect("save");
    assert_eq!(saved.name, "Earnings season");
    assert_eq!(saved.ordinal, 0);
    assert_eq!(saved.layout_json.as_deref(), Some("{\"grid\":{}}"));

    let all = store.list_cockpit_layouts().expect("list");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, saved.id);
}

#[test]
fn save_upserts_by_name() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();

    let first = store
        .save_cockpit_layout(input("Daily triage"))
        .expect("save");
    let mut second_input = input("Daily triage");
    second_input.panels_json = "{\"panels\":[\"feed\",\"inspector\"]}".to_owned();
    let second = store.save_cockpit_layout(second_input).expect("upsert");

    assert_eq!(first.id, second.id, "same name upserts the same row");
    assert_eq!(second.panels_json, "{\"panels\":[\"feed\",\"inspector\"]}");
    assert_eq!(store.list_cockpit_layouts().expect("list").len(), 1);
}

#[test]
fn save_rejects_empty_name() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let result = state.cockpit_layouts().save_cockpit_layout(input("   "));
    assert!(matches!(
        result,
        Err(StorageError::InvalidCockpitLayoutName { .. })
    ));
}

#[test]
fn layout_without_geometry_is_valid() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();
    let mut no_geo = input("Deep dive");
    no_geo.layout_json = None;
    no_geo.dockview_version = None;
    let saved = store.save_cockpit_layout(no_geo).expect("save");
    assert_eq!(saved.layout_json, None);
    assert_eq!(saved.dockview_version, None);
}

#[test]
fn delete_removes_a_layout() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();
    let saved = store.save_cockpit_layout(input("Temp")).expect("save");
    store.delete_cockpit_layout(&saved.id).expect("delete");
    assert!(store.list_cockpit_layouts().expect("list").is_empty());
}

#[test]
fn list_orders_by_ordinal() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();
    store.save_cockpit_layout(input("First")).expect("save");
    store.save_cockpit_layout(input("Second")).expect("save");
    store.save_cockpit_layout(input("Third")).expect("save");
    let names: Vec<_> = store
        .list_cockpit_layouts()
        .expect("list")
        .into_iter()
        .map(|layout| layout.name)
        .collect();
    assert_eq!(names, vec!["First", "Second", "Third"]);
}

// Rename (issue #89): in-place, id/ordinal preserved; the duplicate guard
// exists because save_cockpit_layout upserts BY NAME — a duplicate rename
// would silently fuse two layouts on the next save.
#[test]
fn rename_keeps_id_and_ordinal_and_updates_name() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();
    store.save_cockpit_layout(input("First")).expect("save");
    let saved = store
        .save_cockpit_layout(input("Morning review"))
        .expect("save");

    let renamed = store
        .rename_cockpit_layout(&saved.id, "Evening review")
        .expect("rename");
    assert_eq!(renamed.id, saved.id);
    assert_eq!(renamed.ordinal, saved.ordinal);
    assert_eq!(renamed.name, "Evening review");

    let names: Vec<_> = store
        .list_cockpit_layouts()
        .expect("list")
        .into_iter()
        .map(|layout| layout.name)
        .collect();
    assert_eq!(names, vec!["First", "Evening review"]);
}

#[test]
fn rename_rejects_a_duplicate_name_but_allows_a_self_rename() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();
    let first = store.save_cockpit_layout(input("First")).expect("save");
    store.save_cockpit_layout(input("Second")).expect("save");

    let error = store
        .rename_cockpit_layout(&first.id, "Second")
        .expect_err("duplicate must be rejected");
    assert!(matches!(
        error,
        StorageError::DuplicateCockpitLayoutName { .. }
    ));

    // Renaming to its own (trimmed) name is a no-op update, not a duplicate.
    let same = store
        .rename_cockpit_layout(&first.id, "  First ")
        .expect("self-rename");
    assert_eq!(same.name, "First");
}

#[test]
fn rename_rejects_empty_names_and_unknown_ids() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let store = state.cockpit_layouts();
    let saved = store.save_cockpit_layout(input("First")).expect("save");

    assert!(matches!(
        store.rename_cockpit_layout(&saved.id, "   "),
        Err(StorageError::InvalidCockpitLayoutName { .. })
    ));
    assert!(matches!(
        store.rename_cockpit_layout("layout_missing", "New name"),
        Err(StorageError::CockpitLayoutNotFound { .. })
    ));
}
