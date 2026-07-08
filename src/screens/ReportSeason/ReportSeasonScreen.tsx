import { useState } from "react";
import { CalendarClock, CheckCircle2, ClipboardCheck } from "lucide-react";
import { useLocale } from "../../shared/locale";
import { useReportSeasonViewModel } from "../../app/state/screenViewModels";
import type { CompanyWorkspaceTab } from "../Companies/companyTypes";
import type { ReportSeasonEntry } from "../../api/reportSeason";
import {
  ActionRow,
  Button,
  EmptyState,
  ErrorText,
  ExpandableRow,
  FieldRow,
  Hint,
  InfoGrid,
  ListRow,
  PanelHeader,
  SectionHeader,
  SelectField,
  Skeleton,
  StatusChip,
} from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import type { Watchlist } from "../../api/types";
import { entryKey, useReportSeason } from "./useReportSeason";

export type ReportSeasonScreenProps = {
  watchlists: Watchlist[];
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
};

const ALL_SCOPE = "all";

export function ReportSeasonScreen() {
  const { watchlists, openCompanyWorkspace } = useReportSeasonViewModel();
  const { text } = useLocale();
  const [scope, setScope] = useState<string>(ALL_SCOPE);
  const {
    season,
    loading,
    error,
    expandedKey,
    cards,
    cardLoadingKey,
    actionInFlightKey,
    toggleExpanded,
    prepare,
    process,
  } = useReportSeason(scope === ALL_SCOPE ? null : scope);

  function preparationChip(status: ReportSeasonEntry["preparationStatus"]) {
    if (status === "processed") {
      return <StatusChip tone="ok">{text("Processed")}</StatusChip>;
    }
    if (status === "prepared") {
      return <StatusChip tone="accent">{text("Prepared")}</StatusChip>;
    }
    return <StatusChip tone="neutral">{text("Upcoming")}</StatusChip>;
  }

  function renderEntry(entry: ReportSeasonEntry, expandable: boolean) {
    const key = entryKey(entry);
    const card = cards[key];
    const isExpanded = expandedKey === key;
    const actionBusy = actionInFlightKey === key;

    // On past rows a default "upcoming" preparation is noise; only surface a
    // chip there when the user actually prepared or processed the report.
    const showChip = expandable || entry.preparationStatus !== "upcoming";
    const row = (
      <ListRow
        title={
          <span className="report-season-row-title">
            <TickerLabel value={entry.qualifiedTicker} />
            <span className="report-season-company">{entry.displayName}</span>
          </span>
        }
        titleAttr={`${entry.qualifiedTicker} · ${entry.displayName}`}
        meta={
          <span className="report-season-row-meta">
            {entry.eventDate}
            {entry.eventTime ? ` · ${entry.eventTime}` : ""}
          </span>
        }
        trailing={showChip ? preparationChip(entry.preparationStatus) : undefined}
      />
    );

    if (!expandable) {
      return (
        <div key={key} className="report-season-row-static">
          {row}
        </div>
      );
    }

    const detail = (
      <div className="report-season-card">
        {cardLoadingKey === key && !card ? (
          <Skeleton variant="list-row" count={2} label={text("Loading…")} />
        ) : card ? (
          <>
            {/* U7-D density (ADR 0076 D6): the pre-report card splits into a prep
                checklist (actions + open questions + unresolved claims — the
                "what to do before the report" the M tier surfaces) and an extended
                context block (last-period KPIs + recent evidence — the fuller
                pre-report card the L tier adds). Container queries fold the
                extended block below L and the whole card at S / short (rows only).
                Assumption recorded: the current card is click-to-expand rather
                than an auto-revealed side column, so M/L gate the card's own
                internal density instead of a separate selection pane. */}
            <div className="report-season-card-prep">
              <ActionRow>
                <Button
                  variant="primary"
                  disabled={actionBusy || entry.preparationStatus === "processed"}
                  onClick={() => prepare(entry)}
                >
                  <ClipboardCheck size={15} />
                  {text("Mark prepared")}
                </Button>
                <Button
                  disabled={actionBusy}
                  onClick={() => process(entry)}
                >
                  <CheckCircle2 size={15} />
                  {text("Mark processed")}
                </Button>
                <Button variant="minimal" onClick={() => openCompanyWorkspace(entry.companyId, "Feed")}>
                  {text("Open workspace")}
                </Button>
                <Button variant="minimal" onClick={() => openCompanyWorkspace(entry.companyId, "Claims")}>
                  {text("Open claims")}
                </Button>
              </ActionRow>

              <SectionHeader level="h4" title={text("Open research questions")} />
              {card.openQuestions.length > 0 ? (
                <ul className="report-season-list">
                  {card.openQuestions.map((question) => (
                    <li key={question.id}>{question.title}</li>
                  ))}
                </ul>
              ) : (
                <Hint>{text("None yet")}</Hint>
              )}

              <SectionHeader level="h4" title={text("Unresolved claims")} />
              <div className="report-season-claim-counts">
                <StatusChip tone="danger">{`${text("Due")}: ${card.unresolvedClaims.due.length}`}</StatusChip>
                <StatusChip tone="warn">{`${text("Overdue")}: ${card.unresolvedClaims.overdue.length}`}</StatusChip>
                <StatusChip tone="neutral">{`${text("Upcoming")}: ${card.unresolvedClaims.upcoming.length}`}</StatusChip>
              </div>
            </div>

            <div className="report-season-card-extended">
              <SectionHeader level="h4" title={text("Last-period KPIs")} />
              {card.lastPeriodKpis.length > 0 ? (
                <InfoGrid
                  ariaLabel={text("Last-period KPIs")}
                  items={card.lastPeriodKpis.map((kpi) => ({
                    label: kpi.label,
                    value: kpi.unit ? `${kpi.valueNumeric} ${kpi.unit}` : kpi.valueNumeric,
                  }))}
                />
              ) : (
                <Hint>{text("No KPIs from the last reported period.")}</Hint>
              )}

              <SectionHeader level="h4" title={text("Recent evidence")} />
              {card.recentEvidence.length > 0 ? (
                card.recentEvidence.map((item) => (
                  <ListRow
                    key={item.id}
                    title={item.title}
                    meta={item.occurredAt}
                    trailing={<StatusChip tone="neutral">{item.evidenceType}</StatusChip>}
                  />
                ))
              ) : (
                <Hint>{text("None yet")}</Hint>
              )}
            </div>
          </>
        ) : (
          <Hint>{text("None yet")}</Hint>
        )}
      </div>
    );

    return (
      <ExpandableRow
        key={key}
        className="report-season-row"
        label={`${entry.displayName} ${entry.eventDate}`}
        isExpanded={isExpanded}
        onToggle={() => toggleExpanded(entry)}
        detail={detail}
      >
        {row}
      </ExpandableRow>
    );
  }

  const upcoming = season?.upcoming ?? [];
  const past = season?.past ?? [];

  return (
    <section className="feed-panel" aria-labelledby="report-season-title">
      <PanelHeader
        paneLead
        title={text("Report Season")}
        description={text("Upcoming report dates across your watchlists, each with a pre-report card.")}
        titleId="report-season-title"
        actions={
          <FieldRow>
            <SelectField
              label={text("Watchlist scope")}
              value={scope}
              onChange={(event) => setScope(event.target.value)}
            >
              <option value={ALL_SCOPE}>{text("All watchlists")}</option>
              {watchlists.map((watchlist) => (
                <option key={watchlist.id} value={watchlist.id}>
                  {watchlist.name}
                </option>
              ))}
            </SelectField>
          </FieldRow>
        }
      />

      <div className="report-season-layout" aria-label={text("Report Season")}>
        {error ? <ErrorText>{error}</ErrorText> : null}
        {season?.calendarFreshness.stale ? (
          <StatusChip tone="warn" className="report-season-stale">
            <CalendarClock size={14} />
            {text("Calendar may be out of date")}
          </StatusChip>
        ) : null}

        <section className="report-season-section" aria-label={text("Upcoming reports")}>
          <SectionHeader
            level="h3"
            variant="accent"
            title={text("Upcoming reports")}
            meta={String(upcoming.length)}
          />
          {loading ? (
            <Skeleton variant="list-row" count={4} label={text("Loading…")} />
          ) : upcoming.length > 0 ? (
            <div className="report-season-rows">
              {upcoming.map((entry) => renderEntry(entry, true))}
            </div>
          ) : (
            <EmptyState>
              {text("No upcoming reports in scope. Widen the watchlist scope to see more.")}
            </EmptyState>
          )}
        </section>

        {past.length > 0 ? (
          <section className="report-season-section" aria-label={text("Past reports")}>
            <SectionHeader level="h3" variant="accent" title={text("Past reports")} meta={String(past.length)} />
            <div className="report-season-rows">
              {past.map((entry) => renderEntry(entry, false))}
            </div>
          </section>
        ) : null}
      </div>
    </section>
  );
}
