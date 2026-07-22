import { describe, expect, it } from "vitest";

import { applyScenarioOverlays, type ScenarioOverlayName } from "./overlays";
import { buildScenario, type ScenarioName } from "./scenarios";

const ALL_OVERLAYS: readonly ScenarioOverlayName[] = [
  "hostile-content",
  "dense-history",
  "partial-data",
  "stale-processing",
  "conflicting-statuses",
  "mixed-locale",
  "attention-overflow",
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
