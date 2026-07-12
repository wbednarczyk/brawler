use super::*;

fn sample_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

fn entry(company_id: &str, kind: &str, rationale: &str, decided_at: &str) -> NewDecisionEntry {
    NewDecisionEntry {
        company_id: company_id.to_owned(),
        kind: kind.to_owned(),
        rationale_md: rationale.to_owned(),
        decided_at: decided_at.to_owned(),
        superseded_by_entry_id: None,
    }
}

#[test]
fn create_and_list_round_trip() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    let created = state
        .decision_journal()
        .create_decision_entry(entry(
            &company.id,
            "keep_watching",
            "Margins improving; **wait** for the FY report.",
            "2026-06-01",
        ))
        .expect("entry should be created");

    assert_eq!(created.company_id, company.id);
    assert_eq!(created.kind, "keep_watching");
    assert_eq!(
        created.rationale_md,
        "Margins improving; **wait** for the FY report."
    );
    assert_eq!(created.decided_at, "2026-06-01");
    assert!(created.superseded_by_entry_id.is_none());
    assert!(!created.created_at.is_empty());

    // Per-company list.
    let per_company = state
        .decision_journal()
        .list_decision_entries(DecisionEntryListInput {
            company_id: Some(company.id.clone()),
            ..DecisionEntryListInput::default()
        })
        .expect("per-company list should work");
    assert_eq!(per_company.len(), 1);
    assert_eq!(per_company[0].id, created.id);

    // Global list.
    let global = state
        .decision_journal()
        .list_decision_entries(DecisionEntryListInput::default())
        .expect("global list should work");
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].id, created.id);
}

#[test]
fn list_orders_by_decided_at_not_created_at() {
    // DoD §C guardrail: the journal is a chronology of the user's DECISIONS, not
    // of row insertion. Seed rows whose created_at order diverges from their
    // decided_at order and assert decided_at DESC wins.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let journal = state.decision_journal();

    // Created FIRST, but decided LAST.
    let decided_later = journal
        .create_decision_entry(entry(&company.id, "buy", "later decision", "2026-03-01"))
        .expect("create");
    // Created SECOND, but decided FIRST.
    let decided_earlier = journal
        .create_decision_entry(entry(&company.id, "pass", "earlier decision", "2026-01-05"))
        .expect("create");

    let listed = journal
        .list_decision_entries(DecisionEntryListInput::default())
        .expect("list");
    assert_eq!(listed.len(), 2);
    assert_eq!(
        listed[0].id, decided_later.id,
        "the most recent DECISION must come first even though it was inserted first"
    );
    assert_eq!(listed[1].id, decided_earlier.id);
}

#[test]
fn sqlite_trigger_rejects_direct_update() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let created = state
        .decision_journal()
        .create_decision_entry(entry(
            &company.id,
            "buy",
            "original rationale",
            "2026-06-01",
        ))
        .expect("create");

    let raw = state.checkout_for_tests().expect("raw connection");
    let result = raw.execute(
        "UPDATE decision_entries SET rationale_md = 'rewritten history' WHERE id = ?1",
        [&created.id],
    );
    assert!(
        result.is_err(),
        "a raw UPDATE must be rejected by the immutability trigger"
    );
    drop(raw);

    // The row is untouched.
    let listed = state
        .decision_journal()
        .list_decision_entries(DecisionEntryListInput::default())
        .expect("list");
    assert_eq!(listed[0].rationale_md, "original rationale");
}

#[test]
fn sqlite_trigger_rejects_delete() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let created = state
        .decision_journal()
        .create_decision_entry(entry(&company.id, "sell_note", "note", "2026-06-01"))
        .expect("create");

    let raw = state.checkout_for_tests().expect("raw connection");
    let result = raw.execute("DELETE FROM decision_entries WHERE id = ?1", [&created.id]);
    assert!(
        result.is_err(),
        "a raw DELETE must be rejected by the immutability trigger"
    );
    let survived: i64 = raw
        .query_row("SELECT COUNT(*) FROM decision_entries", [], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(survived, 1, "the entry must survive the delete attempt");
}

#[test]
fn superseded_by_round_trip() {
    // ADR 0071: corrections are APPENDED as follow-up entries. The link lives on
    // the follow-up (superseded_by_entry_id = id of the entry superseded by this
    // one), so no UPDATE of prior rows is ever needed and the triggers stay
    // absolute.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let journal = state.decision_journal();

    let original = journal
        .create_decision_entry(entry(&company.id, "buy", "first take", "2026-06-01"))
        .expect("create original");

    let follow_up = journal
        .create_decision_entry(NewDecisionEntry {
            company_id: company.id.clone(),
            kind: "pass".to_owned(),
            rationale_md: "corrected after the Q1 report".to_owned(),
            decided_at: "2026-06-15".to_owned(),
            superseded_by_entry_id: Some(original.id.clone()),
        })
        .expect("create follow-up");
    assert_eq!(
        follow_up.superseded_by_entry_id.as_deref(),
        Some(original.id.as_str()),
        "the follow-up must link back to the entry it supersedes"
    );

    // The old row is untouched: same content, no pointer, no update happened.
    let listed = journal
        .list_decision_entries(DecisionEntryListInput::default())
        .expect("list");
    let old = listed
        .iter()
        .find(|e| e.id == original.id)
        .expect("original must still list");
    assert_eq!(old.rationale_md, "first take");
    assert_eq!(old.kind, "buy");
    assert!(old.superseded_by_entry_id.is_none());
}

#[test]
fn kind_check_rejects_unknown() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    // Store-level validation.
    let result = state.decision_journal().create_decision_entry(entry(
        &company.id,
        "hold",
        "not a recorded kind",
        "2026-06-01",
    ));
    assert!(
        matches!(
            result,
            Err(StorageError::InvalidResearchValue { key: "kind", .. })
        ),
        "an unknown kind must be a typed validation error, got: {result:?}"
    );

    // DB-level CHECK backstop against raw writes.
    let raw = state.checkout_for_tests().expect("raw connection");
    let raw_insert = raw.execute(
        "INSERT INTO decision_entries (id, company_id, kind, rationale_md, decided_at)
         VALUES ('bogus1', ?1, 'hold', 'x', '2026-06-01')",
        [&company.id],
    );
    assert!(
        raw_insert.is_err(),
        "the CHECK constraint must reject an unknown kind"
    );
}
