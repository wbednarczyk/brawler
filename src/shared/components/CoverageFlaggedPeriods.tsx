import { useCallback, useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";

import {
  listFlaggedExtractionOutcomes,
  rerunExtractionOutcome,
} from "../../api/fundamentalsExtraction";
import type { ExtractionOutcome } from "../../api/fundamentalsExtraction";
import { useLocale } from "../locale";
import type { LocaleCode } from "../locale";
import { formatFinancialValue } from "../format/financialValue";
import {
  Button,
  EmptyState,
  ErrorText,
  ExpandableRow,
  Hint,
  InfoGrid,
  SectionHeader,
  StatusChip,
} from "../../ui";
import type { InfoGridItem } from "../../ui";

type Translate = (value: string) => string;

// The read has three genuinely different outcomes and the UI must never blur
// them: we do not know yet (loading), we know there is nothing (empty — a GOOD
// state), and we could not find out (error). Collapsing the third into the
// second is the recorded "failed category silently degrades to empty" defect,
// so the state is modelled explicitly rather than inferred from an empty array.
type ReadState = "loading" | "ready" | "failed";

// Human-readable label for a typed `reasonCode`. The backend deliberately emits
// NO prose (ADR 0061 decision 2) — the seven codes are translated here, so a raw
// code can never reach the user. An unrecognized code (a future backend value)
// falls back to an honest generic rather than leaking the token.
function reasonLabel(code: string, text: Translate): string {
  switch (code) {
    case "validation_failed":
      return text("The figures failed a consistency check");
    case "structure_drift":
      return text("The report's layout changed");
    case "witness_disagreement":
      return text("A second source reported different figures");
    case "no_deterministic_tier":
      return text("No reader could handle this document");
    case "no_period_derived":
      return text("The reporting period could not be determined");
    case "document_unreadable":
      return text("The document could not be read");
    case "emitted":
      return text("Data was recorded for this period");
    case "witness_fallback":
      return text("Figures taken from a third-party source (no reader could read the filing)");
    default:
      return text("This period could not be recorded");
  }
}

// The deterministic tier that attempted the document. `null` means no tier could
// read it at all — said plainly, never left blank. An unmapped tier token falls
// through as-is: showing the real value is honest, inventing a label is not.
// The five cases below are the complete `SourceTier::as_str` vocabulary
// (src-tauri/src/fundamentals/extraction/mod.rs) — verified against the enum,
// not guessed. A new tier lands here as a raw token until it gets a label.
function tierLabel(tier: string | null, text: Translate): string {
  if (tier === null || tier.trim() === "") return text("No reader");
  switch (tier) {
    case "esef":
      return text("ESEF");
    case "structured_xhtml":
      return text("Structured HTML");
    case "espi_cover_note":
      return text("ESPI cover note");
    case "pdf":
      return text("Structured read (xHTML)");
    case "html_aggregator":
      return text("Web cross-check");
    case "manual":
      // Not a `SourceTier` — the value the reversed-witnessing seam stamps on a
      // MANUAL-held slot's disagreement (ADR 0086 dec. 3, 2026-07-22), so the
      // user's own entry is named, never shown as a raw token.
      return text("Manual entry");
    default:
      return tier;
  }
}

// Period label matching the coverage table's idiom (`2025 Q3`, `2024 FY`). The
// SENTINEL period — `fiscalYear: 0` with an empty `periodType` — is what a
// `no_period_derived` failure carries, because a period-less failure has no slot
// key. Rendering it as "0 " would invent a fiscal year that does not exist.
export function flaggedPeriodLabel(
  outcome: Pick<ExtractionOutcome, "fiscalYear" | "periodType">,
  text: Translate,
): string {
  if (outcome.fiscalYear === 0 || outcome.periodType.trim() === "") {
    return text("Period unknown");
  }
  const periodType =
    outcome.periodType.toLowerCase() === "annual" ? "FY" : outcome.periodType.toUpperCase();
  return `${outcome.fiscalYear} ${periodType}`;
}

// `detailJson` carries the failing identities/cross-checks with
// expected/actual/residual WHERE THE GATE PRODUCED THEM. The KNOWN gate shape
// (failedIdentities / failedCrossChecks / witnessDisagreements, each an array of
// { id?, label?, metricKey?, detail: { actual, expected, residual? } }) renders
// as investor language with formatted amounts — raw JSON keys, braces, and
// empty-array noise never reach the user (owner dogfooding, 2026-07-21). Any
// UNKNOWN shape still falls through to the generic label/value flatten rather
// than being dropped: unfamiliar evidence is shown, never swallowed.
const GATE_DETAIL_KEYS = ["failedIdentities", "failedCrossChecks", "witnessDisagreements"] as const;

function formatGateAmount(raw: unknown, locale: LocaleCode): string | null {
  const numeric = String(raw ?? "").trim();
  if (numeric === "" || !Number.isFinite(Number(numeric))) return null;
  return formatFinancialValue({ valueNumeric: numeric, currency: "PLN" }, locale);
}

// One failing check → a human sentence. Falls back to null when the detail
// doesn't carry comparable amounts, so the caller can keep the generic path.
function gateCheckValue(
  detail: Record<string, unknown>,
  kind: (typeof GATE_DETAIL_KEYS)[number],
  text: Translate,
  locale: LocaleCode,
): string | null {
  const actual = formatGateAmount(detail.actual, locale);
  const expected = formatGateAmount(detail.expected, locale);
  if (actual === null || expected === null) return null;
  if (kind === "witnessDisagreements") {
    // `actual` is the filing's figure, `expected` the third-party source's.
    return text("filing reports {actual}, the third-party source {expected}")
      .replace("{actual}", actual)
      .replace("{expected}", expected);
  }
  const residual = formatGateAmount(detail.residual, locale);
  const base = text("reported {actual}, expected {expected}")
    .replace("{actual}", actual)
    .replace("{expected}", expected);
  if (residual === null) return base;
  return `${base} ${text("(difference {residual})").replace("{residual}", residual)}`;
}

function gateCheckLabel(element: Record<string, unknown>, text: Translate): string {
  if (element.id === "balance_sheet_identity") return text("Assets = Liabilities + Equity");
  if (typeof element.label === "string" && element.label.trim() !== "") return element.label;
  if (typeof element.metricKey === "string" && element.metricKey.trim() !== "") {
    return element.metricKey;
  }
  return text("Failing check");
}

function detailItems(detailJson: string | null, text: Translate, locale: LocaleCode): InfoGridItem[] {
  if (detailJson === null || detailJson.trim() === "") return [];
  let parsed: unknown;
  try {
    parsed = JSON.parse(detailJson);
  } catch {
    // Not JSON — show it verbatim rather than swallowing the only evidence.
    return [{ label: text("Failing check"), value: detailJson }];
  }
  const entries: InfoGridItem[] = [];
  const pushObject = (value: Record<string, unknown>) => {
    for (const [key, raw] of Object.entries(value)) {
      entries.push({
        label: key,
        value: typeof raw === "object" && raw !== null ? JSON.stringify(raw) : String(raw),
      });
    }
  };
  const pushGateElement = (element: unknown, kind: (typeof GATE_DETAIL_KEYS)[number]) => {
    if (typeof element !== "object" || element === null) {
      entries.push({ label: text("Failing check"), value: String(element) });
      return;
    }
    const record = element as Record<string, unknown>;
    const detail =
      typeof record.detail === "object" && record.detail !== null
        ? (record.detail as Record<string, unknown>)
        : record;
    const value = gateCheckValue(detail, kind, text, locale);
    if (value !== null) {
      entries.push({ label: gateCheckLabel(record, text), value });
    } else {
      pushObject(record);
    }
  };
  if (Array.isArray(parsed)) {
    for (const element of parsed) {
      if (typeof element === "object" && element !== null) {
        pushObject(element as Record<string, unknown>);
      } else {
        entries.push({ label: text("Failing check"), value: String(element) });
      }
    }
  } else if (typeof parsed === "object" && parsed !== null) {
    const record = parsed as Record<string, unknown>;
    const isGateShape = GATE_DETAIL_KEYS.some((key) => Array.isArray(record[key]));
    if (isGateShape) {
      for (const key of GATE_DETAIL_KEYS) {
        const checks = record[key];
        if (!Array.isArray(checks)) continue;
        // An empty array is a PASSING category — noise, not evidence. Skip it.
        for (const element of checks) pushGateElement(element, key);
      }
      // Anything the gate shape doesn't cover still surfaces generically.
      for (const [key, raw] of Object.entries(record)) {
        if ((GATE_DETAIL_KEYS as readonly string[]).includes(key)) continue;
        entries.push({
          label: key,
          value: typeof raw === "object" && raw !== null ? JSON.stringify(raw) : String(raw),
        });
      }
    } else {
      pushObject(record);
    }
  } else {
    entries.push({ label: text("Failing check"), value: String(parsed) });
  }
  return entries;
}

type CoverageFlaggedPeriodsProps = {
  companyId: string;
  // Bump to force a reload (e.g. after an extraction elsewhere in the panel).
  reloadKey?: number;
  // Called after a successful re-run so the hosting panel can reload siblings —
  // a re-run that now emits writes facts the coverage map must pick up.
  onRerun?: () => void;
};

/// The company's flagged extraction periods (ADR 0061 decision 2, ADR 0084
/// decision 4/6): the periods where the deterministic pipeline ran and REFUSED to
/// emit. This is the UI half of the "never silently wrong" guarantee — when no
/// tier can read a document or the validation gate refuses a value, the period is
/// flagged with a typed reason, and this section is where a human sees it and
/// acts. Lives inside the Coverage panel: coverage is the company's "what do we
/// have / what is missing / what needs review" surface, and the Review queue
/// panel that used to host human review was retired with the in-app AI layer.
export function CoverageFlaggedPeriods({
  companyId,
  reloadKey = 0,
  onRerun,
}: CoverageFlaggedPeriodsProps) {
  const { text, locale } = useLocale();
  const [outcomes, setOutcomes] = useState<ExtractionOutcome[]>([]);
  const [readState, setReadState] = useState<ReadState>("loading");
  const [readError, setReadError] = useState<string | null>(null);
  const [expandedIds, setExpandedIds] = useState<string[]>([]);
  // Non-null while a re-run is in flight — the id of the outcome being retried.
  // Doubles as the busy guard: EVERY re-run action disables, so a second click
  // (on this row or a neighbour) cannot dispatch a second blocking run.
  const [rerunningId, setRerunningId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  // Bumped by a retry / a completed re-run to re-trigger the read effect.
  const [refreshTick, setRefreshTick] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setReadState("loading");
    setReadError(null);
    listFlaggedExtractionOutcomes(companyId)
      .then((rows) => {
        if (cancelled) return;
        setOutcomes(rows);
        setReadState("ready");
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        // The read failed: say so. Never fall through to an empty list, which
        // would read as the reassuring "nothing flagged".
        setOutcomes([]);
        setReadError(reason instanceof Error ? reason.message : String(reason));
        setReadState("failed");
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, reloadKey, refreshTick]);

  const toggleExpanded = useCallback((id: string) => {
    setExpandedIds((current) =>
      current.includes(id) ? current.filter((value) => value !== id) : [...current, id],
    );
  }, []);

  const rerun = async (outcome: ExtractionOutcome) => {
    if (rerunningId !== null) return;
    setRerunningId(outcome.id);
    setActionError(null);
    try {
      await rerunExtractionOutcome({ outcomeId: outcome.id });
      // The backend updates the SAME row in place, so a period whose cause is
      // fixed leaves this list — refetch instead of mutating locally, or a stale
      // flag would sit beside a fresh success.
      setRefreshTick((tick) => tick + 1);
      onRerun?.();
    } catch (reason: unknown) {
      setActionError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setRerunningId(null);
    }
  };

  const renderBody = () => {
    if (readState === "loading") {
      return <Hint>{text("Checking for flagged periods…")}</Hint>;
    }
    if (readState === "failed") {
      return (
        <div className="coverage-flagged-failed">
          <ErrorText>{text("Couldn't load flagged periods.")}</ErrorText>
          {readError ? <Hint>{readError}</Hint> : null}
          <Button variant="secondary" onClick={() => setRefreshTick((tick) => tick + 1)}>
            <RefreshCw size={14} aria-hidden="true" />
            {text("Retry")}
          </Button>
        </div>
      );
    }
    if (outcomes.length === 0) {
      // A GOOD state, phrased as one — not a bare "nothing here".
      return <EmptyState>{text("Nothing flagged — every attempted period produced data.")}</EmptyState>;
    }
    return (
      <ul className="coverage-flagged-list">
        {outcomes.map((outcome) => {
          const periodLabel = flaggedPeriodLabel(outcome, text);
          const isRerunning = rerunningId === outcome.id;
          const details = detailItems(outcome.detailJson, text, locale);
          return (
            <li key={outcome.id}>
              <ExpandableRow
                className="coverage-flagged-row"
                isExpanded={expandedIds.includes(outcome.id)}
                label={`${periodLabel} — ${reasonLabel(outcome.reasonCode, text)}`}
                onToggle={() => toggleExpanded(outcome.id)}
                detail={
                  <div className="coverage-flagged-detail">
                    {details.length > 0 ? (
                      <InfoGrid ariaLabel={text("Failing check")} items={details} />
                    ) : (
                      <Hint>{text("No further detail was recorded for this period.")}</Hint>
                    )}
                    <Hint>
                      {text("Attempts: {n}").replace("{n}", String(outcome.attemptCount))}
                    </Hint>
                  </div>
                }
                actions={
                  <Button
                    variant="secondary"
                    className="coverage-flagged-rerun"
                    disabled={rerunningId !== null}
                    onClick={() => void rerun(outcome)}
                  >
                    <RefreshCw size={14} aria-hidden="true" />
                    {isRerunning ? text("Trying again…") : text("Try again")}
                  </Button>
                }
              >
                {/* Two lines, not four columns. The identity line holds the FIXED
                    SLOTS (period · reader chip · drift chip — same x on every
                    row, so an absent drift chip never shifts its neighbours);
                    the reason is PROSE and gets its own full-width line below.
                    Squeezing prose into a narrow flex/grid slot is what made it
                    wrap one character per line in the (narrow) Coverage pane. */}
                <span className="coverage-flagged-head">
                  <span className="coverage-flagged-period">{periodLabel}</span>
                  <span className="coverage-flagged-tier">
                    <StatusChip tone={outcome.tier === null ? "warn" : "neutral"}>
                      {tierLabel(outcome.tier, text)}
                    </StatusChip>
                  </span>
                  <span className="coverage-flagged-drift">
                    {outcome.structureChanged ? (
                      <StatusChip tone="warn">{text("Layout changed")}</StatusChip>
                    ) : null}
                  </span>
                </span>
                <span className="coverage-flagged-reason">
                  {reasonLabel(outcome.reasonCode, text)}
                </span>
              </ExpandableRow>
            </li>
          );
        })}
      </ul>
    );
  };

  return (
    <section className="coverage-flagged" aria-label={text("Flagged periods")}>
      <SectionHeader
        level="h3"
        title={text("Flagged periods")}
        meta={readState === "ready" ? outcomes.length : undefined}
      />
      <Hint>
        {text("Periods the app refused to record rather than guess — nothing is silently missing.")}
      </Hint>
      {renderBody()}
      {actionError ? <ErrorText>{actionError}</ErrorText> : null}
    </section>
  );
}
