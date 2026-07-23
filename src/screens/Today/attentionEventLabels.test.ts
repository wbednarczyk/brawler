import { describe, expect, it } from "vitest";
import type { AlertRule, AttentionEvent } from "../../api/attention";
import { attentionEventBadgeText, attentionEventTitleText } from "./attentionEventLabels";

// Identity translator (asserts the English source strings; pl coverage is the
// translation-completeness ratchet's job).
const text = (key: string) => key;

function event(overrides: Partial<AttentionEvent>): AttentionEvent {
  return {
    id: "attn_1",
    ruleId: null,
    triggerType: "source_reconciliation",
    companyId: "company_1",
    evidenceType: "source_reconciliation",
    evidenceRef: "recon_1",
    firedAt: "2026-07-14T00:00:00Z",
    seen: false,
    dismissed: false,
    // Typed severity now on the payload (ADR 0087 dec. 2); inert for label tests.
    severity: "notable",
    // Evidence specifics from the backend join (v0.60 D6); overridden per case.
    evidenceTitle: null,
    evidenceDetail: null,
    ...overrides,
  };
}

describe("attentionEventLabels — source_reconciliation (ADR 0069 D2)", () => {
  it("labels the reconciliation badge", () => {
    // A system event carries no owning rule; the badge must still resolve.
    expect(attentionEventBadgeText(event({}), text)).toBe("Reconciliation");
  });

  it("states the missed report title + the registry that caught it (v0.60 D6)", () => {
    const reconciliation = event({
      evidenceTitle: "Raport bieżący 15/2026 — zawarcie znaczącej umowy",
      evidenceDetail: "GPW ESPI/EBI",
    });
    expect(attentionEventTitleText(reconciliation, undefined, text)).toBe(
      "Raport bieżący 15/2026 — zawarcie znaczącej umowy — missed by the primary source, backfilled from GPW ESPI/EBI",
    );
  });

  it("falls back to the generic reconciliation copy when no title is present", () => {
    expect(attentionEventTitleText(event({}), undefined, text)).toBe(
      "Official report missed by the primary source",
    );
  });

  it("still labels rule-backed triggers", () => {
    const signal = event({ triggerType: "signal_category", ruleId: "rule_1", evidenceType: "company_signal" });
    expect(attentionEventBadgeText(signal, text)).toBe("Signal");
  });
});

describe("attentionEventLabels — concrete statements (v0.60 D6)", () => {
  it("a signal event states its own filing title, not the category", () => {
    const signal = event({
      triggerType: "signal_category",
      ruleId: "rule_1",
      evidenceType: "company_signal",
      evidenceTitle: "Wstępne wyniki produkcyjne i sprzedażowe za czerwiec 2026",
    });
    // Even with a rule/category available, the specific filing title wins.
    expect(
      attentionEventTitleText(signal, rule({ signalCategory: "profit_warning" }), text),
    ).toBe("Wstępne wyniki produkcyjne i sprzedażowe za czerwiec 2026");
  });

  it("an autopilot-completion event names the processed report and its status", () => {
    const autopilot = event({
      triggerType: "autopilot_run_completed",
      evidenceType: "autopilot_run",
      evidenceTitle: "Skonsolidowany raport kwartalny Q2 2026",
      evidenceDetail: "succeeded",
    });
    expect(attentionEventTitleText(autopilot, undefined, text)).toBe(
      "Skonsolidowany raport kwartalny Q2 2026 — Succeeded",
    );
  });

  it("an autopilot-completion event falls back to generic copy without a title", () => {
    const autopilot = event({
      triggerType: "autopilot_run_completed",
      evidenceType: "autopilot_run",
    });
    expect(attentionEventTitleText(autopilot, undefined, text)).toBe("Autopilot finished");
  });
});

function rule(overrides: Partial<AlertRule>): AlertRule {
  return {
    id: "rule_1",
    triggerType: "signal_category",
    signalCategory: null,
    priceMin: null,
    priceMax: null,
    scopeType: "company",
    scopeRef: "company_1",
    enabled: true,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

// D3 fix (v0.57 fix wave 2, owner screenshot 27-toast-stack.png): a
// signal-category toast/row showed the raw backend enum code
// ("insider_transaction") verbatim instead of its human display name — never
// translated in EITHER locale (the English default is a passthrough too), not
// just "untranslated in Polish". The category → display-name mapping must
// match the backend's `signal_categories.display_name` seed exactly (see
// migrations 0041/0044/0079/0080/0085/0092) so `text()` resolves the same
// locale entries `SignalBadges`/`text(signal.categoryDisplayName)` already use
// — never a second, drifting copy of the same names.
describe("attentionEventLabels — signal_category display name (D3 fix)", () => {
  it("renders the category's human display name, never the raw enum code", () => {
    const signal = event({
      triggerType: "signal_category",
      ruleId: "rule_1",
      evidenceType: "company_signal",
    });
    expect(
      attentionEventTitleText(signal, rule({ signalCategory: "insider_transaction" }), text),
    ).toBe("Insider transaction");
    expect(
      attentionEventTitleText(signal, rule({ signalCategory: "major_holdings_change" }), text),
    ).toBe("Major holdings change");
    expect(
      attentionEventTitleText(signal, rule({ signalCategory: "own_shares" }), text),
    ).toBe("Own-share transactions");
  });

  it("falls back to the raw trigger type only when the rule (and thus the category) is unknown", () => {
    const signal = event({ triggerType: "signal_category", ruleId: null, evidenceType: "company_signal" });
    expect(attentionEventTitleText(signal, undefined, text)).toBe("signal_category");
  });
});
