import { useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { ArrowRight, CheckCheck, ExternalLink, Link, Plus, RefreshCw, Sparkles, Trash2, X } from "lucide-react";
import type { Company, Watchlist, WatchlistMembership } from "../../api/types";
import type {
  EvidenceLink,
  ResearchBriefJob,
  ResearchEvidenceItem,
  ResearchEvidenceType,
  ResearchQuestion,
  ResearchQuestionStatus,
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
  questions: ResearchQuestion[];
  selectedQuestionId: string | null;
  questionTitle: string;
  questionBody: string;
  questionLinks: EvidenceLink[];
  briefJobs: ResearchBriefJob[];
  error: string | null;
  loading: boolean;
  reviewInFlight: boolean;
  questionInFlight: boolean;
  briefInFlight: boolean;
  setMode: (mode: ResearchMode) => void;
  setSelectedCompanyId: (companyId: string | null) => void;
  setSelectedWatchlistId: (watchlistId: string | null) => void;
  setSelectedWatchlistCompanyId: (companyId: string | null) => void;
  setSelectedQuestionId: (questionId: string | null) => void;
  setQuestionTitle: (title: string) => void;
  setQuestionBody: (body: string) => void;
  setCascadeToCompanies: (cascade: boolean) => void;
  setChangedOnly: (changedOnly: boolean) => void;
  toggleEvidenceType: (evidenceType: ResearchEvidenceType) => void;
  clearEvidenceTypes: () => void;
  refreshTimeline: () => void;
  markReviewed: () => void;
  createQuestion: () => void;
  updateQuestionStatus: (questionId: string, status: ResearchQuestionStatus) => void;
  deleteQuestion: (questionId: string) => void;
  linkEvidence: (item: ResearchEvidenceItem) => void;
  unlinkEvidence: (linkId: string) => void;
  startBrief: () => void;
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
  questions,
  selectedQuestionId,
  questionTitle,
  questionBody,
  questionLinks,
  briefJobs,
  error,
  loading,
  reviewInFlight,
  questionInFlight,
  briefInFlight,
  setMode,
  setSelectedCompanyId,
  setSelectedWatchlistId,
  setSelectedWatchlistCompanyId,
  setSelectedQuestionId,
  setQuestionTitle,
  setQuestionBody,
  setCascadeToCompanies,
  setChangedOnly,
  toggleEvidenceType,
  clearEvidenceTypes,
  refreshTimeline,
  markReviewed,
  createQuestion,
  updateQuestionStatus,
  deleteQuestion,
  linkEvidence,
  unlinkEvidence,
  startBrief,
  openEvidence,
  openEvidenceUrl,
  formatTimestamp,
}: ResearchScreenProps) {
  const { text } = useLocale();
  const workspaceRef = useRef<HTMLDivElement | null>(null);
  const resizeStartRef = useRef<{
    handle: "watchlistQueue" | "brief";
    clientX: number;
    watchlistQueueWidth: number;
    briefPanelWidth: number;
  } | null>(null);
  const [watchlistQueueWidth, setWatchlistQueueWidth] = useState(220);
  const [briefPanelWidth, setBriefPanelWidth] = useState<number | null>(null);
  const [questionDialogOpen, setQuestionDialogOpen] = useState(false);
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
  const selectedQuestion = questions.find((question) => question.id === selectedQuestionId) ?? null;
  const latestBriefJob = briefJobs[0] ?? null;
  const latestBrief = latestBriefJob?.brief ?? null;
  const briefRunning = briefJobs.some((job) => job.status === "queued" || job.status === "running");
  const latestBriefTitle =
    mode === "company" && selectedCompany
      ? `${text("Investor research brief")}: ${selectedCompany.displayName}`
      : mode === "watchlist" && selectedWatchlist
        ? `${text("Investor research brief")}: ${selectedWatchlist.name}`
        : text("Investor research brief");
  const latestBriefParagraphs =
    latestBrief?.contentMarkdown
      .split(/\n+/)
      .map((line) => line.replace(/^##\s*/, "").trim())
      .filter(Boolean) ?? [];
  const linkedEvidenceKeys = new Set(
    questionLinks.map((link) =>
      link.fromType === "research_question"
        ? `${link.toType}:${link.toId}`
        : `${link.fromType}:${link.fromId}`,
    ),
  );
  const visibleItems =
    mode === "watchlist" && selectedWatchlistCompany
      ? items.filter((item) => item.companyId === selectedWatchlistCompany.id)
      : items;
  const researchLayoutStyle = {
    "--research-watchlist-queue-width": `${watchlistQueueWidth}px`,
    ...(briefPanelWidth === null ? {} : { "--research-brief-panel-width": `${briefPanelWidth}px` }),
  } as CSSProperties;

  function clampPanelWidth(width: number, minWidth: number, maxShare: number) {
    const workspaceWidth = workspaceRef.current?.getBoundingClientRect().width ?? 0;
    const maxWidth = workspaceWidth > 0 ? Math.max(minWidth, workspaceWidth * maxShare) : minWidth * 2;

    return Math.round(Math.min(Math.max(width, minWidth), maxWidth));
  }

  function startResearchResize(handle: "watchlistQueue" | "brief", event: PointerEvent<HTMLDivElement>) {
    resizeStartRef.current = {
      handle,
      clientX: event.clientX,
      watchlistQueueWidth,
      briefPanelWidth: briefPanelWidth ?? 520,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function resizeResearchPanels(event: PointerEvent<HTMLDivElement>) {
    const resizeStart = resizeStartRef.current;

    if (!resizeStart || !event.currentTarget.hasPointerCapture(event.pointerId)) {
      return;
    }

    const delta = event.clientX - resizeStart.clientX;

    if (resizeStart.handle === "watchlistQueue") {
      setWatchlistQueueWidth(clampPanelWidth(resizeStart.watchlistQueueWidth + delta, 180, 0.28));
      return;
    }

    setBriefPanelWidth(clampPanelWidth(resizeStart.briefPanelWidth - delta, 420, 0.5));
  }

  function stopResearchResize(event: PointerEvent<HTMLDivElement>) {
    resizeStartRef.current = null;

    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function resizeResearchPanelWithKeyboard(
    handle: "watchlistQueue" | "brief",
    event: KeyboardEvent<HTMLDivElement>,
  ) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return;
    }

    event.preventDefault();
    const delta = event.key === "ArrowRight" ? 24 : -24;

    if (handle === "watchlistQueue") {
      setWatchlistQueueWidth((current) => clampPanelWidth(current + delta, 180, 0.28));
      return;
    }

    setBriefPanelWidth((current) => clampPanelWidth((current ?? 520) - delta, 420, 0.5));
  }

  function openCitationEvidence(evidenceType: ResearchEvidenceType, evidenceId: string) {
    const item = items.find(
      (current) => current.evidenceType === evidenceType && current.sourceId === evidenceId,
    );

    if (item) {
      openEvidence(item);
    }
  }

  function isCitationOpenable(evidenceType: ResearchEvidenceType, evidenceId: string) {
    return items.some((item) => item.evidenceType === evidenceType && item.sourceId === evidenceId);
  }

  function openQuestionDialog() {
    setQuestionTitle("");
    setQuestionBody("");
    setQuestionDialogOpen(true);
  }

  function submitQuestionDialog() {
    createQuestion();
    setQuestionDialogOpen(false);
  }

  const briefSection = (
    <section className="research-briefs" aria-label={text("AI research briefs")}>
      <div className="research-section-heading">
        <h2>{text("AI research briefs")}</h2>
        <span>{briefJobs.length}</span>
      </div>
      <div className="research-brief-actions">
        <Button
          className="compact-button"
          disabled={
            briefInFlight ||
            briefRunning ||
            (mode === "company" ? !selectedCompany : !selectedWatchlist)
          }
          onClick={startBrief}
        >
          <Sparkles size={15} />
          {briefRunning ? text("Generating") : text("Generate brief")}
        </Button>
      </div>
      {latestBrief ? (
        <article className="research-brief-card">
          <div className="research-brief-card-header">
            <div>
              <h3>{latestBriefTitle}</h3>
              <p className="research-brief-summary">{latestBrief.summary}</p>
            </div>
            <span>{formatTimestamp(latestBrief.generatedAt)}</span>
          </div>
          <div className="research-brief-content">
            {latestBriefParagraphs.map((line, index) => (
              <p key={`${latestBrief.id}-${index}`}>{line}</p>
            ))}
          </div>
          <details className="research-brief-citations">
            <summary>{text("Citations")} ({latestBrief.citations.length})</summary>
            <div className="research-brief-citation-list">
              {latestBrief.citations.map((citation) => (
                <button
                  className="research-brief-citation"
                  disabled={!isCitationOpenable(citation.evidenceType, citation.evidenceId)}
                  key={citation.id}
                  type="button"
                  onClick={() => openCitationEvidence(citation.evidenceType, citation.evidenceId)}
                >
                  <strong>{citation.citationKey}</strong>
                  {citation.label}
                </button>
              ))}
            </div>
          </details>
          <div className="research-brief-provenance">
            <span>{text("Generated")} {formatTimestamp(latestBrief.generatedAt)}</span>
            <span>{text(formatAiProvider(latestBrief.providerId))}</span>
            <span>{latestBrief.model}</span>
          </div>
        </article>
      ) : latestBriefJob?.status === "failed" ? (
        <p className="error-text">{latestBriefJob.error ?? text("Research brief failed")}</p>
      ) : (
        <EmptyState>{text("No research brief generated yet.")}</EmptyState>
      )}
    </section>
  );
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

      <div className="research-workspace" ref={workspaceRef}>
        <section className="research-toolbar" aria-label={text("Research filters")}>
          <div className="research-mode-switch" aria-label={text("Research mode")}>
            <button
              className={mode === "company" ? "research-mode-option active" : "research-mode-option"}
              type="button"
              onClick={() => setMode("company")}
            >
              {text("Company")}
            </button>
            <button
              className={mode === "watchlist" ? "research-mode-option active" : "research-mode-option"}
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

        <div className="research-main-layout" style={researchLayoutStyle}>
          <div className={mode === "watchlist" ? "research-main-stack watchlist" : "research-main-stack company"}>
            {mode === "company" ? (
              <section className="research-questions" aria-label={text("Research questions")}>
                <div className="research-question-strip">
                  <div className="research-section-heading">
                    <h2>{text("Research questions")}</h2>
                    <span>{questions.length}</span>
                  </div>
                  <Button className="compact-button" disabled={!selectedCompany} onClick={openQuestionDialog}>
                    <Plus size={15} />
                    {text("Add question")}
                  </Button>
                </div>
                <div className="research-question-card-list">
                  {questions.map((question) => (
                    <div
                      className={question.id === selectedQuestionId ? "research-question-row selected" : "research-question-row"}
                      key={question.id}
                    >
                      <button
                        className="research-question-row-main"
                        type="button"
                        onClick={() => setSelectedQuestionId(question.id)}
                      >
                        <strong>{question.title}</strong>
                        <span>{text(formatQuestionStatus(question.status))}</span>
                      </button>
                      <button
                        aria-label={text("Delete research question")}
                        className="research-question-delete"
                        disabled={questionInFlight}
                        title={text("Delete research question")}
                        type="button"
                        onClick={() => deleteQuestion(question.id)}
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  ))}
                  {questions.length === 0 ? <EmptyState>{text("No research questions yet.")}</EmptyState> : null}
                </div>
                {selectedQuestion ? (
                  <div className="research-active-question">
                    <div className="research-active-question-main">
                      <span>{text("Active question")}</span>
                      <strong>{selectedQuestion.title}</strong>
                      {selectedQuestion.body ? <p>{selectedQuestion.body}</p> : null}
                      <div className="research-linked-evidence">
                        <span>{text("Linked evidence")}</span>
                        {questionLinks.map((link) => (
                          <button key={link.id} type="button" onClick={() => unlinkEvidence(link.id)}>
                            <X size={13} />
                            {text(formatEvidenceType(link.fromType === "research_question" ? link.toType : link.fromType))}
                          </button>
                        ))}
                        {questionLinks.length === 0 ? <small>{text("No linked evidence yet.")}</small> : null}
                      </div>
                    </div>
                    <div className="research-question-actions">
                      <Button
                        className="compact-button"
                        disabled={questionInFlight || selectedQuestion.status === "answered"}
                        onClick={() => updateQuestionStatus(selectedQuestion.id, "answered")}
                      >
                        {text("Answered")}
                      </Button>
                      <Button
                        className="compact-button"
                        disabled={questionInFlight || selectedQuestion.status === "closed"}
                        onClick={() => updateQuestionStatus(selectedQuestion.id, "closed")}
                      >
                        {text("Close")}
                      </Button>
                      <Button
                        className="compact-button"
                        disabled={questionInFlight || selectedQuestion.status === "open"}
                        onClick={() => updateQuestionStatus(selectedQuestion.id, "open")}
                      >
                        {text("Reopen")}
                      </Button>
                      <Button className="compact-button" onClick={() => setSelectedQuestionId(null)}>
                        {text("Clear selection")}
                      </Button>
                    </div>
                  </div>
                ) : null}
              </section>
            ) : null}

            <div className={mode === "watchlist" ? "research-review-layout" : "research-review-region"}>
              {mode === "watchlist" ? (
                <>
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
                  <EmptyState>
                    {companies.length === 0
                      ? text("No companies tracked yet.")
                      : text("No evidence for selected filters.")}
                  </EmptyState>
                ) : null}
              </section>
            </div>
          </div>

          <div
            aria-label={text("Resize AI research brief panel")}
            aria-orientation="vertical"
            aria-valuemax={720}
            aria-valuemin={420}
            aria-valuenow={briefPanelWidth ?? 520}
            className="research-resizer"
            onKeyDown={(event) => resizeResearchPanelWithKeyboard("brief", event)}
            onPointerDown={(event) => startResearchResize("brief", event)}
            onPointerMove={resizeResearchPanels}
            onPointerUp={stopResearchResize}
            role="separator"
            tabIndex={0}
            title={text("Drag to resize AI research brief panel")}
          />
          <aside className="research-aside">
            {briefSection}
          </aside>
        </div>
        {questionDialogOpen ? (
          <div className="research-dialog-backdrop" role="presentation">
            <form
              aria-label={text("Add research question")}
              className="research-question-dialog"
              role="dialog"
              onSubmit={(event) => {
                event.preventDefault();
                submitQuestionDialog();
              }}
            >
              <div className="research-dialog-header">
                <h2>{text("Add research question")}</h2>
                <button
                  aria-label={text("Close")}
                  className="research-question-delete"
                  type="button"
                  onClick={() => setQuestionDialogOpen(false)}
                >
                  <X size={14} />
                </button>
              </div>
              <label>
                <span>{text("Question title")}</span>
                <input
                  aria-label={text("Question title")}
                  placeholder={text("Question title")}
                  value={questionTitle}
                  onChange={(event) => setQuestionTitle(event.target.value)}
                />
              </label>
              <label>
                <span>{text("Question context")}</span>
                <textarea
                  aria-label={text("Question context")}
                  placeholder={text("Question context")}
                  value={questionBody}
                  onChange={(event) => setQuestionBody(event.target.value)}
                />
              </label>
              <div className="research-dialog-actions">
                <Button className="compact-button" type="button" onClick={() => setQuestionDialogOpen(false)}>
                  {text("Cancel")}
                </Button>
                <Button className="compact-button" disabled={questionInFlight || !questionTitle.trim()} type="submit">
                  {text("Save question")}
                </Button>
              </div>
            </form>
          </div>
        ) : null}
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
  onLink: (item: ResearchEvidenceItem) => void;
  canLink: boolean;
};

function EvidenceRow({
  item,
  changed,
  text,
  formatTimestamp,
  onOpen,
  onOpenUrl,
  onLink,
  canLink,
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
        {canLink ? (
          <Button className="icon-button" onClick={() => onLink(item)} title={text("Link evidence")}>
            <Link size={16} />
          </Button>
        ) : null}
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

function formatQuestionStatus(status: ResearchQuestionStatus) {
  switch (status) {
    case "open":
      return "Open";
    case "answered":
      return "Answered";
    case "closed":
      return "Closed";
  }
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
