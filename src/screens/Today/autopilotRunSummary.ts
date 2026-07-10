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
  pl: ["obietnica", "obietnice", "obietnic"],
};

// Folds noun + adjective agreement into one three-form set (like `FACT_FORMS` +
// "recorded" in `FundamentalsPanel`, but combined) rather than composing two
// separately-declined words — simpler for a fixed phrase that always appears
// together.
const OPEN_RESEARCH_QUESTION_FORMS: PluralForms = {
  en: ["open research question", "open research questions"],
  pl: ["otwarte pytanie badawcze", "otwarte pytania badawcze", "otwartych pytań badawczych"],
};

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
// delta (jobs/autopilot.rs, jobs/structured_extraction.rs). Each stored reason
// maps to an honest, product-facing line rather than one blanket "no AI provider"
// string (card b85ba3c). Keep the copy short and free of implementation vocabulary
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
 * reason recorded (oldest legacy rows) keeps the historical generic line.
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
      const proposed = numberField(delta, "factsProposed");
      const confirmed = numberField(delta, "factsAutoConfirmed");
      if (run.mode === "autopilot" && proposed !== null && confirmed !== null) {
        parts.push(
          text("{confirmed} KPI auto-confirmed (unreviewed) of {proposed} extracted")
            .replace("{confirmed}", String(confirmed))
            .replace("{proposed}", String(proposed)),
        );
      } else if (run.mode !== "autopilot" && proposed !== null) {
        parts.push(
          text("{proposed} KPI extracted, pending your confirmation").replace(
            "{proposed}",
            String(proposed),
          ),
        );
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
