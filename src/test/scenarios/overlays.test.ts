import { describe, expect, it } from "vitest";

import {
  applyScenarioOverlays,
  DEGRADED_MEDIA_LAST_ERROR,
  DEGRADED_REPORTS_DETAIL_WARNING,
  DEGRADED_REPORTS_LAST_ERROR,
  FAILED_RUN_LAST_ERROR,
  FAILED_RUN_REPORT_TITLE,
  JOB_FAILED_KIND,
  JOB_FAILED_SUBJECT,
  JOB_FAILED_SYSTEM_ERROR,
  JOB_FAILED_SYSTEM_KIND,
  PRUNED_CLEAN_SNAPSHOT,
  PRUNED_GLUED_SNAPSHOT,
  SCENARIO_OVERLAY_NAMES,
  type ScenarioOverlayName,
} from "./overlays";
import { buildScenario, type ScenarioName } from "./scenarios";

// The canonical list itself — never hand-copied, so a new overlay is composed,
// idempotence-checked, and cross-base-checked the moment it is registered.
const ALL_OVERLAYS: readonly ScenarioOverlayName[] = SCENARIO_OVERLAY_NAMES;
const BASES: readonly ScenarioName[] = ["empty", "minimal", "rich"];

describe("scenario overlays — composition (ADR 0081 Q2, Radicle a9992e2)", () => {
  for (const base of BASES) {
    for (const overlay of ALL_OVERLAYS) {
      it(`${overlay} composes with ${base} without throwing`, () => {
        expect(() => buildScenario({ base, overlays: [overlay] })).not.toThrow();
      });
    }
  }

  it("composes with empty/minimal/rich together (all overlays at once)", () => {
    for (const base of BASES) {
      expect(() => buildScenario({ base, overlays: ALL_OVERLAYS })).not.toThrow();
    }
  });

  it("duplicate overlay names are idempotent", () => {
    const once = buildScenario({ base: "minimal", overlays: ["hostile-content"] });
    const duped = buildScenario({ base: "minimal", overlays: ["hostile-content", "hostile-content", "hostile-content"] });
    expect(duped).toEqual(once);
  });

  it("overlay application order does not affect the result (fixed canonical order)", () => {
    const orderA = buildScenario({ base: "minimal", overlays: ["hostile-content", "dense-history"] });
    const orderB = buildScenario({ base: "minimal", overlays: ["dense-history", "hostile-content"] });
    expect(orderB).toEqual(orderA);
  });

  it("buildScenario still accepts the bare ScenarioName (pre-Q2 callers)", () => {
    const viaName = buildScenario("minimal");
    const viaSpec = buildScenario({ base: "minimal" });
    expect(viaSpec).toEqual(viaName);
  });

  it("two builds from the same spec are deeply isolated (no shared references)", () => {
    const a = buildScenario({ base: "minimal", overlays: ["hostile-content"] });
    const b = buildScenario({ base: "minimal", overlays: ["hostile-content"] });
    a.feedItems.push({ ...a.feedItems[0], id: "mutated_only_in_a" });
    expect(b.feedItems.some((item) => item.id === "mutated_only_in_a")).toBe(false);
  });

  it("applyScenarioOverlays is pure — never mutates the input data", () => {
    const base = buildScenario("minimal");
    const before = base.feedItems;
    const result = applyScenarioOverlays(base, ["hostile-content"]);
    expect(base.feedItems).toBe(before);
    expect(result.feedItems).not.toBe(before);
  });
});

describe("scenario overlays — required content", () => {
  it("hostile-content: unbreakable URL, long strings, and Polish diacritics", () => {
    const data = buildScenario({ base: "minimal", overlays: ["hostile-content"] });
    const item = data.feedItems.find((feedItem) => feedItem.id === "feed_overlay_hostile_1");
    expect(item).toBeTruthy();
    expect(item!.sourceUrl).not.toMatch(/\s/);
    expect(item!.sourceUrl.length).toBeGreaterThan(80);
    expect(item!.title.length).toBeGreaterThan(80);
    expect(item!.title).toMatch(/[żółśąęćłńŻÓŁŚĄĘĆŃ]/);
  });

  it("dense-history: hundreds of rows ONLY when explicitly selected", () => {
    const plain = buildScenario({ base: "minimal", overlays: ["hostile-content"] });
    const dense = buildScenario({ base: "minimal", overlays: ["dense-history"] });
    const plainDenseRows = plain.feedItems.filter((item) => item.id.startsWith("feed_overlay_dense_"));
    const denseRows = dense.feedItems.filter((item) => item.id.startsWith("feed_overlay_dense_"));
    expect(plainDenseRows).toHaveLength(0);
    expect(denseRows.length).toBeGreaterThanOrEqual(200);
  });

  it("partial-data: financial period exists but the matching financial fact is missing", () => {
    const data = buildScenario({ base: "minimal", overlays: ["partial-data"] });
    const period = data.financialPeriods.find((p) => p.id.includes("partial"));
    expect(period).toBeTruthy();
    const fact = data.financialFacts.find((f) => f.companyId === period!.companyId);
    expect(fact).toBeUndefined();
  });

  it("stale-processing: an old research result stays visible", () => {
    const data = buildScenario({ base: "minimal", overlays: ["stale-processing"] });
    const evidence = data.researchEvidence.find((e) => e.id === "research_overlay_stale_1");
    expect(evidence).toBeTruthy();
  });

  it("conflicting-statuses: adapter health and its latest ingestion result disagree", () => {
    const data = buildScenario({ base: "minimal", overlays: ["conflicting-statuses"] });
    const adapter = data.sourceAdapters.find((a) => a.id === "bankier-company-komunikaty");
    const ingestion = data.lastIngestionResults.find((r) => r.adapterId === "bankier-company-komunikaty");
    expect(adapter?.healthStatus).toBe("attention");
    expect(adapter?.lastError).toBeTruthy();
    expect(ingestion?.detailItemsFailed).toBe(0);
    expect(ingestion?.itemsFetched).toBeGreaterThan(0);
  });

  it("attention-overflow: 20 additional unseen attention events (persistent-toast cap regression)", () => {
    const data = buildScenario({ base: "minimal", overlays: ["attention-overflow"] });
    const overflowEvents = data.attentionEvents.filter((event) => event.id.startsWith("attn_overlay_overflow_"));
    expect(overflowEvents.length).toBe(20);
    expect(overflowEvents.every((event) => !event.seen && !event.dismissed)).toBe(true);
  });

  it("attention-mixed-severity: exactly 5 urgent + 15 notable, owning the attention set (ADR 0087 dec. 3)", () => {
    const data = buildScenario({ base: "rich", overlays: ["attention-mixed-severity"] });
    const urgent = data.attentionEvents.filter((e) => e.severity === "urgent");
    const notable = data.attentionEvents.filter((e) => e.severity === "notable");
    // Owns the set (replaces base) — no base-scenario urgent perturbs the mix.
    expect(data.attentionEvents.length).toBe(20);
    expect(urgent.length).toBe(5);
    expect(notable.length).toBe(15);
    expect(data.attentionEvents.every((e) => !e.seen && !e.dismissed)).toBe(true);
  });

  it("attention-notable-only: many notable events, ZERO urgent (no persistent toast can fire)", () => {
    const data = buildScenario({ base: "rich", overlays: ["attention-notable-only"] });
    expect(data.attentionEvents.length).toBeGreaterThanOrEqual(10);
    expect(data.attentionEvents.some((e) => e.severity === "urgent")).toBe(false);
    expect(data.attentionEvents.every((e) => e.severity === "notable")).toBe(true);
  });

  it("morning-review: 2 urgent + a notable ×2 group + 6 routine autopilot runs (J1 seed, ADR 0087)", () => {
    const data = buildScenario({ base: "rich", overlays: ["morning-review"] });
    const urgent = data.attentionEvents.filter((e) => e.id.startsWith("attn_mr_") && e.severity === "urgent");
    const group = data.attentionEvents.filter((e) => e.id.startsWith("attn_mr_group_"));
    const runs = data.autopilotRuns.filter((r) => r.id.startsWith("run_mr_"));
    expect(urgent.length).toBe(2);
    // The reconciliation is a system event (no user rule).
    expect(data.attentionEvents.some((e) => e.id === "attn_mr_recon" && e.ruleId === null)).toBe(true);
    expect(group.length).toBe(2);
    expect(group.every((e) => e.severity === "notable")).toBe(true);
    expect(runs.length).toBe(6);
    expect(runs.every((r) => r.severity === "routine")).toBe(true);
  });

  it("today-dense: one routine autopilot run per company (a wall that must aggregate)", () => {
    const data = buildScenario({ base: "rich", overlays: ["today-dense"] });
    const denseRuns = data.autopilotRuns.filter((r) => r.id.startsWith("run_dense_"));
    expect(denseRuns.length).toBe(data.companies.length);
    expect(denseRuns.length).toBeGreaterThanOrEqual(3);
    expect(denseRuns.every((r) => r.severity === "routine")).toBe(true);
  });

  it("orphaned-evidence: signal events with a null title AND a dangling (unresolved) rule id", () => {
    const data = buildScenario({ base: "rich", overlays: ["orphaned-evidence"] });
    const orphans = data.attentionEvents.filter((e) => e.id.startsWith("attn_overlay_orphan_"));
    expect(orphans.length).toBeGreaterThanOrEqual(2);
    // The orphan STATE: no surviving snapshot title, and a rule id that resolves to
    // no alertRules row (cascade-pruned) — the FE must fall back, never crash/blank.
    expect(orphans.every((e) => e.evidenceTitle === null)).toBe(true);
    expect(orphans.every((e) => e.triggerType === "signal_category")).toBe(true);
    const ruleIds = new Set(data.alertRules.map((r) => r.id));
    expect(orphans.every((e) => e.ruleId !== null && !ruleIds.has(e.ruleId))).toBe(true);
    // And the cited signal is gone (no matching signal row).
    const signalIds = new Set(data.signals.map((s) => s.id));
    expect(orphans.every((e) => !signalIds.has(e.evidenceRef))).toBe(true);
  });

  it("pruned-feed: snapshot titles survive though the live feed item is gone (clean + glued)", () => {
    const data = buildScenario({ base: "rich", overlays: ["pruned-feed"] });
    const clean = data.attentionEvents.find((e) => e.id === "attn_overlay_pruned_clean");
    const glued = data.attentionEvents.find((e) => e.id === "attn_overlay_pruned_glued");
    expect(clean?.evidenceTitle).toBe(PRUNED_CLEAN_SNAPSHOT);
    expect(glued?.evidenceTitle).toBe(PRUNED_GLUED_SNAPSHOT);
    // The cited feed items are pruned (no matching feed row).
    const feedIds = new Set(data.feedItems.map((f) => f.id));
    expect(feedIds.has(clean!.evidenceRef)).toBe(false);
    expect(feedIds.has(glued!.evidenceRef)).toBe(false);
  });

  it("failed-autopilot-run: the run, its completion event and the rule it fired from agree", () => {
    const data = buildScenario({ base: "rich", overlays: ["failed-autopilot-run"] });
    const run = data.autopilotRuns.find((r) => r.id === "run_overlay_failed_1");
    const event = data.attentionEvents.find((e) => e.id === "attn_overlay_failed_run");

    // The run's triple: failed + a concrete cause + the notable severity the
    // backend derives from that status (`severity_for_autopilot_run`).
    expect(run?.status).toBe("failed");
    expect(run?.lastError).toBe(FAILED_RUN_LAST_ERROR);
    expect(run?.severity).toBe("notable");
    // Still an unread notification, and it names the report it choked on.
    expect(run?.notificationState).toBe("unread");
    expect(run?.reportDocumentTitle).toBe(FAILED_RUN_REPORT_TITLE);
    // A failed run produced nothing — no facts to undo.
    expect(run?.producedFactIds).toEqual([]);

    // The completion event points at THAT run and carries its raw status as the
    // secondary datum, so the Today row can state "<report> — Failed".
    expect(event?.triggerType).toBe("autopilot_run_completed");
    expect(event?.evidenceType).toBe("autopilot_run");
    expect(event?.evidenceRef).toBe(run!.id);
    expect(event?.evidenceTitle).toBe(FAILED_RUN_REPORT_TITLE);
    expect(event?.evidenceDetail).toBe(run!.status);
    expect(event?.severity).toBe("notable");
    expect(event?.seen).toBe(false);
    expect(event?.dismissed).toBe(false);

    // An autopilot completion is never a system event: its rule must resolve.
    const rule = data.alertRules.find((r) => r.id === event!.ruleId);
    expect(rule?.triggerType).toBe("autopilot_run_completed");
    expect(rule?.enabled).toBe(true);
    expect(rule?.scopeRef).toBe(run!.companyId);
    // ...and the company it happened to is in the store, so the row has a ticker.
    expect(data.companies.some((c) => c.id === run!.companyId)).toBe(true);
  });

  it("job-failed-event: both scopes a terminal job failure can have are coherent system events", () => {
    const data = buildScenario({ base: "rich", overlays: ["job-failed-event"] });
    const scoped = data.attentionEvents.find((e) => e.id === "attn_overlay_job_failed");
    const systemWide = data.attentionEvents.find(
      (e) => e.id === "attn_overlay_job_failed_system",
    );

    for (const event of [scoped, systemWide]) {
      // A job failure is ALWAYS a system event (no user rule can fire one) whose
      // evidence is the job row, notable for every kind (ADR 0091 dec. 1).
      expect(event?.triggerType).toBe("job_failed");
      expect(event?.evidenceType).toBe("job");
      expect(event?.ruleId).toBeNull();
      expect(event?.severity).toBe("notable");
      expect(event?.seen).toBe(false);
      expect(event?.dismissed).toBe(false);
      // The raw kind travels as the detail; the statement is never empty, so the
      // row can NAME the failure instead of rendering a bare category.
      expect(event?.evidenceDetail).toBeTruthy();
      expect(event?.evidenceTitle).toBeTruthy();
    }

    // Company-scoped: the handler named the company + the report it died on, and
    // that company is in the store so the row has a ticker.
    expect(scoped?.evidenceDetail).toBe(JOB_FAILED_KIND);
    expect(scoped?.evidenceTitle).toBe(JOB_FAILED_SUBJECT);
    expect(data.companies.some((c) => c.id === scoped!.companyId)).toBe(true);

    // Workspace-wide: no company at all (nullable since migration 0118), and with
    // no subject the statement is the job's own error text.
    expect(systemWide?.companyId).toBeNull();
    expect(systemWide?.evidenceDetail).toBe(JOB_FAILED_SYSTEM_KIND);
    expect(systemWide?.evidenceTitle).toBe(JOB_FAILED_SYSTEM_ERROR);
  });

  it("degraded-sources: adapters report attention WITH a concrete last error", () => {
    const data = buildScenario({ base: "rich", overlays: ["degraded-sources"] });
    const reports = data.sourceAdapters.find((a) => a.id === "zzz-degraded-official-reports");
    const media = data.sourceAdapters.find((a) => a.id === "zzz-degraded-media-rss");

    for (const adapter of [reports, media]) {
      // The pairing the backend writes: an error string, when it happened, and
      // the health status derived from it — never an error with a green light.
      expect(adapter?.healthStatus).toBe("attention");
      expect(adapter?.lastError).toBeTruthy();
      expect(adapter?.lastErrorAt).toBeTruthy();
      expect(adapter?.enabled).toBe(true);
    }
    expect(reports?.lastError).toBe(DEGRADED_REPORTS_LAST_ERROR);
    expect(media?.lastError).toBe(DEGRADED_MEDIA_LAST_ERROR);

    // The reports adapter is the triage entry point (`openSourceStatus` selects
    // the FIRST adapter with a lastError) and carries the failed-detail counters
    // plus the warning text the Sources detail panel renders.
    expect(data.sourceAdapters.find((a) => a.lastError)?.id).toBe("zzz-degraded-official-reports");
    expect(reports?.lastDetailWarning).toBe(DEGRADED_REPORTS_DETAIL_WARNING);
    expect(reports?.lastDetailItemsFailed).toBe(3);
    expect((reports?.lastDetailItemsStored ?? 0) + (reports?.lastDetailItemsFailed ?? 0)).toBe(
      reports?.lastDetailItemsAttempted,
    );
  });

  it("unknown overlay names fail loudly instead of being silently skipped", () => {
    expect(() =>
      applyScenarioOverlays(buildScenario("minimal"), ["not-an-overlay" as ScenarioOverlayName]),
    ).toThrow(/Unknown scenario overlay/);
  });

  it("mixed-locale: realistic Polish AND English source strings, not planted UI literals", () => {
    const data = buildScenario({ base: "minimal", overlays: ["mixed-locale"] });
    const pl = data.feedItems.find((item) => item.id === "feed_overlay_mixed_pl");
    const en = data.feedItems.find((item) => item.id === "feed_overlay_mixed_en");
    expect(pl?.language).toBe("pl");
    expect(en?.language).toBe("en");
    expect(pl!.title).toMatch(/[żółśąęćłńŻÓŁŚĄĘĆŃ]/);
    expect(en!.title).not.toMatch(/[żółśąęćłńŻÓŁŚĄĘĆŃ]/);
    expect(pl!.title).not.toBe(en!.title);
  });
});
