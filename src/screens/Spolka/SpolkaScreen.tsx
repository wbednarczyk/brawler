import { useEffect, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { getCompanyView } from "../../api/companyView";
import type { CompanyView } from "../../api/generated/CompanyView";
import type { Company, FeedItem } from "../../api/types";
import { useCommandQuery } from "../../shared/state/useCommandQuery";
import { useLocale } from "../../shared/locale";
import { CALENDAR_EVENT_FORMS, CLAIM_FORMS, FACT_FORMS, SIGNAL_FORMS, pluralNoun } from "../../shared/locale/plural";
import { formatFinancialValue } from "../../shared/format/financialValue";
import { formatLocalIsoDate } from "../../shared/format/datetime";
import { Button, CandlestickChart, DenseRow, EmptyState, ErrorText, PanelHeader, SectionHeader, Skeleton } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { GlanceBar } from "./GlanceBar";
import { CoreKpiTable } from "./CoreKpiTable";
import { renderTool } from "./toolRegistry";
import { SpolkaToolHostProvider, ToolHostConfirmModal, type SpolkaToolHostApi } from "./ToolHost";
import type { Tool } from "./route";

export type SpolkaScreenProps = {
  companyId: string;
  company: Company;
  spolkaTool: SpolkaToolHostApi;
  feedItems: FeedItem[];
  /** Today's `openCompanyClaims` global highlight (F2 S3) — the `tezy` tool's
   * fallback when it carries no `claimId` of its own. */
  rootHighlightClaimId: string | null;
  onOpenDocument: (documentRef: string) => void;
  onOpenFeedItem: (feedItemId: string) => void;
  refreshCompletionCount: number;
};

// The Spółka screen (F3a S1+S2, ADR 0107): glance bar + core sections + a
// workshop bar that opens ONE tool at a time in place of the core (the
// tool-host seam, `./ToolHost.tsx`), off a single composed `get_company_view`
// read.
export function SpolkaScreen({
  companyId,
  company,
  spolkaTool,
  feedItems,
  rootHighlightClaimId,
  onOpenDocument,
  onOpenFeedItem,
  refreshCompletionCount,
}: SpolkaScreenProps) {
  const { text, locale } = useLocale();
  const query = useCommandQuery([companyId, refreshCompletionCount], () => getCompanyView(companyId));
  const layoutRef = useRef<HTMLDivElement>(null);
  const lastCoreScrollTopRef = useRef(0);

  // A tool only belongs to THIS render if it was opened for THIS company — a
  // late `get_company_view` response, or a tool left open from a prior
  // company, can never reopen/leak across companies (plan §11).
  const isToolActive = spolkaTool.tool !== null && spolkaTool.toolCompanyId === companyId;

  // Closing a tool restores `.spolka-layout`'s scroll position exactly (plan
  // §8 "closing a tool restores core scroll and selection") — the core stays
  // mounted (`hidden`, never unmounted) so its own state (the selected feed
  // row) survives on its own; only the shared scroll container needs help.
  useEffect(() => {
    if (!isToolActive && layoutRef.current) {
      layoutRef.current.scrollTop = lastCoreScrollTopRef.current;
    }
  }, [isToolActive]);

  function openTool(tool: Tool) {
    // Capture the core's scroll position at the moment it's about to be
    // hidden — jsdom (and the "set scrollTop directly" test scenario) does
    // not reliably fire `scroll` events, so a continuous listener would miss
    // it; a point-in-time read here is exact.
    if (!isToolActive && layoutRef.current) {
      lastCoreScrollTopRef.current = layoutRef.current.scrollTop;
    }
    spolkaTool.openTool(companyId, tool);
  }

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

      <SpolkaToolHostProvider host={spolkaTool}>
        <div className="spolka-layout" ref={layoutRef}>
          {isToolActive && query.status === "success" ? (
            <SummaryStrip data={query.data} locale={locale} text={text} />
          ) : null}

          {query.status === "loading" && !isToolActive ? (
            <>
              <Skeleton variant="block" label={text("Loading company view…")} />
              <Skeleton variant="list-row" count={5} />
            </>
          ) : null}

          {query.status === "error" && !isToolActive ? (
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
              onOpenTool={openTool}
              onOpenDocument={onOpenDocument}
              text={text}
              locale={locale}
              hidden={isToolActive}
              activeToolKind={isToolActive ? spolkaTool.tool?.t ?? null : null}
            />
          ) : null}

          {isToolActive && spolkaTool.tool ? (
            renderTool(spolkaTool.tool, {
              companyId,
              company,
              feedItems,
              rootHighlightClaimId,
              onOpenTool: openTool,
              onOpenDocument,
              onOpenFeedItem,
              onCloseTool: spolkaTool.closeTool,
            })
          ) : null}
        </div>
        <ToolHostConfirmModal host={spolkaTool} />
      </SpolkaToolHostProvider>
    </section>
  );
}

function formatCount(n: number): string {
  return n > 99 ? "99+" : String(n);
}

// Tool-open layout (plan §3): the core collapses to a one-line strip — ticker,
// the 4 counters, last price — while the tool fills the zone.
function SummaryStrip({
  data,
  locale,
  text,
}: {
  data: CompanyView;
  locale: "en" | "pl";
  text: (value: string) => string;
}) {
  const counters = data.counters;
  return (
    <div role="group" aria-label={text("Company summary strip")} className="spolka-core-strip">
      <TickerLabel value={data.qualifiedTicker} />
      {counters ? (
        <>
          <span>
            <span className="num-tabular">{formatCount(counters.signals.unacked)}</span>{" "}
            {pluralNoun(locale, counters.signals.unacked, SIGNAL_FORMS)}
          </span>
          <span>
            <span className="num-tabular">{formatCount(counters.claims.open)}</span>{" "}
            {pluralNoun(locale, counters.claims.open, CLAIM_FORMS)}
          </span>
          <span>
            <span className="num-tabular">
              {formatFinancialValue({ valueNumeric: String(counters.shorts.activeSumPct), valueKind: "percentage" }, locale)}
            </span>{" "}
            {text("short")}
          </span>
          <span>
            <span className="num-tabular">{formatCount(counters.events.upcoming)}</span>{" "}
            {pluralNoun(locale, counters.events.upcoming, CALENDAR_EVENT_FORMS)}
          </span>
        </>
      ) : null}
      {data.price && !data.price.emptyReason ? (
        <span className="num-tabular">
          {formatFinancialValue(
            { valueNumeric: String(data.price.lastClose), currency: data.price.currency, valueKind: "monetary", unit: "per_share" },
            locale,
          )}{" "}
          · {formatLocalIsoDate(data.price.asOf)}
        </span>
      ) : null}
    </div>
  );
}

type SpolkaBodyProps = {
  data: CompanyView;
  onOpenTool: (tool: Tool) => void;
  onOpenDocument: (documentRef: string) => void;
  text: (value: string) => string;
  locale: "en" | "pl";
  hidden: boolean;
  activeToolKind: Tool["t"] | null;
};

function shortDate(iso: string): string {
  return `${iso.slice(8, 10)}.${iso.slice(5, 7)}`;
}

function SpolkaBody({ data, onOpenTool, onOpenDocument, text, locale, hidden, activeToolKind }: SpolkaBodyProps) {
  const { sectionErrors } = data;
  // The core stays mounted (never unmounted) across a tool open/close, so this
  // selection survives on its own — no extra restore wiring needed.
  const [selectedFeedItemId, setSelectedFeedItemId] = useState<string | null>(null);

  function openFeedItemTool(feedItemId: string) {
    setSelectedFeedItemId(feedItemId);
    onOpenTool({ t: "feedItem", feedItemId });
  }

  return (
    <>
      <div hidden={hidden} className="spolka-core-wrap">
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
                  <li key={item.feedItemId} data-selected={item.feedItemId === selectedFeedItemId || undefined}>
                    <DenseRow as="button" unread={!item.read} onClick={() => openFeedItemTool(item.feedItemId)}>
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
      </div>

      <WorkshopBar onOpenTool={onOpenTool} text={text} activeToolKind={activeToolKind} />
    </>
  );
}

const WORKSHOP_TOOLS: Array<{ tool: Tool; label: string }> = [
  { tool: { t: "tezy" }, label: "Open claims" },
  { tool: { t: "notatnik" }, label: "Open notebook" },
  { tool: { t: "dziennik" }, label: "Open decision journal" },
  { tool: { t: "jakosc" }, label: "Open quality" },
  { tool: { t: "diff" }, label: "Open report diff" },
  { tool: { t: "research" }, label: "Open research" },
  { tool: { t: "akcjonariat" }, label: "Open ownership" },
  { tool: { t: "sygnaly" }, label: "Open signals" },
  { tool: { t: "dokumenty" }, label: "Open documents" },
];

// Stays visible whether or not a tool is open (deliverable 3) and marks the
// active tool kind (`aria-pressed`).
function WorkshopBar({
  onOpenTool,
  text,
  activeToolKind,
}: {
  onOpenTool: (tool: Tool) => void;
  text: (value: string) => string;
  activeToolKind: Tool["t"] | null;
}) {
  return (
    <div role="group" aria-label={text("Workshop")} className="spolka-workshop">
      {WORKSHOP_TOOLS.map(({ tool, label }) => (
        <Button
          key={label}
          variant="ghost"
          aria-pressed={tool.t === activeToolKind}
          onClick={() => onOpenTool(tool)}
        >
          {text(label)}
        </Button>
      ))}
    </div>
  );
}
