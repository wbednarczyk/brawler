// Every registered durable-queue job `kind` (ADR 0109, #133 gate parity).
// Hand-maintained — the Rust gate `job_kind_list_matches_registry`
// (`jobs::activity_identity::tests`) reads this file from disk and asserts
// set equality with `handlers::build_worker(state).registered_kinds()`, so a
// new/retired kind reddens here until this list (and its
// `formatJobKindDisplayName` label, `labels.test.ts`) is updated.
export const JOB_KINDS: readonly string[] = [
  "morning_briefing",
  "history_sweep",
  "pipeline_reextraction",
  "ownership_extraction",
  "management_holdings_extraction",
  "autopilot_stage",
  "scheduled_source_refresh",
  "source_company_refresh",
  "scheduled_registry_refresh",
  "quote_backfill",
  "company_backfill",
  "fx_daily_pull",
  "aggregator_fundamentals_pull",
  "kpi_ingest_validate",
  "kpi_ingest_commit",
];
