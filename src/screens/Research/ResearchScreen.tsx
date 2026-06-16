import { useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import {
  CheckCheck,
  RefreshCw,
} from "lucide-react";
import type { Company, Watchlist, WatchlistMembership } from "../../api/types";
import type {
  EvidenceLink,
  ResearchBriefJob,
  ResearchDigestJob,
  ResearchEvidenceItem,
  ResearchEvidenceType,
  ResearchQuestion,
  ResearchQuestionStatus,
  ResearchReminder,
  ResearchTimelineResult,
} from "../../api/researchTypes";
import type { ResearchMode } from "../../app/useResearchController";
import { useLocale } from "../../shared/locale";
import { ActionRow, Button, PanelHeader } from "../../ui";
import { ResearchAiPanel } from "./ResearchAiPanel";
import { AddQuestionDialog, AddReminderDialog } from "./ResearchDialogs";
import { ResearchEvidencePanel } from "./ResearchEvidencePanel";
import { ResearchQuestionsPanel } from "./ResearchQuestionsPanel";
import { ResearchRemindersPanel } from "./ResearchRemindersPanel";
import { ResearchScopeBar } from "./ResearchScopeBar";
import { ResearchSummaryStrip } from "./ResearchSummaryStrip";

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
  digestJobs: ResearchDigestJob[];
  reminders: ResearchReminder[];
  error: string | null;
  loading: boolean;
  reviewInFlight: boolean;
  questionInFlight: boolean;
  briefInFlight: boolean;
  digestInFlight: boolean;
  reminderInFlight: boolean;
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
  startDigest: () => void;
  createReminder: (title: string, body: string, dueAt: string | null) => void;
  completeReminder: (reminderId: string) => void;
  snoozeReminder: (reminderId: string) => void;
  reopenReminder: (reminderId: string) => void;
  deleteReminder: (reminderId: string) => void;
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
  digestJobs,
  reminders,
  error,
  loading,
  reviewInFlight,
  questionInFlight,
  briefInFlight,
  digestInFlight,
  reminderInFlight,
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
  startDigest,
  createReminder,
  completeReminder,
  snoozeReminder,
  reopenReminder,
  deleteReminder,
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
  const [reminderDialogOpen, setReminderDialogOpen] = useState(false);
  const [reminderTitle, setReminderTitle] = useState("");
  const [reminderBody, setReminderBody] = useState("");
  const [reminderDueAt, setReminderDueAt] = useState("");
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
  const items = timeline?.items ?? [];
  const selectedQuestion = questions.find((question) => question.id === selectedQuestionId) ?? null;
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

  function openQuestionDialog() {
    setQuestionTitle("");
    setQuestionBody("");
    setQuestionDialogOpen(true);
  }

  function submitQuestionDialog() {
    createQuestion();
    setQuestionDialogOpen(false);
  }

  function openReminderDialog() {
    setReminderTitle("");
    setReminderBody("");
    setReminderDueAt("");
    setReminderDialogOpen(true);
  }

  function submitReminderDialog() {
    createReminder(reminderTitle, reminderBody, reminderDueAt || null);
    setReminderDialogOpen(false);
  }

  const companySummaryById = new Map(
    (timeline?.summary.companySummaries ?? []).map((summary) => [summary.companyId, summary]),
  );
  const markReviewedDisabled =
    reviewInFlight ||
    (mode === "company" ? !selectedCompany : !selectedWatchlist || watchlistCompanies.length === 0);

  return (
    <section className="feed-panel research-panel" aria-labelledby="research-title">
      <PanelHeader
        className="research-header"
        title={text("Research")}
        description={text("Company evidence timeline and review checkpoint.")}
        titleId="research-title"
        actions={
          <ActionRow className="research-header-actions">
            <Button className="compact-button" disabled={loading} onClick={refreshTimeline}>
              <RefreshCw size={15} />
              {loading ? text("Refreshing") : text("Refresh")}
            </Button>
            <Button className="compact-button" disabled={markReviewedDisabled} onClick={markReviewed}>
              <CheckCheck size={15} />
              {reviewInFlight ? text("Marking reviewed") : text("Mark reviewed")}
            </Button>
          </ActionRow>
        }
      />

      <div className="research-workspace" ref={workspaceRef}>
        <ResearchScopeBar
          cascadeToCompanies={cascadeToCompanies}
          changedOnly={changedOnly}
          clearEvidenceTypes={clearEvidenceTypes}
          companies={companies}
          mode={mode}
          selectedCompanyId={selectedCompanyId}
          selectedEvidenceTypes={selectedEvidenceTypes}
          selectedWatchlistId={selectedWatchlistId}
          setCascadeToCompanies={setCascadeToCompanies}
          setChangedOnly={setChangedOnly}
          setMode={setMode}
          setSelectedCompanyId={setSelectedCompanyId}
          setSelectedWatchlistId={setSelectedWatchlistId}
          text={text}
          toggleEvidenceType={toggleEvidenceType}
          watchlists={watchlists}
        />

        <ResearchSummaryStrip
          formatTimestamp={formatTimestamp}
          mode={mode}
          selectedCompany={selectedCompany}
          selectedWatchlist={selectedWatchlist}
          text={text}
          timeline={timeline}
          watchlistCompanyCount={watchlistCompanies.length}
        />

        {error ? <p className="error-text">{text("Research timeline failed")}: {error}</p> : null}

        <div className="research-main-layout" style={researchLayoutStyle}>
          <div className={mode === "watchlist" ? "research-main-stack watchlist" : "research-main-stack company"}>
            <ResearchRemindersPanel
              canAdd={mode === "company" ? Boolean(selectedCompany) : Boolean(selectedWatchlist)}
              completeReminder={completeReminder}
              deleteReminder={deleteReminder}
              formatTimestamp={formatTimestamp}
              onAdd={openReminderDialog}
              reminderInFlight={reminderInFlight}
              reminders={reminders}
              reopenReminder={reopenReminder}
              snoozeReminder={snoozeReminder}
              text={text}
            />
            {mode === "company" ? (
              <ResearchQuestionsPanel
                canAdd={Boolean(selectedCompany)}
                deleteQuestion={deleteQuestion}
                onAdd={openQuestionDialog}
                questionInFlight={questionInFlight}
                questionLinks={questionLinks}
                questions={questions}
                selectedQuestion={selectedQuestion}
                selectedQuestionId={selectedQuestionId}
                setSelectedQuestionId={setSelectedQuestionId}
                text={text}
                unlinkEvidence={unlinkEvidence}
                updateQuestionStatus={updateQuestionStatus}
              />
            ) : null}
            <ResearchEvidencePanel
              companiesCount={companies.length}
              companySummaryById={companySummaryById}
              formatTimestamp={formatTimestamp}
              linkEvidence={linkEvidence}
              linkedEvidenceKeys={linkedEvidenceKeys}
              mode={mode}
              openEvidence={openEvidence}
              openEvidenceUrl={openEvidenceUrl}
              resizeResearchPanelWithKeyboard={resizeResearchPanelWithKeyboard}
              resizeResearchPanels={resizeResearchPanels}
              selectedQuestion={selectedQuestion}
              selectedWatchlistCompanyId={selectedWatchlistCompanyId}
              setSelectedWatchlistCompanyId={setSelectedWatchlistCompanyId}
              startResearchResize={startResearchResize}
              stopResearchResize={stopResearchResize}
              text={text}
              visibleItems={visibleItems}
              watchlistCompanies={watchlistCompanies}
              watchlistQueueWidth={watchlistQueueWidth}
            />
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
            <ResearchAiPanel
              briefInFlight={briefInFlight}
              briefJobs={briefJobs}
              digestInFlight={digestInFlight}
              digestJobs={digestJobs}
              formatTimestamp={formatTimestamp}
              items={items}
              mode={mode}
              openEvidence={openEvidence}
              selectedCompany={selectedCompany}
              selectedWatchlist={selectedWatchlist}
              startBrief={startBrief}
              startDigest={startDigest}
              text={text}
            />
          </aside>
        </div>
        {questionDialogOpen ? (
          <AddQuestionDialog
            onCancel={() => setQuestionDialogOpen(false)}
            onSubmit={submitQuestionDialog}
            questionBody={questionBody}
            questionInFlight={questionInFlight}
            questionTitle={questionTitle}
            setQuestionBody={setQuestionBody}
            setQuestionTitle={setQuestionTitle}
            text={text}
          />
        ) : null}
        {reminderDialogOpen ? (
          <AddReminderDialog
            onCancel={() => setReminderDialogOpen(false)}
            onSubmit={submitReminderDialog}
            reminderBody={reminderBody}
            reminderDueAt={reminderDueAt}
            reminderInFlight={reminderInFlight}
            reminderTitle={reminderTitle}
            setReminderBody={setReminderBody}
            setReminderDueAt={setReminderDueAt}
            setReminderTitle={setReminderTitle}
            text={text}
          />
        ) : null}
      </div>
    </section>
  );
}
