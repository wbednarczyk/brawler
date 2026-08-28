import type { AutopilotRun } from "../../api/autopilot";
import type { LocaleCode } from "../../shared/locale";
import { pluralNoun, type PluralForms } from "../../shared/locale/plural";

type TextFn = (value: string) => string;

/**
 * A run's fields the summary is composed from — a subset of `AutopilotRun` so
 * this stays unit-testable without a full fixture.
 */
export type AutopilotRunSummarySource = Pick<
  AutopilotRun,
  "mode" | "kpiDeltaJson" | "reportDiffRef" | "crossRefsJson"
>;

const CLAIM_FORMS: PluralForms = {
  en: ["claim", "claims"],
  pl: ["teza", "tezy", "tez"],
};

// Folds noun + adjective agreement into one three-form set (like `FACT_FORMS` +
// "recorded" in `FundamentalsPanel`, but combined) rather than composing two
// separately-declined words — simpler for a fixed phrase that always appears
// together.
const OPEN_RESEARCH_QUESTION_FORMS: PluralForms = {
  en: ["open research question", "open research questions"],
  pl: ["otwarte pytanie badawcze", "otwarte pytania badawcze", "otwartych pytań badawczych"],
};

// ADR 0084 decision 6 (completion): the backend `compose_summary` emits TYPED
// reason CODES for an unavailable run (jobs/autopilot.rs `KpiUnavailableReason`).
// These map each code to a short, product-facing sentence (free of implementation
// vocabulary — dev-speak locale guard). `witness_fallback` is not a failure: the
// aggregator sourced the figures because no reader could read the filing.
const KPI_UNAVAILABLE_BY_CODE: Record<string, string> = {
  quota_exhausted: "KPI extraction unavailable (a third-party quota was exhausted)",
  provider_not_configured: "KPI extraction unavailable (no AI provider configured)",
  provider_error: "KPI extraction unavailable (the AI provider returned an error)",
  no_deterministic_tier: "KPI extraction unavailable (no reader could read this report)",
  witness_fallback: "Figures taken from a third-party source (no reader could read the filing)",
  pdf_document: "PDF report \u2014 core figures arrive from the aggregator source",
};

// The localized line for an `kpi_extraction_unavailable:<code>` token. A known
// typed code gets its sentence; an unknown/newer code surfaces its RAW code so it
// is never silently swallowed (mirrors the data-path `extractionUnavailableText`).
function kpiUnavailableTextByCode(code: string, text: TextFn): string {
  const known = KPI_UNAVAILABLE_BY_CODE[code];
  if (known) return text(known);
  return text("KPI extraction unavailable ({reason})").replace("{reason}", code);
}

function isCount(value: string): boolean {
  return /^\d+$/.test(value);
}

/**
 * Render a single stored summary token to its localized fragment, or `null` when
 * the string is not a recognized token (the signal that a stored summary is
 * legacy English prose rather than a token stream). The base `report_processed`
 * token contributes nothing (empty string) — the caller supplies the sentence.
 */
function renderSummaryToken(token: string, text: TextFn, locale: LocaleCode): string | null {
  if (token === "report_processed") return "";
  if (token === "report_diff_available") {
    return text("report diff vs the previous statement available");
  }
  if (token === "expectations_to_review") {
    return text("expectations recorded — review vs actuals");
  }

  const colon = token.indexOf(":");
  const name = colon === -1 ? token : token.slice(0, colon);
  const rest = colon === -1 ? "" : token.slice(colon + 1);

  switch (name) {
    case "kpi_extraction_unavailable":
      return rest === "" ? null : kpiUnavailableTextByCode(rest, text);
    case "kpi_confirmed": {
      const [confirmed, proposed] = rest.split(":");
      if (!isCount(confirmed) || !isCount(proposed)) return null;
      return text("{confirmed} of {proposed} KPI recorded")
        .replace("{confirmed}", confirmed)
        .replace("{proposed}", proposed);
    }
    // Legacy tokens only (ADR 0086 dec. 5): the backend no longer emits
    // `kpi_pending` — facts are review-free — but an existing DB may still hold
    // one, so read it tolerantly as the same review-free "recorded" phrasing.
    case "kpi_pending": {
      if (!isCount(rest)) return null;
      return text("{proposed} KPI recorded").replace("{proposed}", rest);
    }
    case "claims_to_verify": {
      if (!isCount(rest)) return null;
      const n = Number(rest);
      return `${n} ${pluralNoun(locale, n, CLAIM_FORMS)} ${text("to verify")}`;
    }
    case "research_questions": {
      if (!isCount(rest)) return null;
      const n = Number(rest);
      return `${n} ${pluralNoun(locale, n, OPEN_RESEARCH_QUESTION_FORMS)}`;
    }
    default:
      return null;
  }
}

/**
 * True when the whole stored summary parses as a clean typed-token stream — the
 * router at the call site (TodayScreen) uses this to send new runs to the token
 * renderer and legacy-prose runs to the data-recompose fallback.
 */
export function isTokenizedSummary(summaryText: string | null): boolean {
  if (!summaryText || summaryText.trim() === "") return false;
  const identity: TextFn = (value) => value;
  return summaryText.split("; ").every((token) => renderSummaryToken(token, identity, "en") !== null);
}

/**
 * ADR 0084 decision 6 seam: turn the backend's typed-token `summaryText` into a
 * localized sentence. Every fragment the backend emits is a machine token
 * (`kpi_confirmed:c:p`, `kpi_extraction_unavailable:<code>`, `claims_to_verify:n`,
 * `research_questions:n`, `report_diff_available`, `expectations_to_review`,
 * `report_processed`), translated here through `text()`/`pluralNoun`.
 *
 * BACKWARD COMPATIBILITY (hard requirement): the owner's live DB holds already-
 * stored summaries as English prose. Any string that is not a clean token stream
 * passes through VERBATIM (tolerant read) — never mangled, never double-wrapped.
 */
export function renderAutopilotSummaryTokens(
  summaryText: string | null,
  text: TextFn,
  locale: LocaleCode,
): string {
  if (!summaryText || summaryText.trim() === "") return text("New report processed.");
  const parts: string[] = [];
  for (const token of summaryText.split("; ")) {
    const rendered = renderSummaryToken(token, text, locale);
    if (rendered === null) return summaryText; // legacy/unknown → verbatim
    if (rendered !== "") parts.push(rendered);
  }
  if (parts.length === 0) return text("New report processed.");
  return `${text("New report processed")} — ${parts.join("; ")}.`;
}

function parseJsonObject(json: string | null): Record<string, unknown> | null {
  if (!json) return null;
  try {
    const parsed: unknown = JSON.parse(json);
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function numberField(source: Record<string, unknown>, key: string): number | null {
  const value = source[key];
  return typeof value === "number" ? value : null;
}

// Tier-4 KPI-extraction degrade reasons carried on an `extractionAvailable:false`
// delta (jobs/autopilot.rs, jobs/structured_extraction.rs): each stored reason
// maps to an honest, product-facing line, free of implementation vocabulary
// (dev-speak locale guard). `sweep_deterministic_only` covers legacy rows.
const EXTRACTION_UNAVAILABLE_TEXT: Record<string, string> = {
  no_ai_provider: "KPI extraction unavailable (no AI provider configured)",
  no_vision_provider: "KPI extraction unavailable (no document-reading AI provider configured)",
  not_extractable: "KPI extraction unavailable (report format isn't extractable)",
  not_pdf: "KPI extraction unavailable (report isn't a readable PDF)",
  no_stored_file: "KPI extraction unavailable (no stored file for this report)",
  no_extraction: "KPI extraction unavailable (no values were extracted)",
  bootstrap_failed: "KPI extraction unavailable (extraction setup failed)",
  sweep_deterministic_only: "KPI extraction unavailable (only basic non-AI values available)",
  skipped_budget: "KPI extraction unavailable (AI budget for this run was used up)",
  automation_off: "KPI extraction unavailable (automation is off)",
};

/**
 * The honest, localized "extraction unavailable" line for a degraded run. Known
 * reasons get a specific message; a `provider_error:<code>` reason (prefix match)
 * gets the generic provider-error text; an unknown/new reason surfaces its RAW
 * code so it is never silently swallowed into a blanket message; a delta with no
 * reason recorded (older rows lacking the field) falls back to the generic line.
 */
function extractionUnavailableText(delta: Record<string, unknown>, text: TextFn): string {
  const reason = typeof delta.reason === "string" ? delta.reason : null;
  if (!reason) {
    return text("KPI extraction unavailable (no AI provider configured)");
  }
  if (reason.startsWith("provider_error")) {
    return text("KPI extraction unavailable (the AI provider returned an error)");
  }
  const known = EXTRACTION_UNAVAILABLE_TEXT[reason];
  if (known) return text(known);
  return text("KPI extraction unavailable ({reason})").replace("{reason}", reason);
}

/**
 * Composes the Today/Pulse autopilot run card's "what changed" sentence from
 * the run's DATA fields (bug e77a1a2, part 2) — `kpiDeltaJson`/`crossRefsJson`
 * carry counts/flags, and this function is the one place they become a
 * localized, `text()`/`pluralNoun`-routed sentence. The backend's `summaryText`
 * is composed in Rust for backward-compat/legacy fallback only (contracts.md §
 * Autonomous Report Pipeline) — it is English-only and must never be rendered
 * verbatim in the Polish UI.
 *
 * Renders nothing for a KPI count that is present but not backed by an honest
 * figure (e.g. a delta shaped `{ extractionAvailable: true }` with no
 * `factsProposed`/`factsAutoConfirmed`) rather than defaulting to 0 — showing
 * "0 KPI auto-confirmed of 0 extracted" for a run that never actually reported
 * those counts is exactly bug e77a1a2's symptom.
 */
export function composeAutopilotRunSummary(
  run: AutopilotRunSummarySource,
  text: TextFn,
  locale: LocaleCode,
): string {
  const parts: string[] = [];

  const delta = parseJsonObject(run.kpiDeltaJson);
  if (delta) {
    if (delta.extractionAvailable === false) {
      parts.push(extractionUnavailableText(delta, text));
    } else {
      // Review-free (ADR 0086 dec. 5): facts land recorded in BOTH modes — the
      // count reads as recorded, never "pending your confirmation". Show both
      // counts when present, else the bare proposed count.
      const proposed = numberField(delta, "factsProposed");
      const confirmed = numberField(delta, "factsAutoConfirmed");
      if (proposed !== null && confirmed !== null) {
        parts.push(
          text("{confirmed} of {proposed} KPI recorded")
            .replace("{confirmed}", String(confirmed))
            .replace("{proposed}", String(proposed)),
        );
      } else if (proposed !== null) {
        parts.push(text("{proposed} KPI recorded").replace("{proposed}", String(proposed)));
      }
    }
  }

  if (run.reportDiffRef) {
    parts.push(text("report diff vs the previous statement available"));
  }

  const crossRefs = parseJsonObject(run.crossRefsJson);
  if (crossRefs) {
    const overdue = numberField(crossRefs, "claimsOverdue") ?? 0;
    const due = numberField(crossRefs, "claimsDue") ?? 0;
    const questions = numberField(crossRefs, "openQuestions") ?? 0;
    const toVerify = overdue + due;
    if (toVerify > 0) {
      parts.push(`${toVerify} ${pluralNoun(locale, toVerify, CLAIM_FORMS)} ${text("to verify")}`);
    }
    if (questions > 0) {
      parts.push(`${questions} ${pluralNoun(locale, questions, OPEN_RESEARCH_QUESTION_FORMS)}`);
    }
  }

  if (parts.length === 0) {
    return text("New report processed.");
  }
  return `${text("New report processed")} — ${parts.join("; ")}.`;
}
