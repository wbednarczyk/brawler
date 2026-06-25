import { useMemo } from "react";
import {
  CalendarClock,
  CheckCircle2,
  FileText,
  Inbox,
  ShieldQuestion,
  Sparkles,
  X,
} from "lucide-react";

import type { CompanyWorkspaceTab } from "../Companies/companyTypes";
import type { Company, FeedItem, Watchlist } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { formatFeedTimestamp } from "../../shared/format/timestamp";
import { TickerLabel } from "../../shared/components/TickerLabel";
import {
  Button,
  EmptyState,
  ErrorText,
  ListRow,
  PanelHeader,
  SectionHeader,
  StatusPill,
} from "../../ui";
import { useTodayPulse } from "./useTodayPulse";

export type TodayScreenProps = {
  companies: Company[];
  watchlists: Watchlist[];
  pinnedCompanyIds: string[];
  recentFeedItems: FeedItem[];
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
  openInbox: () => void;
};

const MAX_ROWS = 6;

// Today/Pulse — the mode-based shell home (ADR 0054): a Triage-style attention
// digest that leads with the app's superpowers — what changed, what to verify,
// what's coming — over a watchlist conviction rollup, with the feed as secondary
// input. Composes existing app-wide read models (report season + claims for the
// pinned spine); the full accept/snooze/dismiss triage state and the autonomous
// "one notification" arrive with the v0.48/v0.49 epics.
export function TodayScreen({
  companies,
  watchlists,
  pinnedCompanyIds,
  recentFeedItems,
  openCompanyWorkspace,
  openInbox,
}: TodayScreenProps) {
  const { t, text } = useLocale();
  const {
    season,
    claims,
    claimsLoading,
    claimsError,
    autopilotRuns,
    autopilotLoading,
    dismissAutopilotRun,
  } = useTodayPulse(pinnedCompanyIds, companies);

  const companyByTicker = useMemo(
    () => new Map(companies.map((company) => [company.qualifiedTicker, company])),
    [companies],
  );
  const companyById = useMemo(
    () => new Map(companies.map((company) => [company.id, company])),
    [companies],
  );

  // "What changed" = freshly-arrived report disclosures from the feed, newest
  // first — the North-Star "what changed". Drawn from the live feed (not the
  // report-season calendar, whose past entries can lag days behind), so the
  // dates are current.
  const whatChanged = useMemo(() => {
    const reportTime = (item: FeedItem) => item.publishedAt || item.time;
    return recentFeedItems
      .filter((item) => /report|raport/i.test(item.type))
      .slice()
      .sort((a, b) => reportTime(b).localeCompare(reportTime(a)))
      .slice(0, MAX_ROWS);
  }, [recentFeedItems]);

  const upcoming = (season.season?.upcoming ?? []).slice(0, MAX_ROWS);
  // Overdue claims lead the verify queue, then due — both are actionable now.
  const verifyClaims = [...claims].sort((a, b) =>
    a.bucket === b.bucket ? 0 : a.bucket === "overdue" ? -1 : 1,
  );
  const recentFeed = recentFeedItems.slice(0, 5);
  const trackedCount = pinnedCompanyIds.length;
  const watchlistCount = watchlists.length;

  function reviewButton(companyId: string, tab: CompanyWorkspaceTab) {
    return (
      <Button onClick={() => openCompanyWorkspace(companyId, tab)} type="button" variant="ghost">
        {text("Review")}
      </Button>
    );
  }

  // A feed item's Review opens the resolved company's workspace, or the Inbox
  // when the item's company is not in the registry.
  function feedReviewButton(item: FeedItem) {
    const company = companyByTicker.get(item.company);
    return company ? (
      reviewButton(company.id, "Feed")
    ) : (
      <Button onClick={openInbox} type="button" variant="ghost">
        {text("Review")}
      </Button>
    );
  }

  return (
    <section className="today-screen feed-panel" aria-labelledby="today-title">
      <PanelHeader title={t("today.title")} description={t("today.description")} titleId="today-title" />

      <div className="today-body">
        <section className="today-card" aria-label={text("What changed")}>
          <SectionHeader level="h3" variant="accent" title={text("What changed")} />
          {whatChanged.length === 0 ? (
            <EmptyState>{text("No new reports have arrived.")}</EmptyState>
          ) : (
            whatChanged.map((item) => (
              <ListRow
                key={item.id}
                icon={<FileText size={15} aria-hidden="true" />}
                title={item.title}
                titleAttr={item.title}
                meta={
                  <span className="today-row-meta">
                    <TickerLabel value={item.company} />
                    <span>{formatFeedTimestamp(item.time)}</span>
                  </span>
                }
                trailing={feedReviewButton(item)}
              />
            ))
          )}
        </section>

        {autopilotLoading || autopilotRuns.length > 0 ? (
          <section className="today-card" aria-label={text("Autopilot")}>
            <SectionHeader level="h3" variant="accent" title={text("Autopilot")} />
            {autopilotLoading && autopilotRuns.length === 0 ? (
              <EmptyState>{text("Checking autopilot runs…")}</EmptyState>
            ) : (
              autopilotRuns.map((run) => {
                const company = companyById.get(run.companyId);
                const failed = run.status === "failed" || run.status === "partial";
                return (
                  <ListRow
                    key={run.id}
                    icon={<Sparkles size={15} aria-hidden="true" />}
                    title={run.summaryText ?? text("New report processed.")}
                    titleAttr={run.summaryText ?? undefined}
                    meta={
                      <span className="today-row-meta">
                        {company ? <TickerLabel value={company.qualifiedTicker} /> : null}
                        {failed ? (
                          <StatusPill tone="danger">
                            {run.status === "partial" ? text("Partial") : text("Failed")}
                          </StatusPill>
                        ) : null}
                      </span>
                    }
                    trailing={
                      <span className="today-row-actions">
                        {company ? reviewButton(company.id, "Fundamentals") : null}
                        <Button
                          onClick={() => dismissAutopilotRun(run.id)}
                          type="button"
                          variant="ghost"
                          aria-label={text("Dismiss")}
                        >
                          <X size={14} aria-hidden="true" />
                        </Button>
                      </span>
                    }
                  />
                );
              })
            )}
          </section>
        ) : null}

        <section className="today-card" aria-label={text("To verify")}>
          <SectionHeader level="h3" variant="accent" title={text("To verify")} />
          {claimsError ? <ErrorText>{claimsError}</ErrorText> : null}
          {trackedCount === 0 ? (
            <EmptyState>{text("Pin companies to track their claims here.")}</EmptyState>
          ) : claimsLoading ? (
            <EmptyState>{text("Checking claims to verify…")}</EmptyState>
          ) : verifyClaims.length === 0 ? (
            <EmptyState>{text("No claims are due for your pinned companies.")}</EmptyState>
          ) : (
            verifyClaims.map((item) => (
              <ListRow
                key={item.claim.id}
                icon={<CheckCircle2 size={15} aria-hidden="true" />}
                title={item.claim.statement}
                titleAttr={item.claim.statement}
                meta={
                  <span className="today-row-meta">
                    <TickerLabel value={item.qualifiedTicker} />
                    <StatusPill tone={item.bucket === "overdue" ? "danger" : "warn"}>
                      {item.bucket === "overdue" ? text("Overdue") : text("Due")}
                    </StatusPill>
                  </span>
                }
                trailing={reviewButton(item.companyId, "Claims")}
              />
            ))
          )}
        </section>

        <section className="today-card" aria-label={text("Upcoming reports")}>
          <SectionHeader level="h3" variant="accent" title={text("Upcoming reports")} />
          {season.loading ? (
            <EmptyState>{text("Loading the report calendar…")}</EmptyState>
          ) : upcoming.length === 0 ? (
            <EmptyState>{text("No upcoming report dates on the calendar.")}</EmptyState>
          ) : (
            upcoming.map((entry) => (
              <ListRow
                key={`${entry.companyId}::${entry.eventKey}`}
                icon={<CalendarClock size={15} aria-hidden="true" />}
                title={entry.displayName}
                titleAttr={entry.displayName}
                meta={
                  <span className="today-row-meta">
                    <TickerLabel value={entry.qualifiedTicker} />
                    <span>{entry.eventDate}</span>
                  </span>
                }
                trailing={reviewButton(entry.companyId, "Feed")}
              />
            ))
          )}
        </section>

        <section className="today-card" aria-label={text("Conviction")}>
          <SectionHeader level="h3" variant="accent" title={text("Conviction")} />
          <div className="today-conviction-rollup">
            <ShieldQuestion size={18} aria-hidden="true" />
            <div>
              <p className="today-conviction-headline">
                {text("Tracking {pinned} pinned of {watchlist} watchlist companies")
                  .replace("{pinned}", String(trackedCount))
                  .replace("{watchlist}", String(watchlistCount))}
              </p>
              <p className="today-conviction-note">
                {text("Per-company conviction status arrives with the valuation and thesis arc.")}
              </p>
            </div>
          </div>
        </section>

        <section className="today-card today-feed-peek" aria-label={text("Recent activity")}>
          <SectionHeader
            level="h3"
            title={text("Recent activity")}
            actions={
              <Button onClick={openInbox} type="button" variant="ghost">
                <Inbox size={14} aria-hidden="true" />
                {text("Open Inbox")}
              </Button>
            }
          />
          {recentFeed.length === 0 ? (
            <EmptyState>{text("No recent feed items.")}</EmptyState>
          ) : (
            recentFeed.map((item) => (
              <ListRow
                key={item.id}
                title={item.title}
                titleAttr={item.title}
                meta={formatFeedTimestamp(item.time)}
              />
            ))
          )}
        </section>
      </div>
    </section>
  );
}
