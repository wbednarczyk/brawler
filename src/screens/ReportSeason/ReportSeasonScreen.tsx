import { useState } from "react";
import { CalendarClock, CheckCircle2, ClipboardCheck } from "lucide-react";
import { useLocale } from "../../shared/locale";
import { useReportSeasonViewModel } from "../../app/state/screenViewModels";
import type { CompanyWorkspaceTab } from "../Companies/companyTypes";
import type { ReportSeasonEntry } from "../../api/reportSeason";
import {
  ActionButton,
  ActionRow,
  EmptyState,
  ErrorText,
  ExpandableRow,
  Figure,
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
import { ExpectationsSection } from "./ExpectationsSection";

export type ReportSeasonScreenProps = {
  watchlists: Watchlist[];
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
};

const ALL_SCOPE = "all";

export function ReportSeasonScreen() {
  const { watchlists, openCompanyWorkspace } = useReportSeasonViewModel();
  const { t, text } = useLocale();
  const [scope, setScope] = useState<string>(ALL_SCOPE);
  const {
    season,
    loading,
    error,
    cardErrors,
    expandedKey,
    cards,
    cardLoadingKey,
    actionInFlightKey,
    expectations,
    expectationBusyKey,
    toggleExpanded,
    reloadCard,
    prepare,
    process,
    writeExpectation,
    resolveExpectation,
  } = useReportSeason(scope === ALL_SCOPE ? null : scope);
  // The card-level primary choreography (F4b S4, contract § Report Season):
  // one filled action per expanded card, coordinated across the prep
  // checklist AND the expectations composer/review — `composerOpenKey`
  // tracks which card's expectation composer is open (only one at a time,
  // mirroring `expandedKey`).
  const [composerOpenKey, setComposerOpenKey] = useState<string | null>(null);

  function preparationChip(status: ReportSeasonEntry["preparationStatus"]) {
    // Keyed t() entries (fix wave, integrator review): the free-text
    // `text()` map is a single flat EN→PL dictionary, so a bare "Upcoming"/
    // "Prepared" here would collide with the claim-counts' plural/neuter PL
    // form on this same screen — keyed lookups don't share that namespace,
    // so both languages read as the plain masculine adjective (contract §
    // Report Season: chips Nadchodzący/Przygotowany/Przejrzany).
    if (status === "processed") {
      return <StatusChip tone="ok">{t("reportSeason.status.reviewed")}</StatusChip>;
    }
    if (status === "prepared") {
      return <StatusChip tone="accent">{t("reportSeason.status.prepared")}</StatusChip>;
    }
    return <StatusChip tone="neutral">{t("reportSeason.status.upcoming")}</StatusChip>;
  }

  function renderEntry(entry: ReportSeasonEntry, expandable: boolean) {
    const key = entryKey(entry);
    const card = cards[key];
    const isExpanded = expandedKey === key;
    const actionBusy = actionInFlightKey === key;
    const cardError = cardErrors[key];
    const expectationState = expectations[key];
    const hasExpectation = Boolean(expectationState?.expectation);
    const review = expectationState?.review ?? null;
    const frozen = review?.factsAvailable ?? false;
    const resolved = review ? review.resolutionNoteMd !== null : false;
    const composerOpen = composerOpenKey === key;
    // Derived enum (decision 5 precedent, applied per-card): composer open →
    // `save`; frozen + unresolved → `saveVerdict`; an expectation exists →
    // `markAsPrepared`; else → `addExpectations`.
    const cardPrimary = composerOpen
      ? "save"
      : frozen && !resolved
        ? "saveVerdict"
        : hasExpectation
          ? "markAsPrepared"
          : "addExpectations";

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
            <Figure kind="date" value={entry.eventDate} />
            {entry.eventTime ? ` · ${entry.eventTime}` : ""}
          </span>
        }
        trailing={showChip ? preparationChip(entry.preparationStatus) : undefined}
      />
    );

    if (!expandable) {
      return (
        <div key={key} className="report-season-row-static">
          {/* ListRow renders an <li>; a lone <li> needs a list parent (axe listitem). */}
          <ul className="ui-list-rows">{row}</ul>
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
                <ActionButton
                  verb="markAs"
                  variant={cardPrimary === "markAsPrepared" ? "primary" : "secondary"}
                  data-ux-primary-action={cardPrimary === "markAsPrepared" ? "true" : undefined}
                  disabled={actionBusy || entry.preparationStatus === "processed"}
                  onClick={() => prepare(entry)}
                >
                  <ClipboardCheck size={15} />
                  {text("Mark as prepared")}
                </ActionButton>
                <ActionButton verb="markAs" variant="secondary" disabled={actionBusy} onClick={() => process(entry)}>
                  <CheckCircle2 size={15} />
                  {text("Mark as reviewed")}
                </ActionButton>
                <ActionButton
                  kind="destination"
                  variant="minimal"
                  onClick={() => openCompanyWorkspace(entry.companyId, "Feed")}
                >
                  {text("Company")}
                </ActionButton>
                <ActionButton
                  kind="destination"
                  variant="minimal"
                  onClick={() => openCompanyWorkspace(entry.companyId, "Claims")}
                >
                  {text("Claims")}
                </ActionButton>
              </ActionRow>

              <SectionHeader level="h3" title={text("Open research questions")} />
              {card.openQuestions.length > 0 ? (
                <ul className="report-season-list">
                  {card.openQuestions.map((question) => (
                    <li key={question.id}>{question.title}</li>
                  ))}
                </ul>
              ) : (
                <EmptyState kind="quiet" reason={text("No open questions")} />
              )}

              <SectionHeader level="h3" title={text("Unresolved claims")} />
              <div className="report-season-claim-counts">
                <StatusChip tone="danger">
                  {text("Due")}: <Figure value={card.unresolvedClaims.due.length} />
                </StatusChip>
                <StatusChip tone="warn">
                  {text("Overdue")}: <Figure value={card.unresolvedClaims.overdue.length} />
                </StatusChip>
                <StatusChip tone="neutral">
                  {text("Upcoming")}: <Figure value={card.unresolvedClaims.upcoming.length} />
                </StatusChip>
              </div>

              <ExpectationsSection
                entry={entry}
                kpis={card.lastPeriodKpis}
                state={expectations[key]}
                busy={expectationBusyKey === key}
                addPrimary={cardPrimary === "addExpectations"}
                saveVerdictPrimary={cardPrimary === "saveVerdict"}
                composerOpen={composerOpen}
                onComposerOpenChange={(open) => setComposerOpenKey(open ? key : null)}
                onWrite={(draft) => writeExpectation(entry, draft)}
                onResolve={(note) => resolveExpectation(entry, note)}
              />
            </div>

            <div className="report-season-card-extended">
              <SectionHeader level="h3" title={text("Last-period KPIs")} />
              {card.lastPeriodKpis.length > 0 ? (
                <InfoGrid
                  ariaLabel={text("Last-period KPIs")}
                  items={card.lastPeriodKpis.map((kpi) => ({
                    label: kpi.label,
                    value: (
                      <>
                        <Figure kind="money" value={kpi.valueNumeric} />
                        {kpi.unit ? ` ${kpi.unit}` : ""}
                      </>
                    ),
                  }))}
                />
              ) : (
                <Hint>{text("No KPIs from the last reported period.")}</Hint>
              )}

              <SectionHeader level="h3" title={text("Recent evidence")} />
              {card.recentEvidence.length > 0 ? (
                <ul className="ui-list-rows">
                  {card.recentEvidence.map((item) => (
                    <ListRow
                      key={item.id}
                      title={item.title}
                      meta={<Figure kind="datetime" value={item.occurredAt} />}
                      trailing={<StatusChip tone="neutral">{item.evidenceType}</StatusChip>}
                    />
                  ))}
                </ul>
              ) : (
                <EmptyState kind="quiet" reason={text("No evidence from the last period")} />
              )}
            </div>
          </>
        ) : cardError ? (
          <>
            <ErrorText>{cardError}</ErrorText>
            <ActionRow>
              <ActionButton verb="refresh" variant="secondary" onClick={() => reloadCard(entry)}>
                {text("Refresh card")}
              </ActionButton>
            </ActionRow>
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
        label={`${text("Open report")}: ${entry.displayName} ${entry.eventDate}`}
        isExpanded={isExpanded}
        onToggle={() => toggleExpanded(entry)}
        detail={detail}
      >
        {/* ListRow renders an <li>; a lone <li> needs a list parent (axe listitem). */}
        <ul className="ui-list-rows">{row}</ul>
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
            level="h2"
            variant="accent"
            title={text("Upcoming reports")}
            meta={<Figure value={upcoming.length} />}
          />
          {loading ? (
            <Skeleton variant="list-row" count={4} label={text("Loading…")} />
          ) : upcoming.length > 0 ? (
            <div className="report-season-rows">
              {upcoming.map((entry) => renderEntry(entry, true))}
            </div>
          ) : (
            <EmptyState
              kind="invitation"
              title={text("No upcoming reports")}
              source={text("No upcoming reports in scope. Widen the watchlist scope to see more.")}
              action={
                // F4b S4 deviation (see report): the contract names this a
                // `destination` ("Otwórz listy"/"Open watchlists"), but no
                // cross-screen navigation callback reaches this screen today
                // and wiring one is out of this slice's AppStateRoot diff
                // (contract note: "keep your diff to the Sources dead-prop
                // removal and nothing else"). Implemented instead as a local
                // `control` that widens the scope filter in place — the
                // actual, reachable fix for "no reports in this scope".
                <ActionButton kind="control" variant="secondary" onClick={() => setScope(ALL_SCOPE)}>
                  {text("Show all watchlists")}
                </ActionButton>
              }
            />
          )}
        </section>

        <section className="report-season-section" aria-label={text("Past reports")}>
          <SectionHeader
            level="h2"
            variant="accent"
            title={text("Past reports")}
            meta={<Figure value={past.length} />}
          />
          {past.length > 0 ? (
            <div className="report-season-rows">
              {past.map((entry) => renderEntry(entry, false))}
            </div>
          ) : (
            <EmptyState kind="quiet" reason={text("No past reports yet")} />
          )}
        </section>
      </div>
    </section>
  );
}
