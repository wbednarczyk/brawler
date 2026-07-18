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
    ...overrides,
  };
}

describe("attentionEventLabels — source_reconciliation (ADR 0069 D2)", () => {
  it("labels the reconciliation badge and title without a rule", () => {
    const reconciliation = event({});
    // A system event carries no owning rule; the labels must still resolve.
    expect(attentionEventBadgeText(reconciliation, text)).toBe("Reconciliation");
    expect(attentionEventTitleText(reconciliation, undefined, text)).toBe(
      "Official report missed by the primary source",
    );
  });

  it("still labels rule-backed triggers", () => {
    const signal = event({ triggerType: "signal_category", ruleId: "rule_1", evidenceType: "company_signal" });
    expect(attentionEventBadgeText(signal, text)).toBe("Signal");
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
