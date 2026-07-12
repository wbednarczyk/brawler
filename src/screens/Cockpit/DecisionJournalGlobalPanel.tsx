import { useEffect, useMemo, useState } from "react";
import type { Company } from "../../api/types";
import * as decisionJournalApi from "../../api/decisionJournal";
import type { DecisionEntry } from "../../api/decisionJournal";
import { DECISION_KINDS } from "./useCockpitDecisionJournal";
import { decisionKindLabel } from "./DecisionJournalSection";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { MarkdownNoteBody } from "../../shared/components/MarkdownNoteBody";
import { useLocale } from "../../shared/locale";
import { pluralNoun } from "../../shared/locale/plural";
import {
  EmptyState,
  ErrorText,
  FilterToolbar,
  SectionHeader,
  SelectField,
  StatusChip,
  StatusPill,
} from "../../ui";

// The global, cross-company decision journal (ADR 0071, J3): a read-only
// chronological list (decided_at, newest first — the backend orders it) with
// filters by decision kind and company. The composer lives in the per-company
// section; this panel is the calibration-loop review surface. Prop-fed with the
// tracked companies so rows render a `TickerLabel` and the filter resolves names.
export function DecisionJournalGlobalPanel({ companies }: { companies: Company[] }) {
  const { text, locale } = useLocale();
  const [entries, setEntries] = useState<DecisionEntry[]>([]);
  const [kindFilter, setKindFilter] = useState("");
  const [companyFilter, setCompanyFilter] = useState("");
  const [error, setError] = useState<string | null>(null);

  const companyById = useMemo(
    () => new Map(companies.map((company) => [company.id, company])),
    [companies],
  );

  useEffect(() => {
    decisionJournalApi
      .listDecisionEntries({
        ...(companyFilter ? { companyId: companyFilter } : {}),
        ...(kindFilter ? { kind: kindFilter } : {}),
      })
      .then((rows) => {
        setEntries(rows);
        setError(null);
      })
      .catch((reason) => setError(String(reason)));
  }, [kindFilter, companyFilter]);

  const noun = pluralNoun(locale, entries.length, {
    en: ["decision", "decisions"],
    pl: ["decyzja", "decyzje", "decyzji"],
  });

  return (
    <div className="feed-panel decision-journal-global-panel" aria-label={text("Decision journal")}>
      {/* Compact in-pane header (ADR 0076 D6): this panel only ever renders
          inside a cockpit dock pane whose tab already reads "Journal (all
          companies)", so `paneLead` clips the duplicating title (kept in the
          accessible tree via .cockpit-pane). The entry count lives in the meta
          slot, which survives compaction, rather than the dropped subtitle. */}
      <SectionHeader
        level="h3"
        paneLead
        title={text("Decision journal")}
        meta={`${entries.length} ${noun}`}
      />
      <div className="decision-journal-global-body">
        <FilterToolbar ariaLabel={text("Decision journal filters")}>
          <SelectField
            label={text("Decision")}
            aria-label={text("Filter by decision kind")}
            value={kindFilter}
            onChange={(event) => setKindFilter(event.target.value)}
          >
            <option value="">{text("All decisions")}</option>
            {DECISION_KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {decisionKindLabel(kind, text)}
              </option>
            ))}
          </SelectField>
          <SelectField
            label={text("Company")}
            aria-label={text("Filter by company")}
            value={companyFilter}
            onChange={(event) => setCompanyFilter(event.target.value)}
          >
            <option value="">{text("All companies")}</option>
            {companies.map((company) => (
              <option key={company.id} value={company.id}>
                {company.qualifiedTicker} - {company.displayName}
              </option>
            ))}
          </SelectField>
        </FilterToolbar>

        <div className="decision-journal-global-list" aria-label={text("Decision entries")}>
          {entries.map((entry) => {
            const company = companyById.get(entry.companyId);
            return (
              <article className="decision-journal-global-row" key={entry.id}>
                <div className="decision-journal-global-row-head">
                  {company ? (
                    <TickerLabel value={company.qualifiedTicker} />
                  ) : (
                    <span className="membership-empty">{entry.companyId}</span>
                  )}
                  <StatusPill>{decisionKindLabel(entry.kind, text)}</StatusPill>
                  {entry.supersededByEntryId ? (
                    <StatusChip tone="accent">{text("Follow-up")}</StatusChip>
                  ) : null}
                  <time className="decision-journal-row-date num-tabular" dateTime={entry.decidedAt}>
                    {entry.decidedAt}
                  </time>
                </div>
                <MarkdownNoteBody
                  ariaLabel={text("Decision rationale")}
                  body={entry.rationaleMd}
                />
              </article>
            );
          })}
          {entries.length === 0 ? (
            <EmptyState>{text("No decisions recorded yet.")}</EmptyState>
          ) : null}
        </div>
      </div>

      {error ? (
        <ErrorText>
          {text("Decision journal command failed")}: {error}
        </ErrorText>
      ) : null}
    </div>
  );
}
