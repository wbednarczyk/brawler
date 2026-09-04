use crate::{
    app_state,
    providers::transcripts::{registry, VideoTranscriptProvider},
    storage,
};

pub fn run_video_transcript_job(
    state: &app_state::AppState,
    job_id: &str,
    provider_mode: Option<&str>,
) -> Result<storage::TranscriptJob, String> {
    let started_at = std::time::Instant::now();
    let provider_mode = provider_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("provider_gemini");
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let provider: Box<dyn VideoTranscriptProvider> = registry::build_transcript_provider(
        provider_mode,
        &settings.ai_providers.youtube_transcription_model,
        settings.ai_providers.youtube_transcription_timeout_seconds,
    )?;
    let job = state
        .get_transcript_job(job_id)
        .map_err(|error| error.to_string())?;

    if job.status == "completed" {
        return Ok(job);
    }

    state
        .mark_transcript_job_running(job_id)
        .map_err(|error| error.to_string())?;

    // Direct-activity registry (ADR 0109 dec. 3): transcripts are awaited, not
    // queue jobs — this is the ONE writer for this family, so no double-count
    // seam exists to guard against. `start` always returns a guard (sol diff
    // R1 #3): even a `begin_attempt` storage failure still records the work
    // as visible, never `None`.
    let guard = crate::storage::activity_registry::start(
        state,
        crate::jobs::activity_identity::identity_for_transcript(&job),
    );

    // Every post-`running` exit funnels through ONE finalizer below (sol
    // diff R1 #6): a segment-insert failure, a completion-write failure, and
    // a failure-write error itself all terminalize the transcript row AND
    // the occurrence the SAME way — never an early `?` return that skips
    // both and leaves the row `running` with only the guard's Drop (which
    // settles `interrupted`, not the real outcome) closing the ledger side.
    let transcribe_outcome = transcribe_and_store_segments(state, job_id, &job, provider.as_ref());

    match transcribe_outcome {
        Ok(()) => match state.mark_transcript_job_completed(job_id) {
            Ok(completed) => {
                record_transcript_metrics(state, &completed.provider_id, "succeeded", started_at);
                guard.settle(Ok(()));
                Ok(completed)
            }
            Err(storage_error) => finalize_as_failed(
                state,
                job_id,
                "unknown",
                &storage_error.to_string(),
                guard,
                started_at,
            ),
        },
        Err((code, message)) => {
            finalize_as_failed(state, job_id, code, &message, guard, started_at)
        }
    }
}

/// Run the provider and persist its segments — never itself marks the
/// transcript row terminal (that is the caller's single finalizer's job
/// alone, sol diff R1 #6). `Err` carries the `(error_code, message)` pair
/// `mark_transcript_job_failed` needs, whether the failure came from the
/// provider or from writing a segment.
fn transcribe_and_store_segments(
    state: &app_state::AppState,
    job_id: &str,
    job: &storage::TranscriptJob,
    provider: &dyn VideoTranscriptProvider,
) -> Result<(), (&'static str, String)> {
    let output = tauri::async_runtime::block_on(provider.transcribe(job))
        .map_err(|error| (error.code(), error.to_string()))?;
    for segment in output.segments {
        state
            .create_transcript_segment(storage::NewTranscriptSegment {
                transcript_job_id: job_id.to_owned(),
                company_id: job.company_id.clone(),
                start_seconds: segment.start_seconds,
                end_seconds: segment.end_seconds,
                speaker: segment.speaker,
                text: segment.text,
                language: segment.language,
            })
            .map_err(|error| ("unknown", error.to_string()))?;
    }
    Ok(())
}

/// The finalizer's failed branch: mark the transcript row failed and settle
/// the occurrence failed. If the failure-write ITSELF errors (sol diff R1
/// #6's third branch — a genuinely broken store), still settle the
/// occurrence best-effort so the ledger side closes; the domain row may stay
/// `running` until the read model's stalled-transcript derivation or a
/// restart's reconcile catches it — logged loudly since nothing more can be
/// done here.
fn finalize_as_failed(
    state: &app_state::AppState,
    job_id: &str,
    code: &str,
    message: &str,
    guard: crate::storage::activity_registry::ActivityGuard,
    started_at: std::time::Instant,
) -> Result<storage::TranscriptJob, String> {
    match state.mark_transcript_job_failed(job_id, code, message) {
        Ok(failed) => {
            record_transcript_metrics(state, &failed.provider_id, "failed", started_at);
            guard.settle(Err(message));
            Ok(failed)
        }
        Err(storage_error) => {
            log::error!(
                "transcript runner: job {job_id} failed (\"{message}\") and recording that \
                 failure ALSO failed: {storage_error} — the row may stay `running` until \
                 startup reconcile or the read model's stalled-transcript derivation"
            );
            guard.settle(Err(message));
            Err(format!(
                "transcribe failed ({message}) and the failure could not be recorded: {storage_error}"
            ))
        }
    }
}

fn record_transcript_metrics(
    state: &app_state::AppState,
    provider_id: &str,
    status: &str,
    started_at: std::time::Instant,
) {
    state.increment_runtime_counter(
        "brawler_transcript_runs_total",
        &[("provider_id", provider_id), ("status", status)],
    );
    state.observe_runtime_duration_seconds(
        "brawler_transcript_duration_seconds",
        &[("provider_id", provider_id), ("status", status)],
        started_at.elapsed().as_secs_f64(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{open_in_memory_database, NewTranscriptJob};

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("db"))
    }

    /// A job whose provider call SUCCEEDS (`test_sample` requires a
    /// youtube.com/youtu.be URL) — used by the segment-insert-failure and
    /// completion-write-failure tests, which need to reach past the provider
    /// call itself.
    fn queue_succeeding_job(state: &AppState) -> String {
        state
            .create_transcript_job(NewTranscriptJob {
                company_id: None,
                provider_id: None,
                source_url: "https://www.youtube.com/watch?v=mock".to_owned(),
                source_label: Some("Earnings call".to_owned()),
                recognized_company_candidates: None,
            })
            .expect("create transcript job")
            .id
    }

    fn occurrence_status(state: &AppState, job_id: &str) -> Option<String> {
        let connection = state.checkout_for_tests().expect("checkout");
        connection
            .query_row(
                "SELECT status FROM job_runs WHERE run_key = ?1",
                [format!("direct:transcript:{job_id}")],
                |row| row.get(0),
            )
            .ok()
    }

    #[test]
    fn a_segment_insert_failure_finalizes_both_the_row_and_the_occurrence_as_failed() {
        // sol diff R1 #6, branch 1: the provider succeeds, but persisting a
        // segment fails. The OLD code's early `?` return skipped BOTH
        // `mark_transcript_job_failed` and `guard.settle` — the row stayed
        // `running` forever (only the guard's Drop settled `interrupted`,
        // never the real outcome). The ONE finalizer must catch this.
        let state = state();
        let job_id = queue_succeeding_job(&state);
        state
            .checkout_for_tests()
            .expect("checkout")
            .execute("DROP TABLE transcript_segments", [])
            .expect("poison segment inserts");

        let result = run_video_transcript_job(&state, &job_id, Some("test_sample"));
        let job = result.expect("the function itself still returns Ok with the failed row");
        assert_eq!(job.status, "failed");
        assert_eq!(
            occurrence_status(&state, &job_id).as_deref(),
            Some("failed")
        );
    }

    #[test]
    fn a_completion_write_failure_still_finalizes_the_row_as_failed() {
        // sol diff R1 #6, branch 2: the provider succeeds and segments are
        // stored, but `mark_transcript_job_completed`'s own write fails. The
        // finalizer must still record SOME terminal truth (`failed`, since
        // completion could not be durably recorded) rather than leaving the
        // row `running` with a lost `?` return.
        let state = state();
        let job_id = queue_succeeding_job(&state);
        state
            .checkout_for_tests()
            .expect("checkout")
            .execute_batch(
                "CREATE TRIGGER poison_completion BEFORE UPDATE ON transcript_jobs
                 WHEN NEW.status = 'completed'
                 BEGIN SELECT RAISE(ABORT, 'completion poisoned for test'); END;",
            )
            .expect("install poison trigger");

        let result = run_video_transcript_job(&state, &job_id, Some("test_sample"));
        let job = result.expect("finalize_as_failed still returns Ok once the failure IS recorded");
        assert_eq!(job.status, "failed");
        assert_eq!(
            occurrence_status(&state, &job_id).as_deref(),
            Some("failed")
        );
    }

    #[test]
    fn a_failure_write_error_still_settles_the_occurrence_best_effort() {
        // sol diff R1 #6, branch 3: the provider call itself fails (no
        // youtube.com URL), AND recording that failure onto the transcript
        // row ALSO fails. Nothing more can durably record the outcome, but
        // the occurrence must still settle (best-effort) rather than leaking
        // an open `job_runs` row, and the function must surface an error
        // rather than silently claiming success.
        let state = state();
        let job_id = state
            .create_transcript_job(NewTranscriptJob {
                company_id: None,
                provider_id: None,
                source_url: "https://example.test/not-youtube".to_owned(),
                source_label: Some("Bad source".to_owned()),
                recognized_company_candidates: None,
            })
            .expect("create transcript job")
            .id;
        state
            .checkout_for_tests()
            .expect("checkout")
            .execute_batch(
                "CREATE TRIGGER poison_failure_write BEFORE UPDATE ON transcript_jobs
                 WHEN NEW.status = 'failed'
                 BEGIN SELECT RAISE(ABORT, 'transcript_jobs failure-write poisoned for test'); END;",
            )
            .expect("install poison trigger (mark_transcript_job_running, status='running', still succeeds)");

        let result = run_video_transcript_job(&state, &job_id, Some("test_sample"));
        assert!(
            result.is_err(),
            "when even the failure write fails, the caller must see an error, not a fake success"
        );
        assert_eq!(
            occurrence_status(&state, &job_id).as_deref(),
            Some("failed"),
            "the occurrence still settles best-effort even though the domain row could not be written"
        );
    }
}
