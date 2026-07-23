import { describe, expect, it } from "vitest";
import type { MorningBriefingItem } from "../../api/generated/MorningBriefingItem";
import { briefingItemText } from "./briefingItemText";

/** A minimal composed briefing item, overridable per test. */
function item(overrides: Partial<MorningBriefingItem> = {}): MorningBriefingItem {
  return {
    id: "b1",
    briefingId: "brief_1",
    position: 0,
    itemType: "signal",
    companyId: "c1",
    domainDate: "2026-07-14",
    citationKey: "b1",
    evidenceType: "company_signal",
    evidenceRef: "ref_1",
    title: "T",
    detail: null,
    createdAt: "2026-07-14T00:00:00Z",
    ...overrides,
  };
}

// A tiny pl dictionary standing in for the real plText map — enough to prove
// the renderer translates rather than leaking codes/tokens or English prose.
const dict: Record<string, string> = {
  "New report processed": "Przetworzono nowy raport",
  "report diff vs the previous statement available":
    "różnica raportu względem poprzedniego dostępna",
  Signal: "Sygnał",
  "Due {period} {year}": "Termin: {period} {year}",
  succeeded: "ukończone",
  "Profit warning / estimate": "Ostrzeżenie o wynikach / szacunki",
};
const pl = (key: string) => dict[key] ?? key;

describe("briefingItemText — the translation seam (ADR 0087 dec. 4)", () => {
  it("renders a tokenized autopilot summary title into the active locale, never the raw token", () => {
    const out = briefingItemText(
      item({ itemType: "autopilot_run", title: "report_diff_available", detail: "succeeded" }),
      pl,
      "pl",
    );
    expect(out).toContain("Przetworzono nowy raport");
    expect(out).toContain("różnica raportu");
    expect(out).not.toContain("report_diff_available");
  });

  it("renders attention-event codes as translated labels, never the raw code join", () => {
    const out = briefingItemText(
      item({ itemType: "attention_event", title: "signal_category", detail: "company_signal" }),
      pl,
      "pl",
    );
    expect(out).toContain("Sygnał");
    expect(out).not.toContain("signal_category");
    expect(out).not.toContain("company_signal");
  });

  it("passes a legacy English-prose autopilot title through verbatim (tolerant read)", () => {
    const legacy = "Autopilot run completed";
    const out = briefingItemText(
      item({ itemType: "autopilot_run", title: legacy, detail: "succeeded" }),
      pl,
      "pl",
    );
    expect(out).toContain(legacy);
  });

  it("renders the claim due token as a translated due phrase, and tolerates legacy prose", () => {
    const tokenOut = briefingItemText(
      item({ itemType: "claim_due", title: "Zwiększymy marżę", detail: "due:Q2:2026" }),
      pl,
      "pl",
    );
    expect(tokenOut).toContain("Zwiększymy marżę");
    expect(tokenOut).toContain("Termin: Q2 2026");
    expect(tokenOut).not.toContain("due:Q2:2026");

    const legacyOut = briefingItemText(
      item({ itemType: "claim_due", title: "Zwiększymy marżę", detail: "Due Q2 2026" }),
      pl,
      "pl",
    );
    expect(legacyOut).toContain("Due Q2 2026"); // legacy prose passes through verbatim
  });

  it("renders the signal category code through the shared display formatter", () => {
    const out = briefingItemText(
      item({ itemType: "signal", title: "Spadek zysku", detail: "profit_warning" }),
      pl,
      "pl",
    );
    expect(out).toContain("Spadek zysku");
    expect(out).toContain("Ostrzeżenie o wynikach / szacunki");
    expect(out).not.toContain("profit_warning");
  });

  it("renders report_date items as title plus qualified ticker", () => {
    const out = briefingItemText(
      item({ itemType: "report_date", title: "Q2 report", detail: "GPW:CDR" }),
      pl,
      "pl",
    );
    expect(out).toContain("Q2 report");
    expect(out).toContain("GPW:CDR");
  });
});
