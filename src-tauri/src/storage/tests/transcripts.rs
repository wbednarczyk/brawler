use super::*;

#[test]
fn creates_and_lists_unresolved_transcript_jobs() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let created = state
        .create_transcript_job(NewTranscriptJob {
            company_id: None,
            provider_id: None,
            source_url: "https://www.youtube.com/watch?v=conference".to_owned(),
            source_label: Some("Q2 conference".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("transcript job should be created");
    let jobs = state
        .list_transcript_jobs(TranscriptJobListInput { company_id: None })
        .expect("transcript jobs should list");

    assert_eq!(jobs.len(), 1);
    assert_eq!(created.provider_id, "provider_gemini");
    assert_eq!(created.source_type, "youtube_url");
    assert_eq!(created.company_id, None);
    assert_eq!(created.company_resolution_status, "unresolved");
    assert_eq!(created.status, "queued");
    assert_eq!(jobs[0].source_label.as_deref(), Some("Q2 conference"));
}

#[test]
fn updates_transcript_job_description() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let created = state
        .create_transcript_job(NewTranscriptJob {
            company_id: None,
            provider_id: None,
            source_url: "https://www.youtube.com/watch?v=conference-description".to_owned(),
            source_label: Some("Initial description".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("transcript job should be created");
    let updated = state
        .update_transcript_job(UpdateTranscriptJobInput {
            job_id: created.id.clone(),
            source_label: Some("Updated description".to_owned()),
        })
        .expect("transcript description should update");
    let cleared = state
        .update_transcript_job(UpdateTranscriptJobInput {
            job_id: created.id,
            source_label: Some("   ".to_owned()),
        })
        .expect("blank transcript description should clear");

    assert_eq!(updated.source_label.as_deref(), Some("Updated description"));
    assert_eq!(cleared.source_label, None);
}

#[test]
fn reuses_existing_transcript_job_for_duplicate_url_and_company_scope() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let first_unlinked = state
        .create_transcript_job(NewTranscriptJob {
            company_id: None,
            provider_id: None,
            source_url: "https://www.youtube.com/watch?v=conference".to_owned(),
            source_label: Some("First conference label".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("first unlinked job should be created");
    let duplicate_unlinked = state
        .create_transcript_job(NewTranscriptJob {
            company_id: None,
            provider_id: None,
            source_url: "https://www.youtube.com/watch?v=conference".to_owned(),
            source_label: Some("Duplicate conference label".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("duplicate unlinked job should reuse existing row");
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");
    let linked = state
        .create_transcript_job(NewTranscriptJob {
            company_id: Some(company.id),
            provider_id: None,
            source_url: "https://www.youtube.com/watch?v=conference".to_owned(),
            source_label: Some("Company conference".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("linked job should be separate from unlinked scope");
    let jobs = state
        .list_transcript_jobs(TranscriptJobListInput { company_id: None })
        .expect("jobs should list");

    assert_eq!(first_unlinked.id, duplicate_unlinked.id);
    assert_eq!(
        duplicate_unlinked.source_label.as_deref(),
        Some("First conference label")
    );
    assert_ne!(first_unlinked.id, linked.id);
    assert_eq!(jobs.len(), 2);
}

#[test]
fn deletes_transcript_job_and_segments() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let job = state
        .create_transcript_job(NewTranscriptJob {
            company_id: None,
            provider_id: None,
            source_url: "https://www.youtube.com/watch?v=conference-delete".to_owned(),
            source_label: Some("Conference to delete".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("transcript job should be created");
    state
        .create_transcript_segment(NewTranscriptSegment {
            transcript_job_id: job.id.clone(),
            company_id: None,
            start_seconds: Some(0),
            end_seconds: Some(30),
            speaker: None,
            text: "Segment to delete with parent job.".to_owned(),
            language: Some("en".to_owned()),
        })
        .expect("segment should be created");

    state
        .delete_transcript_job(&job.id)
        .expect("job should delete");

    let jobs = state
        .list_transcript_jobs(TranscriptJobListInput { company_id: None })
        .expect("jobs should list");
    let segments = state
        .list_transcript_segments(&job.id)
        .expect("segments should list");

    assert!(jobs.is_empty());
    assert!(segments.is_empty());
}

#[test]
fn creates_transcript_segments_and_keeps_text_immutable() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");
    let job = state
        .create_transcript_job(NewTranscriptJob {
            company_id: Some(company.id.clone()),
            provider_id: Some("provider_gemini".to_owned()),
            source_url: "https://www.youtube.com/watch?v=cdr-q2".to_owned(),
            source_label: None,
            recognized_company_candidates: None,
        })
        .expect("transcript job should be created");
    let segment = state
        .create_transcript_segment(NewTranscriptSegment {
            transcript_job_id: job.id.clone(),
            company_id: None,
            start_seconds: Some(120),
            end_seconds: Some(168),
            speaker: None,
            text: "Management expects a milestone within two quarters.".to_owned(),
            language: Some("en".to_owned()),
        })
        .expect("transcript segment should be created");
    let segments = state
        .list_transcript_segments(&job.id)
        .expect("transcript segments should list");

    assert_eq!(segments.len(), 1);
    assert_eq!(segment.company_id.as_deref(), Some(company.id.as_str()));
    assert_eq!(segments[0].start_seconds, Some(120));
    assert_eq!(
        segments[0].text,
        "Management expects a milestone within two quarters."
    );

    let connection = state.checkout().expect("database connection");
    let update_result = connection.execute(
        "UPDATE transcript_segments SET text = ?1 WHERE id = ?2",
        params!["Changed source text", segment.id],
    );

    assert!(update_result.is_err());
}

#[test]
fn creates_notebook_entry_from_resolved_transcript_segments() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");
    let job = state
        .create_transcript_job(NewTranscriptJob {
            company_id: Some(company.id.clone()),
            provider_id: Some("provider_gemini".to_owned()),
            source_url: "https://www.youtube.com/watch?v=cdr-q2".to_owned(),
            source_label: Some("Q2 conference".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("transcript job should be created");
    let first_segment = state
        .create_transcript_segment(NewTranscriptSegment {
            transcript_job_id: job.id.clone(),
            company_id: None,
            start_seconds: Some(120),
            end_seconds: Some(168),
            speaker: Some("CEO".to_owned()),
            text: "Management expects a milestone within two quarters.".to_owned(),
            language: Some("en".to_owned()),
        })
        .expect("first segment should be created");
    let second_segment = state
        .create_transcript_segment(NewTranscriptSegment {
            transcript_job_id: job.id.clone(),
            company_id: None,
            start_seconds: Some(169),
            end_seconds: Some(210),
            speaker: Some("CFO".to_owned()),
            text: "Margin should normalize after launch costs fade.".to_owned(),
            language: Some("en".to_owned()),
        })
        .expect("second segment should be created");
    state
        .mark_transcript_job_completed(&job.id)
        .expect("job should complete");

    let note = state
        .create_note_from_transcript_selection(CreateNoteFromTranscriptSelectionInput {
            transcript_job_id: job.id.clone(),
            transcript_segment_ids: vec![first_segment.id.clone(), second_segment.id.clone()],
            note_draft: TranscriptNoteDraft {
                title: "Q2 conference promises".to_owned(),
                body: "Management expects the milestone and margin normalization.".to_owned(),
                tags: vec!["conference".to_owned(), "management-guidance".to_owned()],
                kind: "claim".to_owned(),
                claim_status: Some("open".to_owned()),
                event_date: None,
                follow_up_after: Some("2026-Q4".to_owned()),
                follow_up_date: None,
            },
        })
        .expect("transcript selection should create a note");

    assert_eq!(note.company_id, company.id);
    assert_eq!(note.title, "Q2 conference promises");
    assert_eq!(note.kind, "claim");
    assert_eq!(note.claim_status.as_deref(), Some("open"));
    assert_eq!(note.origins.len(), 2);
    assert_eq!(note.origins[0].source_type, "transcript_segment");
    assert_eq!(
        note.origins[0].source_id.as_deref(),
        Some(first_segment.id.as_str())
    );
    assert_eq!(
        note.origins[0].source_url.as_deref(),
        Some("https://www.youtube.com/watch?v=cdr-q2")
    );
    assert!(note.origins[0]
        .label
        .as_deref()
        .expect("origin label should exist")
        .contains(&job.id));
}

#[test]
fn rejects_transcript_note_creation_when_company_is_unresolved() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let job = state
        .create_transcript_job(NewTranscriptJob {
            company_id: None,
            provider_id: Some("provider_gemini".to_owned()),
            source_url: "https://www.youtube.com/watch?v=unknown-q2".to_owned(),
            source_label: Some("Unknown Q2 conference".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("transcript job should be created");
    let segment = state
        .create_transcript_segment(NewTranscriptSegment {
            transcript_job_id: job.id.clone(),
            company_id: None,
            start_seconds: Some(0),
            end_seconds: Some(42),
            speaker: None,
            text: "Unresolved company segment.".to_owned(),
            language: Some("en".to_owned()),
        })
        .expect("segment should be created");
    state
        .mark_transcript_job_completed(&job.id)
        .expect("job should complete");

    let result =
        state.create_note_from_transcript_selection(CreateNoteFromTranscriptSelectionInput {
            transcript_job_id: job.id,
            transcript_segment_ids: vec![segment.id],
            note_draft: TranscriptNoteDraft {
                title: "Unresolved note".to_owned(),
                body: "This should not save yet.".to_owned(),
                tags: vec!["conference".to_owned()],
                kind: "observation".to_owned(),
                claim_status: None,
                event_date: None,
                follow_up_after: None,
                follow_up_date: None,
            },
        });

    assert!(result.is_err());
}
