import { useEffect, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { getCompanyView } from "../../api/companyView";
import type { CompanyView } from "../../api/generated/CompanyView";
import type { Company, FeedItem } from "../../api/types";
import { useCommandQuery } from "../../shared/state/useCommandQuery";
import { useLocale } from "../../shared/locale";
import { CALENDAR_EVENT_FORMS, CLAIM_FORMS, SIGNAL_FORMS, pluralNoun } from "../../shared/locale/plural";
import { deltaToneClass, formatFinancialValue } from "../../shared/format/financialValue";
import { formatLocalIsoDate } from "../../shared/format/datetime";
import { Button, CandlestickChart, DenseRow, EmptyState, ErrorText, PanelHeader, SectionHeader, SelectField, Skeleton, StatusChip } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { GlanceBar, formatCount } from "./GlanceBar";
import { CoreKpiTable } from "./CoreKpiTable";
import { renderTool } from "./toolRegistry";
import { SpolkaToolHostProvider, ToolHostConfirmModal, type SpolkaToolHostApi } from "./ToolHost";
import type { Tool } from "./route";
import { useCommandPaletteCommands } from "../../app/commandPalette";
import type { PaletteCommand } from "../../shared/components/CommandPalette";

// Contextual palette entries (F3a S3, plan "Trasy powierzchni globalnych" +
// ADR 0104 dec. 3) while the Spółka screen is active: one `Open <tool>` entry
// per parameterless Tool variant (`feedItem` is excluded — it always needs a
// specific `feedItemId`, so it has no standalone command), plus `Open
// overview` (`tool: null`) for the closed-tool state — the same set the
// workshop bar lists (owner dogfooding v0.74 wave 2, item 1). A palette
// command is an ACTION ("Otwórz X") — the workshop-bar/core buttons that open
// the SAME tool are DESTINATIONS and carry the noun form instead
// (`WORKSHOP_TOOLS` below, ADR 0104 dec. 3 amendment, owner dogfooding v0.74
// item 4); the two labels legitimately diverge.
export const SPOLKA_TOOL_COMMANDS: ReadonlyArray<{ tool: Tool | null; label: string; actionKey: string }> = [
  { tool: null, label: "Open overview", actionKey: "tool.open.overview" },
  { tool: { t: "fundamenty" }, label: "Open fundamentals", actionKey: "tool.open.fundamenty" },
  { tool: { t: "feed" }, label: "Open feed", actionKey: "tool.open.feed" },
  { tool: { t: "pokrycie" }, label: "Open coverage", actionKey: "tool.open.pokrycie" },
  { tool: { t: "rekomendacje" }, label: "Open recommendations", actionKey: "tool.open.rekomendacje" },
  { tool: { t: "tezy" }, label: "Open claims", actionKey: "tool.open.tezy" },
  { tool: { t: "notatnik" }, label: "Open notebook", actionKey: "tool.open.notatnik" },
  { tool: { t: "dziennik" }, label: "Open decision journal", actionKey: "tool.open.dziennik" },
  { tool: { t: "jakosc" }, label: "Open quality", actionKey: "tool.open.jakosc" },
  { tool: { t: "diff" }, label: "Open report diff", actionKey: "tool.open.diff" },
  { tool: { t: "research" }, label: "Open research", actionKey: "tool.open.research" },
  { tool: { t: "akcjonariat" }, label: "Open ownership", actionKey: "tool.open.akcjonariat" },
  { tool: { t: "sygnaly" }, label: "Open signals", actionKey: "tool.open.sygnaly" },
  { tool: { t: "dokumenty" }, label: "Open documents", actionKey: "tool.open.dokumenty" },
  { tool: { t: "wydarzenia" }, label: "Open events", actionKey: "tool.open.wydarzenia" },
];

export type SpolkaScreenProps = {
  companyId: string;
  company: Company;
  /** Every tracked company, for the header's company picker (owner dogfooding
   * v0.74, 2026-08-27, item 6) — a screen with no way to switch company. */
  companies: Company[];
  spolkaTool: SpolkaToolHostApi;
  feedItems: FeedItem[];
  /** Today's `openCompanyClaims` global highlight (F2 S3) — the `tezy` tool's
   * fallback when it carries no `claimId` of its own. */
  rootHighlightClaimId: string | null;
  onOpenDocument: (documentRef: string) => void;
  /** A KPI provenance ticket that names a URL, not a stored document (a
   * BiznesRadar aggregator fact, ADR 0086) — opens in the system browser via
   * the root's `openExternalUrl` seam (owner dogfooding v0.74 wave 2, item 3). */
  onOpenExternalUrl: (url: string) => void;
  onOpenFeedItem: (feedItemId: string) => void;
  /** Picking a different company from the header — routed through the SAME
   * guarded atomic transition every entry point uses (`useSpolkaNavigate`),
   * never a direct `selectedCompanyId` set (owner dogfooding v0.74, item 6). */
  onSwitchCompany: (companyId: string) => void;
  refreshCompletionCount: number;
};

// The Spółka screen (F3a S1+S2, ADR 0107): glance bar + core sections + a
// workshop bar that opens ONE tool at a time in place of the core (the
// tool-host seam, `./ToolHost.tsx`), off a single composed `get_company_view`
// read.
export function SpolkaScreen({
  companyId,
  company,
  companies,
  spolkaTool,
  feedItems,
  rootHighlightClaimId,
  onOpenDocument,
  onOpenExternalUrl,
  onOpenFeedItem,
  onSwitchCompany,
  refreshCompletionCount,
}: SpolkaScreenProps) {
  const { text, locale } = useLocale();
  const query = useCommandQuery([companyId, refreshCompletionCount], () => getCompanyView(companyId));
  // The scroll-restore ref lives on `.spolka-body-scroll` — the element that
  // ACTUALLY scrolls (owner dogfooding v0.74, item 1): `.spolka-layout`
  // itself never scrolls any more, so the workshop bar (its fixed sibling)
  // can never scroll out of view with it.
  const bodyScrollRef = useRef<HTMLDivElement>(null);
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
    if (!isToolActive && bodyScrollRef.current) {
      bodyScrollRef.current.scrollTop = lastCoreScrollTopRef.current;
    }
  }, [isToolActive]);

  function openTool(tool: Tool) {
    // Capture the core's scroll position at the moment it's about to be
    // hidden — jsdom (and the "set scrollTop directly" test scenario) does
    // not reliably fire `scroll` events, so a continuous listener would miss
    // it; a point-in-time read here is exact.
    if (!isToolActive && bodyScrollRef.current) {
      lastCoreScrollTopRef.current = bodyScrollRef.current.scrollTop;
    }
    spolkaTool.openTool(companyId, tool);
  }

  // Contribute the tool-open commands to the global ⌘K palette while this
  // screen is mounted (F3a S3): a distinct source id, distinct `actionKey`
  // namespace.
  const spolkaToolCommands: PaletteCommand[] = SPOLKA_TOOL_COMMANDS.map(({ tool, label, actionKey }) => ({
    id: `spolka-tool:${actionKey}`,
    label: text(label),
    verb: "open",
    actionKey,
    run: () => (tool ? openTool(tool) : spolkaTool.closeTool()),
  }));
  useCommandPaletteCommands("spolka", spolkaToolCommands);

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
        actions={
          companies.length > 1 ? (
            <SelectField
              label={text("Company")}
              value={companyId}
              onChange={(event) => onSwitchCompany(event.target.value)}
            >
              {companies.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.qualifiedTicker} · {c.displayName}
                </option>
              ))}
            </SelectField>
          ) : undefined
        }
      />

      <SpolkaToolHostProvider host={spolkaTool}>
        <div className="spolka-layout">
          <div className="spolka-body-scroll" ref={bodyScrollRef}>
            {isToolActive && query.status === "success" ? (
              <SummaryStrip data={query.data} locale={locale} text={text} onOverview={spolkaTool.closeTool} />
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
                onOpenExternalUrl={onOpenExternalUrl}
                text={text}
                locale={locale}
                hidden={isToolActive}
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

          <WorkshopBar
            onOpenTool={openTool}
            onOverview={spolkaTool.closeTool}
            text={text}
            activeToolKind={isToolActive ? spolkaTool.tool?.t ?? null : null}
          />
        </div>
        <ToolHostConfirmModal host={spolkaTool} />
      </SpolkaToolHostProvider>
    </section>
  );
}


// Tool-open layout (plan §3): the core collapses to a one-line strip — ticker,
// the 4 counters, last price — while the tool fills the zone. The ticker
// doubles as an "Overview" shortcut (owner dogfooding v0.74, item 5) — the
// same core the tool's own header button returns to.
function SummaryStrip({
  data,
  locale,
  text,
  onOverview,
}: {
  data: CompanyView;
  locale: "en" | "pl";
  text: (value: string) => string;
  onOverview: () => void;
}) {
  const counters = data.counters;
  return (
    <div role="group" aria-label={text("Company summary strip")} className="spolka-core-strip">
      <button
        type="button"
        className="spolka-core-strip-ticker"
        aria-label={text("Back to overview")}
        onClick={onOverview}
      >
        <TickerLabel value={data.qualifiedTicker} />
      </button>
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
  onOpenExternalUrl: (url: string) => void;
  text: (value: string) => string;
  locale: "en" | "pl";
  hidden: boolean;
};

function shortDate(iso: string): string {
  return `${iso.slice(8, 10)}.${iso.slice(5, 7)}`;
}

const DASH = "—";

function SpolkaBody({ data, onOpenTool, onOpenDocument, onOpenExternalUrl, text, locale, hidden }: SpolkaBodyProps) {
  const { sectionErrors } = data;
  // The core stays mounted (never unmounted) across a tool open/close, so this
  // selection survives on its own — no extra restore wiring needed.
  const [selectedFeedItemId, setSelectedFeedItemId] = useState<string | null>(null);

  function openFeedItemTool(feedItemId: string) {
    setSelectedFeedItemId(feedItemId);
    onOpenTool({ t: "feedItem", feedItemId });
  }

  return (
    <div hidden={hidden} className="spolka-core-wrap">
      <GlanceBar counters={data.counters} sectionErrors={sectionErrors} onOpenTool={onOpenTool} />

        <div role="group" aria-label={text("Company core")} className="spolka-core">
          <CoreKpiTable
            kpi={data.kpi}
            error={Boolean(sectionErrors.kpi)}
            onOpenTool={onOpenTool}
            onOpenDocument={onOpenDocument}
            onOpenExternalUrl={onOpenExternalUrl}
          />

          {/* tabIndex on every card (item 1): each now scrolls its own
              overflow, so axe's scrollable-region-focusable needs it
              keyboard-reachable even before it has any focusable child. */}
          <div role="group" aria-label={text("Company feed")} className="spolka-section spolka-feed" tabIndex={0}>
            <SectionHeader level="h2" title={text("Company feed")} />
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
              {text("Feed")}
            </Button>
          </div>

          <div role="group" aria-label={text("Price chart")} className="spolka-section spolka-price" tabIndex={0}>
            <SectionHeader level="h2" title={text("Price chart")} />
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
                      <span className="num-tabular spolka-price-value">
                        {formatFinancialValue(
                          { valueNumeric: String(price.lastClose), currency: price.currency, valueKind: "monetary", unit: "per_share" },
                          locale,
                        )}
                      </span>
                      <span className="spolka-price-asof">
                        {text("GPW")} · {formatLocalIsoDate(price.asOf)}
                      </span>
                      {price.deltaYtdPct !== undefined ? (
                        <span className={["num-tabular", deltaToneClass(price.deltaYtdPct)].filter(Boolean).join(" ")}>
                          {formatFinancialValue({ valueNumeric: String(price.deltaYtdPct), valueKind: "percentage" }, locale)}{" "}
                          {text("YTD")}
                        </span>
                      ) : null}
                      {price.delta1mPct !== undefined ? (
                        <span className={["num-tabular", deltaToneClass(price.delta1mPct)].filter(Boolean).join(" ")}>
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

          <div role="group" aria-label={text("Report coverage")} className="spolka-section spolka-coverage" tabIndex={0}>
            <SectionHeader level="h2" title={text("Report coverage")} />
            {sectionErrors.coverage ? (
              <ErrorText>{text("Couldn't load report coverage. The rest of the view is up to date.")}</ErrorText>
            ) : data.coverage.length === 0 ? (
              <EmptyState>{text("No coverage tracked yet for this company.")}</EmptyState>
            ) : (
              // Cap to the 8 newest periods (owner dogfooding v0.74, item 2):
              // the real base carries 30+ rows; the rest lives behind the
              // `pokrycie` tool's own button below. A compact table (wave 2,
              // item 2) — period as a mono id, status as a StatusChip
              // (read → ok, fetched → neutral, expected → caution — the
              // mockup's amber "oczekiwany"), facts count in the figure face.
              <table className="spolka-coverage-table">
                <thead>
                  <tr>
                    <th>{text("Period")}</th>
                    <th>{text("Status")}</th>
                    <th className="num-tabular">{text("Facts")}</th>
                  </tr>
                </thead>
                <tbody>
                  {data.coverage.slice(0, 8).map((period) => {
                    const status = period.report ? (period.report.fetched ? "read" : "fetched") : "expected";
                    const tone = status === "read" ? "ok" : status === "expected" ? "warn" : "neutral";
                    return (
                      <tr key={`${period.fiscalYear}-${period.periodType}`}>
                        <td className="spolka-coverage-period">
                          {period.periodType} {period.fiscalYear}
                        </td>
                        <td>
                          <StatusChip tone={tone}>{text(status)}</StatusChip>
                        </td>
                        <td className="num-tabular">{period.report ? period.facts.total : DASH}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
            <Button variant="secondary" onClick={() => onOpenTool({ t: "pokrycie" })}>
              {text("Coverage")}
            </Button>
          </div>

          <div role="group" aria-label={text("Recommendations")} className="spolka-section spolka-recommendations" tabIndex={0}>
            <SectionHeader level="h2" title={text("Recommendations")} />
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
              {text("Recommendations")}
            </Button>
          </div>
        </div>
      </div>
  );
}

// Destination labels (owner dogfooding v0.74, item 4; ADR 0104 dec. 3
// amendment): nouns, not verbs — "Otwórz X" is reserved for the ⌘K palette
// command that performs the same open (`SPOLKA_TOOL_COMMANDS` above). Every
// tool the screen hosts, in one bar (owner dogfooding v0.74 wave 2, item 1) —
// the card buttons for fundamenty/feed/pokrycie/rekomendacje/wydarzenia stay
// as ADDITIONAL entry points, this is the exhaustive one.
const WORKSHOP_TOOLS: Array<{ tool: Tool; label: string }> = [
  { tool: { t: "fundamenty" }, label: "Fundamentals" },
  { tool: { t: "feed" }, label: "Feed" },
  { tool: { t: "pokrycie" }, label: "Coverage" },
  { tool: { t: "rekomendacje" }, label: "Recommendations" },
  { tool: { t: "tezy" }, label: "Claims" },
  { tool: { t: "notatnik" }, label: "Notebook" },
  { tool: { t: "dziennik" }, label: "Decision journal" },
  { tool: { t: "jakosc" }, label: "Quality" },
  { tool: { t: "diff" }, label: "Report diff" },
  { tool: { t: "research" }, label: "Research" },
  { tool: { t: "akcjonariat" }, label: "Ownership" },
  { tool: { t: "sygnaly" }, label: "Signals" },
  { tool: { t: "dokumenty" }, label: "Documents" },
  { tool: { t: "wydarzenia" }, label: "Events" },
];

// Stays visible whether or not a tool is open (deliverable 3, now rendered as
// a FIXED sibling of the scrolling body — owner dogfooding v0.74, item 1) and
// marks the active tab (`aria-pressed`) with a filled/accent-bordered tab
// look (wave 2, item 1) — selection is a sanctioned filled-cyan use (ADR 0104
// dec. 1). "Overview" leads the bar — the main (no-tool) view, active
// whenever no tool is open — mirroring the mockup's "Warsztat" eyebrow.
function WorkshopBar({
  onOpenTool,
  onOverview,
  text,
  activeToolKind,
}: {
  onOpenTool: (tool: Tool) => void;
  onOverview: () => void;
  text: (value: string) => string;
  activeToolKind: Tool["t"] | null;
}) {
  return (
    <div role="group" aria-label={text("Workshop")} className="spolka-workshop">
      <span className="ui-section-eyebrow spolka-workshop-eyebrow">{text("Workshop")}</span>
      <Button variant="ghost" aria-pressed={activeToolKind === null} onClick={onOverview}>
        {text("Overview")}
      </Button>
      {WORKSHOP_TOOLS.map(({ tool, label }) => (
        <Button
          key={tool.t}
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
