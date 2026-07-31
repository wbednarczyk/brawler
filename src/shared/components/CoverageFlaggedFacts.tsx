import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";

import { listFlaggedFactProvenance } from "../../api/fundamentalsExtraction";
import type { FlaggedFact } from "../../api/fundamentalsExtraction";
import { useLocale } from "../locale";
import { localizedKpiLabel } from "../locale/kpiLabels";
import { formatFinancialValue } from "../format/financialValue";
import { Button, EmptyState, ErrorText, Hint, ListRow, SectionHeader, StatusChip } from "../../ui";

type Translate = (value: string) => string;

// Same three-state model as the sibling flagged-PERIODS section: we do not know
// yet / we know there is nothing (a GOOD state) / we could not find out. An
// error must never degrade into the reassuring empty list.
type ReadState = "loading" | "ready" | "failed";

// The deterministic tier that produced the flagged value. Mirrors the sibling
// section's mapping (the complete `SourceTier::as_str` vocabulary plus the
// `manual` marker); an unmapped token shows as-is — honest, never invented.
function tierLabel(tier: string, text: Translate): string {
  switch (tier) {
    case "esef":
      return text("ESEF");
    case "structured_xhtml":
      return text("Structured HTML");
    case "espi_cover_note":
      return text("ESPI cover note");
    case "pdf":
      return text("Structured read (xHTML)");
    case "agent":
      // MCP acquisition tier (ADR 0093, epic #285): an agent's read of an
      // issuer document, recorded through the port's write tools.
      return text("Agent (MCP)");
    case "html_aggregator":
      return text("Web cross-check");
    case "manual":
      return text("Manual entry");
    default:
      return tier;
  }
}

function periodLabel(fact: Pick<FlaggedFact, "fiscalYear" | "periodType">): string {
  const periodType =
    fact.periodType.toLowerCase() === "annual" ? "FY" : fact.periodType.toUpperCase();
  return `${fact.fiscalYear} ${periodType}`;
}

type CoverageFlaggedFactsProps = {
  companyId: string;
  // Bump to force a reload (e.g. after an extraction elsewhere in the panel).
  reloadKey?: number;
};

/// The company's flagged FACTS — values the pipeline DID record but marked as
/// drifted or contradicted (ADR 0061 decision 2, ADR 0086 decision 5). The
/// sibling "Flagged periods" section covers the periods where nothing emitted;
/// this one covers the figures that landed and want a human eye. Informational,
/// never a to-do the user owes: facts are review-free, so the row states what
/// the pipeline saw (metric, value, period, reader, source label) and nothing is
/// asked of the reader.
export function CoverageFlaggedFacts({ companyId, reloadKey = 0 }: CoverageFlaggedFactsProps) {
  const { text, locale } = useLocale();
  const [facts, setFacts] = useState<FlaggedFact[]>([]);
  const [readState, setReadState] = useState<ReadState>("loading");
  const [readError, setReadError] = useState<string | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setReadState("loading");
    setReadError(null);
    listFlaggedFactProvenance(companyId)
      .then((rows) => {
        if (cancelled) return;
        setFacts(rows);
        setReadState("ready");
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setFacts([]);
        setReadError(reason instanceof Error ? reason.message : String(reason));
        setReadState("failed");
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, reloadKey, refreshTick]);

  const renderBody = () => {
    if (readState === "loading") {
      return <Hint>{text("Checking for flagged figures…")}</Hint>;
    }
    if (readState === "failed") {
      return (
        <div className="coverage-flagged-failed">
          <ErrorText>{text("Couldn't load flagged figures.")}</ErrorText>
          {readError ? <Hint>{readError}</Hint> : null}
          <Button variant="secondary" onClick={() => setRefreshTick((tick) => tick + 1)}>
            <RefreshCw size={14} aria-hidden="true" />
            {text("Retry")}
          </Button>
        </div>
      );
    }
    if (facts.length === 0) {
      return (
        <EmptyState>{text("No flagged figures — every recorded value passed its checks.")}</EmptyState>
      );
    }
    return (
      <ul className="ui-list-rows coverage-flagged-list">
        {facts.map((fact) => {
          const metric = localizedKpiLabel({ metricKey: fact.metricKey, label: fact.label }, locale);
          const value = formatFinancialValue(
            { valueNumeric: fact.valueNumeric, currency: fact.currency },
            locale,
          );
          const citation = fact.citation?.trim();
          return (
            <ListRow
              key={fact.factId}
              title={`${periodLabel(fact)} · ${metric} — ${value}`}
              titleAttr={`${periodLabel(fact)} · ${metric} — ${value}`}
              meta={citation === undefined || citation === "" ? undefined : citation}
              // Fixed trailing slot: the reader chip lands at the same x on
              // every row (ui-authoring styling rules).
              trailing={<StatusChip tone="warn">{tierLabel(fact.sourceTier, text)}</StatusChip>}
            />
          );
        })}
      </ul>
    );
  };

  return (
    <section className="coverage-flagged-figures" aria-label={text("Flagged figures")}>
      <SectionHeader
        level="h3"
        title={text("Flagged figures")}
        meta={readState === "ready" ? facts.length : undefined}
      />
      <Hint>
        {text("Values the app recorded but couldn't fully verify — shown with where they came from.")}
      </Hint>
      {renderBody()}
    </section>
  );
}
