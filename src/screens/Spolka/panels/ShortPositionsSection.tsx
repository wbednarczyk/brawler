import type { Company } from "../../../api/types";
import type {
  ShortPositionEventRow,
  ShortPositionsView,
} from "../../../api/shortPositions";
import { TickerLabel } from "../../../shared/components/TickerLabel";
import { useLocale } from "../../../shared/locale";
import { formatFixedDecimal, formatFixedPercent } from "../../../shared/format/financialValue";
import { formatDetailTimestamp } from "../../../shared/format/datetime";
import { EmptyState, ErrorText, Hint, SectionHeader, StatusChip } from "../../../ui";

// The company-scoped KNF short-selling panel (v0.55 T4b, ADR 0069 decision 3):
// current net short positions from the KNF register (≥ 0.5% threshold), the
// change history that feeds the `short_position_change` signal, and a
// most-common EMPTY state. Read-only — the register is populated by the daily
// adapter. Palette-only cockpit panel (not in the curated default set).
export type ShortPositionsSectionProps = {
  company: Company;
  view: ShortPositionsView | null;
  error: string | null;
};

/** A KNF register change, phrased by kind: `entered | increased | decreased | exited`. */
function eventKindLabel(kind: string, text: (value: string) => string): string {
  switch (kind) {
    case "entered":
      return text("Entered");
    case "increased":
      return text("Increased");
    case "decreased":
      return text("Decreased");
    case "exited":
      return text("Exited");
    default:
      return kind;
  }
}

/** Rising short interest reads as supply pressure (warning tone); a decrease or
 * exit reads as relief (ok tone). */
function eventTone(kind: string): "danger" | "ok" | "neutral" {
  if (kind === "entered" || kind === "increased") return "danger";
  if (kind === "decreased" || kind === "exited") return "ok";
  return "neutral";
}

export function ShortPositionsSection({ company, view, error }: ShortPositionsSectionProps) {
  const { text, locale } = useLocale();

  const positions = view?.positions ?? [];
  const events = view?.events ?? [];
  const aggregatePct = view?.aggregatePct ?? 0;
  const delta = view?.delta30dPp ?? 0;
  const lastExit = view?.lastExit ?? null;
  const isEmpty = positions.length === 0;

  const deltaTone =
    delta > 1e-9 ? "short-positions-stat-danger" : delta < -1e-9 ? "short-positions-stat-ok" : "";
  const deltaArrow = delta > 1e-9 ? "▲ " : delta < -1e-9 ? "▼ " : "";
  const deltaSign = delta > 1e-9 ? "+" : delta < -1e-9 ? "−" : "";
  const deltaLabel = `${deltaArrow}${deltaSign}${formatFixedDecimal(Math.abs(delta), locale)} pp`;

  const changeText = (event: ShortPositionEventRow): string => {
    const to = event.toPct != null ? formatFixedPercent(event.toPct, locale) : "—";
    const from = event.fromPct != null ? formatFixedPercent(event.fromPct, locale) : "—";
    if (event.kind === "entered") return `${text("new")} ${to}`;
    if (event.kind === "exited") return `${from} → —`;
    return `${from} → ${to}`;
  };

  return (
    <div className="company-tab-panel short-positions-panel" aria-label={text("Short selling (KNF)")}>
      <SectionHeader level="h3" paneLead title={text("Short selling (KNF)")} />
      {/* Source attribution — kept as its own line because the paneLead header
          drops its subtitle (ADR 0076 D6 compact header). */}
      <p className="short-positions-attr">
        {text("Source")}: {text("KNF short-selling register")} ·{" "}
        <TickerLabel value={company.qualifiedTicker} />
        {view?.registerUpdatedAt
          ? ` · ${text("updated")} ${formatDetailTimestamp(view.registerUpdatedAt)}`
          : null}
      </p>

      {error ? (
        <ErrorText>
          {text("Could not load short positions")}: {error}
        </ErrorText>
      ) : null}

      <div role="group" className="short-positions-summary" aria-label={text("Short-selling summary")}>
        <div>
          <strong className={aggregatePct > 1e-9 ? "short-positions-stat-danger" : ""}>
            {formatFixedPercent(aggregatePct, locale)}
          </strong>
          <span>{text("Total net short position")}</span>
        </div>
        <div>
          <strong className="num-tabular">{positions.length}</strong>
          <span>{text("Holders with a position ≥ 0.5%")}</span>
        </div>
        <div>
          <strong className={deltaTone}>{deltaLabel}</strong>
          <span>{text("Change / 30 days")}</span>
        </div>
      </div>

      {isEmpty ? (
        <EmptyState className="short-positions-empty" wrapText={false}>
          <span className="short-positions-empty-title">
            {text("No registered short positions")}
          </span>
          <span>
            {text("No holder has reported a position ≥ 0.5% for")}{" "}
            <TickerLabel value={company.qualifiedTicker} />.
          </span>
          {lastExit ? (
            <span className="short-positions-empty-exit">
              {text("Last presence in the register")}: {lastExit.exitedOn} ({lastExit.holderName},{" "}
              {text("exit")})
            </span>
          ) : null}
        </EmptyState>
      ) : (
        <>
          <div className="short-positions-table-scroll" data-hscroll>
            <table className="short-positions-table">
              <thead>
                <tr>
                  <th scope="col">{text("Position holder")}</th>
                  <th scope="col" className="num">
                    {text("Net position")}
                  </th>
                  <th scope="col" className="num">
                    {text("Calculation date")}
                  </th>
                </tr>
              </thead>
              <tbody>
                {positions.map((position) => (
                  <tr key={position.holderName}>
                    <td>
                      <span className="short-positions-holder">{position.holderName}</span>
                      {position.recentlyChanged ? (
                        <StatusChip tone="danger" className="short-positions-change-chip">
                          {text("changed")}
                        </StatusChip>
                      ) : null}
                    </td>
                    <td className="num num-tabular">
                      {formatFixedPercent(position.netPositionPct, locale)}
                    </td>
                    <td className="num num-tabular">{position.positionDate}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {events.length > 0 ? (
            <>
              <SectionHeader level="h4" title={text("Change history")} />
              <ul className="short-positions-history" aria-label={text("Change history")}>
                {events.map((event, index) => (
                  <li key={`${event.holderName}-${event.positionDate}-${event.kind}-${index}`}>
                    <span className="short-positions-history-main">
                      <span className="short-positions-holder">{event.holderName}</span>
                      <span className="short-positions-history-change num-tabular">
                        {changeText(event)}
                      </span>
                    </span>
                    <StatusChip tone={eventTone(event.kind)} className="short-positions-history-kind">
                      {eventKindLabel(event.kind, text)}
                    </StatusChip>
                    <time className="short-positions-history-date num-tabular" dateTime={event.positionDate}>
                      {event.positionDate}
                    </time>
                  </li>
                ))}
              </ul>
            </>
          ) : null}
        </>
      )}

      <Hint className="short-positions-foot">
        {isEmpty
          ? text(
              "The company appearing in the register will raise a signal and (optionally) an alert.",
            )
          : text(
              "The register covers only positions ≥ 0.5% of capital. Every change raises a short-position signal in the company feed — you can attach an alert rule.",
            )}
      </Hint>
    </div>
  );
}
