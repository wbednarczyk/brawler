import type { Company } from "../../api/types";
import type {
  AnalystRecommendationRow,
  AnalystRecommendationsView,
} from "../../api/analystRecommendations";
import { TickerLabel } from "./TickerLabel";
import { useLocale } from "../locale";
import { formatFinancialValue, formatFixedPercent } from "../format/financialValue";
import { formatDetailTimestamp, formatLocalIsoDate } from "../format/datetime";
import { Button, EmptyState, ErrorText, Hint, SectionHeader, StatusChip } from "../../ui";

// The company-scoped analyst-recommendations cockpit panel (v0.58 A3, ADR 0073).
// A quiet read surface (experience contract § 6: NO primary action) rendering
// attributed third-party opinions — never advice. Every row carries firm + date
// inseparably from the numbers; ratings are quoted verbatim in the source
// vocabulary. Follows the redFlags/shortPositions precedent: props-driven, with
// cockpit-owned state (`useCockpitAnalystRecommendations`). Composes src/ui
// primitives only.
export type AnalystRecommendationsSectionProps = {
  company: Company;
  view: AnalystRecommendationsView | null;
  error: string | null;
  loading: boolean;
  onRetry: () => void;
  // Current close (from Price context) for the per-row "vs price" upside — the
  // panel is standalone, so it may be absent; the row then omits the delta.
  lastClose?: number | null;
  currency?: string | null;
};

const DASH = "—";
const SOURCE_LABEL = "BiznesRadar.pl";

type ChipTone = "neutral" | "accent" | "ok" | "warn" | "danger";

/** Coarse rating tone in the Polish sell-side vocabulary: buy-side → ok (green),
 * hold → neutral, sell-side → danger (red). Unknown ratings stay neutral. */
function ratingTone(rating: string): ChipTone {
  switch (rating.trim().toLowerCase()) {
    case "kupuj":
    case "akumuluj":
      return "ok";
    case "redukuj":
    case "sprzedaj":
      return "danger";
    default:
      return "neutral";
  }
}

function parseDecimal(value: string | null): number | null {
  if (value === null) return null;
  const parsed = Number.parseFloat(value.trim().replace(",", "."));
  return Number.isFinite(parsed) ? parsed : null;
}

function formatTargetPrice(
  targetPrice: string,
  currency: string | null,
  locale: "en" | "pl",
): string {
  return formatFinancialValue(
    {
      valueNumeric: targetPrice.replace(",", "."),
      currency: currency ?? "PLN",
      valueKind: "monetary",
      unit: "per_share",
    },
    locale,
  );
}

export function AnalystRecommendationsSection({
  company,
  view,
  error,
  loading,
  onRetry,
  lastClose,
  currency,
}: AnalystRecommendationsSectionProps) {
  const { text, locale } = useLocale();

  const entries = view?.entries ?? [];
  const latestTarget = view?.latestTarget ?? null;
  const lastRefreshedAt = view?.lastRefreshedAt ?? null;
  const sourceUrl = entries[0]?.sourceUrl ?? null;

  // Direction sub-label + tone. Upgrade/downgrade carry the same-firm prior rating
  // (verbatim); initiate/reiterate are neutral context.
  function directionLabel(row: AnalystRecommendationRow): string {
    switch (row.direction) {
      case "upgrade":
        return row.ratingPrev ? `▲ ${text("from")} ${row.ratingPrev}` : `▲ ${text("upgrade")}`;
      case "downgrade":
        return row.ratingPrev ? `▼ ${text("from")} ${row.ratingPrev}` : `▼ ${text("downgrade")}`;
      case "reiterate":
        return `= ${text("maintained")}`;
      default:
        return text("new");
    }
  }

  function directionClass(direction: string): string {
    if (direction === "upgrade") return "analyst-recs-dir analyst-recs-dir-up";
    if (direction === "downgrade") return "analyst-recs-dir analyst-recs-dir-down";
    return "analyst-recs-dir";
  }

  function renderRow(row: AnalystRecommendationRow, index: number) {
    const rowCurrency = row.targetCurrency ?? currency ?? null;
    const target = parseDecimal(row.targetPrice);
    const close = typeof lastClose === "number" && Number.isFinite(lastClose) ? lastClose : null;
    const deltaPct = target !== null && close !== null && close > 0 ? ((target - close) / close) * 100 : null;
    const deltaClass =
      deltaPct === null
        ? ""
        : deltaPct >= 0
          ? "analyst-recs-delta analyst-recs-delta-up"
          : "analyst-recs-delta analyst-recs-delta-down";

    return (
      <li key={`${row.firm}-${row.publishedAt}-${index}`} className="analyst-recs-row">
        <span className="analyst-recs-rating-slot">
          <StatusChip tone={ratingTone(row.rating)}>{row.rating}</StatusChip>
          <span className={directionClass(row.direction)}>{directionLabel(row)}</span>
        </span>
        <span className="analyst-recs-main">
          <span className="analyst-recs-target num-tabular">
            {row.targetPrice
              ? `${text("target")} ${formatTargetPrice(row.targetPrice, rowCurrency, locale)}`
              : DASH}
            {deltaPct !== null ? (
              <span className={deltaClass}>
                {deltaPct >= 0 ? "+" : "−"}
                {formatFixedPercent(Math.abs(deltaPct), locale)} {text("vs price")}
              </span>
            ) : null}
          </span>
          <span className="analyst-recs-firm">
            {row.firm}
            {row.analyst ? <span className="analyst-recs-analyst"> · {row.analyst}</span> : null}
          </span>
        </span>
        <span className="analyst-recs-side">
          <time className="analyst-recs-date num-tabular" dateTime={row.publishedAt}>
            {formatLocalIsoDate(row.publishedAt)}
          </time>
          {row.reportUrl ? (
            <a
              className="analyst-recs-pdf"
              href={row.reportUrl}
              target="_blank"
              rel="noreferrer noopener"
            >
              {text("Broker PDF")} ↗
            </a>
          ) : (
            <span className="analyst-recs-pdf-empty">{DASH}</span>
          )}
        </span>
      </li>
    );
  }

  return (
    <div className="company-tab-panel analyst-recs-panel" aria-label={text("Analyst recommendations")}>
      <SectionHeader level="h3" paneLead title={text("Analyst recommendations")} />
      {/* Attribution — its own line because the paneLead header drops its subtitle
          (ADR 0076 D6). ADR 0073: opinions, not advice — stated inline, always. */}
      <p className="analyst-recs-attr">
        {text("Brokerage opinions — not investment advice")} ·{" "}
        <TickerLabel value={company.qualifiedTicker} /> · {text("Source")}:{" "}
        {sourceUrl ? (
          <a href={sourceUrl} target="_blank" rel="noreferrer noopener">
            {SOURCE_LABEL}
          </a>
        ) : (
          SOURCE_LABEL
        )}
      </p>

      {error ? (
        <ErrorText>
          {text("Could not load analyst recommendations")}: {error}{" "}
          <Button className="compact-button" onClick={onRetry}>
            {text("Try again")}
          </Button>
        </ErrorText>
      ) : null}

      {loading ? (
        <div className="analyst-recs-skeleton" aria-hidden>
          <span className="analyst-recs-skel" />
          <span className="analyst-recs-skel" />
          <span className="analyst-recs-skel" />
        </div>
      ) : entries.length === 0 ? (
        <EmptyState className="analyst-recs-empty" wrapText={false}>
          <span className="analyst-recs-empty-title">
            {text("No analyst recommendations for this company yet")}
          </span>
          <span>{text("Entries appear automatically after the next source refresh.")}</span>
        </EmptyState>
      ) : (
        <>
          <section className="analyst-recs-summary" aria-label={text("Recommendations summary")}>
            {latestTarget ? (
              <div className="analyst-recs-stat">
                <span className="analyst-recs-stat-value num-tabular">
                  {formatTargetPrice(latestTarget.targetPrice, latestTarget.targetCurrency, locale)}
                </span>
                <span className="analyst-recs-stat-label">
                  {text("Latest target price")}
                  <span className="analyst-recs-stat-attr">
                    {latestTarget.firm} · {formatLocalIsoDate(latestTarget.publishedAt)}
                  </span>
                </span>
              </div>
            ) : null}
            <div className="analyst-recs-stat">
              <span className="analyst-recs-stat-value num-tabular">{entries.length}</span>
              <span className="analyst-recs-stat-label">{text("Entries in local history")}</span>
            </div>
            <div className="analyst-recs-stat">
              <span className="analyst-recs-stat-value num-tabular">
                {formatLocalIsoDate(entries[0].publishedAt)}
              </span>
              <span className="analyst-recs-stat-label">{text("Last change")}</span>
            </div>
          </section>

          <ul className="analyst-recs-list" aria-label={text("Analyst recommendations")}>
            {entries.map(renderRow)}
          </ul>
        </>
      )}

      <Hint className="analyst-recs-foot">
        {text("Ratings quoted verbatim from the source.")}{" "}
        {text("History is built locally from ingestion start — the source shows only the latest entries.")}
        {lastRefreshedAt ? (
          <>
            {" · "}
            {text("Last refresh")}: {formatDetailTimestamp(lastRefreshedAt)}
          </>
        ) : null}
      </Hint>
    </div>
  );
}
