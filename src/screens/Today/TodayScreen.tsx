import { useEffect, useMemo, useRef, useState, type ReactElement, type RefObject } from "react";
import { RefreshCw } from "lucide-react";

import { setAutopilotRunNotificationState, undoAutopilotRun } from "../../api/autopilot";
import { getTodayView, markTodayVisited, type TodayView } from "../../api/today";
import type { TodayItem } from "../../api/generated/TodayItem";
import { listAttentionEvents, type AlertRule, type AttentionEvent } from "../../api/attention";
import type { Company, SourceAdapter } from "../../api/types";
import type { CompanyWorkspaceTab } from "../Companies/companyTypes";
import type { AttentionController } from "../../app/useAttentionController";
import { useCommandQuery } from "../../shared/state/useCommandQuery";
import { formatDayRangeLabel } from "../../shared/format/datetime";
import { useLocale, type LocaleCode } from "../../shared/locale";
import { COMPANY_FORMS, ITEM_FORMS, MEDIA_ITEM_FORMS, pluralNoun } from "../../shared/locale/plural";
import { Button, EmptyState, ErrorText, PanelHeader, Skeleton } from "../../ui";
import {
  allSeen,
  bucketByLocalDay,
  capDayRows,
  formatDeltaHeadline,
  itemTimestamp,
  pickPrimary,
  splitDisplayBuckets,
  suppressCapturedNonArrivals,
  type DayBucket,
  type DayRow,
  type MediaCluster,
  type PrimaryCandidate,
} from "./dayQueueModel";
import { AttentionRow } from "./rows2/AttentionRow";
import { AutopilotRunRow } from "./rows2/AutopilotRunRow";
import { CalendarRow } from "./rows2/CalendarRow";
import { ClaimRow } from "./rows2/ClaimRow";
import { DayHeader } from "./rows2/DayHeader";
import { DeltaHeader, type DeltaAction } from "./rows2/DeltaHeader";
import { EarlierRollupRow } from "./rows2/EarlierRollupRow";
import { FilingRow } from "./rows2/FilingRow";
import { MediaClusterRow } from "./rows2/MediaClusterRow";
import { NonArrivalRow } from "./rows2/NonArrivalRow";
import { ConfigBanner, type ConfigCondition } from "./ConfigBanner";

export type TodayScreenProps = {
  /** The app-level attention state (ADR 0106 dec. 5) — root-fed, merged into
   * the day queue on the frontend (`dayQueueModel`). */
  attention: AttentionController;
  companies: Company[];
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
  /** "Otwórz komunikat"/"Otwórz artykuł" (plan decision 6): scope Inbox to the
   * company, select EXACTLY this item, raise the S-overlay. */
  openInboxItem: (feedItemId: string, companyId: string) => void;
  /** "Otwórz w Inbox" (a real media cluster): scope Inbox to the company only. */
  openCompanyInbox: (company: Company) => void;
  /** The un-anchored Inbox entry (attention-evidence fallback, footer link). */
  openInbox: () => void;
  /** "Otwórz tezę" (plan decision 6): the claims-panel highlight seam. */
  openCompanyClaims: (companyId: string, claimId: string) => void;
  /** A reconciliation event's missed report (ADR 0097 dec. 8). */
  openExternalUrl: (url: string) => void;
  sourceAdapters: SourceAdapter[];
  openSources: () => void;
  refreshSources: (trigger: "manual") => Promise<void> | Promise<unknown>;
  todayReviewedDays: string[];
  updateTodayReviewedDays: (days: string[]) => void;
  /** Increments once per COMPLETED source refresh (fix wave B finding 1a,
   * `useTodayScreenWiring`'s `useRefreshCompletionSignal`) — the
   * `get_today_view` query key, decoupled from whether the root-fed attention
   * state's own identity happened to change on that pass. */
  refreshCompletionCount: number;
};

// Composed read window (plan decision 1): the backend clamps to 1..7 anyway;
// 7 gives the fullest "unreviewed days" queue + the widest calendar look-ahead.
const DAY_LIMIT = 7;

// S-tier row cap (plan Dense row "S top-3+zwiń", Compact.dc.html): measured
// directly against the hosting pane size container, same technique as
// EventsScreen's `usePaneCompact` — width-only here (Today's cap is a width
// concern, not the merged width/height "compact" Events needs for its
// week/list switch). jsdom has no `ResizeObserver` (some SSR too) → stays
// `false`, matching EventsScreen's precedent (`density-matrix.spec.ts`
// proves the live behavior in a real browser).
function useNarrowPane(ref: RefObject<HTMLElement | null>): boolean {
  const [narrow, setNarrow] = useState(false);
  useEffect(() => {
    const host = ref.current;
    if (!host || typeof ResizeObserver === "undefined") return;
    const pane = (host.closest(".cockpit-pane, .workspace") as HTMLElement | null) ?? host;
    const measure = () => {
      const width = pane.clientWidth;
      setNarrow(width > 0 && width < 420);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(pane);
    return () => observer.disconnect();
  }, [ref]);
  return narrow;
}

function shortTicker(qualifiedTicker: string | null | undefined): string {
  if (!qualifiedTicker) return "";
  const idx = qualifiedTicker.indexOf(":");
  return idx >= 0 ? qualifiedTicker.slice(idx + 1) : qualifiedTicker;
}

// Dziś/Wczoraj/Today — the day-section home (ADR 0054), rebuilt to the F2
// Dziś v2 experience contract: a per-day decision queue anchored to the
// user's last visit (`get_today_view` + `mark_today_visited`, F2 S1/S2), the
// delta header leading (ADR 0068 amendment 2026-08-20 — the retired
// MorningBriefingStrip's replacement), one composed read instead of four
// independent per-category fetches.
export function TodayScreen({
  attention,
  companies,
  openCompanyWorkspace,
  openInboxItem,
  openCompanyInbox,
  openInbox,
  openCompanyClaims,
  openExternalUrl,
  sourceAdapters,
  openSources,
  refreshSources,
  todayReviewedDays,
  updateTodayReviewedDays,
  refreshCompletionCount,
}: TodayScreenProps) {
  const { t, text, locale } = useLocale();

  const rootRef = useRef<HTMLElement | null>(null);
  const narrow = useNarrowPane(rootRef);

  // Refetch on the dedicated completion signal (fix wave B finding 1a) — NOT
  // `attention.events`, whose array identity a same-shape poll leaves
  // unchanged even after a refresh that ingested new filings/media (that
  // fragile coupling silently missed a Today refetch). Combined with the
  // render-level non-arrival suppression below, this guarantees exactly one
  // row for a captured non-arrival regardless of which of the two
  // independent fetches (this one, root-fed attention) lands first.
  const query = useCommandQuery(["todayView", refreshCompletionCount], () => getTodayView(DAY_LIMIT));

  // `mark_today_visited` fires ONCE, after the first SUCCESSFUL render with a
  // TRUSTWORTHY anchor — never on error/unmount (plan decision 4), and never
  // when the read that PRODUCED the anchor is itself degraded (fix wave B
  // finding 5): `sectionErrors.feed` means the delta count is a lie, and
  // `sectionErrors.anchor` means `previousVisitAt` itself may be stale/wrong
  // — stamping a new visit over either would silently swallow un-shown items
  // into "already seen". The ref guards against re-firing on every later
  // refetch once a trustworthy visit has stamped.
  const visitedRef = useRef(false);
  useEffect(() => {
    if (query.status !== "success" || visitedRef.current) return;
    if (query.data.sectionErrors.feed || query.data.sectionErrors.anchor) return;
    visitedRef.current = true;
    void markTodayVisited().catch(() => {});
  }, [query.status, query.data]);

  // Seen = "was on screen the last time Today was open" (ADR 0097 dec. 5):
  // batch-mark every loaded unseen event once the day queue has real data, so
  // the sidebar badge clears on a visit. Gated on the query's FIRST SUCCESS
  // (fix wave B finding 4) — marking seen off a stale/loading/errored read
  // would clear the badge for events the user never actually saw rendered.
  // One IPC call; the optimistic flip empties the unseen set, so the effect
  // self-quiesces on the next render.
  const { markManySeen } = attention;
  useEffect(() => {
    if (query.status !== "success") return;
    const unseenIds = attention.events
      .filter((event) => !event.seen && !event.dismissed)
      .map((event) => event.id);
    if (unseenIds.length > 0) void markManySeen(unseenIds).catch(() => {});
  }, [query.status, attention.events, markManySeen]);

  const companyById = useMemo(() => new Map(companies.map((company) => [company.id, company])), [companies]);

  // Locally-expanded days (session-only, decision 5's "Otwórz dzień"); marking
  // a day reviewed collapses it, expanding a manually-reviewed day again
  // undoes that mark ("Oznacz jako nieprzejrzany" is the removal, not a
  // separate control).
  const [expandedDays, setExpandedDays] = useState<Set<string>>(() => new Set());
  function expandDay(day: string) {
    setExpandedDays((prior) => new Set(prior).add(day));
    if (todayReviewedDays.includes(day)) {
      updateTodayReviewedDays(todayReviewedDays.filter((reviewed) => reviewed !== day));
    }
  }
  function markDayReviewed(day: string) {
    if (!todayReviewedDays.includes(day)) {
      updateTodayReviewedDays([...todayReviewedDays, day]);
    }
  }

  // Archive (dismissed attention, KEPT per plan decision 7) — a quiet footer
  // toggle, fetched lazily on first open.
  const [showArchive, setShowArchive] = useState(false);
  const [archivedEvents, setArchivedEvents] = useState<AttentionEvent[]>([]);
  const [archiveLoading, setArchiveLoading] = useState(false);
  function toggleArchive() {
    setShowArchive((current) => {
      const next = !current;
      if (next && archivedEvents.length === 0) {
        setArchiveLoading(true);
        listAttentionEvents({ companyId: null, includeDismissed: true })
          .then((events) => setArchivedEvents(events.filter((event) => event.dismissed)))
          .catch(() => setArchivedEvents([]))
          .finally(() => setArchiveLoading(false));
      }
      return next;
    });
  }

  // Evidence click-through for a root-fed attention row (ADR 0097 dec. 8):
  // mark it seen, then jump to its evidence surface.
  function openAttentionRowAction(event: AttentionEvent) {
    void attention.markSeen(event.id).catch(() => {});
    if (event.evidenceType === "source_reconciliation" && event.witnessUrl) {
      openExternalUrl(event.witnessUrl);
      return;
    }
    const company = event.companyId ? companyById.get(event.companyId) : undefined;
    if (!company) {
      openInbox();
      return;
    }
    switch (event.evidenceType) {
      case "company_signal":
      case "source_reconciliation":
        openCompanyWorkspace(company.id, "Feed");
        return;
      case "autopilot_run":
      case "daily_quote":
        openCompanyWorkspace(company.id, "Fundamentals");
        return;
      default:
        openInbox();
    }
  }

  function openFilingItem(item: Extract<TodayItem, { kind: "filing" }>) {
    if (item.presentationKind === "report") openCompanyWorkspace(item.companyId, "Feed");
    else openInboxItem(item.feedItemId, item.companyId);
  }

  function openMediaClusterItem(item: MediaCluster) {
    if (item.count === 1) {
      openInboxItem(item.feedItemIds[0], item.companyId);
      return;
    }
    const company = companyById.get(item.companyId);
    if (company) openCompanyInbox(company);
  }

  function itemRowKey(item: DayRow): string {
    switch (item.kind) {
      case "filing":
        return `filing:${item.feedItemId}`;
      case "mediaCluster":
        return `media:${item.companyId}:${item.day}`;
      case "nonArrival":
        return `nonArrival:${item.eventKey}`;
      case "calendar":
        return `calendar:${item.eventKey}`;
      case "autopilotRun":
        return `autopilot:${item.run.id}`;
    }
  }

  function renderItemRow(item: DayRow) {
    switch (item.kind) {
      case "filing":
        return <FilingRow key={itemRowKey(item)} item={item} onOpen={() => openFilingItem(item)} />;
      case "mediaCluster":
        return (
          <MediaClusterRow key={itemRowKey(item)} item={item} onOpen={() => openMediaClusterItem(item)} />
        );
      case "nonArrival":
        return (
          <NonArrivalRow key={itemRowKey(item)} item={item} onOpen={() => void refreshSources("manual")} />
        );
      case "calendar":
        return <CalendarRow key={itemRowKey(item)} item={item} />;
      case "autopilotRun":
        return (
          <AutopilotRunRow
            key={itemRowKey(item)}
            item={item}
            qualifiedTicker={companyById.get(item.run.companyId)?.qualifiedTicker ?? null}
            onOpen={() => openCompanyWorkspace(item.run.companyId, "Fundamentals")}
            onUndo={
              item.run.producedFactIds.length > 0
                ? () => void undoAutopilotRun(item.run.id).catch(() => {})
                : null
            }
            onDismiss={() =>
              void setAutopilotRunNotificationState({ runId: item.run.id, notificationState: "read" }).catch(
                () => {},
              )
            }
          />
        );
    }
  }

  function renderAttentionRow(event: AttentionEvent) {
    const company = event.companyId ? companyById.get(event.companyId) : undefined;
    return (
      <AttentionRow
        key={event.id}
        event={event}
        qualifiedTicker={company?.qualifiedTicker ?? event.companyId}
        rule={event.ruleId ? attention.rulesById.get(event.ruleId) : undefined}
        actionLabel={text("Review")}
        onOpen={() => openAttentionRowAction(event)}
        onDismiss={() => void attention.dismiss(event.id).catch(() => {})}
      />
    );
  }

  // Within a day, items and root-fed attention interleave by their own
  // timestamp — both lists arrive newest-first (bucketByLocalDay), so a
  // straight sort-by-key merge is enough (no need for the model's internal
  // comparator to be re-derived here).
  function dayRows(bucket: DayBucket) {
    const entries: Array<{ key: string; ts: string; node: ReactElement }> = [
      ...bucket.items.map((item) => ({ key: itemRowKey(item), ts: itemTimestamp(item), node: renderItemRow(item) })),
      ...bucket.attention.map((event) => ({ key: event.id, ts: event.firedAt, node: renderAttentionRow(event) })),
    ];
    // A true 3-way compare — see `dayQueueModel.ts`'s `compareDesc` note: a
    // bare `a < b ? 1 : -1` never returns 0 on a tie, so it cannot promise a
    // stable merge for two rows landing in the same second.
    entries.sort((a, b) => (a.ts === b.ts ? 0 : a.ts < b.ts ? 1 : -1));
    return entries.map((entry) => <li key={entry.key}>{entry.node}</li>);
  }

  // The header's ONE primary action (plan decision 8/§6): the most-urgent
  // candidate, or none on a clean morning.
  function primaryFromCandidate(candidate: PrimaryCandidate): DeltaAction {
    if (candidate.kind === "attention") {
      const event = candidate.event;
      const company = event.companyId ? companyById.get(event.companyId) : undefined;
      const suffix = company ? ` ${shortTicker(company.qualifiedTicker)}` : "";
      return { label: `${text("Review")}${suffix}`, onClick: () => openAttentionRowAction(event) };
    }
    const item = candidate.item;
    switch (item.kind) {
      case "filing": {
        const suffix = ` ${shortTicker(item.qualifiedTicker)}`;
        return item.presentationKind === "report"
          ? { label: `${text("Read report")}${suffix}`, onClick: () => openFilingItem(item) }
          : { label: `${text("Open filing")}${suffix}`, onClick: () => openFilingItem(item) };
      }
      case "mediaCluster": {
        const suffix = ` ${shortTicker(item.qualifiedTicker)}`;
        return item.count === 1
          ? { label: `${text("Open article")}${suffix}`, onClick: () => openMediaClusterItem(item) }
          : { label: `${text("Open in the Inbox")}${suffix}`, onClick: () => openMediaClusterItem(item) };
      }
      case "nonArrival":
        return {
          label: `${text("Refresh sources")} ${shortTicker(item.qualifiedTicker)}`,
          onClick: () => void refreshSources("manual"),
        };
      case "autopilotRun": {
        const suffix = ` ${shortTicker(companyById.get(item.run.companyId)?.qualifiedTicker)}`;
        return {
          label: `${text("Read report")}${suffix}`,
          onClick: () => openCompanyWorkspace(item.run.companyId, "Fundamentals"),
        };
      }
      case "calendar":
        // Unreachable: `dayQueueModel.pickPrimary` never selects a calendar
        // row (it carries no landing destination, `isActionable`).
        return { label: text("Review"), onClick: () => {} };
    }
  }

  // Config-state banner conditions (source health), unchanged from v1.
  const configConditions: ConfigCondition[] = useMemo(
    () =>
      sourceAdapters
        .filter((adapter) => adapter.enabled && adapter.healthStatus === "attention")
        .map((adapter) => ({
          id: `source_attention_${adapter.id}`,
          message: `${text("Source")} ${adapter.displayName} ${text("isn't responding — signals may be delayed")}`,
          action: { label: text("Diagnostics"), onClick: openSources },
        })),
    [sourceAdapters, text, openSources],
  );

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

  return (
    <section className="today-screen feed-panel" aria-labelledby="today-title" ref={rootRef}>
      <PanelHeader title={t("today.title")} titleId="today-title" />
      <ConfigBanner conditions={configConditions} />

      <div className="dayq-screen-body">
        {query.status === "loading" ? (
          <Skeleton variant="list-row" count={3} label={text("Checking what's new since your last visit…")} />
        ) : null}

        {query.status === "error" ? (
          <div className="today-error-strip" role="alert">
            <ErrorText>{text("Couldn't load your Today view.")}</ErrorText>
            <Button onClick={query.refetch} type="button" variant="ghost">
              <RefreshCw aria-hidden="true" size={13} />
              {text("Try again")}
            </Button>
          </div>
        ) : null}

        {query.status === "success" ? (
          <TodayBody
            data={query.data}
            attentionEvents={attention.events}
            attentionError={attention.error}
            attentionRefresh={attention.refresh}
            rulesById={attention.rulesById}
            locale={locale}
            text={text}
            narrow={narrow}
            todayReviewedDays={todayReviewedDays}
            expandedDays={expandedDays}
            expandDay={expandDay}
            markDayReviewed={markDayReviewed}
            dayRows={dayRows}
            primaryFromCandidate={primaryFromCandidate}
            openCompanyClaims={openCompanyClaims}
            openInbox={openInbox}
            refreshSources={refreshSources}
            errorStrip={errorStrip}
            refetch={query.refetch}
          />
        ) : null}

        <div className="dayq-footer">
          <span className="dayq-footer-count">
            {companies.length} {pluralNoun(locale, companies.length, COMPANY_FORMS)} {text("tracked")}
          </span>
          <span className="dayq-footer-links">
            <Button variant="minimal" type="button" onClick={toggleArchive}>
              {showArchive ? t("today.view.active") : t("today.archive.subtitle")}
            </Button>
            {" · "}
            <Button variant="minimal" type="button" onClick={openInbox}>
              {text("Open Inbox")}
            </Button>
          </span>
        </div>

        {showArchive ? (
          <section className="today-stream-region" aria-label={t("today.archive.subtitle")}>
            {archiveLoading && archivedEvents.length === 0 ? (
              <Skeleton variant="list-row" count={3} label={t("today.archive.subtitle")} />
            ) : archivedEvents.length === 0 ? (
              <EmptyState>{t("today.archive.empty")}</EmptyState>
            ) : (
              <ul className="today-stream ui-list-rows">
                {archivedEvents.map((event) => {
                  const company = event.companyId ? companyById.get(event.companyId) : undefined;
                  return (
                    <li key={event.id}>
                      <AttentionRow
                        event={event}
                        qualifiedTicker={company?.qualifiedTicker ?? event.companyId}
                        rule={event.ruleId ? attention.rulesById.get(event.ruleId) : undefined}
                        actionLabel={text("Review")}
                        onOpen={() => openAttentionRowAction(event)}
                      />
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        ) : null}
      </div>
    </section>
  );
}

type TodayBodyProps = {
  data: TodayView;
  attentionEvents: AttentionEvent[];
  /** The root-fed attention state's last fetch failure (fix wave B finding
   * 4) — an errored attention state must never look quiet: it renders its
   * own error strip and suppresses the "clean morning" empty state, which
   * would otherwise falsely read as confidence that nothing is pending. */
  attentionError: string | null;
  attentionRefresh: () => void;
  rulesById: Map<string, AlertRule>;
  locale: LocaleCode;
  text: (value: string) => string;
  /** S-tier row cap (plan Dense row "S top-3+zwiń") — see `useNarrowPane`. */
  narrow: boolean;
  todayReviewedDays: string[];
  expandedDays: Set<string>;
  expandDay: (day: string) => void;
  markDayReviewed: (day: string) => void;
  dayRows: (bucket: DayBucket) => ReactElement[];
  primaryFromCandidate: (candidate: PrimaryCandidate) => DeltaAction;
  openCompanyClaims: (companyId: string, claimId: string) => void;
  openInbox: () => void;
  refreshSources: (trigger: "manual") => Promise<void> | Promise<unknown>;
  errorStrip: (message: string, onRetry: () => void) => ReactElement;
  refetch: () => void;
};

// Split out so the JSX above stays scannable — pure presentation over the
// already-composed `TodayView` + bucketed days; no data fetching of its own.
function TodayBody({
  data,
  attentionEvents,
  attentionError,
  attentionRefresh,
  rulesById,
  locale,
  text,
  narrow,
  todayReviewedDays,
  expandedDays,
  expandDay,
  markDayReviewed,
  dayRows,
  primaryFromCandidate,
  openCompanyClaims,
  openInbox,
  refreshSources,
  errorStrip,
  refetch,
}: TodayBodyProps) {
  const now = useMemo(() => new Date(), []);
  // Render-level non-arrival suppression (plan decision 2 / fix wave B
  // finding 1b): drop a `nonArrival` row whose company already has a fired
  // `report_delay` attention event BEFORE bucketing, so exactly one row shows
  // regardless of which of the two independent fetches (this composed read,
  // root-fed attention) landed first or is momentarily stale.
  const items = useMemo(
    () => suppressCapturedNonArrivals(data.items, attentionEvents, rulesById),
    [data.items, attentionEvents, rulesById],
  );
  const buckets = useMemo(() => bucketByLocalDay(items, attentionEvents, now), [items, attentionEvents, now]);
  const primaryCandidate = useMemo(() => pickPrimary(buckets), [buckets]);
  const primaryAction = primaryCandidate ? primaryFromCandidate(primaryCandidate) : null;

  // The "Wcześniej" rollup (plan decision 5 / contract §5): every bucket
  // beyond the two freshest-with-content display slots (DZIŚ + WCZORAJ)
  // collapses into ONE line; "Otwórz dni" reveals them session-locally.
  const { visible: visibleBuckets, earlier } = useMemo(() => splitDisplayBuckets(buckets), [buckets]);
  const [earlierExpanded, setEarlierExpanded] = useState(false);

  const hasContent = buckets.some((bucket) => bucket.total > 0) || data.toVerify.length > 0;
  // An errored attention state must never look quiet (fix wave B finding 4) —
  // the "clean morning" empty state below is a POSITIVE claim ("sources are
  // connected, nothing due"), which a failed attention fetch cannot honestly
  // back.
  const showEmptyState = !hasContent && !attentionError;
  const totalCount = buckets.reduce((sum, bucket) => sum + bucket.total, 0);

  // A day section's rows, S-tier capped to the newest 3 (plan Dense row "S
  // top-3+zwiń") — reusing `expandDay`'s `expandedDays` set as the same
  // "show everything for this day" override the natural-collapse recovery
  // link already uses (a day not naturally collapsed is never in that set,
  // so this is a no-op there until the cap actually needs overriding).
  function renderDayRows(bucket: DayBucket) {
    const rows = dayRows(bucket);
    const { visible, hidden } = capDayRows(rows, narrow, expandedDays.has(bucket.day));
    return (
      <>
        <ul className="dayq-day-rows">{visible}</ul>
        {hidden > 0 ? (
          <div className="dayq-day-cap">
            <span className="dayq-day-cap-count">
              +{hidden} {pluralNoun(locale, hidden, ITEM_FORMS)}
            </span>
            <Button variant="minimal" type="button" onClick={() => expandDay(bucket.day)}>
              {text("Open day")}
            </Button>
          </div>
        ) : null}
      </>
    );
  }

  function renderDaySection(bucket: DayBucket) {
    // "Days older than dayLimit always collapsed" (plan decision 5) is
    // already enforced by the BACKEND's fetch window — a bucket this far
    // back only exists here because `get_today_view` returned it, so every
    // bucket (today/yesterday/earlier alike) collapses by the SAME rule: the
    // manual reviewed-set, or full derivation (`allSeen`).
    const naturallyCollapsed = todayReviewedDays.includes(bucket.day) || allSeen(bucket);
    const collapsed = naturallyCollapsed && !expandedDays.has(bucket.day);
    return (
      <section className="dayq-section" key={bucket.day} aria-label={bucket.day}>
        <DayHeader
          day={bucket.day}
          relativeDay={bucket.relativeDay}
          total={bucket.total}
          unseen={bucket.unseen}
          collapsed={collapsed}
          onExpand={collapsed ? () => expandDay(bucket.day) : undefined}
          onMarkReviewed={!collapsed ? () => markDayReviewed(bucket.day) : undefined}
        />
        {!collapsed ? renderDayRows(bucket) : null}
      </section>
    );
  }

  const headline = formatDeltaHeadline(data.deltaSummary, locale, text);
  const eyebrow = `${text("Today")} · ${now.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" })}`;
  const note =
    data.deltaSummary.mediaCount > 0
      ? `${text("Plus")} ${data.deltaSummary.mediaCount} ${pluralNoun(locale, data.deltaSummary.mediaCount, MEDIA_ITEM_FORMS)}`
      : null;

  return (
    <>
      <DeltaHeader
        eyebrow={eyebrow}
        headline={headline}
        note={note}
        primaryAction={primaryAction}
        secondaryAction={totalCount > 0 ? { label: `${text("Open Inbox")} (${totalCount})`, onClick: openInbox } : null}
      />

      {data.sectionErrors.feed ? errorStrip(text("Couldn't load new filings/media."), refetch) : null}
      {data.sectionErrors.calendar ? errorStrip(text("Couldn't load the calendar."), refetch) : null}
      {data.sectionErrors.claims ? errorStrip(text("Couldn't load claims to verify."), refetch) : null}
      {data.sectionErrors.autopilot ? errorStrip(text("Couldn't load autopilot runs."), refetch) : null}
      {attentionError ? errorStrip(text("Couldn't load attention signals."), attentionRefresh) : null}

      {showEmptyState ? (
        // The headline beat is already the DeltaHeader above (its zero-count
        // case, `formatDeltaHeadline`) — the empty state adds only the other
        // two of the mockup's 3 taktów (Delta.dc.html Stan B): the reassurance
        // detail + the quiet refresh action.
        <div className="dayq-empty-state">
          <EmptyState>
            <p className="dayq-empty-detail">{text("Sources are connected and the calendar names nothing due today.")}</p>
          </EmptyState>
          <Button variant="secondary" type="button" onClick={() => void refreshSources("manual")}>
            {text("Refresh sources")}
          </Button>
        </div>
      ) : (
        <>
          {visibleBuckets.map(renderDaySection)}

          {earlier ? (
            earlierExpanded ? (
              earlier.buckets.map(renderDaySection)
            ) : (
              <section className="dayq-section" aria-label={text("Earlier")}>
                <EarlierRollupRow
                  rangeLabel={formatDayRangeLabel(earlier.oldestDay, earlier.newestDay, locale)}
                  count={earlier.count}
                  unseen={earlier.unseen}
                  onExpand={() => setEarlierExpanded(true)}
                />
              </section>
            )
          ) : null}

          {data.toVerify.length > 0 ? (
            <section className="dayq-section" aria-label={text("To verify")}>
              <div className="dayq-day-header" data-dayq-day-collapsed="false">
                <span className="dayq-day-label">{text("To verify")}</span>
                <span className="dayq-day-count">
                  {data.toVerify.length} {pluralNoun(locale, data.toVerify.length, ITEM_FORMS)}
                </span>
              </div>
              <ul className="dayq-day-rows">
                {data.toVerify.map((entry) => (
                  <li key={entry.claim.id}>
                    <ClaimRow entry={entry} onOpen={() => openCompanyClaims(entry.claim.companyId, entry.claim.id)} />
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </>
      )}
    </>
  );
}
