import { useLocale, type LocaleCode } from "../locale";
import { formatFinancialValue } from "../format/financialValue";
import {
  EmptyState,
  ErrorText,
  Hint,
  SectionHeader,
  Skeleton,
  StatusChip,
} from "../../ui";
import type {
  InsiderOverview,
  InsiderTransactionEntry,
  ManagementHoldingEntry,
  WindowAggregate,
} from "../../api/insider";

// "Insiderzy" block — extends the Ownership area of the Basic Info panel with the
// parsed MAR art. 19 transaction timeline, the latest management holdings, and the
// rolling net-direction aggregates (90d / 12m) (ADR 0083 Decision 7, v0.57 T6).
// Pure-render against the InsiderOverview DTO; the host (CompanyBasicInfoPanel)
// owns the IPC fetch. Decision support only — counts, volumes, and who; never
// "bullish/bearish". An aggregate NEVER renders below the 2-transaction minimum
// (the DTO's tagged `belowMinimum` state structurally prevents it).

export type InsiderBlockProps = {
  /** `null` while loading. */
  data: InsiderOverview | null;
  error?: string | null;
  className?: string;
};

const DASH = "—";

function formatCount(value: string | null | undefined, locale: LocaleCode): string {
  if (value === null || value === undefined) return DASH;
  return formatFinancialValue({ valueNumeric: value, valueKind: "count" }, locale);
}

/** Signed net with a real minus sign; `+N` / `0` / `−N`. */
function formatNet(net: number): string {
  if (net > 0) return `+${net}`;
  if (net < 0) return `−${Math.abs(net)}`;
  return "0";
}

export function InsiderBlock({ data, error, className }: InsiderBlockProps) {
  const { text, locale } = useLocale();

  const roleLabels: Record<string, string> = {
    management: text("Management board"),
    supervisory: text("Supervisory board"),
    closely_associated: text("Closely associated"),
  };
  const roleLabel = (role: string | null): string | null =>
    role ? (roleLabels[role] ?? role) : null;

  const directionLabels: Record<string, string> = {
    buy: text("buy"),
    sell: text("sell"),
    other: text("other"),
  };
  const directionLabel = (direction: string | null): string =>
    direction ? (directionLabels[direction] ?? direction) : text("undetermined");
  const directionKind = (direction: string | null): string => {
    if (direction === "buy") return "buy";
    if (direction === "sell") return "sell";
    return "other";
  };

  const heading = (
    <SectionHeader level="h4" title={text("Insiders")} titleId="insider-title" />
  );

  const wrap = (children: React.ReactNode) => (
    <section
      aria-labelledby="insider-title"
      className={["insider-section", className].filter(Boolean).join(" ")}
    >
      {heading}
      {children}
    </section>
  );

  if (error) {
    return wrap(
      <ErrorText>
        {text("Failed to load insiders")}: {error}
      </ErrorText>,
    );
  }
  if (!data) {
    return wrap(<Skeleton variant="list-row" count={3} label={text("Loading…")} />);
  }

  const hasAnything = data.transactions.length > 0 || data.holdings.length > 0;
  if (!hasAnything) {
    return wrap(
      <EmptyState>
        {text("No insider filings parsed from the saved reports yet.")}
      </EmptyState>,
    );
  }

  // ----- Aggregate window card (never renders an aggregate below the min) -----
  const renderWindow = (label: string, agg: WindowAggregate) => {
    if (agg.state === "belowMinimum") {
      return (
        <div className="insider-window" role="group" aria-label={label}>
          <span className="eyebrow">{label}</span>
          <span className="insider-window-below num-tabular">
            {agg.count === 0
              ? text("No transactions in this window")
              : `${agg.count} ${text("tx — too few for an aggregate (min. 2)")}`}
          </span>
        </div>
      );
    }
    const coverage = `${text("volume known for")} ${agg.volumeKnown}/${agg.volumeTotal}`;
    return (
      <div className="insider-window" role="group" aria-label={label}>
        <span className="eyebrow">{label}</span>
        <span className={`insider-net insider-net-${agg.net > 0 ? "buy" : agg.net < 0 ? "sell" : "flat"}`}>
          <b className="num-tabular">{formatNet(agg.net)}</b>{" "}
          <span className="insider-net-split num-tabular">
            {agg.buys} {text("buys")} · {agg.sells} {text("sells")}
            {agg.undetermined > 0 ? ` · ${agg.undetermined} ${text("undetermined")}` : ""}
          </span>
        </span>
        {agg.buyVolume || agg.sellVolume ? (
          <span className="insider-window-vol num-tabular">
            {text("Volume")}: {text("buys")} {formatCount(agg.buyVolume, locale)} ·{" "}
            {text("sells")} {formatCount(agg.sellVolume, locale)}
          </span>
        ) : null}
        <span className="insider-window-coverage">{coverage}</span>
      </div>
    );
  };

  const renderHolding = (holding: ManagementHoldingEntry, index: number) => (
    <li key={`${holding.person}-${index}`} className="insider-holding-row">
      <div className="insider-holding-main">
        <span className="insider-person">{holding.person}</span>
        <span className="insider-role-slot">
          {roleLabel(holding.role) ? (
            <StatusChip tone="neutral">{roleLabel(holding.role)}</StatusChip>
          ) : null}
        </span>
      </div>
      <div className="insider-holding-meta num-tabular">
        {holding.shares !== null ? (
          <span>
            {formatCount(holding.shares, locale)} {text("shares")}
          </span>
        ) : (
          <span className="insider-muted">{text("count not stated")}</span>
        )}
        {holding.indirectVia ? (
          <span className="insider-muted">
            {text("via")} {holding.indirectVia}
          </span>
        ) : null}
      </div>
    </li>
  );

  const renderTransaction = (txn: InsiderTransactionEntry) => (
    <li key={txn.id} className="insider-tx-row">
      <div className="insider-tx-main">
        <span className={`insider-dir insider-dir-${directionKind(txn.direction)}`}>
          <span aria-hidden="true" className="insider-dir-mark">
            {txn.direction === "buy" ? "+" : txn.direction === "sell" ? "−" : "•"}
          </span>
          {directionLabel(txn.direction)}
        </span>
        <span className="insider-person">
          {txn.person}
          {txn.relatedPdmr ? (
            <span className="insider-muted">
              {" "}
              ({text("for")} {txn.relatedPdmr})
            </span>
          ) : null}
        </span>
        <span className="insider-role-slot">
          {roleLabel(txn.role) ? (
            <StatusChip tone="neutral">{roleLabel(txn.role)}</StatusChip>
          ) : null}
        </span>
      </div>
      <div className="insider-tx-meta num-tabular">
        <span className="insider-tx-date">
          {txn.effectiveDate ?? DASH}
          {txn.dateSource === "filing" ? (
            <span className="insider-muted"> ({text("filing date")})</span>
          ) : null}
        </span>
        {txn.volume ? (
          <span>
            {formatCount(txn.volume, locale)} {text("shares")}
            {txn.price ? ` · ${txn.price} ${txn.currency ?? ""}`.trimEnd() : ""}
          </span>
        ) : (
          <span className="insider-muted">{text("figures in the attachment")}</span>
        )}
        {txn.sourceUrl ? (
          <a
            className="insider-tx-link"
            href={txn.sourceUrl}
            target="_blank"
            rel="noreferrer"
          >
            {text("filing")}
          </a>
        ) : null}
      </div>
    </li>
  );

  return wrap(
    <>
      <div className="insider-windows">
        {renderWindow(text("Last 90 days"), data.window90d)}
        {renderWindow(text("Last 12 months"), data.window12m)}
      </div>

      {data.holdings.length > 0 ? (
        <div className="insider-group">
          <span className="eyebrow">{text("Management and supervisory board")}</span>
          <ul className="insider-holdings">{data.holdings.map(renderHolding)}</ul>
        </div>
      ) : null}

      {data.transactions.length > 0 ? (
        <div className="insider-group">
          <span className="eyebrow">{text("Transactions")}</span>
          <ul className="insider-timeline">{data.transactions.map(renderTransaction)}</ul>
        </div>
      ) : null}

      <Hint>
        {text(
          "Counts and who — decision support only. Volume, price and date are often in the notification attachment (not yet fetched), so aggregates state their coverage.",
        )}
      </Hint>
    </>,
  );
}
