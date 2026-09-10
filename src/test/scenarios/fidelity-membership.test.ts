import { describe, expect, it } from "vitest";

import baseline from "./fidelity-membership.baseline.json";
import corpus from "./fidelity-corpus.json";
import headlessOnly from "./headless-only.json";

// Fidelity-corpus MEMBERSHIP gate (ADR 0049 / testing.md § Mock-runtime
// fidelity) — distinct from fidelity.test.ts (which replays the corpus).
// This test asks a narrower question: does every `#[tauri::command]` in the
// Rust source have an accounted-for fate? A command is accounted for when it
// is one of:
//   1. a corpus step (`fidelity-corpus.json`) — the dual-execution contract
//      actually exercises it;
//   2. declared `headless-only.json` — no mock half exists (MCP-only or a
//      headless acquisition driver), with a one-line reason;
//   3. a `fidelity-membership.baseline.json` entry — today's known offender,
//      the ratchet floor. The baseline may only SHRINK: a command that has
//      since gained a corpus step must be removed from it in the same change.
// A command missing from all three fails the build with the three ways out.

// Rust sources read through Vite's glob (the repo's pattern for source-scan
// tests — the frontend tsconfig carries no Node types), eager + raw.
const rustSources = import.meta.glob("/src-tauri/src/**/*.rs", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

function extractCommandNames(): string[] {
  const names: string[] = [];
  for (const [, content] of Object.entries(rustSources)) {
    for (const match of content.matchAll(/#\[tauri::command\]/g)) {
      // The attribute is followed by 0-3 lines of doc comments/other
      // attributes before `fn <name>` — scan the next few lines rather than
      // assuming it is the very next token.
      const after = content.slice(match.index! + match[0].length);
      const window = after.split("\n").slice(0, 4).join("\n");
      const fnMatch = window.match(/fn\s+([A-Za-z0-9_]+)/);
      if (fnMatch) names.push(fnMatch[1]);
    }
  }
  return names;
}

function corpusCommandNames(): Set<string> {
  const names = new Set<string>();
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      value.forEach(walk);
    } else if (value && typeof value === "object") {
      const obj = value as Record<string, unknown>;
      if (typeof obj.command === "string") names.add(obj.command);
      Object.values(obj).forEach(walk);
    }
  };
  walk(corpus);
  return names;
}

// B1 (owner-approved hard gate 2026-09-07): the baseline is today's known
// offender list, not a way for a NEW offender to go green — the only ways
// out of the "every command must be accounted for" test below are a corpus
// step or a headless-only declaration. This ceiling is the second lock: it
// may only be LOWERED as entries are fixed, never raised to fit a new one.
const BASELINE_CEILING = 116;

// B1 fix (owner-approved hard gate 2026-09-07, Astra re-verification of PR
// #477 finding (a)): the ceiling alone permits regrowth-by-replacement — drop
// one baseline entry, add a different new offender, and the count still
// passes <= BASELINE_CEILING. This frozen set is the primary lock: the exact
// 117 approved offenders as of today (generated from
// fidelity-membership.baseline.json, sorted). Any baseline name outside this
// set is a NEW offender — it needs a fidelity-corpus step or a
// headless-only.json declaration, never a baseline swap. Edit this set only
// to REMOVE a name once it is fixed; the ceiling above stays a second lock.
const FROZEN_BASELINE = new Set([
  "add_company_to_watchlist",
  "apply_research_import",
  "apply_settings_import",
  "backfill_company_history",
  "backfill_ownership_extraction",
  "backup_status",
  "clear_diagnostic_events",
  "clear_license_key",
  "clear_provider_api_key",
  "clone_framework",
  "confirm_company_signal",
  "confirm_derived_event",
  "create_backup",
  "create_evidence_link",
  "create_kpi_definition",
  "create_management_claim",
  "create_note_from_transcript_selection",
  "create_notebook_entry",
  "create_research_question",
  "create_research_reminder",
  "database_status",
  "delete_evidence_link",
  "delete_financial_fact",
  "delete_framework_criterion",
  "delete_framework_evaluation",
  "delete_notebook_entry",
  "delete_quality_framework",
  "delete_research_question",
  "delete_research_reminder",
  "delete_video_transcript_job",
  "disable_developer_mode",
  "evaluate_framework",
  "export_research_data",
  "export_settings_data",
  "extract_report_document_data",
  "extract_report_sections",
  "fetch_report_document",
  "get_backfill_progress",
  "get_company_ir_reports_url",
  "get_company_sector",
  "get_diagnostic_summary",
  "get_license_status",
  "get_local_metrics_snapshot",
  "get_log_status",
  "get_pre_report_card",
  "get_provider_credential_status",
  "get_report_diff",
  "get_scheduler_status",
  "get_settings",
  "health",
  "list_available_metric_keys",
  "list_claims_to_verify",
  "list_company_events",
  "list_company_registry_entries",
  "list_company_sectors",
  "list_company_signals",
  "list_company_timeline",
  "list_diagnostic_events",
  "list_evidence_links",
  "list_fact_provenance",
  "list_feed_items",
  "list_financial_facts",
  "list_financial_periods",
  "list_framework_evaluations",
  "list_kpi_definitions",
  "list_kpi_relevance",
  "list_log_entries",
  "list_management_claims",
  "list_notebook_entries",
  "list_quality_frameworks",
  "list_report_diff_candidates",
  "list_report_season",
  "list_research_evidence",
  "list_research_questions",
  "list_research_reminders",
  "list_source_adapters",
  "list_source_reconciliation",
  "list_transcript_segments",
  "list_video_transcript_jobs",
  "list_watchlist_memberships",
  "lookup_company",
  "mark_report_prepared",
  "mark_report_processed",
  "mark_research_scope_reviewed",
  "open_logs_directory",
  "preview_research_import",
  "preview_settings_import",
  "promote_uncrosswalked_concept",
  "refresh_gpw_company_registry",
  "refresh_gpw_company_registry_if_stale",
  "refresh_source",
  "refresh_sources",
  "reject_company_signal",
  "remove_company_from_watchlist",
  "rerun_extraction_outcome",
  "reset_framework_to_template",
  "resolve_transcript_job_company",
  "restore_backup",
  "run_video_transcript_job",
  "search",
  "set_autopilot_run_notification_state",
  "set_claim_verdict",
  "set_company_ir_reports_url",
  "set_company_sector",
  "set_provider_api_key",
  "set_source_adapter_enabled",
  "submit_license_key",
  "undo_autopilot_run",
  "unlock_developer_mode",
  "update_feed_item_state",
  "update_notebook_entry",
  "update_research_question",
  "update_research_reminder",
  "update_settings",
  "update_video_transcript_job",
  "validate_criterion_expression",
]);

describe("fidelity-corpus membership (ADR 0049)", () => {
  const registered = new Set(extractCommandNames());
  const inCorpus = corpusCommandNames();
  const headlessNames = new Set(Object.keys(headlessOnly as Record<string, string>));
  const baselineNames = new Set(baseline as string[]);

  it("found the expected commands and manifest sizes (sanity check)", () => {
    expect(registered.size).toBeGreaterThan(0);
    expect(inCorpus.size).toBeGreaterThan(0);
  });

  it("every #[tauri::command] is a corpus step, declared headless-only, or a known baseline offender", () => {
    const unaccounted = [...registered].filter(
      (name) => !inCorpus.has(name) && !headlessNames.has(name) && !baselineNames.has(name),
    );
    expect(
      unaccounted,
      `${unaccounted.length} command(s) have no fidelity-corpus step, no headless-only.json ` +
        "entry, and are not in the baseline. The baseline is NOT a way to go green for a NEW " +
        "offender — fix by (a) adding a journey step to src/test/scenarios/fidelity-corpus.json, " +
        "or (b) declaring the command in src/test/scenarios/headless-only.json with a one-line " +
        "reason (no mock half — MCP-only or a headless acquisition driver). " +
        `Unaccounted: ${unaccounted.join(", ")}`,
    ).toEqual([]);
  });

  it("the baseline ratchet only shrinks (an entry now covered by the corpus must be removed)", () => {
    const stale = [...baselineNames].filter((name) => inCorpus.has(name));
    expect(
      stale,
      `${stale.length} baseline entr(y/ies) already have a fidelity-corpus step ` +
        `and must be removed from fidelity-membership.baseline.json: ${stale.join(", ")}`,
    ).toEqual([]);
  });

  it("the baseline ceiling only shrinks, never grows (BASELINE_CEILING is the floor for new offenders)", () => {
    expect(
      baselineNames.size,
      `fidelity-membership.baseline.json has ${baselineNames.size} entries, above ` +
        `BASELINE_CEILING (${BASELINE_CEILING}) in fidelity-membership.test.ts. The ceiling may ` +
        "only be lowered as entries are fixed, never raised — a new offender must go through the " +
        "corpus or headless-only.json instead, never the baseline.",
    ).toBeLessThanOrEqual(BASELINE_CEILING);
  });

  it("the baseline is a subset of the frozen approved set (no new offender via swap)", () => {
    const unfrozen = [...baselineNames].filter((name) => !FROZEN_BASELINE.has(name));
    expect(
      unfrozen,
      `${unfrozen.length} baseline entr(y/ies) are not in FROZEN_BASELINE — a new command cannot ` +
        "be added to fidelity-membership.baseline.json by swapping out a fixed entry. It needs a " +
        "fidelity-corpus step or a headless-only.json declaration instead. " +
        `Unfrozen: ${unfrozen.join(", ")}`,
    ).toEqual([]);
  });

  it("the frozen approved set and the baseline are EQUAL (an offender that gained coverage leaves both — a reviewed edit)", () => {
    const orphaned = [...FROZEN_BASELINE].filter((name) => !baselineNames.has(name));
    expect(
      orphaned,
      `${orphaned.length} FROZEN_BASELINE entr(y/ies) are no longer in fidelity-membership.baseline.json — ` +
        "delete them from FROZEN_BASELINE in this test too (review 2026-09-08: otherwise a command that " +
        "lost its corpus step could silently re-enter the baseline). Orphaned: " +
        orphaned.join(", "),
    ).toEqual([]);
  });

  it("every baseline entry is still a real #[tauri::command] (stale entries must be removed)", () => {
    const stale = [...baselineNames].filter((name) => !registered.has(name));
    expect(
      stale,
      `${stale.length} baseline entr(y/ies) no longer name a #[tauri::command] — remove them ` +
        `from fidelity-membership.baseline.json: ${stale.join(", ")}`,
    ).toEqual([]);
  });

  it("every headless-only.json key is a real #[tauri::command]", () => {
    const stale = [...headlessNames].filter((name) => !registered.has(name));
    expect(
      stale,
      `${stale.length} headless-only.json entr(y/ies) no longer name a #[tauri::command] — ` +
        `remove them: ${stale.join(", ")}`,
    ).toEqual([]);
  });

  it("every headless-only.json value is a non-empty reason", () => {
    const empty = Object.entries(headlessOnly as Record<string, string>)
      .filter(([, reason]) => reason.trim().length === 0)
      .map(([name]) => name);
    expect(
      empty,
      `headless-only.json entr(y/ies) with an empty reason: ${empty.join(", ")}`,
    ).toEqual([]);
  });
});
