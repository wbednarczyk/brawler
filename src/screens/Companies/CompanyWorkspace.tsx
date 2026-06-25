import { useEffect, useRef, useState } from "react";
import {
  BookOpenText,
  CheckCircle2,
  FileText,
  Gauge,
  Inbox,
  LayoutGrid,
  Pin,
  PinOff,
  TrendingUp,
  Video,
} from "lucide-react";
import type { Company } from "../../api/types";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";
import { CompanyBackfillPanel } from "../../shared/components/CompanyBackfillPanel";
import { CompanyClaimsPanel } from "../../shared/components/CompanyClaimsPanel";
import { CompanyReportDocumentsPanel } from "../../shared/components/CompanyReportDocumentsPanel";
import { ReportDiffPanel } from "./ReportDiffPanel";
import { QualityPanel } from "../../shared/components/QualityPanel";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { usePinnedCompanyIds } from "../../app/state/SettingsContext";
import {
  Button,
  EmptyState,
  InfoGrid,
  SegmentedControl,
  SegmentedControlOption,
} from "../../ui";
import type { CompaniesScreenProps } from "./CompaniesScreen";
import { FundamentalsPanel } from "./FundamentalsPanel";
import { CompanyFeedSection } from "./CompanyFeedSection";
import { CompanyNotebookSection } from "./CompanyNotebookSection";
import type { FinancialFactForm, FundamentalsForm } from "../../app/useFundamentalsController";

type CompanyWorkspaceProps = Pick<
  CompaniesScreenProps,
  | "membershipsByCompany"
  | "selectedCompanyFeedStats"
  | "companyWorkspaceTab"
  | "selectedCompanyFeedItems"
  | "selectedCompanyFeedItem"
  | "aiAnalysisJobsByFeedItemId"
  | "aiAnalysisErrorByFeedItemId"
  | "aiAnalysisRequestInFlightByFeedItemId"
  | "selectedCompanyNotebookEntries"
  | "isNotebookComposerOpen"
  | "notebookForm"
  | "selectedNotebookEntryId"
  | "selectedNotebookEntry"
  | "notebookEditMode"
  | "notebookEditForm"
  | "isNotebookEditDirty"
  | "notebookError"
  | "setCompanyWorkspaceTab"
  | "toggleCompanyFeedItem"
  | "selectCompanyFeedItemFromKeyboard"
  | "updateFeedItemState"
  | "inspectCompanyFeedItem"
  | "openFeedItemNoteDraft"
  | "startFeedItemAiAnalysis"
  | "retryFeedItemAiAnalysis"
  | "openCompanyInboxFilter"
  | "setNotebookComposerOpen"
  | "updateNotebookForm"
  | "createNotebookEntry"
  | "setSelectedNotebookEntryId"
  | "saveNotebookEntry"
  | "cancelNotebookEdit"
  | "setNotebookEditMode"
  | "updateNotebookEditForm"
  | "NotebookDateField"
  | "NotebookQuarterField"
  | "MarkdownNoteBody"
  | "renderNotebookOrigins"
  | "formatTimestamp"
  | "feedItemSummary"
> & {
  selectedCompany: Company;
  onTogglePin: () => void;
  onOpenAdvancedLayout: () => void;
  autoFocusOnOpen: boolean;
  onAutoFocusHandled: () => void;
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
  fundamentalsForm: FundamentalsForm;
  financialFactForm: FinancialFactForm;
  selectedFinancialFactId: string | null;
  isFinancialFactEditMode: boolean;
  fundamentalsError: string | null;
  fundamentalsLoadError: string | null;
  createFinancialPeriod: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  saveFinancialFact: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  deleteFinancialFact: (id: string) => Promise<void>;
  selectFinancialFact: (id: string) => void;
  startEditingFinancialFact: () => void;
  cancelEditingFinancialFact: () => void;
  updateFundamentalsForm: (field: keyof FundamentalsForm, value: string) => void;
  updateFinancialFactForm: (field: keyof FinancialFactForm, value: string) => void;
};

export function CompanyWorkspace({
  selectedCompany,
  onTogglePin,
  onOpenAdvancedLayout,
  autoFocusOnOpen,
  onAutoFocusHandled,
  membershipsByCompany,
  selectedCompanyFeedStats,
  companyWorkspaceTab,
  selectedCompanyFeedItems,
  selectedCompanyFeedItem,
  aiAnalysisJobsByFeedItemId,
  aiAnalysisErrorByFeedItemId,
  aiAnalysisRequestInFlightByFeedItemId,
  selectedCompanyNotebookEntries,
  isNotebookComposerOpen,
  notebookForm,
  selectedNotebookEntry,
  notebookEditMode,
  notebookEditForm,
  isNotebookEditDirty,
  notebookError,
  setCompanyWorkspaceTab,
  toggleCompanyFeedItem,
  selectCompanyFeedItemFromKeyboard,
  updateFeedItemState,
  inspectCompanyFeedItem,
  openFeedItemNoteDraft,
  startFeedItemAiAnalysis,
  retryFeedItemAiAnalysis,
  openCompanyInboxFilter,
  setNotebookComposerOpen,
  updateNotebookForm,
  createNotebookEntry,
  setSelectedNotebookEntryId,
  saveNotebookEntry,
  cancelNotebookEdit,
  setNotebookEditMode,
  updateNotebookEditForm,
  NotebookDateField,
  NotebookQuarterField,
  MarkdownNoteBody,
  renderNotebookOrigins,
  formatTimestamp,
  feedItemSummary,
  financialPeriods,
  financialFacts,
  kpiDefinitions,
  fundamentalsForm,
  financialFactForm,
  selectedFinancialFactId,
  isFinancialFactEditMode,
  fundamentalsError,
  fundamentalsLoadError,
  createFinancialPeriod,
  saveFinancialFact,
  deleteFinancialFact,
  selectFinancialFact,
  startEditingFinancialFact,
  cancelEditingFinancialFact,
  updateFundamentalsForm,
  updateFinancialFactForm,
}: CompanyWorkspaceProps) {
  const { text } = useLocale();
  const isPinned = usePinnedCompanyIds().includes(selectedCompany.id);
  const selectedCompanyMemberships = membershipsByCompany[selectedCompany.id] ?? [];
  const workspaceRef = useRef<HTMLElement>(null);
  // Bumped when a backfill finishes so the report-documents panel reloads.
  const [reportDocsReloadKey, setReportDocsReloadKey] = useState(0);

  // The workspace expands inline beneath the selected company row inside the
  // scrollable list, so opening it via "Open company" from a feed item can land
  // it off-screen. Only that cross-screen path requests auto-focus; in-list
  // click/keyboard navigation must not have focus yanked to the workspace.
  useEffect(() => {
    if (!autoFocusOnOpen) return;
    const node = workspaceRef.current;
    onAutoFocusHandled();
    if (!node) return;
    // Scroll only the internal company-list container — not via scrollIntoView,
    // which would scroll every scrollable ancestor (including the app shell) and
    // shove the whole UI. Confine the movement to the list that owns the row.
    const scroller = node.closest<HTMLElement>('[data-company-list="true"]');
    if (scroller && typeof scroller.scrollTo === "function") {
      const nodeTop = node.getBoundingClientRect().top;
      const scrollerTop = scroller.getBoundingClientRect().top;
      scroller.scrollTo({ top: scroller.scrollTop + nodeTop - scrollerTop - 12, behavior: "smooth" });
    } else {
      node.scrollIntoView?.({ block: "nearest" });
    }
    node.focus?.({ preventScroll: true });
  }, [autoFocusOnOpen, onAutoFocusHandled]);

  return (
    <section
      aria-label={text("Company workspace")}
      className="company-workspace"
      ref={workspaceRef}
      tabIndex={-1}
    >
      <div className="company-workspace-header">
        <div>
          <span className="eyebrow">{text("Company workspace")}</span>
          <h2>
            <TickerLabel value={selectedCompany.qualifiedTicker} />
            <Button
              className="company-pin-toggle"
              onClick={onTogglePin}
              type="button"
              variant={isPinned ? "secondary" : "ghost"}
              aria-pressed={isPinned}
              title={isPinned ? text("Unpin from sidebar") : text("Pin to sidebar")}
            >
              {isPinned ? <PinOff size={14} /> : <Pin size={14} />}
              {isPinned ? text("Pinned") : text("Pin")}
            </Button>
            <Button
              className="company-advanced-layout"
              onClick={onOpenAdvancedLayout}
              type="button"
              variant="ghost"
              title={text("Open this company in the advanced dockview layout")}
            >
              <LayoutGrid size={14} />
              {text("Advanced layout")}
            </Button>
          </h2>
          <p>{selectedCompany.displayName}</p>
        </div>
        <div className="company-workspace-header-side">
          <div className="company-workspace-meta" aria-label={text("Selected company metadata")}>
            <span>{selectedCompany.exchange}</span>
            <span>{selectedCompany.isin ?? text("No ISIN")}</span>
            <span>{selectedCompanyFeedStats.total} {text("feed")}</span>
            <span>{selectedCompanyFeedStats.unread} {text("unread")}</span>
            <span>{selectedCompanyFeedStats.saved} {text("saved")}</span>
            {selectedCompanyMemberships.map((membership) => (
              <span key={membership.watchlistId}>{membership.watchlistName}</span>
            ))}
          </div>
          <CompanyBackfillPanel
            companyId={selectedCompany.id}
            onComplete={() => setReportDocsReloadKey((key) => key + 1)}
          />
        </div>
      </div>
    
      <SegmentedControl ariaLabel={text("Company workspace tabs")} className="company-tabs">
        {(["Feed", "Notebook", "Claims", "Transcripts", "Fundamentals", "Quality", "Metadata"] as const).map(
          (tab) => {
            const TabIcon =
              tab === "Feed"
                ? Inbox
                : tab === "Notebook"
                  ? BookOpenText
                  : tab === "Claims"
                    ? CheckCircle2
                    : tab === "Transcripts"
                      ? Video
                      : tab === "Fundamentals"
                        ? TrendingUp
                        : tab === "Quality"
                          ? Gauge
                          : FileText;

            return (
              <SegmentedControlOption active={companyWorkspaceTab === tab} key={tab} onClick={() => setCompanyWorkspaceTab(tab)}>
                <TabIcon size={14} />
                {text(tab)}
              </SegmentedControlOption>
            );
          },
        )}
      </SegmentedControl>
    
      {companyWorkspaceTab === "Feed" ? (
        <CompanyFeedSection
          company={selectedCompany}
          feedItems={selectedCompanyFeedItems}
          selectedFeedItem={selectedCompanyFeedItem}
          aiAnalysisJobsByFeedItemId={aiAnalysisJobsByFeedItemId}
          aiAnalysisErrorByFeedItemId={aiAnalysisErrorByFeedItemId}
          aiAnalysisRequestInFlightByFeedItemId={aiAnalysisRequestInFlightByFeedItemId}
          toggleFeedItem={toggleCompanyFeedItem}
          selectFeedItemFromKeyboard={selectCompanyFeedItemFromKeyboard}
          updateFeedItemState={updateFeedItemState}
          inspectFeedItem={inspectCompanyFeedItem}
          openFeedItemNoteDraft={openFeedItemNoteDraft}
          startFeedItemAiAnalysis={startFeedItemAiAnalysis}
          retryFeedItemAiAnalysis={retryFeedItemAiAnalysis}
          openInboxFilter={openCompanyInboxFilter}
          formatTimestamp={formatTimestamp}
          feedItemSummary={feedItemSummary}
        />
      ) : null}
    
      {companyWorkspaceTab === "Notebook" ? (
        <CompanyNotebookSection
          company={selectedCompany}
          notebookEntries={selectedCompanyNotebookEntries}
          isComposerOpen={isNotebookComposerOpen}
          notebookForm={notebookForm}
          selectedNotebookEntry={selectedNotebookEntry}
          notebookEditMode={notebookEditMode}
          notebookEditForm={notebookEditForm}
          isNotebookEditDirty={isNotebookEditDirty}
          notebookError={notebookError}
          setComposerOpen={setNotebookComposerOpen}
          updateNotebookForm={updateNotebookForm}
          createNotebookEntry={createNotebookEntry}
          setSelectedNotebookEntryId={setSelectedNotebookEntryId}
          saveNotebookEntry={saveNotebookEntry}
          cancelNotebookEdit={cancelNotebookEdit}
          setNotebookEditMode={setNotebookEditMode}
          updateNotebookEditForm={updateNotebookEditForm}
          NotebookDateField={NotebookDateField}
          NotebookQuarterField={NotebookQuarterField}
          MarkdownNoteBody={MarkdownNoteBody}
          renderNotebookOrigins={renderNotebookOrigins}
        />
      ) : null}

    
      {companyWorkspaceTab === "Claims" ? (
        <div className="company-tab-panel claims-panel" aria-label={text("Company claims")}>
          <CompanyClaimsPanel companyId={selectedCompany.id} />
        </div>
      ) : null}
    
      {companyWorkspaceTab === "Transcripts" ? (
        <EmptyState className="company-tab-panel">
          {text("YouTube transcript workflows start in Milestone 7.")}
        </EmptyState>
      ) : null}

      {companyWorkspaceTab === "Fundamentals" ? (
        <FundamentalsPanel
          companyId={selectedCompany.id}
          financialPeriods={financialPeriods}
          financialFacts={financialFacts}
          kpiDefinitions={kpiDefinitions}
          fundamentalsForm={fundamentalsForm}
          financialFactForm={financialFactForm}
          selectedFinancialFactId={selectedFinancialFactId}
          isFinancialFactEditMode={isFinancialFactEditMode}
          fundamentalsError={fundamentalsError}
          fundamentalsLoadError={fundamentalsLoadError}
          createFinancialPeriod={createFinancialPeriod}
          saveFinancialFact={saveFinancialFact}
          deleteFinancialFact={deleteFinancialFact}
          selectFinancialFact={selectFinancialFact}
          startEditingFinancialFact={startEditingFinancialFact}
          cancelEditingFinancialFact={cancelEditingFinancialFact}
          updateFundamentalsForm={updateFundamentalsForm}
          updateFinancialFactForm={updateFinancialFactForm}
        />
      ) : null}

      {companyWorkspaceTab === "Fundamentals" ? (
        <CompanyReportDocumentsPanel
          companyId={selectedCompany.id}
          reloadKey={reportDocsReloadKey}
        />
      ) : null}

      {companyWorkspaceTab === "Fundamentals" ? (
        <ReportDiffPanel companyId={selectedCompany.id} />
      ) : null}

      {companyWorkspaceTab === "Quality" ? (
        <QualityPanel companyId={selectedCompany.id} />
      ) : null}

      {companyWorkspaceTab === "Metadata" ? (
        <InfoGrid
          ariaLabel={text("Company metadata")}
          className="company-tab-panel metadata-grid"
          items={[
            {
              label: text("Qualified ticker"),
              value: <TickerLabel value={selectedCompany.qualifiedTicker} />,
            },
            { label: text("Exchange"), value: selectedCompany.exchange },
            { label: text("Ticker"), value: selectedCompany.ticker },
            { label: "ISIN", value: selectedCompany.isin ?? text("Not set") },
            { label: "CIK", value: selectedCompany.cik ?? text("Not set") },
            { label: "LEI", value: selectedCompany.lei ?? text("Not set") },
          ]}
        />
      ) : null}
    </section>
  );
}
