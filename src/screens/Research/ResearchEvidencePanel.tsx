import type { KeyboardEvent, PointerEvent } from "react";
import { Building2 } from "lucide-react";
import type { Company } from "../../api/types";
import type {
  ResearchEvidenceItem,
  ResearchQuestion,
} from "../../api/researchTypes";
import type { ResearchMode } from "../../app/useResearchController";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { ActionButton, ActionRow, EmptyState, Figure, SectionHeader } from "../../ui";
import { EvidenceRow } from "./EvidenceRow";

type CompanySummary = {
  changedSinceReview: number;
};

type ResearchEvidencePanelProps = {
  mode: ResearchMode;
  companiesCount: number;
  visibleItems: ResearchEvidenceItem[];
  watchlistCompanies: Company[];
  selectedWatchlistCompanyId: string | null;
  watchlistQueueWidth: number;
  companySummaryById: Map<string, CompanySummary>;
  selectedQuestion: ResearchQuestion | null;
  linkedEvidenceKeys: Set<string>;
  hasActiveFilters: boolean;
  onClearFilters: () => void;
  setSelectedWatchlistCompanyId: (companyId: string | null) => void;
  openCompanyWorkspaceById: (companyId: string) => void;
  resizeResearchPanelWithKeyboard: (
    handle: "watchlistQueue",
    event: KeyboardEvent<HTMLDivElement>,
  ) => void;
  startResearchResize: (handle: "watchlistQueue", event: PointerEvent<HTMLDivElement>) => void;
  resizeResearchPanels: (event: PointerEvent<HTMLDivElement>) => void;
  stopResearchResize: (event: PointerEvent<HTMLDivElement>) => void;
  openEvidence: (item: ResearchEvidenceItem) => void;
  openEvidenceUrl: (url: string) => void;
  linkEvidence: (item: ResearchEvidenceItem) => void;
  text: (value: string) => string;
};

export function ResearchEvidencePanel({
  mode,
  companiesCount,
  visibleItems,
  watchlistCompanies,
  selectedWatchlistCompanyId,
  watchlistQueueWidth,
  companySummaryById,
  selectedQuestion,
  linkedEvidenceKeys,
  hasActiveFilters,
  onClearFilters,
  setSelectedWatchlistCompanyId,
  openCompanyWorkspaceById,
  resizeResearchPanelWithKeyboard,
  startResearchResize,
  resizeResearchPanels,
  stopResearchResize,
  openEvidence,
  openEvidenceUrl,
  linkEvidence,
  text,
}: ResearchEvidencePanelProps) {
  return (
    <div className={mode === "watchlist" ? "research-review-layout" : "research-review-region"}>
      {mode === "watchlist" ? (
        <>
          <div role="group" className="research-company-queue" aria-label={text("Watchlist company review queue")}>
            {watchlistCompanies.map((company) => {
              const summary = companySummaryById.get(company.id);
              const isSelected = selectedWatchlistCompanyId === company.id;

              return (
                // Outer container is a non-interactive div (not a <button>) so
                // the "Company" destination action (F4c S3: noun label,
                // kind="destination", dec. 4) can sit alongside the primary
                // select button without nesting interactives (ADR 0076 D9,
                // same pattern as CompaniesScreen's company-row).
                <div
                  className={isSelected ? "research-company-queue-row selected" : "research-company-queue-row"}
                  key={company.id}
                >
                  <button
                    className="research-company-queue-select"
                    data-action-kind="control"
                    type="button"
                    onClick={() => setSelectedWatchlistCompanyId(company.id)}
                  >
                    <span>
                      <TickerLabel value={company.qualifiedTicker} />
                      <strong>{company.displayName}</strong>
                    </span>
                    <span>
                      {summary?.changedSinceReview ? <strong>{summary.changedSinceReview}</strong> : <strong>0</strong>}
                      {text("Changed")}
                    </span>
                  </button>
                  <ActionRow className="research-company-queue-actions">
                    <ActionButton
                      aria-label={`${text("Company")}: ${company.displayName}`}
                      kind="destination"
                      onClick={() => openCompanyWorkspaceById(company.id)}
                      variant="ghost"
                    >
                      <Building2 size={14} />
                      {text("Company")}
                    </ActionButton>
                  </ActionRow>
                </div>
              );
            })}
            {watchlistCompanies.length === 0 ? (
              <EmptyState kind="quiet" reason={text("Selected watchlist has no companies.")} />
            ) : null}
          </div>
          <div
            aria-label={text("Resize watchlist company list")}
            aria-orientation="vertical"
            aria-valuemax={360}
            aria-valuemin={180}
            aria-valuenow={watchlistQueueWidth}
            className="research-resizer"
            onKeyDown={(event) => resizeResearchPanelWithKeyboard("watchlistQueue", event)}
            onPointerDown={(event) => startResearchResize("watchlistQueue", event)}
            onPointerMove={resizeResearchPanels}
            onPointerUp={stopResearchResize}
            role="separator"
            tabIndex={0}
            title={text("Drag to resize watchlist company list")}
          />
        </>
      ) : null}

      <div role="group" className="research-timeline-shell" aria-label={text("Evidence timeline")}>
        <SectionHeader
          className="research-section-evidence"
          description={text("Source items, notes, events, and transcripts for this scope.")}
          meta={<Figure value={visibleItems.length} />}
          title={text("Evidence")}
          variant="accent"
        />
        <div className="research-timeline">
          {visibleItems.map((item) => (
            <EvidenceRow
              changed={
                mode === "watchlist"
                  ? item.reviewState.changedSinceWatchlistReview
                  : item.reviewState.changedSinceCompanyReview
              }
              item={item}
              key={item.id}
              onOpen={openEvidence}
              onOpenUrl={openEvidenceUrl}
              onLink={linkEvidence}
              canLink={Boolean(
                selectedQuestion &&
                  !(item.evidenceType === "research_question" && item.sourceId === selectedQuestion.id) &&
                  !linkedEvidenceKeys.has(`${item.evidenceType}:${item.sourceId}`),
              )}
              text={text}
            />
          ))}
          {visibleItems.length === 0 ? (
            companiesCount === 0 ? (
              <EmptyState kind="quiet" reason={text("No companies tracked yet.")} />
            ) : (
              <EmptyState
                kind="quiet"
                reason={text("No evidence for selected filters.")}
                action={
                  hasActiveFilters ? (
                    <ActionButton kind="control" onClick={onClearFilters}>
                      {text("Clear filters")}
                    </ActionButton>
                  ) : undefined
                }
              />
            )
          ) : null}
        </div>
      </div>
    </div>
  );
}
