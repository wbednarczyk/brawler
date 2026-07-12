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
