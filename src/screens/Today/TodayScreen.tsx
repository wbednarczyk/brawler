import { useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import {
  CalendarClock,
  CheckCircle2,
  ChevronRight,
  FileText,
  Inbox,
  Sparkles,
} from "lucide-react";

import type { CompanyWorkspaceTab } from "../Companies/companyTypes";
import type { Company, FeedItem } from "../../api/types";
import type { ReportSeasonEntry } from "../../api/generated/ReportSeasonEntry";
import { useLocale } from "../../shared/locale";
import { FACT_FORMS, pluralNoun } from "../../shared/locale/plural";
import { formatListTimestamp } from "../../shared/format/datetime";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { DriftDiff, parseDrift, type ParsedDrift } from "../../shared/components/DriftDiff";
import { composeAutopilotRunSummary } from "./autopilotRunSummary";
import {
  Button,
  EmptyState,
  ErrorText,
  InlineConfirm,
  PanelHeader,
  Skeleton,
  StatusChip,
} from "../../ui";
import { useTodayPulse, type PulseClaim } from "./useTodayPulse";
import type { AutopilotRun } from "../../api/autopilot";

export type TodayScreenProps = {
  companies: Company[];
  pinnedCompanyIds: string[];
  recentFeedItems: FeedItem[];
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
  openInbox: () => void;
};

// J1 caps (ADR 0076 U-Rb D2): the two unbounded attention queues cap at 8 rows,
// upcoming reports at 6; a capped queue ends with a "Show all" link into its
// full surface so nothing is silently hidden.
const VERIFY_CAP = 8;
const CHANGED_CAP = 8;
const UPCOMING_CAP = 6;

type StreamCategory = "autopilot" | "verify" | "changed" | "upcoming";

/**
 * Parses an autopilot run's opaque `kpiDeltaJson` envelope defensively for the
 * "structure changed" drift a structured-extraction attempt may have carried
 * (ADR 0061 wave 2 — `{ structureChanged: true, driftJson: "<DriftReport>" }`
 * merged into the composed delta). A legacy run (no such fields), a run whose
 * delta never drifted, or malformed JSON all yield `null` — no crash, no card.
 */
function autopilotRunDrift(kpiDeltaJson: string | null): ParsedDrift | null {
  if (!kpiDeltaJson) return null;
  try {
    const parsed = JSON.parse(kpiDeltaJson) as { structureChanged?: unknown; driftJson?: unknown };
    if (parsed.structureChanged !== true) return null;
    return parseDrift(typeof parsed.driftJson === "string" ? parsed.driftJson : null);
  } catch {
    return null;
  }
}

// Today/Pulse — the mode-based shell home (ADR 0054), redesigned to journey J1
// (ADR 0076 U-Rb): a single prioritized "what changed" stream (autopilot →
// to-verify → what-changed → upcoming, one action per row, j/k navigation) plus
// a narrow counters column that doubles as a category filter. The quiet state is
// the goal. Composes existing app-wide read models (report season + claims for
// the pinned spine + unread autopilot runs).
export function TodayScreen({
  companies,
  pinnedCompanyIds,
  recentFeedItems,
  openCompanyWorkspace,
  openInbox,
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
  } = useTodayPulse(pinnedCompanyIds, companies);
  // The Undo two-step (ADR 0055 §4): which run, if any, is mid-confirm.
  const [confirmingUndoRunId, setConfirmingUndoRunId] = useState<string | null>(null);
  // Autopilot rows whose in-place detail (undo/dismiss/drift) is expanded.
  const [expandedRunIds, setExpandedRunIds] = useState<Set<string>>(() => new Set());
  // The active counter filter, or null for the full stream.
  const [filter, setFilter] = useState<StreamCategory | null>(null);
  const streamRef = useRef<HTMLUListElement>(null);

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

  const autopilotRows = autopilotRuns;
  const verifyRows = verifyClaims.slice(0, VERIFY_CAP);
  const changedRows = whatChanged.slice(0, CHANGED_CAP);
  const upcomingRows = upcomingAll.slice(0, UPCOMING_CAP);

  // Counter tiles show the full live counts (pre-cap), ADR 0076 U-Rb D5.
  const counts = {
    autopilot: autopilotRuns.length,
    verify: verifyClaims.length,
    upcoming: upcomingAll.length,
  };

  const showAutopilot = filter === null || filter === "autopilot";
  const showVerify = filter === null || filter === "verify";
  const showChanged = filter === null;
  const showUpcoming = filter === null || filter === "upcoming";

  // Roving focus across the stream's primary action buttons (ADR 0076 U-Rb D4):
  // ArrowUp/ArrowDown and j/k move; Enter/Space trigger the focused button
  // natively. Mirrors the Inbox feed-row idiom, scoped to the Today stream.
  function onRowKeyDown(event: KeyboardEvent<HTMLElement>) {
    const isDown = event.key === "ArrowDown" || event.key === "j";
    const isUp = event.key === "ArrowUp" || event.key === "k";
    if (!isDown && !isUp) return;
    event.preventDefault();
    const buttons = Array.from(
      streamRef.current?.querySelectorAll<HTMLElement>('[data-today-row="true"]') ?? [],
    );
    const currentIndex = buttons.indexOf(event.currentTarget);
    const nextIndex = Math.min(Math.max(currentIndex + (isDown ? 1 : -1), 0), buttons.length - 1);
    buttons[nextIndex]?.focus();
  }

  function RowAction({ onClick }: { onClick: () => void }) {
    return (
      <Button
        className="today-row-review"
        data-today-row="true"
        onClick={onClick}
        onKeyDown={onRowKeyDown}
        type="button"
        variant="ghost"
      >
        {text("Review")}
      </Button>
    );
  }

  // The shared row anatomy (ADR 0076 U-Rb D3): leading icon · full ticker
  // (never truncated) · type badge · full date · title wrapping to 2 lines · the
  // one primary action. Autopilot rows pass extra chips + a trailing disclosure.
  function rowLine(args: {
    icon: ReactNode;
    ticker: string;
    badge: ReactNode;
    extraChips?: ReactNode;
    date: string | null;
    title: string;
    action: ReactNode;
    disclosure?: ReactNode;
  }) {
    return (
      <div className="today-row-line">
        <span className="today-row-icon">{args.icon}</span>
        <div className="today-row-body">
          <div className="today-row-head">
            <TickerLabel value={args.ticker} />
            {args.badge}
            {args.extraChips}
            <span className="today-row-date num-tabular">
              {formatListTimestamp(args.date, locale)}
            </span>
          </div>
          <p className="today-row-title" title={args.title}>
            {args.title}
          </p>
        </div>
        <div className="today-row-actions">
          {args.action}
          {args.disclosure}
        </div>
      </div>
    );
  }

  function autopilotRow(run: AutopilotRun) {
    const company = companyById.get(run.companyId);
    const failed = run.status === "failed" || run.status === "partial";
    const drift = autopilotRunDrift(run.kpiDeltaJson);
    // Undo reverts exactly `producedFactIds` (contracts.md § Autonomous Report
    // Pipeline) — reserved for `autopilot` mode, where facts land auto-committed;
    // `assist` proposals land `pending` for the confirm/reject review instead.
    const undoEligible = run.mode === "autopilot" && run.producedFactIds.length > 0;
    const revertedCount = undoneAutopilotRuns[run.id];
    const confirmingUndo = confirmingUndoRunId === run.id;
    const expanded = expandedRunIds.has(run.id);
    // The backend `summaryText` is English-only (legacy/fallback); the Today row
    // always composes its own localized sentence from the run's DATA fields.
    const summary = composeAutopilotRunSummary(run, text, locale);

    function toggleDetail() {
      setExpandedRunIds((prior) => {
        const next = new Set(prior);
        if (next.has(run.id)) next.delete(run.id);
        else next.add(run.id);
        return next;
      });
    }

    const extraChips = (
      <>
        {failed ? (
          <StatusChip tone="danger">
            {run.status === "partial" ? text("Partial") : text("Failed")}
          </StatusChip>
        ) : null}
        {revertedCount !== undefined ? (
          <StatusChip tone="ok">
            {text("Reverted")} {revertedCount} {pluralNoun(locale, revertedCount, FACT_FORMS)}
          </StatusChip>
        ) : null}
      </>
    );

    return (
      <li key={run.id} className="today-stream-row" data-category="autopilot">
        {rowLine({
          icon: <Sparkles size={15} aria-hidden="true" />,
          ticker: company?.qualifiedTicker ?? "",
          badge: <StatusChip tone="accent">{text("Autopilot")}</StatusChip>,
          extraChips,
          date: run.createdAt,
          title: summary,
          action: (
            <RowAction
              onClick={() =>
                company ? openCompanyWorkspace(company.id, "Fundamentals") : openInbox()
              }
            />
          ),
          disclosure: (
            <button
              type="button"
              className={["today-row-disclosure", expanded ? "today-row-disclosure-open" : ""]
                .filter(Boolean)
                .join(" ")}
              aria-expanded={expanded}
              aria-label={text("Details")}
              onClick={toggleDetail}
            >
              <ChevronRight size={15} aria-hidden="true" />
            </button>
          ),
        })}
        {expanded ? (
          <div className="today-run-detail">
            <div className="today-run-detail-actions">
              {undoEligible && !confirmingUndo ? (
                <Button
                  onClick={() => setConfirmingUndoRunId(run.id)}
                  type="button"
                  variant="ghost"
                >
                  {text("Undo")}
                </Button>
              ) : null}
              <Button
                onClick={() => dismissAutopilotRun(run.id)}
                type="button"
                variant="ghost"
              >
                {text("Dismiss")}
              </Button>
            </div>
            {confirmingUndo ? (
              <InlineConfirm
                cancelLabel={text("Cancel")}
                confirmLabel={text("Undo")}
                onCancel={() => setConfirmingUndoRunId(null)}
                onConfirm={() => {
                  setConfirmingUndoRunId(null);
                  void undoAutopilotRun(run.id);
                }}
              >
                {text("Undo this run and revert its facts?")}
              </InlineConfirm>
            ) : null}
            {drift ? (
              <section className="today-run-drift" aria-label={text("Structure changed")}>
                <span className="eyebrow">{text("Structure changed")}</span>
                <DriftDiff drift={drift} />
              </section>
            ) : null}
          </div>
        ) : null}
      </li>
    );
  }

  function verifyRow(item: PulseClaim) {
    return (
      <li key={item.claim.id} className="today-stream-row" data-category="verify">
        {rowLine({
          icon: <CheckCircle2 size={15} aria-hidden="true" />,
          ticker: item.qualifiedTicker,
          badge: (
            <StatusChip tone={item.bucket === "overdue" ? "danger" : "warn"}>
              {item.bucket === "overdue" ? text("Overdue") : text("Due")}
            </StatusChip>
          ),
          date: item.claim.madeAt ?? item.claim.createdAt,
          title: item.claim.statement,
          action: <RowAction onClick={() => openCompanyWorkspace(item.companyId, "Claims")} />,
        })}
      </li>
    );
  }

  function changedRow(item: FeedItem) {
    const company = companyByTicker.get(item.company);
    return (
      <li key={item.id} className="today-stream-row" data-category="changed">
        {rowLine({
          icon: <FileText size={15} aria-hidden="true" />,
          ticker: item.company,
          badge: <StatusChip tone="official">{text("Report")}</StatusChip>,
          date: item.publishedAt || item.time,
          title: item.title,
          action: (
            <RowAction
              onClick={() => (company ? openCompanyWorkspace(company.id, "Feed") : openInbox())}
            />
          ),
        })}
      </li>
    );
  }

  function upcomingRow(entry: ReportSeasonEntry) {
    return (
      <li key={`${entry.companyId}::${entry.eventKey}`} className="today-stream-row" data-category="upcoming">
        {rowLine({
          icon: <CalendarClock size={15} aria-hidden="true" />,
          ticker: entry.qualifiedTicker,
          badge: <StatusChip tone="neutral">{text("Upcoming")}</StatusChip>,
          date: entry.eventDate,
          title: entry.displayName,
          action: <RowAction onClick={() => openCompanyWorkspace(entry.companyId, "Feed")} />,
        })}
      </li>
    );
  }

  function showAllRow(key: string, label: string, onClick: () => void) {
    return (
      <li key={key} className="today-stream-showall">
        <Button
          className="today-row-review"
          data-today-row="true"
          onClick={onClick}
          onKeyDown={onRowKeyDown}
          type="button"
          variant="ghost"
        >
          {label}
        </Button>
      </li>
    );
  }

  const hasAttention =
    autopilotRows.length > 0 || verifyRows.length > 0 || changedRows.length > 0;
  const anyLoading = autopilotLoading || claimsLoading || season.loading;
  const showQuietState = filter === null && !hasAttention;

  function counterTile(category: StreamCategory, label: string, count: number) {
    const active = filter === category;
    return (
      <button
        type="button"
        className={["today-counter", count === 0 ? "today-counter-empty" : ""]
          .filter(Boolean)
          .join(" ")}
        aria-pressed={active}
        aria-label={`${label}: ${count}`}
        onClick={() => setFilter(active ? null : category)}
        data-counter={category}
      >
        <span className="today-counter-count num-tabular">{count}</span>
        <span className="today-counter-label">{label}</span>
      </button>
    );
  }

  return (
    <section className="today-screen feed-panel" aria-labelledby="today-title">
      <PanelHeader
        title={t("today.title")}
        description={t("today.description")}
        titleId="today-title"
        actions={
          <Button onClick={openInbox} type="button" variant="ghost">
            <Inbox size={14} aria-hidden="true" />
            {text("Open Inbox")}
          </Button>
        }
      />

      <div className="today-body">
        <div className="today-counters" role="group" aria-label={text("Filter the stream")}>
          {counterTile("autopilot", text("Autopilot"), counts.autopilot)}
          {counterTile("verify", text("To verify"), counts.verify)}
          {counterTile("upcoming", text("Upcoming reports"), counts.upcoming)}
        </div>

        <section className="today-stream-region" aria-label={text("Attention stream")}>
          {claimsError ? <ErrorText>{claimsError}</ErrorText> : null}
          {showQuietState && anyLoading ? (
            <Skeleton variant="list-row" count={3} label={text("Checking what needs your attention…")} />
          ) : (
            <ul className="today-stream ui-list-rows" ref={streamRef}>
              {showQuietState ? (
                <li className="today-stream-quiet">
                  <EmptyState>{text("Nothing needs your attention.")}</EmptyState>
                </li>
              ) : null}

              {showAutopilot ? autopilotRows.map((run) => autopilotRow(run)) : null}

              {showVerify ? verifyRows.map((item) => verifyRow(item)) : null}
              {showVerify && verifyClaims.length > VERIFY_CAP
                ? showAllRow("verify-showall", text("Show all in Claims"), () =>
                    openCompanyWorkspace(verifyClaims[0].companyId, "Claims"),
                  )
                : null}

              {showChanged ? changedRows.map((item) => changedRow(item)) : null}
              {showChanged && whatChanged.length > CHANGED_CAP
                ? showAllRow("changed-showall", text("Show all in Inbox"), openInbox)
                : null}

              {showUpcoming ? upcomingRows.map((entry) => upcomingRow(entry)) : null}
            </ul>
          )}
        </section>
      </div>
    </section>
  );
}
