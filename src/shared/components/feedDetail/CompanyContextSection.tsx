import { getCompanyContext, type CompanyContext, type CompanyContextFact } from "../../../api/companyContext";
import { useCommandQuery } from "../../state/useCommandQuery";
import { Button, EmptyState, ErrorText, ProvenanceFigure, Skeleton } from "../../../ui";
import { formatListTimestamp } from "../../format/datetime";
import { formatFinancialValue } from "../../format/financialValue";
import { useLocale, type LocaleCode } from "../../locale";
import { localizedKpiLabelForKey } from "../../locale/kpiLabels";

export type CompanyContextSectionProps = {
  companyId: string;
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

// The Inbox detail pane's company-context block (ADR 0104 dec. 5 — the space a
// shrunk detail frees carries company context; ADR 0106 dec. 3's single
// `get_company_context` read). Loads on `companyId` change; degrades
// gracefully on error (storyboard frames 3/5 — the base item detail above
// always stays visible even when this block fails).
export function CompanyContextSection({ companyId }: CompanyContextSectionProps) {
  const { text } = useLocale();
  const { status, data, refetch } = useCommandQuery(
    ["companyContext", companyId],
    () => getCompanyContext(companyId),
  );

  return (
    <section className="feed-context-section" aria-label={text("Company context")}>
      <div className="feed-body-heading">
        <span>{text("Company context")}</span>
      </div>
      {status === "loading" ? (
        <Skeleton variant="list-row" count={3} label={text("Loading company context…")} />
      ) : null}
      {status === "error" ? (
        <div className="feed-context-error">
          <ErrorText>{text("Could not load company context.")}</ErrorText>
          <Button className="compact-button" onClick={refetch}>
            {text("Refresh")}
          </Button>
        </div>
      ) : null}
      {status === "success" ? <CompanyContextBody context={data} /> : null}
    </section>
  );
}

function CompanyContextBody({ context }: { context: CompanyContext }) {
  const { text, locale } = useLocale();

  return (
    <div className="feed-context-body">
      <FactsBlock context={context} locale={locale} text={text} />
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

function FactsBlock({ context, locale, text }: BlockProps) {
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
