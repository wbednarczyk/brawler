import { ArrowRight, CheckCheck, ExternalLink, RefreshCw } from "lucide-react";
import type { Company, Watchlist, WatchlistMembership } from "../../api/types";
import type {
  ResearchEvidenceItem,
  ResearchEvidenceType,
  ResearchTimelineResult,
  ResearchTrustCategory,
} from "../../api/researchTypes";
import type { ResearchMode } from "../../app/useResearchController";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatAiProvider, formatCompanyEventType } from "../../shared/formatting/labels";
import { useLocale } from "../../shared/locale";

const evidenceTypeOptions: Array<{ value: ResearchEvidenceType; label: string }> = [
  { value: "feed_item", label: "Feed items" },
  { value: "notebook_entry", label: "Notes" },
  { value: "claim", label: "Claims" },
  { value: "company_event", label: "Events" },
  { value: "transcript_segment", label: "Transcripts" },
  { value: "ai_analysis", label: "AI analysis" },
];

type ResearchScreenProps = {
  companies: Company[];
  watchlists: Watchlist[];
  watchlistMemberships: WatchlistMembership[];
  mode: ResearchMode;
  selectedCompanyId: string | null;
  selectedWatchlistId: string | null;
  selectedWatchlistCompanyId: string | null;
  cascadeToCompanies: boolean;
  selectedEvidenceTypes: ResearchEvidenceType[];
  changedOnly: boolean;
  timeline: ResearchTimelineResult | null;
  error: string | null;
  loading: boolean;
  reviewInFlight: boolean;
  setMode: (mode: ResearchMode) => void;
  setSelectedCompanyId: (companyId: string | null) => void;
  setSelectedWatchlistId: (watchlistId: string | null) => void;
  setSelectedWatchlistCompanyId: (companyId: string | null) => void;
  setCascadeToCompanies: (cascade: boolean) => void;
  setChangedOnly: (changedOnly: boolean) => void;
  toggleEvidenceType: (evidenceType: ResearchEvidenceType) => void;
  clearEvidenceTypes: () => void;
  refreshTimeline: () => void;
  markReviewed: () => void;
  openEvidence: (item: ResearchEvidenceItem) => void;
  openEvidenceUrl: (url: string) => void;
  formatTimestamp: (value: string | null | undefined) => string;
};

export function ResearchScreen({
  companies,
  watchlists,
  watchlistMemberships,
  mode,
  selectedCompanyId,
  selectedWatchlistId,
  selectedWatchlistCompanyId,
  cascadeToCompanies,
  selectedEvidenceTypes,
  changedOnly,
  timeline,
  error,
  loading,
  reviewInFlight,
  setMode,
  setSelectedCompanyId,
  setSelectedWatchlistId,
  setSelectedWatchlistCompanyId,
  setCascadeToCompanies,
  setChangedOnly,
  toggleEvidenceType,
  clearEvidenceTypes,
  refreshTimeline,
  markReviewed,
  openEvidence,
  openEvidenceUrl,
  formatTimestamp,
}: ResearchScreenProps) {
  const { text } = useLocale();
  const selectedCompany = companies.find((company) => company.id === selectedCompanyId) ?? null;
  const selectedWatchlist = watchlists.find((watchlist) => watchlist.id === selectedWatchlistId) ?? null;
  const watchlistCompanyIds = selectedWatchlistId
    ? watchlistMemberships
        .filter((membership) => membership.watchlistId === selectedWatchlistId)
        .map((membership) => membership.companyId)
    : [];
  const watchlistCompanies = watchlistCompanyIds
    .map((companyId) => companies.find((company) => company.id === companyId))
    .filter((company): company is Company => Boolean(company));
  const selectedWatchlistCompany =
    watchlistCompanies.find((company) => company.id === selectedWatchlistCompanyId) ?? null;
  const selectedEvidenceTypeSet = new Set(selectedEvidenceTypes);
  const items = timeline?.items ?? [];
  const visibleItems =
    mode === "watchlist" && selectedWatchlistCompany
      ? items.filter((item) => item.companyId === selectedWatchlistCompany.id)
      : items;
  const companySummaryById = new Map(
    (timeline?.summary.companySummaries ?? []).map((summary) => [summary.companyId, summary]),
  );
  const markReviewedDisabled =
    reviewInFlight ||
    (mode === "company" ? !selectedCompany : !selectedWatchlist || watchlistCompanies.length === 0);

  return (
    <section className="feed-panel research-panel" aria-labelledby="research-title">
      <div className="panel-header research-header">
        <div>
          <h1 id="research-title">{text("Research")}</h1>
          <p>{text("Company evidence timeline and review checkpoint.")}</p>
        </div>
        <div className="research-header-actions">
          <Button className="compact-button" disabled={loading} onClick={refreshTimeline}>
            <RefreshCw size={15} />
            {loading ? text("Refreshing") : text("Refresh")}
          </Button>
          <Button className="compact-button" disabled={markReviewedDisabled} onClick={markReviewed}>
            <CheckCheck size={15} />
            {reviewInFlight ? text("Marking reviewed") : text("Mark reviewed")}
          </Button>
        </div>
      </div>

      <div className="research-workspace">
        <section className="research-toolbar" aria-label={text("Research filters")}>
          <div className="research-mode-switch" aria-label={text("Research mode")}>
            <button
              className={mode === "company" ? "research-filter active" : "research-filter"}
              type="button"
              onClick={() => setMode("company")}
            >
              {text("Company")}
            </button>
            <button
              className={mode === "watchlist" ? "research-filter active" : "research-filter"}
              type="button"
              onClick={() => setMode("watchlist")}
            >
              {text("Watchlist")}
            </button>
          </div>

          {mode === "company" ? (
            <label className="research-company-picker">
              <span>{text("Company")}</span>
              <select
                value={selectedCompanyId ?? ""}
                onChange={(event) => setSelectedCompanyId(event.target.value || null)}
              >
                {companies.length === 0 ? <option value="">{text("No companies tracked yet.")}</option> : null}
                {companies.map((company) => (
                  <option key={company.id} value={company.id}>
                    {company.qualifiedTicker} - {company.displayName}
                  </option>
                ))}
              </select>
            </label>
          ) : (
            <label className="research-company-picker">
              <span>{text("Watchlist")}</span>
              <select
                value={selectedWatchlistId ?? ""}
                onChange={(event) => setSelectedWatchlistId(event.target.value || null)}
              >
                {watchlists.length === 0 ? <option value="">{text("No watchlists yet.")}</option> : null}
                {watchlists.map((watchlist) => (
                  <option key={watchlist.id} value={watchlist.id}>
                    {watchlist.name}
                  </option>
                ))}
              </select>
            </label>
          )}

          <div className="research-filter-group" aria-label={text("Evidence type filters")}>
            {evidenceTypeOptions.map((option) => (
              <button
                className={selectedEvidenceTypeSet.has(option.value) ? "research-filter active" : "research-filter"}
                key={option.value}
                type="button"
                onClick={() => toggleEvidenceType(option.value)}
              >
                {text(option.label)}
              </button>
            ))}
            {selectedEvidenceTypes.length > 0 ? (
              <button className="research-filter research-filter-clear" type="button" onClick={clearEvidenceTypes}>
                {text("All evidence")}
              </button>
            ) : null}
          </div>

          <label className="research-toggle">
            <input
              checked={changedOnly}
              type="checkbox"
              onChange={(event) => setChangedOnly(event.target.checked)}
            />
            <span>{text("Changed since review")}</span>
          </label>
          {mode === "watchlist" ? (
            <label className="research-toggle">
              <input
                checked={cascadeToCompanies}
                type="checkbox"
                onChange={(event) => setCascadeToCompanies(event.target.checked)}
              />
              <span>{text("Also mark member companies reviewed")}</span>
            </label>
          ) : null}
        </section>

        <section className="research-summary" aria-label={text("Research review summary")}>
          <div>
            <span>{text("Evidence")}</span>
            <strong>{timeline?.summary.total ?? 0}</strong>
          </div>
          <div>
            <span>{text("Changed")}</span>
            <strong>{timeline?.summary.changedSinceReview ?? 0}</strong>
          </div>
          {mode === "watchlist" ? (
            <>
              <div>
                <span>{text("Companies")}</span>
                <strong>{timeline?.summary.memberCompanyCount ?? watchlistCompanies.length}</strong>
              </div>
              <div>
                <span>{text("Need review")}</span>
                <strong>{timeline?.summary.companiesWithChangedEvidence ?? 0}</strong>
              </div>
            </>
          ) : null}
          <div>
            <span>{text("Last reviewed")}</span>
            <strong>{timeline?.summary.lastReviewedAt ? formatTimestamp(timeline.summary.lastReviewedAt) : text("Never")}</strong>
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
        </section>

        {error ? <p className="error-text">{text("Research timeline failed")}: {error}</p> : null}

        <div className={mode === "watchlist" ? "research-review-layout" : undefined}>
          {mode === "watchlist" ? (
            <section className="research-company-queue" aria-label={text("Watchlist company review queue")}>
              {watchlistCompanies.map((company) => {
                const summary = companySummaryById.get(company.id);
                const isSelected = selectedWatchlistCompanyId === company.id;

                return (
                  <button
                    className={isSelected ? "research-company-queue-row selected" : "research-company-queue-row"}
                    key={company.id}
                    type="button"
                    onClick={() => setSelectedWatchlistCompanyId(company.id)}
                  >
                    <span>
                      <TickerLabel value={company.qualifiedTicker} />
                      <strong>{company.displayName}</strong>
                    </span>
                    <span>
                      {summary?.changedSinceReview ? (
                        <strong>{summary.changedSinceReview}</strong>
                      ) : (
                        <strong>0</strong>
                      )}
                      {text("Changed")}
                    </span>
                  </button>
                );
              })}
              {mode === "watchlist" && watchlistCompanies.length === 0 ? (
                <EmptyState>{text("Selected watchlist has no companies.")}</EmptyState>
              ) : null}
            </section>
          ) : null}

          <section className="research-timeline" aria-label={text("Evidence timeline")}>
            {visibleItems.map((item) => (
              <EvidenceRow
                changed={mode === "watchlist"
                  ? item.reviewState.changedSinceWatchlistReview
                  : item.reviewState.changedSinceCompanyReview}
                formatTimestamp={formatTimestamp}
                item={item}
                key={item.id}
                onOpen={openEvidence}
                onOpenUrl={openEvidenceUrl}
                text={text}
              />
            ))}
            {visibleItems.length === 0 ? (
              <EmptyState>
                {companies.length === 0
                  ? text("No companies tracked yet.")
                  : text("No evidence for selected filters.")}
              </EmptyState>
            ) : null}
          </section>
        </div>
      </div>
    </section>
  );
}

type EvidenceRowProps = {
  item: ResearchEvidenceItem;
  changed: boolean;
  text: (value: string) => string;
  formatTimestamp: (value: string | null | undefined) => string;
  onOpen: (item: ResearchEvidenceItem) => void;
  onOpenUrl: (url: string) => void;
};

function EvidenceRow({
  item,
  changed,
  text,
  formatTimestamp,
  onOpen,
  onOpenUrl,
}: EvidenceRowProps) {
  return (
    <article className="research-evidence-row">
      <div className="research-evidence-marker" aria-hidden="true" />
      <div className="research-evidence-main">
        <div className="research-evidence-meta">
          <span>{text(formatEvidenceType(item.evidenceType))}</span>
          <span>{text(formatTrustCategory(item.trustCategory))}</span>
          {changed ? <span className="research-change-pill">{text("Changed")}</span> : null}
          <time dateTime={item.occurredAt}>{formatTimestamp(item.occurredAt)}</time>
        </div>
        <h2>{text(formatEvidenceTitle(item))}</h2>
        {formatEvidenceSummary(item) ? <p>{text(formatEvidenceSummary(item) ?? "")}</p> : null}
        {formatEvidenceAttribution(item) ? (
          <span className="research-attribution">{text(formatEvidenceAttribution(item) ?? "")}</span>
        ) : null}
      </div>
      <div className="research-evidence-actions">
        <Button className="icon-button" onClick={() => onOpen(item)} title={text("Open evidence")}>
          <ArrowRight size={16} />
        </Button>
        {item.sourceUrl ? (
          <Button className="icon-button" onClick={() => onOpenUrl(item.sourceUrl ?? "")} title={text("Open source URL")}>
            <ExternalLink size={16} />
          </Button>
        ) : null}
      </div>
    </article>
  );
}

function formatEvidenceTitle(item: ResearchEvidenceItem) {
  if (item.evidenceType === "ai_analysis" && item.title === "AI analysis") {
    return "AI analysis";
  }

  return item.title;
}

function formatEvidenceSummary(item: ResearchEvidenceItem) {
  if (!item.summary) {
    return null;
  }

  if (item.evidenceType === "company_event") {
    return formatCompanyEventType(item.summary);
  }

  return item.summary;
}

function formatEvidenceAttribution(item: ResearchEvidenceItem) {
  if (!item.attribution) {
    return null;
  }

  if (item.attribution === "provider_gemini") {
    return formatAiProvider(item.attribution);
  }

  return item.attribution;
}

function formatEvidenceType(evidenceType: ResearchEvidenceType) {
  switch (evidenceType) {
    case "feed_item":
      return "Feed item";
    case "notebook_entry":
      return "Note";
    case "claim":
      return "Claim";
    case "transcript_segment":
      return "Transcript";
    case "company_event":
      return "Event";
    case "ai_analysis":
      return "AI analysis";
    case "research_question":
      return "Research question";
    case "reminder":
      return "Reminder";
    case "ai_brief":
      return "AI brief";
    case "digest":
      return "Digest";
  }
}

function formatTrustCategory(trustCategory: ResearchTrustCategory) {
  switch (trustCategory) {
    case "official_report":
      return "Official report";
    case "company_publication":
      return "Company publication";
    case "public_media":
      return "Market news";
    case "market_calendar":
      return "Calendar";
    case "transcript":
      return "Transcript";
    case "user_note":
      return "Personal note";
    case "ai_generated":
      return "AI analysis";
    case "unknown":
      return "Unknown";
  }
}
