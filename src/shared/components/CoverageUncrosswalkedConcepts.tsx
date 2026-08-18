import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";

import {
  listUncrosswalkedConcepts,
  promoteUncrosswalkedConcept,
} from "../../api/taggedFactPromotion";
import type { UncrosswalkedConceptRow } from "../../api/taggedFactPromotion";
import { useLocale } from "../locale";
import { pluralNoun, type PluralForms } from "../locale/plural";
import { Button, EmptyState, ErrorText, Hint, ListRow, SectionHeader, StatusChip } from "../../ui";

type Translate = (value: string) => string;

const CONCEPT_FORMS: PluralForms = {
  en: ["position", "positions"],
  pl: ["pozycja", "pozycje", "pozycji"],
};

type ReadState = "loading" | "ready" | "failed";

function periodNatureLabel(periodNature: string, text: Translate): string {
  return periodNature === "instant" ? text("As of a date") : text("For the period");
}

// `statement_group` from the backend is `balance | income | cash_flow |
// other`. `src/shared` cannot import the screens-level `statementTabLabel`
// (modularization-design § layer contract — shared code never depends on a
// composition root), so this mirrors its exact same-keyed strings instead:
// "other" reads as "Operating", the same bucket FundamentalsPanel's own
// matrix sorts a company-scoped definition into by default (factMatrix.ts),
// keeping this row's language consistent with where a promoted concept will
// actually land.
function rowStatementLabel(statementGroup: string, text: Translate): string {
  switch (statementGroup) {
    case "income":
      return text("Income statement");
    case "balance":
      return text("Balance sheet");
    case "cash_flow":
      return text("Cash flow");
    default:
      return text("Operating");
  }
}

type Props = {
  companyId: string;
  reloadKey?: number;
};

/// "Positions the program doesn't know yet" (ADR 0100 decision 10, epic #398):
/// every captured concept with no crosswalk entry at this company, ranked by
/// how many companies across the corpus report it. Each row names the
/// position (the issuer's own published label, or — honestly marked — the raw
/// taxonomy name when no curated translation exists yet), the statement and
/// window it was tagged under, and the ONE action the ADR opens: promote it
/// into this company's Fundamentals. Company-scoped only; never reachable by
/// an agent (MCP registry: excluded).
export function CoverageUncrosswalkedConcepts({ companyId, reloadKey = 0 }: Props) {
  const { text, locale } = useLocale();
  const [rows, setRows] = useState<UncrosswalkedConceptRow[]>([]);
  const [readState, setReadState] = useState<ReadState>("loading");
  const [readError, setReadError] = useState<string | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const [promotingConcept, setPromotingConcept] = useState<string | null>(null);
  const [promoteError, setPromoteError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setReadState("loading");
    setReadError(null);
    listUncrosswalkedConcepts(companyId)
      .then((value) => {
        if (cancelled) return;
        setRows(value);
        setReadState("ready");
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setRows([]);
        setReadError(reason instanceof Error ? reason.message : String(reason));
        setReadState("failed");
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, reloadKey, refreshTick]);

  const promote = async (row: UncrosswalkedConceptRow) => {
    setPromotingConcept(row.conceptLocalName);
    setPromoteError(null);
    try {
      const promoted = await promoteUncrosswalkedConcept(
        companyId,
        row.conceptNamespaceUri,
        row.conceptLocalName,
      );
      setRows((current) =>
        current.map((existing) =>
          // Identity is (namespace, local name) — a same-named extension row
          // must not light up when the standard concept is promoted.
          existing.conceptLocalName === row.conceptLocalName &&
          existing.conceptNamespaceUri === row.conceptNamespaceUri
            ? { ...existing, alreadyPromoted: true, promotedDefinitionId: promoted.definitionId }
            : existing,
        ),
      );
    } catch (reason) {
      setPromoteError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setPromotingConcept(null);
    }
  };

  const renderBody = () => {
    if (readState === "loading") {
      return <Hint>{text("Checking for unnamed positions…")}</Hint>;
    }
    if (readState === "failed") {
      return (
        <div className="coverage-uncrosswalked-failed">
          <ErrorText>{text("Couldn't load unnamed positions.")}</ErrorText>
          {readError ? <Hint>{readError}</Hint> : null}
          <Button variant="secondary" onClick={() => setRefreshTick((tick) => tick + 1)}>
            <RefreshCw size={14} aria-hidden="true" />
            {text("Retry")}
          </Button>
        </div>
      );
    }
    if (rows.length === 0) {
      return (
        <EmptyState>
          {text("Nothing captured yet that the program doesn't already know how to name.")}
        </EmptyState>
      );
    }
    return (
      <ul className="ui-list-rows coverage-uncrosswalked-list">
        {rows.map((row) => {
          const where = [
            rowStatementLabel(row.statementGroup, text),
            periodNatureLabel(row.periodNature, text),
            text("name in the report: {concept}").replace("{concept}", row.conceptLocalName),
          ].join(" · ");
          const busy = promotingConcept === row.conceptLocalName;
          return (
            <ListRow
              key={`${row.conceptNamespaceUri}#${row.conceptLocalName}`}
              title={row.humanLabel}
              titleAttr={row.humanLabel}
              meta={
                row.labelSource === "technical"
                  ? `${where} · ${text("no translation yet")}`
                  : `${where} · ${text("issuer's own position, named by the company")}`
              }
              trailing={
                row.alreadyPromoted ? (
                  <StatusChip tone="ok">{text("In Fundamentals")}</StatusChip>
                ) : (
                  <Button
                    variant="secondary"
                    disabled={busy}
                    onClick={() => promote(row)}
                    aria-label={`${text("Show in Fundamentals")} — ${row.humanLabel}`}
                  >
                    {busy ? text("Adding…") : text("Show in Fundamentals")}
                  </Button>
                )
              }
            />
          );
        })}
      </ul>
    );
  };

  const count = readState === "ready" ? rows.length : undefined;

  return (
    <div
      role="group"
      className="coverage-uncrosswalked-concepts"
      aria-label={text("Positions the program doesn't know yet")}
    >
      <SectionHeader
        level="h3"
        title={text("Positions the program doesn't know yet")}
        meta={
          count === undefined
            ? undefined
            : `${count} ${pluralNoun(locale, count, CONCEPT_FORMS)}`
        }
      />
      <Hint>
        {text(
          "Captured from tagged reports but not (yet) named in the catalog — ranked by how many companies report them.",
        )}
      </Hint>
      {renderBody()}
      {promoteError ? <ErrorText>{promoteError}</ErrorText> : null}
    </div>
  );
}
