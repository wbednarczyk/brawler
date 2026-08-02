import { ExternalLink } from "lucide-react";
import { useLocale } from "../../shared/locale";
import {
  formatDetailTimestamp,
  formatListTimestamp,
  formatLocalIsoDate,
} from "../../shared/format/datetime";
import { formatFinancialValue } from "../../shared/format/financialValue";
import { Button, CandlestickChart, EmptyState, InfoGrid, SectionHeader, StatusChip } from "../../ui";
import type { PriceContext } from "../../api/generated/PriceContext";
import type { PriceContextRatios } from "../../api/generated/PriceContextRatios";
import type { PriceContextHistoryPoint } from "../../api/generated/PriceContextHistoryPoint";
import type { AnalystRecommendationTarget } from "../../api/analystRecommendations";

// v0.53 T5 — fundamentals-adjacent "Price context" section (ADR 0067, plan
// docs/plans/v0.53-market-data-foundation.md). Renders against the
// `PriceContext` DTO; the orchestrator wires the real IPC command + mock
// runtime. Decision-support only: no cheap/expensive or buy/sell wording.

export type { PriceContext, PriceContextRatios, PriceContextHistoryPoint };

export type PriceContextSectionProps = {
  data: PriceContext;
  className?: string;
  // Cross-screen action, optional like CompanyFeedSection's — the workspace
  // wires it to Sources navigation, a standalone render can omit it.
  onViewSourceHealth?: () => void;
  // The newest attributed analyst target (v0.58 A3, ADR 0073) for the "vs target"
  // readout beside the close. Optional: absent when the company has no target.
  // ADR 0073 hard rule — the readout NEVER renders a bare number; firm + date sit
  // inseparably beneath it, labelled an external opinion.
  analystTarget?: AnalystRecommendationTarget | null;
  // Optional cross-panel focus: pins/focuses the analyst-recommendations panel
  // (storyboard frame 8). Omitted when no cheap navigation seam is available.
  onFocusRecommendations?: () => void;
};

const DASH = "—";

// A per-share quote (close, change, 52w high/low) keeps 2 decimal digits —
// the `unit: "per_share"` branch of the shared formatter, as opposed to the
// auto-scaled thousand/million/billion path used for aggregate monetary
// figures like market cap.
function formatPricePerShare(value: number, currency: string, locale: "en" | "pl"): string {
  return formatFinancialValue(
    { valueNumeric: String(value), currency, valueKind: "monetary", unit: "per_share" },
    locale,
  );
}

function formatMarketCap(value: number, currency: string, locale: "en" | "pl"): string {
  return formatFinancialValue({ valueNumeric: String(value), currency, valueKind: "monetary" }, locale);
}

function formatRatio(value: number | null, locale: "en" | "pl"): string {
  if (value === null || !Number.isFinite(value)) return DASH;
  return formatFinancialValue({ valueNumeric: String(value), valueKind: "ratio" }, locale);
}

function formatPercent(value: number | null, locale: "en" | "pl"): string {
  if (value === null || !Number.isFinite(value)) return DASH;
  return formatFinancialValue({ valueNumeric: String(value), valueKind: "percentage" }, locale);
}


export function PriceContextSection({
  data,
  className,
  onViewSourceHealth,
  analystTarget,
  onFocusRecommendations,
}: PriceContextSectionProps) {
  const { text, locale } = useLocale();

  const staleness = text("Prices as of") + " " + formatDetailTimestamp(data.fetchedAt, text("Unknown"));

  if (data.emptyReason) {
    const message =
      data.emptyReason === "unmapped_ticker"
        ? text("This company's ticker is not mapped to a market data source yet.")
        : text("No price data is available for this company yet.");

    return (
      <div
        role="group"
        aria-label={text("Price context")}
        className={["price-context-section", className].filter(Boolean).join(" ")}
      >
        <SectionHeader level="h4" title={text("Price context")} />
        <EmptyState
          actions={
            data.emptyReason === "unmapped_ticker" && onViewSourceHealth ? (
              <Button className="compact-button" onClick={onViewSourceHealth}>
                <ExternalLink size={15} />
                {text("Check source health")}
              </Button>
            ) : undefined
          }
        >
          {message}
        </EmptyState>
      </div>
    );
  }

  const changeTone = data.changePct > 0 ? "ok" : data.changePct < 0 ? "danger" : "neutral";
  const changeSign = data.changePct > 0 ? "+" : "";

  // Daily sessions as candlesticks (owner request 2026-07-14) with the
  // readable framing: min/mid/max price on the y axis (scaled to the candle
  // extremes), covered date range on the x axis.
  const historyPoints = data.history.map((point) => ({
    label: point.date,
    open: point.open,
    high: point.high,
    low: point.low,
    close: point.close,
  }));

  return (
    <div
      role="group"
      aria-label={text("Price context")}
      className={["price-context-section", className].filter(Boolean).join(" ")}
    >
      <SectionHeader
        level="h4"
        title={text("Price context")}
       
        meta={<span className="price-context-staleness">{staleness}</span>}
      />

      <div className="price-context-headline">
        <span className="price-context-close num-tabular">
          {formatPricePerShare(data.lastClose, data.currency, locale)}
        </span>
        <StatusChip tone={changeTone}>
          {changeSign}
          {formatPricePerShare(data.changeAbs, data.currency, locale)} ({changeSign}
          {formatPercent(data.changePct, locale)})
        </StatusChip>
        <span className="price-context-close-date">
          {formatListTimestamp(data.lastDate, locale)}
        </span>
      </div>

      <InfoGrid
        ariaLabel={text("52-week range")}
        className="price-context-range-grid"
        items={[
          {
            label: text("52w high"),
            value: `${formatPricePerShare(data.week52High, data.currency, locale)} (${formatPercent(data.week52HighDistPct, locale)})`,
          },
          {
            label: text("52w low"),
            value: `${formatPricePerShare(data.week52Low, data.currency, locale)} (${formatPercent(data.week52LowDistPct, locale)})`,
          },
          {
            label: text("Market cap"),
            value: data.marketCap === null ? DASH : formatMarketCap(data.marketCap, data.currency, locale),
          },
        ]}
      />

      <InfoGrid
        ariaLabel={text("Ratios")}
        className="price-context-ratio-grid"
        items={[
          { label: text("P/E"), value: formatRatio(data.ratios.pe, locale) },
          { label: text("P/BV"), value: formatRatio(data.ratios.pbv, locale) },
          { label: text("EV/EBITDA"), value: formatRatio(data.ratios.evEbitda, locale) },
          { label: text("Dividend yield"), value: formatPercent(data.ratios.divYield, locale) },
          { label: text("FCF yield"), value: formatPercent(data.ratios.fcfYield, locale) },
          {
            label: text("Price vs 52w range (percentile)"),
            value:
              data.ratios.ownHistPercentile === null
                ? DASH
                : formatPercent(data.ratios.ownHistPercentile * 100, locale),
          },
        ]}
      />

      {analystTarget
        ? (() => {
            const targetNum = Number.parseFloat(analystTarget.targetPrice.replace(",", "."));
            const hasTarget = Number.isFinite(targetNum);
            const upsidePct =
              hasTarget && data.lastClose > 0 ? ((targetNum - data.lastClose) / data.lastClose) * 100 : null;
            const upsideTone =
              upsidePct === null ? "" : upsidePct >= 0 ? "price-context-target-up" : "price-context-target-down";
            return (
              <div className="price-context-target">
                <span className="price-context-target-value">
                  {text("Analyst target price")}:{" "}
                  <strong className="num-tabular">
                    {formatPricePerShare(targetNum, analystTarget.targetCurrency ?? data.currency, locale)}
                  </strong>
                  {upsidePct !== null ? (
                    <span className={`price-context-target-delta ${upsideTone}`}>
                      {" "}
                      ({upsidePct >= 0 ? "+" : ""}
                      {formatPercent(upsidePct, locale)})
                    </span>
                  ) : null}
                </span>
                <span className="price-context-target-attr">
                  {text("per")} {analystTarget.firm}, {formatLocalIsoDate(analystTarget.publishedAt)} —{" "}
                  {text("external opinion (BiznesRadar)")}
                </span>
                {onFocusRecommendations ? (
                  <span className="price-context-target-action">
                    <Button className="compact-button" onClick={onFocusRecommendations}>
                      {text("View recommendations")}
                    </Button>
                  </span>
                ) : null}
              </div>
            );
          })()
        : null}

      {historyPoints.length > 1 ? (
        <div className="price-context-chart">
          <span className="eyebrow">{text("Price history")}</span>
          <CandlestickChart
            ariaLabel={text("Price history")}
            points={historyPoints}
            height={120}
            formatValue={(value) => formatPricePerShare(value, data.currency, locale)}
            className="price-context-history-chart"
          />
        </div>
      ) : null}
    </div>
  );
}
