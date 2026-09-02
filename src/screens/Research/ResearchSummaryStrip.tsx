import type { Company, Watchlist } from "../../api/types";
import type { ResearchTimelineResult } from "../../api/researchTypes";
import type { ResearchMode } from "../../app/useResearchController";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { Figure } from "../../ui";

type ResearchSummaryStripProps = {
  mode: ResearchMode;
  timeline: ResearchTimelineResult | null;
  selectedCompany: Company | null;
  selectedWatchlist: Watchlist | null;
  watchlistCompanyCount: number;
  text: (value: string) => string;
};

export function ResearchSummaryStrip({
  mode,
  timeline,
  selectedCompany,
  selectedWatchlist,
  watchlistCompanyCount,
  text,
}: ResearchSummaryStripProps) {
  return (
    <div role="group" className="research-summary" aria-label={text("Research review summary")}>
      <div>
        <span>{text("Evidence")}</span>
        <strong><Figure value={timeline?.summary.total ?? 0} /></strong>
      </div>
      <div>
        <span>{text("Changed")}</span>
        <strong><Figure value={timeline?.summary.changedSinceReview ?? 0} /></strong>
      </div>
      {mode === "watchlist" ? (
        <>
          <div>
            <span>{text("Companies")}</span>
            <strong><Figure value={timeline?.summary.memberCompanyCount ?? watchlistCompanyCount} /></strong>
          </div>
          <div>
            <span>{text("Need review")}</span>
            <strong><Figure value={timeline?.summary.companiesWithChangedEvidence ?? 0} /></strong>
          </div>
        </>
      ) : null}
      <div>
        <span>{text("Last reviewed")}</span>
        <strong>
          {timeline?.summary.lastReviewedAt ? (
            <Figure kind="datetime" value={timeline.summary.lastReviewedAt} />
          ) : (
            text("Never")
          )}
        </strong>
      </div>
      {mode === "company" && selectedCompany ? (
        <div className="research-summary-company">
          <TickerLabel value={selectedCompany.qualifiedTicker} />
          <strong>{selectedCompany.displayName}</strong>
        </div>
      ) : null}
      {mode === "watchlist" && selectedWatchlist ? (
        <div className="research-summary-company">
          <span>{text("Watchlist")}</span>
          <strong>{selectedWatchlist.name}</strong>
        </div>
      ) : null}
    </div>
  );
}
