import { useEffect, useMemo, useState } from "react";
import { Inbox, RefreshCw } from "lucide-react";

import type { CompanyWorkspaceTab } from "../Companies/companyTypes";
import type { Company, FeedItem, SourceAdapter } from "../../api/types";
import type { AttentionEvent } from "../../api/attention";
import type { MorningBriefingItem } from "../../api/generated/MorningBriefingItem";
import { useLocale } from "../../shared/locale";
import {
  Button,
  EmptyState,
  PanelHeader,
  SegmentedControl,
  SegmentedControlOption,
  Skeleton,
} from "../../ui";
import { useTodayPulse } from "./useTodayPulse";
import { useMorningBriefing } from "./useMorningBriefing";
import { useAttentionToasts } from "./useAttentionToasts";
import { MorningBriefingStrip } from "./MorningBriefingStrip";
import { SeverityLegend } from "./SeverityLegend";
import { ConfigBanner, type ConfigCondition } from "./ConfigBanner";
import {
  buildStream,
  collapseCauseAggregates,
  collapseRoutineAggregates,
  verifyClaimSeverity,
  type StreamCategory,
  type StreamItem,
} from "./streamModel";
import type { RowContext, StreamPayload } from "./rows/rowContext";
import { StreamRowView } from "./rows/StreamRowView";
import { GroupRow } from "./rows/GroupRow";
import { AggregateRow } from "./rows/AggregateRow";
import { onStreamRowKeyDown } from "./rows/streamRowKit";
import { ArchivedAttentionRow, openAttentionEvidence } from "./rows/AttentionRow";

export type TodayScreenProps = {
  companies: Company[];
  pinnedCompanyIds: string[];
  recentFeedItems: FeedItem[];
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
  openInbox: () => void;
  /** Navigate to the Report-season surface (the upcoming-reports "Show all" target). */
  openReportSeason: () => void;
  /** The loaded source adapters (already fetched app-wide), for the config banner's
   * source-health condition (ADR 0087 dec. 5). */
  sourceAdapters: SourceAdapter[];
  /** Navigate to the Sources/Diagnostics surface (the config banner's action target). */
  openSources: () => void;
};

// J1 caps (ADR 0076 U-Rb D2): the two unbounded attention queues cap at 8 rows,
// upcoming reports at 6; a capped queue ends with a "Show all" link into its full
// surface so nothing is silently hidden. Caps apply to raw items pre-grouping.
const VERIFY_CAP = 8;
const CHANGED_CAP = 8;
const UPCOMING_CAP = 6;

/** A counter-tile selection: a category filter, or the cross-category `urgent` filter. */
type StreamFilter = StreamCategory | "urgent";

// Today/Pulse — the attention home (ADR 0054), redesigned to journey J1 under
// ADR 0087: a grouped, severity-ranked "what changed" stream (dedup → group →
// rank, `streamModel`) with a config banner, a compact briefing strip, four
// counter tiles ("Pilne" first), and per-category error strips. Composes existing
// app-wide read models; severity arrives typed (placeholder adapter until D3a).
export function TodayScreen({
  companies,
  pinnedCompanyIds,
  recentFeedItems,
  openCompanyWorkspace,
  openInbox,
  openReportSeason,
  sourceAdapters,
  openSources,
}: TodayScreenProps) {
  const { t, text, locale } = useLocale();
  const {
    season,
    claims,
    claimsLoading,
    claimsError,
    autopilotRuns,
    autopilotLoading,
    dismissAutopilotRun,
    undoAutopilotRun,
    undoneAutopilotRuns,
    attentionEvents,
    attentionLoading,
    attentionRulesById,
    dismissAttentionEventRow,
    markAttentionEventSeenRow,
    retryClaims,
    archivedAttentionEvents,
    archiveLoading,
    loadArchive,
  } = useTodayPulse(pinnedCompanyIds, companies);
  const morningBriefing = useMorningBriefing();
  // The active counter filter, or null for the full stream.
  const [filter, setFilter] = useState<StreamFilter | null>(null);
  // Two views (owner 2026-07-23): the live "Aktywne" stream and a read-only
  // "Archiwum" of DISMISSED attention events (dismiss is an ack, never a delete).
  const [view, setView] = useState<"active" | "archive">("active");
  // Fetch the archive lazily the first time (and each time) the user opens it.
  useEffect(() => {
    if (view === "archive") loadArchive();
  }, [view, loadArchive]);

  const companyById = useMemo(
    () => new Map(companies.map((company) => [company.id, company])),
    [companies],
  );
  const companyByTicker = useMemo(
    () => new Map(companies.map((company) => [company.qualifiedTicker, company])),
    [companies],
  );

  // "What changed" = freshly-arrived report disclosures from the live feed,
  // newest first (the report-season calendar's past entries lag days behind).
  const whatChanged = useMemo(() => {
    const reportTime = (item: FeedItem) => item.publishedAt || item.time;
    return recentFeedItems
      .filter((item) => /report|raport/i.test(item.type))
      .slice()
      .sort((a, b) => reportTime(b).localeCompare(reportTime(a)));
  }, [recentFeedItems]);

  // Overdue claims lead the verify queue, then due — both actionable now.
  const verifyClaims = useMemo(
    () =>
      [...claims].sort((a, b) => (a.bucket === b.bucket ? 0 : a.bucket === "overdue" ? -1 : 1)),
    [claims],
  );
  const upcomingAll = season.season?.upcoming ?? [];

  // Fired alerts (ADR 0068 T4): grouped by company (contiguous by ticker),
  // newest-fired first within a company — the same order the toast effect walks.
  const attentionRows = useMemo(() => {
    const tickerOf = (companyId: string) => companyById.get(companyId)?.qualifiedTicker ?? companyId;
    return [...attentionEvents].sort((a, b) => {
      const tickerCompare = tickerOf(a.companyId).localeCompare(tickerOf(b.companyId));
      return tickerCompare !== 0 ? tickerCompare : b.firedAt.localeCompare(a.firedAt);
    });
  }, [attentionEvents, companyById]);

  const verifyRows = verifyClaims.slice(0, VERIFY_CAP);
  const changedRows = whatChanged.slice(0, CHANGED_CAP);
  const upcomingRows = upcomingAll.slice(0, UPCOMING_CAP);

  // Everything a category row/member needs to render and act, built once.
  const ctx: RowContext = {
    text,
    locale,
    companyById,
    companyByTicker,
    openCompanyWorkspace,
    openInbox,
    autopilot: { dismissAutopilotRun, undoAutopilotRun, undoneAutopilotRuns },
    attention: { attentionRulesById, dismissAttentionEventRow, markAttentionEventSeenRow },
  };

  // The typed stream inputs. Severity is EXPLICIT (ADR 0087 dec. 2), read straight
  // off the backend payloads: attention rows carry `event.severity`, autopilot rows
  // `run.severity` (both computed once in `storage::severity`, never re-inferred
  // here). Changed/upcoming carry NO backend severity — they are category-level
  // `routine`. Verify claims are the ONE frontend-side severity entry
  // (`verifyClaimSeverity`, product-spec §Attention Routing severity table row):
  // an OVERDUE claim escalates to `notable`, a due-not-yet-overdue claim stays
  // `routine`. Claims raise no toast (they are not attention events).
  const streamItems: StreamItem<StreamPayload>[] = [
    ...autopilotRuns.map<StreamItem<StreamPayload>>((run) => ({
      id: run.id,
      category: "autopilot",
      companyId: run.companyId,
      severity: run.severity,
      timestamp: run.createdAt,
      evidenceKey: run.id,
      payload: { kind: "autopilot", run },
    })),
    ...attentionRows.map<StreamItem<StreamPayload>>((event) => ({
      id: event.id,
      category: "attention",
      companyId: event.companyId,
      severity: event.severity,
      // The systemic-cause key for the urgent cross-company aggregate (ADR 0087
      // amendment 2026-07-23): the trigger, refined to the signal category for a
      // signal event so, e.g., insider and profit-warning walls never merge.
      subCategory:
        event.triggerType === "signal_category"
          ? (event.ruleId ? attentionRulesById.get(event.ruleId)?.signalCategory : undefined) ??
            "signal_category"
          : event.triggerType,
      timestamp: event.firedAt,
      evidenceKey: event.id,
      payload: { kind: "attention", event },
    })),
    ...verifyRows.map<StreamItem<StreamPayload>>((claim) => ({
      id: claim.claim.id,
      category: "verify",
      companyId: claim.companyId,
      severity: verifyClaimSeverity(claim.bucket),
      timestamp: claim.claim.madeAt ?? claim.claim.createdAt,
      evidenceKey: claim.claim.id,
      payload: { kind: "verify", claim },
    })),
    ...changedRows.map<StreamItem<StreamPayload>>((item) => ({
      id: item.id,
      category: "changed",
      companyId: companyByTicker.get(item.company)?.id ?? item.company,
      severity: "routine",
      timestamp: item.publishedAt || item.time,
      evidenceKey: item.id,
      payload: { kind: "changed", item },
    })),
    ...upcomingRows.map<StreamItem<StreamPayload>>((entry) => ({
      id: `${entry.companyId}::${entry.eventKey}`,
      category: "upcoming",
      companyId: entry.companyId,
      severity: "routine",
      timestamp: entry.eventDate,
      evidenceKey: entry.eventKey,
      payload: { kind: "upcoming", entry },
    })),
  ];

  const groups = buildStream(streamItems);
  // Second stage: collapse the walls. Routine same-category rows fold into a
  // "×K spółek" aggregate (ADR 0087 dec. 1); THEN urgent/notable same-cause rows
  // fold into their own aggregates (ADR 0087 amendment 2026-07-23, live-checkpoint
  // P1 + the notable-wall follow-up) — severity partitions first, so an aged
  // (notable) wall no longer merely changes color.
  const renderRows = collapseCauseAggregates(collapseRoutineAggregates(groups));
  const visibleRows = renderRows.filter((row) => {
    if (filter === null) return true;
    if (filter === "urgent") return row.severity === "urgent";
    return row.category === filter;
  });

  const isArchive = view === "archive";
  // The archive is a flat, read-only list of dismissed events, newest-first — not
  // grouped/aggregated (it is a history, not a triage queue).
  const archiveRows = useMemo(
    () => [...archivedAttentionEvents].sort((a, b) => b.firedAt.localeCompare(a.firedAt)),
    [archivedAttentionEvents],
  );

  // Briefing-item evidence click-through (ADR 0068 T5): an attention item resolves
  // through the same `openAttentionEvidence` the stream uses; every other type
  // mirrors its per-type fallback (signal→Feed, autopilot→Fundamentals, claim→
  // Claims, report date→Feed) — an out-of-registry evidence company → Inbox.
  function openBriefingItemEvidence(item: MorningBriefingItem) {
    if (item.itemType === "attention_event") {
      const event = attentionEvents.find((candidate) => candidate.id === item.evidenceRef);
      if (event) {
        openAttentionEvidence(event, ctx);
        return;
      }
      openInbox();
      return;
    }
    const company = companyById.get(item.companyId);
    if (!company) {
      openInbox();
      return;
    }
    switch (item.itemType) {
      case "signal":
        openCompanyWorkspace(company.id, "Feed");
        return;
      case "autopilot_run":
        openCompanyWorkspace(company.id, "Fundamentals");
        return;
      case "claim_due":
        openCompanyWorkspace(company.id, "Claims");
        return;
      case "report_date":
        openCompanyWorkspace(company.id, "Feed");
        return;
      default:
        openInbox();
    }
  }

  // Persistent-toast wiring (unchanged behavior, ADR 0068 T4; severity-driven with
  // D3a). Reuses the pulse attention-events fetch — no new poller.
  useAttentionToasts({
    companies,
    events: attentionRows,
    companyById,
    attentionRulesById,
    onReview: (event: AttentionEvent) => openAttentionEvidence(event, ctx),
    onDismiss: dismissAttentionEventRow,
  });

  // Counter tiles: "Pilne" (urgent rows) first, then the live pre-cap category
  // counts (ADR 0076 U-Rb D5 + the mockup's fourth-tile amendment).
  const urgentCount = groups.filter((group) => group.severity === "urgent").length;
  const counts = {
    autopilot: autopilotRuns.length,
    verify: verifyClaims.length,
    upcoming: upcomingAll.length,
  };

  const hasAttention =
    autopilotRuns.length > 0 ||
    attentionRows.length > 0 ||
    verifyRows.length > 0 ||
    changedRows.length > 0;
  const anyLoading = autopilotLoading || attentionLoading || claimsLoading || season.loading;
  // A failed category is explicit, never false quiet (J1 contract, ADR 0081 Q9):
  // an errored read must not let the stream read as "nothing needs attention".
  const anyCategoryError = Boolean(claimsError || season.error);
  const showQuietState = filter === null && !hasAttention && !anyCategoryError;

  // Config-state banner conditions (ADR 0087 dec. 5): a property of the app itself,
  // stated ONCE above the stream. Wired to source health — an ENABLED adapter in the
  // `attention` state (its last refresh errored) means its signals may be stale. The
  // copy is translated here (typed, product-facing), and Diagnostyka jumps to Sources.
  const configConditions: ConfigCondition[] = useMemo(
    () =>
      sourceAdapters
        .filter((adapter) => adapter.enabled && adapter.healthStatus === "attention")
        .map((adapter) => ({
          id: `source_attention_${adapter.id}`,
          // Name sits mid-sentence (both locales), so the copy is two translated
          // clauses around the adapter's own display name.
          message: `${text("Source")} ${adapter.displayName} ${text("isn't responding — signals may be delayed")}`,
          action: { label: text("Diagnostics"), onClick: openSources },
        })),
    [sourceAdapters, text, openSources],
  );

  const showVerifyShowAll =
    (filter === null || filter === "verify") && verifyClaims.length > VERIFY_CAP;
  const showChangedShowAll = filter === null && whatChanged.length > CHANGED_CAP;
  const showUpcomingShowAll =
    (filter === null || filter === "upcoming") && upcomingAll.length > UPCOMING_CAP;

  function counterTile(kind: StreamFilter, label: string, count: number, hot = false) {
    const active = filter === kind;
    return (
      <button
        type="button"
        className={[
          "today-counter",
          hot ? "today-counter-hot" : "",
          count === 0 ? "today-counter-empty" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        aria-pressed={active}
        aria-label={`${label}: ${count}`}
        onClick={() => setFilter(active ? null : kind)}
        data-counter={kind}
      >
        <span className="today-counter-count num-tabular">{count}</span>
        <span className="today-counter-label">{label}</span>
      </button>
    );
  }

  function errorStrip(message: string, onRetry: () => void) {
    return (
      <div className="today-error-strip" role="alert">
        <span className="today-error-strip-message">{message}</span>
        <Button onClick={onRetry} type="button" variant="ghost">
          <RefreshCw aria-hidden="true" size={13} />
          {text("Try again")}
        </Button>
      </div>
    );
  }

  function showAllRow(key: string, label: string, onClick: () => void) {
    return (
      <li key={key} className="today-stream-showall">
        <Button
          className="today-row-review"
          data-today-row="true"
          onClick={onClick}
          onKeyDown={onStreamRowKeyDown}
          type="button"
          variant="ghost"
        >
          {label}
        </Button>
      </li>
    );
  }

  return (
    <section className="today-screen feed-panel" aria-labelledby="today-title">
      <PanelHeader
        title={t("today.title")}
        description={t("today.description")}
        titleId="today-title"
        actions={
          <>
            <SegmentedControl ariaLabel={t("today.view.label")} className="today-view-switch">
              <SegmentedControlOption active={!isArchive} onClick={() => setView("active")}>
                {t("today.view.active")}
              </SegmentedControlOption>
              <SegmentedControlOption active={isArchive} onClick={() => setView("archive")}>
                {t("today.view.archive")}
              </SegmentedControlOption>
            </SegmentedControl>
            <Button onClick={openInbox} type="button" variant="ghost">
              <Inbox aria-hidden="true" size={14} />
              {text("Open Inbox")}
            </Button>
          </>
        }
      />

      {/* The config banner + morning briefing are active-stream concerns; the
          Archive is only the dismissed-event history. */}
      {!isArchive ? (
        <>
          <ConfigBanner conditions={configConditions} />

          <MorningBriefingStrip
            briefing={morningBriefing.briefing}
            companyById={companyById}
            generating={morningBriefing.generating}
            loading={morningBriefing.loading}
            onGenerate={morningBriefing.generate}
            onOpenItem={openBriefingItemEvidence}
          />
        </>
      ) : null}

      <div className={isArchive ? "today-body today-body-archive" : "today-body"}>
        {isArchive ? (
          <section className="today-stream-region" aria-label={text("Attention stream")}>
            <p className="today-archive-subtitle">{t("today.archive.subtitle")}</p>
            {archiveLoading && archiveRows.length === 0 ? (
              <Skeleton variant="list-row" count={3} label={t("today.archive.subtitle")} />
            ) : (
              <ul className="today-stream ui-list-rows">
                {archiveRows.length === 0 ? (
                  <li className="today-stream-quiet">
                    <EmptyState>{t("today.archive.empty")}</EmptyState>
                  </li>
                ) : (
                  archiveRows.map((event) => (
                    <ArchivedAttentionRow key={event.id} event={event} ctx={ctx} />
                  ))
                )}
              </ul>
            )}
          </section>
        ) : (
          <>
        <div className="today-counters" role="group" aria-label={text("Filter the stream")}>
          {counterTile("urgent", text("Urgent"), urgentCount, true)}
          <SeverityLegend />
          {counterTile("autopilot", text("Autopilot"), counts.autopilot)}
          {counterTile("verify", text("To verify"), counts.verify)}
          {counterTile("upcoming", text("Upcoming reports"), counts.upcoming)}
        </div>

        <section className="today-stream-region" aria-label={text("Attention stream")}>
          {claimsError ? errorStrip(text("Couldn't load claims to verify."), retryClaims) : null}
          {season.error ? errorStrip(text("Couldn't load upcoming reports."), season.reload) : null}

          {showQuietState && anyLoading ? (
            <Skeleton
              variant="list-row"
              count={3}
              label={text("Checking what needs your attention…")}
            />
          ) : (
            <ul className="today-stream ui-list-rows">
              {showQuietState ? (
                <li className="today-stream-quiet">
                  <EmptyState>{text("Nothing needs your attention.")}</EmptyState>
                </li>
              ) : null}

              {visibleRows.map((row) =>
                row.kind === "aggregate" ? (
                  <AggregateRow key={row.key} aggregate={row} ctx={ctx} />
                ) : row.count === 1 ? (
                  <StreamRowView key={row.key} item={row.members[0]} ctx={ctx} />
                ) : (
                  <GroupRow key={row.key} group={row} ctx={ctx} />
                ),
              )}

              {showVerifyShowAll
                ? showAllRow("verify-showall", text("Show all in Claims"), () =>
                    openCompanyWorkspace(verifyClaims[0].companyId, "Claims"),
                  )
                : null}
              {showChangedShowAll
                ? showAllRow("changed-showall", text("Show all in Inbox"), openInbox)
                : null}
              {showUpcomingShowAll
                ? showAllRow(
                    "upcoming-showall",
                    `${text("Show all upcoming reports")} (${upcomingAll.length})`,
                    openReportSeason,
                  )
                : null}
            </ul>
          )}
        </section>
          </>
        )}
      </div>
    </section>
  );
}
