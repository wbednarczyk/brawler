import { describe, expect, it } from "vitest";

import {
  applyScenarioOverlays,
  PRUNED_CLEAN_SNAPSHOT,
  PRUNED_GLUED_SNAPSHOT,
  type ScenarioOverlayName,
} from "./overlays";
import { buildScenario, type ScenarioName } from "./scenarios";

const ALL_OVERLAYS: readonly ScenarioOverlayName[] = [
  "hostile-content",
  "dense-history",
  "partial-data",
  "stale-processing",
  "conflicting-statuses",
  "mixed-locale",
  "attention-overflow",
  "attention-mixed-severity",
  "attention-notable-only",
  "morning-review",
  "today-dense",
  "orphaned-evidence",
  "pruned-feed",
];
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
