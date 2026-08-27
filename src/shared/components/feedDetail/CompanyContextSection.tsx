import { useState } from "react";
import { getCompanyContext, type CompanyContext, type CompanyContextFact } from "../../../api/companyContext";
import { useCommandQuery } from "../../state/useCommandQuery";
import { Button, EmptyState, ErrorText, ExpandableRow, ProvenanceFigure, Skeleton } from "../../../ui";
import { formatListTimestamp } from "../../format/datetime";
import { formatFinancialValue } from "../../format/financialValue";
import { useLocale, type LocaleCode } from "../../locale";
import { localizedKpiLabelForKey } from "../../locale/kpiLabels";
import { FRESH_FACT_FORMS, UPCOMING_EVENT_FORMS, pluralNoun } from "../../locale/plural";

export type CompanyContextSectionProps = {
  companyId: string;
  /**
   * Navigates to the company's report-documents view — the provenance
   * thread's landing point (ADR 0104 dec. 7). The host owns navigation
   * (shared components don't); omit it where the host has none, and the
   * ticket stays non-interactive rather than a dead button.
   */
  onOpenReportDocuments?: () => void;
};

// `CompanyContextFact` (ADR 0106 dec. 3) carries no `value_kind`/`unit` — the
// composed read model intentionally stays a raw metric_key + value_numeric,
// not a full KPI definition join. Per-share keys round to 0 decimals under
// `formatFinancialValue`'s generic monetary fallback otherwise (EPS "3.49"
// would show as "3"), so the known per-share keys get the same explicit
// `unit: "per_share"` hint `AnalystRecommendationsSection` already uses for
// its DTO-less target price. Ratio/percentage metric keys are not covered —
// they are not among the fields the approved mockup surfaces here.
const PER_SHARE_METRIC_KEYS = new Set(["eps_basic", "eps_diluted", "dividend_per_share"]);

function formatContextFactValue(fact: CompanyContextFact, locale: LocaleCode): string {
  return formatFinancialValue(
    {
      valueNumeric: fact.valueNumeric,
      currency: fact.currency,
      metricKey: fact.metricKey,
      unit: PER_SHARE_METRIC_KEYS.has(fact.metricKey) ? "per_share" : null,
    },
    locale,
  );
}

// Owner dogfooding finding (2026-08-27): the company-context block dominated
// the Inbox detail pane over the item's own body. It now sits behind a
// disclosure, collapsed by default, with a one-line teaser. The expanded
// state is a single MODULE-LEVEL flag (not React state, not persisted to
// settings) so it survives switching between feed items/companies within
// this session instead of collapsing again on every selection — expanding it
// once keeps it expanded for the rest of the session. Reset export is
// test-only.
let contextExpandedInSession = false;
export function __resetCompanyContextExpandedForTests(): void {
  contextExpandedInSession = false;
}

// The collapsed row's one-line summary: period · fresh facts · upcoming
// events (the two signals the owner's finding called out by example). Notes
// and claims stay inside the expanded detail only — the teaser is a scan,
// not a full restate.
function buildContextTeaser(context: CompanyContext, locale: LocaleCode): string {
  const factsCount = context.latestPeriodFacts?.facts.length ?? 0;
  const upcomingCount = context.upcomingEvents.length;
  const parts: string[] = [];
  if (context.latestPeriodFacts) parts.push(context.latestPeriodFacts.periodLabel);
  parts.push(`${factsCount} ${pluralNoun(locale, factsCount, FRESH_FACT_FORMS)}`);
  parts.push(`${upcomingCount} ${pluralNoun(locale, upcomingCount, UPCOMING_EVENT_FORMS)}`);
  return parts.join(" · ");
}

// The Inbox detail pane's company-context block (ADR 0104 dec. 5 — the space a
// shrunk detail frees carries company context; ADR 0106 dec. 3's single
// `get_company_context` read). Loads on `companyId` change; degrades
// gracefully on error (storyboard frames 3/5 — the base item detail above
// always stays visible even when this block fails).
export function CompanyContextSection({ companyId, onOpenReportDocuments }: CompanyContextSectionProps) {
  const { text, locale } = useLocale();
  const { status, data, refetch } = useCommandQuery(
    ["companyContext", companyId],
    () => getCompanyContext(companyId),
  );
  const [expanded, setExpanded] = useState(() => contextExpandedInSession);

  function toggleExpanded() {
    setExpanded((current) => {
      const next = !current;
      contextExpandedInSession = next;
      return next;
    });
  }

  return (
    <section className="feed-context-section" aria-label={text("Company context")}>
      {status === "loading" ? (
        <>
          <div className="feed-body-heading">
            <span>{text("Company context")}</span>
          </div>
          <Skeleton variant="list-row" count={3} label={text("Loading company context…")} />
        </>
      ) : null}
      {status === "error" ? (
        <>
          <div className="feed-body-heading">
            <span>{text("Company context")}</span>
          </div>
          <div className="feed-context-error">
            <ErrorText>{text("Could not load company context.")}</ErrorText>
            <Button className="compact-button" onClick={refetch}>
              {text("Refresh")}
            </Button>
          </div>
        </>
      ) : null}
      {status === "success" ? (
        <ExpandableRow
          className="feed-context-toggle-row"
          detail={<CompanyContextBody context={data} onOpenReportDocuments={onOpenReportDocuments} />}
          isExpanded={expanded}
          label={text("Company context")}
          onToggle={toggleExpanded}
        >
          <span className="feed-context-toggle">
            <span className="feed-context-toggle-title">{text("Company context")}</span>
            <span className="feed-context-toggle-teaser">{buildContextTeaser(data, locale)}</span>
          </span>
        </ExpandableRow>
      ) : null}
    </section>
  );
}

function CompanyContextBody({
  context,
  onOpenReportDocuments,
}: {
  context: CompanyContext;
  onOpenReportDocuments: (() => void) | undefined;
}) {
  const { text, locale } = useLocale();

  return (
    <div className="feed-context-body">
      <FactsBlock context={context} locale={locale} text={text} onOpenReportDocuments={onOpenReportDocuments} />
      <EventsBlock context={context} locale={locale} text={text} />
      <NotebookBlock context={context} locale={locale} text={text} />
      <ClaimsBlock context={context} text={text} />
    </div>
  );
}

type BlockProps = {
  context: CompanyContext;
  locale: LocaleCode;
  text: (value: string) => string;
};

function FactsBlock({
  context,
  locale,
  text,
  onOpenReportDocuments,
}: BlockProps & { onOpenReportDocuments: (() => void) | undefined }) {
  const period = context.latestPeriodFacts;
  if (!period || period.facts.length === 0) {
    return (
      <div className="feed-context-block" role="group" aria-label={text("Latest facts")}>
        <span className="feed-context-block-label">{text("Latest facts")}</span>
        <EmptyState wrapText={false}>
          <span>{text("No recorded financial facts yet.")}</span>
        </EmptyState>
      </div>
    );
  }

  return (
    <div className="feed-context-block" role="group" aria-label={`${text("Latest facts")} · ${period.periodLabel}`}>
      <span className="feed-context-block-label">
        {text("Latest facts")} · {period.periodLabel}
      </span>
      <div className="feed-context-facts-grid">
        {period.facts.map((fact) => (
          <ProvenanceFigure
            key={fact.metricKey}
            label={localizedKpiLabelForKey(fact.metricKey, locale)}
            value={formatContextFactValue(fact, locale)}
            sourceTicket={`${period.periodLabel} · ${formatListTimestamp(fact.createdAt, locale)}`}
            onSourceTicketClick={onOpenReportDocuments}
          />
        ))}
      </div>
    </div>
  );
}

function EventsBlock({ context, locale, text }: BlockProps) {
  return (
    <div className="feed-context-block" role="group" aria-label={text("Upcoming")}>
      <span className="feed-context-block-label">{text("Upcoming")}</span>
      {context.upcomingEvents.length === 0 ? (
        <EmptyState wrapText={false}>
          <span>{text("No upcoming events tracked.")}</span>
        </EmptyState>
      ) : (
        <ul className="feed-context-event-list">
          {context.upcomingEvents.map((event) => (
            <li className="feed-context-event-row" key={`${event.eventDate}-${event.title}`}>
              <span className="feed-context-event-date num-tabular">
                {formatListTimestamp(event.eventDate, locale)}
              </span>
              <span>{event.title}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function NotebookBlock({ context, locale, text }: BlockProps) {
  const { count, latestAt } = context.notebook;
  return (
    <div className="feed-context-block" role="group" aria-label={text("Notes")}>
      <span className="feed-context-block-label">
        {text("Notes")} ({count})
      </span>
      {count === 0 ? (
        <EmptyState wrapText={false}>
          <span>{text("No notebook entries for this company yet.")}</span>
        </EmptyState>
      ) : (
        <p className="feed-context-notebook-latest">
          {text("Latest")}: {formatListTimestamp(latestAt, locale, text("Unknown"))}
        </p>
      )}
    </div>
  );
}

function ClaimsBlock({ context, text }: { context: CompanyContext; text: (value: string) => string }) {
  const { due, overdue } = context.claimsDue;
  return (
    <div className="feed-context-block" role="group" aria-label={text("Management claims")}>
      <span className="feed-context-block-label">{text("Management claims")}</span>
      {due === 0 && overdue === 0 ? (
        <EmptyState wrapText={false}>
          <div>
            <span>{text("No management promises awaiting verification.")}</span>
            <p>{text("Promises are logged from reports and management statements.")}</p>
          </div>
        </EmptyState>
      ) : (
        <p className="feed-context-claims-counts">
          {text("Due")}: {due} · {text("Overdue")}: {overdue}
        </p>
      )}
    </div>
  );
}
