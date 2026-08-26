import { RefreshCw } from "lucide-react";
import { getCompanyView } from "../../api/companyView";
import type { CompanyView } from "../../api/generated/CompanyView";
import type { Company } from "../../api/types";
import { useCommandQuery } from "../../shared/state/useCommandQuery";
import { useLocale } from "../../shared/locale";
import { FACT_FORMS, pluralNoun } from "../../shared/locale/plural";
import { formatFinancialValue } from "../../shared/format/financialValue";
import { formatLocalIsoDate } from "../../shared/format/datetime";
import { Button, CandlestickChart, DenseRow, EmptyState, ErrorText, PanelHeader, SectionHeader, Skeleton } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { GlanceBar } from "./GlanceBar";
import { CoreKpiTable } from "./CoreKpiTable";
import type { Tool } from "./route";

export type SpolkaScreenProps = {
  companyId: string;
  company: Company;
  onOpenTool: (tool: Tool) => void;
  onOpenDocument: (documentRef: string) => void;
  onOpenFeedItem: (feedItemId: string) => void;
  refreshCompletionCount: number;
};

// The Spółka screen (F3a S1, ADR 0107): glance bar + core sections + workshop
// bar for ONE company, off a single composed `get_company_view` read. The
// tool host itself (dockview panes for each workshop kind) is S2 — here
// `onOpenTool` only records the request.
export function SpolkaScreen({
  companyId,
  company,
  onOpenTool,
  onOpenDocument,
  onOpenFeedItem,
  refreshCompletionCount,
}: SpolkaScreenProps) {
  const { text, locale } = useLocale();
  const query = useCommandQuery([companyId, refreshCompletionCount], () => getCompanyView(companyId));

  return (
    <section className="feed-panel spolka-screen" role="region" aria-label={text("Company view")} data-company-id={companyId}>
      <PanelHeader
        title={company.displayName}
        titleId="spolka-title"
        description={
          <>
            <TickerLabel value={company.qualifiedTicker} />
            {company.isin ? ` · ${company.isin}` : ""}
          </>
        }
      />

      <div className="spolka-layout">
        {query.status === "loading" ? (
          <>
            <Skeleton variant="block" label={text("Loading company view…")} />
            <Skeleton variant="list-row" count={5} />
          </>
        ) : null}

        {query.status === "error" ? (
          <div className="spolka-error-card" role="alert">
            <ErrorText>{text("Couldn't read this company's data.")}</ErrorText>
            <p>{text("The connection to your data may be interrupted.")}</p>
            <Button variant="primary" type="button" onClick={query.refetch}>
              <RefreshCw aria-hidden="true" size={13} />
              {text("Refresh")}
            </Button>
          </div>
        ) : null}

        {query.status === "success" ? (
          <SpolkaBody
            data={query.data}
            onOpenTool={onOpenTool}
            onOpenDocument={onOpenDocument}
            onOpenFeedItem={onOpenFeedItem}
            text={text}
            locale={locale}
          />
        ) : null}
      </div>
    </section>
  );
}

type SpolkaBodyProps = {
  data: CompanyView;
  onOpenTool: (tool: Tool) => void;
  onOpenDocument: (documentRef: string) => void;
  onOpenFeedItem: (feedItemId: string) => void;
  text: (value: string) => string;
  locale: "en" | "pl";
};

function shortDate(iso: string): string {
  return `${iso.slice(8, 10)}.${iso.slice(5, 7)}`;
}

function SpolkaBody({ data, onOpenTool, onOpenDocument, onOpenFeedItem, text, locale }: SpolkaBodyProps) {
  const { sectionErrors } = data;

  return (
    <>
      <GlanceBar counters={data.counters} sectionErrors={sectionErrors} onOpenTool={onOpenTool} />

      <div role="group" aria-label={text("Company core")} className="spolka-core">
        <CoreKpiTable
          kpi={data.kpi}
          error={Boolean(sectionErrors.kpi)}
          onOpenTool={onOpenTool}
          onOpenDocument={onOpenDocument}
        />

        <div role="group" aria-label={text("Company feed")} className="spolka-section spolka-feed">
          <SectionHeader level="h3" title={text("Company feed")} />
          {sectionErrors.feed ? (
            <ErrorText>{text("Couldn't load the feed. The rest of the view is up to date.")}</ErrorText>
          ) : data.feed.length === 0 ? (
            <EmptyState>{text("No filings or media yet for this company.")}</EmptyState>
          ) : (
            <ul className="spolka-feed-rows">
              {data.feed.slice(0, 6).map((item) => (
                <li key={item.feedItemId}>
                  <DenseRow as="button" unread={!item.read} onClick={() => onOpenFeedItem(item.feedItemId)}>
                    <span className="num-tabular">{shortDate(item.publishedAt)}</span> {item.title}
                  </DenseRow>
                </li>
              ))}
            </ul>
          )}
          <Button variant="secondary" onClick={() => onOpenTool({ t: "feed" })}>
            {text("Open feed")}
          </Button>
        </div>

        <div role="group" aria-label={text("Price chart")} className="spolka-section spolka-price">
          <SectionHeader level="h3" title={text("Price chart")} />
          {sectionErrors.price ? (
            <ErrorText>{text("Couldn't load the price chart. The rest of the view is up to date.")}</ErrorText>
          ) : !data.price || data.price.emptyReason ? (
            <EmptyState>{text("No price data is available for this company yet.")}</EmptyState>
          ) : (
            (() => {
              const price = data.price;
              return (
                <>
                  <div className="spolka-price-headline">
                    <span className="num-tabular">
                      {formatFinancialValue(
                        { valueNumeric: String(price.lastClose), currency: price.currency, valueKind: "monetary", unit: "per_share" },
                        locale,
                      )}
                    </span>
                    <span className="spolka-price-asof">
                      {text("GPW")} · {formatLocalIsoDate(price.asOf)}
                    </span>
                    {price.deltaYtdPct !== undefined ? (
                      <span className="num-tabular">
                        {formatFinancialValue({ valueNumeric: String(price.deltaYtdPct), valueKind: "percentage" }, locale)}{" "}
                        {text("YTD")}
                      </span>
                    ) : null}
                    {price.delta1mPct !== undefined ? (
                      <span className="num-tabular">
                        {formatFinancialValue({ valueNumeric: String(price.delta1mPct), valueKind: "percentage" }, locale)}{" "}
                        {text("1M")}
                      </span>
                    ) : null}
                  </div>
                  {price.candles.length > 1 ? (
                    <CandlestickChart
                      ariaLabel={text("Price history")}
                      points={price.candles.map((c) => ({ label: c.date, open: c.open, high: c.high, low: c.low, close: c.close }))}
                      height={110}
                      formatValue={(v) =>
                        formatFinancialValue({ valueNumeric: String(v), currency: price.currency, valueKind: "monetary", unit: "per_share" }, locale)
                      }
                      className="spolka-price-chart"
                      scale="log"
                    />
                  ) : null}
                  <p className="spolka-price-caption">{text("3 months · daily candles (OHLC) · log scale · decision context, not a signal")}</p>
                </>
              );
            })()
          )}
        </div>

        <div role="group" aria-label={text("Report coverage")} className="spolka-section spolka-coverage">
          <SectionHeader level="h3" title={text("Report coverage")} />
          {sectionErrors.coverage ? (
            <ErrorText>{text("Couldn't load report coverage. The rest of the view is up to date.")}</ErrorText>
          ) : data.coverage.length === 0 ? (
            <EmptyState>{text("No coverage tracked yet for this company.")}</EmptyState>
          ) : (
            <ul className="spolka-coverage-rows">
              {data.coverage.map((period) => (
                <li key={`${period.fiscalYear}-${period.periodType}`}>
                  <span>
                    {period.periodType} {period.fiscalYear}
                  </span>{" "}
                  <span>
                    {period.report
                      ? period.report.fetched
                        ? text("read")
                        : text("fetched")
                      : text("expected")}
                    {period.report ? ` · ${period.facts.total} ${pluralNoun(locale, period.facts.total, FACT_FORMS)}` : ""}
                  </span>
                </li>
              ))}
            </ul>
          )}
          <Button variant="secondary" onClick={() => onOpenTool({ t: "pokrycie" })}>
            {text("Open coverage")}
          </Button>
        </div>

        <div role="group" aria-label={text("Recommendations")} className="spolka-section spolka-recommendations">
          <SectionHeader level="h3" title={text("Recommendations")} />
          {sectionErrors.recommendations ? (
            <ErrorText>{text("Couldn't load recommendations. The rest of the view is up to date.")}</ErrorText>
          ) : data.recommendations.length === 0 ? (
            <EmptyState>{text("No analyst recommendations yet for this company.")}</EmptyState>
          ) : (
            <ul className="spolka-recommendations-rows">
              {data.recommendations.slice(0, 3).map((rec, index) => (
                <li key={`${rec.firm}-${rec.publishedAt}-${index}`}>
                  {rec.firm} · {rec.rating} {rec.targetPrice ?? ""} · {formatLocalIsoDate(rec.publishedAt)}
                </li>
              ))}
            </ul>
          )}
          <Button variant="secondary" onClick={() => onOpenTool({ t: "rekomendacje" })}>
            {text("Open recommendations")}
          </Button>
        </div>
      </div>

      <div role="group" aria-label={text("Workshop")} className="spolka-workshop">
        <Button variant="ghost" onClick={() => onOpenTool({ t: "tezy" })}>
          {text("Open claims")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "notatnik" })}>
          {text("Open notebook")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "dziennik" })}>
          {text("Open decision journal")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "jakosc" })}>
          {text("Open quality")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "diff" })}>
          {text("Open report diff")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "research" })}>
          {text("Open research")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "akcjonariat" })}>
          {text("Open ownership")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "sygnaly" })}>
          {text("Open signals")}
        </Button>
        <Button variant="ghost" onClick={() => onOpenTool({ t: "dokumenty" })}>
          {text("Open documents")}
        </Button>
      </div>
    </>
  );
}
