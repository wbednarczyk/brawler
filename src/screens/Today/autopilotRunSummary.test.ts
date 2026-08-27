import { describe, expect, it } from "vitest";

import {
  composeAutopilotRunSummary,
  isTokenizedSummary,
  renderAutopilotSummaryTokens,
  type AutopilotRunSummarySource,
} from "./autopilotRunSummary";
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
  // "0 of 0 KPI recorded" — the honest counts must flow through when the delta
  // actually carries them. Facts are review-free (ADR 0086 dec. 5): the count
  // reads as recorded, never "pending your confirmation".
  it("reports the honest recorded count for an autopilot-mode run", () => {
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
      "New report processed — 40 of 40 KPI recorded.",
    );
  });

  it("reports the same recorded phrasing for an assist-mode run (review-free)", () => {
    const run: AutopilotRunSummarySource = {
      ...base,
      mode: "assist",
      kpiDeltaJson: JSON.stringify({ extractionAvailable: true, factsProposed: 5 }),
    };
    expect(composeAutopilotRunSummary(run, identity, "en")).toBe(
      "New report processed — 5 KPI recorded.",
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
    expect(forms[0]).toBe("Przetworzono nowy raport — 1 teza do weryfikacji.");
    expect(forms[1]).toBe("Przetworzono nowy raport — 2 tezy do weryfikacji.");
    expect(forms[2]).toBe("Przetworzono nowy raport — 5 tez do weryfikacji.");
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
      "New report processed — 40 of 40 KPI recorded; " +
        "report diff vs the previous statement available; 1 claim to verify; " +
        "2 open research questions.",
    );
  });
});

// ADR 0084 decision 6 (completion): the backend `summaryText` is now a typed
// TOKEN stream (jobs/autopilot.rs `compose_summary`). `renderAutopilotSummaryTokens`
// is the frontend seam that turns those tokens into a localized sentence, and the
// tolerant read that passes legacy/unknown stored summaries through verbatim.
describe("renderAutopilotSummaryTokens", () => {
  it("renders the empty/base token as the plain processed sentence (en + pl)", () => {
    expect(renderAutopilotSummaryTokens("report_processed", identity, "en")).toBe(
      "New report processed.",
    );
    expect(renderAutopilotSummaryTokens("report_processed", pl, "pl")).toBe(
      "Przetworzono nowy raport.",
    );
    expect(renderAutopilotSummaryTokens(null, identity, "en")).toBe("New report processed.");
    expect(renderAutopilotSummaryTokens("", identity, "en")).toBe("New report processed.");
  });

  it("translates the kpi_confirmed token to en AND pl", () => {
    expect(renderAutopilotSummaryTokens("kpi_confirmed:40:40", identity, "en")).toBe(
      "New report processed — 40 of 40 KPI recorded.",
    );
    expect(renderAutopilotSummaryTokens("kpi_confirmed:40:40", pl, "pl")).toBe(
      "Przetworzono nowy raport — zapisano 40 z 40 wskaźników KPI.",
    );
  });

  // Legacy `kpi_pending` tokens can still sit in an existing DB — the renderer
  // reads them tolerantly as the same review-free "recorded" phrasing (ADR 0086
  // dec. 5); no fact ever awaits confirmation anymore.
  it("translates a legacy kpi_pending token to the recorded phrasing", () => {
    expect(renderAutopilotSummaryTokens("kpi_pending:5", identity, "en")).toBe(
      "New report processed — 5 KPI recorded.",
    );
  });

  it("translates the witness_fallback unavailable code to en AND pl", () => {
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:witness_fallback", identity, "en"),
    ).toBe(
      "New report processed — Figures taken from a third-party source (no reader could read the filing).",
    );
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:witness_fallback", pl, "pl"),
    ).toBe(
      "Przetworzono nowy raport — Liczby wzięte z zewnętrznego serwisu (żaden czytnik nie odczytał raportu).",
    );
  });

  it("translates the pdf_document by-design gap to en AND pl (never a failure framing)", () => {
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:pdf_document", identity, "en"),
    ).toBe("New report processed — PDF report — core figures arrive from the aggregator source.");
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:pdf_document", pl, "pl"),
    ).toBe(
      "Przetworzono nowy raport — Raport PDF — podstawowe liczby przychodzą ze źródła agregatora.",
    );
  });

  it("translates the other typed unavailable codes", () => {
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:quota_exhausted", identity, "en"),
    ).toBe("New report processed — KPI extraction unavailable (a third-party quota was exhausted).");
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:no_deterministic_tier", identity, "en"),
    ).toBe("New report processed — KPI extraction unavailable (no reader could read this report).");
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:provider_error", identity, "en"),
    ).toBe("New report processed — KPI extraction unavailable (the AI provider returned an error).");
  });

  it("surfaces an unknown unavailable code verbatim rather than swallowing it", () => {
    expect(
      renderAutopilotSummaryTokens("kpi_extraction_unavailable:brand_new_code", identity, "en"),
    ).toBe("New report processed — KPI extraction unavailable (brand_new_code).");
  });

  it("pluralizes claims_to_verify and research_questions (pl three-form)", () => {
    expect(renderAutopilotSummaryTokens("claims_to_verify:1", identity, "en")).toBe(
      "New report processed — 1 claim to verify.",
    );
    expect(renderAutopilotSummaryTokens("claims_to_verify:5", pl, "pl")).toBe(
      "Przetworzono nowy raport — 5 tez do weryfikacji.",
    );
    expect(renderAutopilotSummaryTokens("research_questions:3", pl, "pl")).toBe(
      "Przetworzono nowy raport — 3 otwarte pytania badawcze.",
    );
  });

  it("joins a multi-token stream in order", () => {
    const summary =
      "kpi_confirmed:40:40; report_diff_available; claims_to_verify:1; research_questions:2";
    expect(renderAutopilotSummaryTokens(summary, identity, "en")).toBe(
      "New report processed — 40 of 40 KPI recorded; " +
        "report diff vs the previous statement available; 1 claim to verify; " +
        "2 open research questions.",
    );
  });

  it("renders the expectations token", () => {
    expect(renderAutopilotSummaryTokens("expectations_to_review", identity, "en")).toBe(
      "New report processed — expectations recorded — review vs actuals.",
    );
  });

  // BACKWARD COMPAT (hard requirement): the owner's live DB holds already-stored
  // summaries as English prose. Any string that is not a clean token stream must
  // pass through VERBATIM — never mangled, never double-wrapped.
  it("passes a legacy English-prose summary through verbatim", () => {
    const legacy =
      "New report processed — 40 KPI auto-confirmed (unreviewed) of 40 extracted.";
    expect(renderAutopilotSummaryTokens(legacy, identity, "en")).toBe(legacy);
    const legacyUnavailable = "New report processed — KPI extraction unavailable (xyz).";
    expect(renderAutopilotSummaryTokens(legacyUnavailable, pl, "pl")).toBe(legacyUnavailable);
  });

  it("classifies token streams vs legacy prose", () => {
    expect(isTokenizedSummary("kpi_confirmed:40:40; report_diff_available")).toBe(true);
    expect(isTokenizedSummary("report_processed")).toBe(true);
    expect(isTokenizedSummary("kpi_extraction_unavailable:witness_fallback")).toBe(true);
    expect(isTokenizedSummary("New report processed — 40 KPI auto-confirmed of 40 extracted.")).toBe(
      false,
    );
    expect(isTokenizedSummary(null)).toBe(false);
    expect(isTokenizedSummary("")).toBe(false);
  });
});
