import { describe, expect, it } from "vitest";

import { composeAutopilotRunSummary, type AutopilotRunSummarySource } from "./autopilotRunSummary";
import { plText } from "../../shared/locale/resources/plText";

const identity = (value: string) => value;
const pl = (value: string) => plText[value] ?? value;

const base: AutopilotRunSummarySource = {
  mode: "autopilot",
  kpiDeltaJson: null,
  reportDiffRef: null,
  crossRefsJson: null,
};

describe("composeAutopilotRunSummary", () => {
  it("falls back to a plain 'processed' sentence when nothing is known yet", () => {
    expect(composeAutopilotRunSummary(base, identity, "en")).toBe("New report processed.");
    expect(composeAutopilotRunSummary(base, pl, "pl")).toBe("Przetworzono nowy raport.");
  });

  // Bug e77a1a2 part 1: a live run stored 40 structured facts but the card said
  // "0 KPI auto-confirmed (unreviewed) of 0 extracted" — the honest counts must
  // flow through when the delta actually carries them.
  it("reports the honest auto-confirmed count for an autopilot-mode run", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({
        extractionAvailable: true,
        structured: true,
        factsProposed: 40,
        factsAutoConfirmed: 40,
      }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — 40 KPI auto-confirmed (unreviewed) of 40 extracted.",
    );
  });

  it("reports the pending-confirmation phrasing for an assist-mode run", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      mode: "assist",
      kpiDeltaJson: JSON.stringify({ extractionAvailable: true, factsProposed: 5 }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — 5 KPI extracted, pending your confirmation.",
    );
  });

  it("reports extraction-unavailable regardless of mode", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({ extractionAvailable: false, reason: "no_ai_provider" }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — KPI extraction unavailable (no AI provider configured).",
    );
  });

  // Card b85ba3c: an `extractionAvailable:false` delta carries an honest `reason`
  // (jobs/autopilot.rs, structured_extraction.rs); the card must not flatten every
  // reason into one generic "no AI provider" line.
  it("maps the bootstrap_failed reason to its specific text", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({ extractionAvailable: false, reason: "bootstrap_failed" }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — KPI extraction unavailable (extraction setup failed).",
    );
  });

  it("maps the skipped_budget reason to the AI-budget text", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({ extractionAvailable: false, reason: "skipped_budget" }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — KPI extraction unavailable (AI budget for this run was used up).",
    );
  });

  it("maps a provider_error:<code> reason to the provider-error text", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({ extractionAvailable: false, reason: "provider_error:429" }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — KPI extraction unavailable (the AI provider returned an error).",
    );
  });

  // Never silently generic: an unknown/new backend reason surfaces its raw code
  // so it is visible rather than swallowed into a blanket message.
  it("falls back to a reason-carrying message for an unknown reason", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({ extractionAvailable: false, reason: "xyz" }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — KPI extraction unavailable (xyz).",
    );
  });

  // Never fabricate "0 of 0" for a delta that never actually carried the
  // normalized counts (e.g. a legacy/malformed shape) — exactly bug e77a1a2's
  // symptom, just with the fields missing instead of zeroed.
  it("omits the KPI clause when the delta lacks the normalized count fields", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({ extractionAvailable: true, structured: true }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe("New report processed.");
  });

  it("tolerates malformed kpiDeltaJson without throwing", () => {
    const run: AutopilotRunSummarySource = { ...base, kpiDeltaJson: "{not valid json" };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe("New report processed.");
  });

  it("adds the report-diff clause when a diff ref is recorded", () => {
    const run: AutopilotRunSummarySource = { ...base, reportDiffRef: JSON.stringify({}) };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — report diff vs the previous statement available.",
    );
  });

  it("pluralizes claims to verify (English one/other)", () => {
    const one: AutopilotRunSummarySource = {
      ...base,
      crossRefsJson: JSON.stringify({ claimsOverdue: 1, claimsDue: 0, openQuestions: 0 }),
    };
    const many: AutopilotRunSummarySource = {
      ...base,
      crossRefsJson: JSON.stringify({ claimsOverdue: 2, claimsDue: 1, openQuestions: 0 }),
    };
    expect(composeAutopilotRunSummary(one, identity, "en")).toBe("New report processed — 1 claim to verify.");
    expect(composeAutopilotRunSummary(many, identity, "en")).toBe(
      "New report processed — 3 claims to verify.",
    );
  });

  it("pluralizes claims to verify (Polish three-form)", () => {
    const forms = [1, 2, 5].map((n) =>
      composeAutopilotRunSummary(
        { ...base, crossRefsJson: JSON.stringify({ claimsOverdue: n, claimsDue: 0, openQuestions: 0 }) },
        pl,
        "pl",
      ),
    );
    expect(forms[0]).toBe("Przetworzono nowy raport — 1 obietnica do weryfikacji.");
    expect(forms[1]).toBe("Przetworzono nowy raport — 2 obietnice do weryfikacji.");
    expect(forms[2]).toBe("Przetworzono nowy raport — 5 obietnic do weryfikacji.");
  });

  it("pluralizes open research questions (Polish three-form, noun+adjective agreement)", () => {
    const forms = [1, 3, 7].map((n) =>
      composeAutopilotRunSummary(
        { ...base, crossRefsJson: JSON.stringify({ claimsOverdue: 0, claimsDue: 0, openQuestions: n }) },
        pl,
        "pl",
      ),
    );
    expect(forms[0]).toBe("Przetworzono nowy raport — 1 otwarte pytanie badawcze.");
    expect(forms[1]).toBe("Przetworzono nowy raport — 3 otwarte pytania badawcze.");
    expect(forms[2]).toBe("Przetworzono nowy raport — 7 otwartych pytań badawczych.");
  });

  it("joins multiple parts with semicolons", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      kpiDeltaJson: JSON.stringify({ extractionAvailable: true, factsProposed: 40, factsAutoConfirmed: 40 }),
      reportDiffRef: JSON.stringify({}),
      crossRefsJson: JSON.stringify({ claimsOverdue: 1, claimsDue: 0, openQuestions: 2 }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — 40 KPI auto-confirmed (unreviewed) of 40 extracted; " +
        "report diff vs the previous statement available; 1 claim to verify; " +
        "2 open research questions.",
    );
  });
});
