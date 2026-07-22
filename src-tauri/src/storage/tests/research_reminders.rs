use super::*;

#[test]
fn derives_and_completes_research_reminders_from_claims_events_and_questions() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Management claim to verify".to_owned(),
            body: "Management promised margin recovery.".to_owned(),
            body_format: None,
            tags: vec![],
            kind: "claim".to_owned(),
            claim_status: Some("open".to_owned()),
            event_date: None,
            follow_up_after: Some("2026-Q4".to_owned()),
            follow_up_date: Some("2026-12-31".to_owned()),
            origins: vec![],
        })
        .expect("claim note should create");
    state
        .create_company_event(NewCompanyEvent {
            company_id: company.id.clone(),
            event_type: "periodic_report".to_owned(),
            title: "Quarterly report".to_owned(),
            event_date: "2026-08-30".to_owned(),
            event_time: None,
            status: Some("scheduled".to_owned()),
            source_type: Some("manual".to_owned()),
            source_adapter_id: None,
            source_event_key: None,
            source_url: None,
            attribution: None,
            fetched_at: None,
        })
        .expect("event should create");
    state
        .create_research_question(NewResearchQuestion {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            title: "Will the claim be delivered?".to_owned(),
            body: None,
        })
        .expect("question should create");

    let reminders = state
        .list_research_reminders(ResearchReminderListInput {
            scope_type: "company".to_owned(),
            scope_id: company.id,
            status: Some("open".to_owned()),
        })
        .expect("reminders should list");

    assert!(reminders
        .iter()
        .any(|reminder| reminder.reminder_kind == "claim_follow_up"));
    assert!(reminders
        .iter()
        .any(|reminder| reminder.reminder_kind == "event_review"));
    assert!(reminders
        .iter()
        .any(|reminder| reminder.reminder_kind == "question_review"));

    let completed = state
        .update_research_reminder(ResearchReminderUpdate {
            id: reminders[0].id.clone(),
            status: Some("completed".to_owned()),
            due_at: None,
            snoozed_until: None,
        })
        .expect("reminder should complete");

    assert_eq!(completed.status, "completed");
    assert!(completed.completed_at.is_some());
}

fn tracked_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create")
}
