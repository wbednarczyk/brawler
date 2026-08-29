import { describe, expect, it } from "vitest";

import type { AlertRule, AttentionEvent } from "../../api/attention";
import { eventDescription, eventWhat, ruleTitle } from "./alertLabels";
import { makeAlertRule } from "../../test/scenarios/entities";

// Every `AlertRule.triggerType` / `AttentionEvent.triggerType` member: the
// `satisfies Record<…, true>` objects fail to compile when the generated
// bindings gain a member this list does not name.
const RULE_TRIGGER_TYPES = {
  "signal_category": true,
  "autopilot_run_completed": true,
  "price_enters_range": true,
  "price_week52_low": true,
} satisfies Record<AlertRule["triggerType"], true>;
const ALL_RULE_TRIGGER_TYPES = Object.keys(RULE_TRIGGER_TYPES) as AlertRule["triggerType"][];

const EVENT_TRIGGER_TYPES = {
  "signal_category": true,
  "autopilot_run_completed": true,
  "price_enters_range": true,
  "price_week52_low": true,
  "source_reconciliation": true,
  "job_failed": true,
} satisfies Record<AttentionEvent["triggerType"], true>;
const ALL_EVENT_TRIGGER_TYPES = Object.keys(EVENT_TRIGGER_TYPES) as AttentionEvent["triggerType"][];

// A stand-in `text()`: returns the key untouched (same identity every real
// locale resolves a KNOWN key through) so the assertion below is exactly
// "did a raw backend token reach the label", independent of EN/PL copy.
const text = (key: string) => key;

function attentionEvent(triggerType: AttentionEvent["triggerType"]): AttentionEvent {
  return {
    id: "event-1",
    ruleId: null,
    triggerType,
    companyId: "company-1",
    evidenceType: "company_signal",
    evidenceRef: "ref-1",
    firedAt: "2026-08-01T00:00:00Z",
    seen: false,
    dismissed: false,
    severity: "urgent",
    evidenceTitle: null,
    evidenceDetail: null,
    witnessUrl: null,
  };
}

// Guards against the D3 class: a raw `snake_case` backend token surfacing as
// if it were already a human label (issue #71).
const SNAKE_CASE = /[a-z]+_[a-z]+/;

describe("alertLabels — no raw backend enum ever reaches the DOM )", () => {
  it.each(ALL_RULE_TRIGGER_TYPES)("ruleTitle never leaks the raw triggerType (%s)", (triggerType) => {
    const rule = makeAlertRule("rule-1", triggerType, "company-1");
    const label = ruleTitle(rule, text);
    expect(label).not.toMatch(SNAKE_CASE);
    expect(label).not.toBe(triggerType);
  });

  it.each(ALL_EVENT_TRIGGER_TYPES)("eventWhat never leaks the raw triggerType (%s)", (triggerType) => {
    const label = eventWhat(attentionEvent(triggerType), text);
    expect(label).not.toMatch(SNAKE_CASE);
    expect(label).not.toBe(triggerType);
  });

  it.each(ALL_EVENT_TRIGGER_TYPES)(
    "eventDescription never leaks the raw triggerType or evidenceType (%s)",
    (triggerType) => {
      const description = eventDescription(attentionEvent(triggerType), text, new Map());
      expect(description).not.toMatch(SNAKE_CASE);
    },
  );
});
