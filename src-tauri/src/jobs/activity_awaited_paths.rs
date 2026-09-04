//! Awaited-path registry + source-scan gate (ADR 0109 dec. 3, sol diff R1
//! #8): a handful of unwrapped "core" functions do real (sometimes
//! long-running) refresh/backfill work with NO activity-ledger occurrence of
//! their own — each has a `_direct` (or equivalently instrumented) wrapper
//! that opens one via `storage::activity_registry::start` before calling the
//! core. A caller that reaches the CORE directly — as `commands::companies
//! ::lookup_company` used to (sol diff R1 #8's exact finding) — does real
//! work invisibly: no occurrence, so the read model can never show it as
//! `active`, and a slow one just looks like nothing is happening.
//!
//! [`DIRECT_PATHS`] is the single source of truth for which core functions
//! this applies to and which files may legitimately call them (their own
//! defining file — covering the `_direct` wrapper and, where applicable, the
//! queue handler sharing that file — plus their own test files). The gate
//! test (`activity_awaited_paths::tests::no_unwrapped_core_is_called_outside_its_direct_paths`)
//! scans every other `.rs` file under `src/` for a call to the core's name
//! and fails if one turns up, exactly like the `no_write_transaction_is_deferred`
//! gate (`storage/tests/schema.rs`) it mirrors.

/// One unwrapped core whose real work must only ever run through its
/// instrumented wrapper (or a queue handler that writes its own occurrence
/// via the dispatch seam).
pub struct DirectPath {
    /// The core function's bare name (used as the scan needle: `"{name}("`).
    pub core_fn: &'static str,
    /// The file (relative to `src-tauri/src/`) where `core_fn` — and the
    /// ONE instrumented wrapper/queue-handler pair allowed to call it — is
    /// defined. A call from any OTHER file (that is not a test file) is a
    /// violation.
    pub defining_file: &'static str,
    /// Any additional non-test file allowed to call `core_fn` directly, with
    /// the reason it is not routed through the `_direct` wrapper. Empty for
    /// every entry except the one documented, deliberate exception below.
    pub extra_allowed_files: &'static [(&'static str, &'static str)],
}

/// The awaited-path registry (sol diff R1 #8). Every entry's `core_fn` is
/// called by exactly one `_direct` wrapper (or, for `run_video_transcript_job`,
/// carries its OWN instrumentation directly — no separate wrapper exists)
/// defined in `defining_file`, plus the queue handler when that kind is ALSO
/// queue-driven (`jobs/handlers.rs`, itself instrumented via the dispatch
/// seam, never this core directly except as an intentional queue-side call
/// that opens its own occurrence a different way).
pub const DIRECT_PATHS: &[DirectPath] = &[
    DirectPath {
        core_fn: "refresh_source_for_trigger",
        defining_file: "jobs/source_refresh.rs",
        extra_allowed_files: &[(
            "jobs/handlers.rs",
            "ScheduledSourceRefreshHandler — the queue's own dispatch seam writes its occurrence",
        )],
    },
    DirectPath {
        core_fn: "run_aggregator_fundamentals_pull_serialized",
        defining_file: "jobs/aggregator_fundamentals_pull.rs",
        extra_allowed_files: &[(
            "jobs/rebuild_fundamentals.rs",
            "documented exception (2026-09-04, this task): `run_rebuild_fundamentals` is a rare, \
             whole-corpus (3-pass) maintenance rebuild command, not itself Activity-ledgered — \
             wrapping only its Pass-1 sub-call would misrepresent the rebuild's true scope. Flagged \
             to the owner rather than silently fixed; revisit if rebuild_fundamentals joins the \
             Activity ledger.",
        )],
    },
    DirectPath {
        core_fn: "backfill_company_history",
        defining_file: "jobs/backfill.rs",
        extra_allowed_files: &[],
    },
    DirectPath {
        core_fn: "refresh_company_directories_for_trigger",
        defining_file: "jobs/source_refresh.rs",
        extra_allowed_files: &[(
            "jobs/handlers.rs",
            "ScheduledRegistryRefreshHandler — the queue's own dispatch seam writes its occurrence",
        )],
    },
    DirectPath {
        core_fn: "run_video_transcript_job",
        defining_file: "jobs/transcript_runner.rs",
        extra_allowed_files: &[(
            "commands/transcripts.rs",
            "the ONE Tauri command entry point — `run_video_transcript_job` carries its OWN \
             `activity_registry::start` instrumentation directly (transcripts are never queue \
             jobs, ADR 0109 dec. 3), so there is no separate `_direct` wrapper to route through",
        )],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `.rs` file under `src/`, relative to `src/` (e.g. `"jobs/queue.rs"`).
    fn all_source_files() -> Vec<std::path::PathBuf> {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut out = Vec::new();
        let mut stack = vec![src_dir.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("readable source dir") {
                let path = entry.expect("readable dir entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path.strip_prefix(&src_dir).expect("under src").to_owned());
                }
            }
        }
        out
    }

    /// sol diff R1 #8: no unwrapped core in [`DIRECT_PATHS`] is called from
    /// anywhere but its defining file (the wrapper + queue handler pair) or
    /// a documented `extra_allowed_files` exception — every OTHER caller
    /// must go through the instrumented wrapper, or the read model can never
    /// show that work as active. Test files (`_tests.rs`, `tests.rs`, or
    /// anything under a `tests/` directory) are always allowed — they call
    /// the core directly on purpose, to unit-test it without the ledger.
    #[test]
    fn no_unwrapped_core_is_called_outside_its_direct_paths() {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let files = all_source_files();
        let mut violations = Vec::new();

        for direct_path in DIRECT_PATHS {
            // The definition line itself — `fn core_fn(`, `pub fn core_fn(`,
            // `pub async fn core_fn(`, `pub(crate) fn core_fn(`, ... — always
            // contains the substring `"fn core_fn("`, which a CALL site
            // never does; exclude it rather than enumerating every modifier
            // combination. This also naturally excludes another function
            // that merely SHARES the same name in a different file (e.g.
            // `commands::sources::backfill_company_history`, the Tauri
            // command) from being mistaken for a call to the core.
            let call_needle = format!("{}(", direct_path.core_fn);
            let def_needle = format!("fn {}(", direct_path.core_fn);

            for file in &files {
                let file_str = file.to_string_lossy().replace('\\', "/");
                let is_defining_file = file_str == direct_path.defining_file;
                let is_test_file = file_str.ends_with("_tests.rs")
                    || file_str.ends_with("/tests.rs")
                    || file_str == "tests.rs"
                    || file_str.contains("/tests/");
                let is_extra_allowed = direct_path
                    .extra_allowed_files
                    .iter()
                    .any(|(allowed, _reason)| *allowed == file_str);
                if is_defining_file || is_test_file || is_extra_allowed {
                    continue;
                }

                let content =
                    std::fs::read_to_string(src_dir.join(file)).expect("readable source file");
                for (line_no, line) in content.lines().enumerate() {
                    if line.contains(&def_needle) {
                        continue;
                    }
                    if line.contains(&call_needle) {
                        violations.push(format!(
                            "{}:{}: calls unwrapped core `{}` — route through its `_direct` \
                             wrapper in {} instead (sol diff R1 #8): {}",
                            file_str,
                            line_no + 1,
                            direct_path.core_fn,
                            direct_path.defining_file,
                            line.trim()
                        ));
                    }
                }
            }
        }

        assert!(
            violations.is_empty(),
            "unwrapped awaited-path core(s) called outside their DIRECT_PATHS allowlist — the \
             read model would never show this work as active:\n{}",
            violations.join("\n")
        );
    }

    /// One identity-resolution smoke test per [`DIRECT_PATHS`] entry (sol
    /// diff R1 #8): each direct path's synthetic `direct:<...>` job id must
    /// resolve to a real (never `Corrupted`) family — the SAME identity
    /// resolution its `_direct` wrapper calls at `start` time — so a
    /// awaited-path family can never silently regress into an unrecognized
    /// activity_key.
    #[test]
    fn every_direct_path_resolves_a_real_activity_identity() {
        use crate::jobs::activity_identity::{identity_for_job, ActivityFamily};
        use crate::storage::{open_in_memory_database, AppState};

        let state = AppState::new(open_in_memory_database().expect("db"));
        let connection = state.checkout_for_tests().expect("checkout");

        let cases: &[(&str, &str, &str)] = &[
            (
                crate::jobs::scheduler::SOURCE_REFRESH_KIND,
                "direct:gpw-espi-ebi",
                r#"{"adapterId":"gpw-espi-ebi"}"#,
            ),
            (
                crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND,
                "direct:aggregator_fundamentals_pull",
                "{}",
            ),
            (
                crate::jobs::backfill::COMPANY_BACKFILL_KIND,
                "direct:company_backfill:company_x",
                r#"{"companyId":"company_x"}"#,
            ),
            (
                crate::jobs::scheduler::REGISTRY_REFRESH_KIND,
                "direct:registry_refresh",
                "{}",
            ),
        ];
        for (kind, job_id, payload) in cases {
            let identity = identity_for_job(kind, job_id, payload, &connection);
            assert!(
                identity.is_some(),
                "direct path {kind} ({job_id}) must resolve to an identity"
            );
            assert_ne!(
                identity.unwrap().family,
                ActivityFamily::Corrupted,
                "direct path {kind} must not resolve to Corrupted given its real synthetic payload"
            );
        }

        // The transcript direct path resolves via `identity_for_transcript`
        // (a different signature — it takes the row, not a kind/payload
        // pair, since transcripts are never queue jobs).
        let identity = crate::jobs::activity_identity::identity_for_transcript(
            &crate::storage::TranscriptJob {
                id: "job_01".to_owned(),
                company_id: None,
                company: None,
                company_name: None,
                provider_id: "provider_gemini".to_owned(),
                source_type: "youtube_url".to_owned(),
                source_url: "https://www.youtube.com/watch?v=mock".to_owned(),
                source_label: Some("Earnings call".to_owned()),
                company_resolution_status: "unresolved".to_owned(),
                recognized_company_candidates: Vec::new(),
                status: "queued".to_owned(),
                error_code: None,
                created_at: "2026-06-01T10:00:00Z".to_owned(),
                started_at: None,
                finished_at: None,
                error: None,
            },
        );
        assert_eq!(identity.family, ActivityFamily::Transcript);
    }
}
