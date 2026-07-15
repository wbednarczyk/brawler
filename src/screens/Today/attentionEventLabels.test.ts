import { describe, expect, it } from "vitest";
import type { AttentionEvent } from "../../api/attention";
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
