import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { LocateFixed, Plus, Trash2 } from "lucide-react";
import type { AiAnalysisJob, Company, CompanyForm, CompanyRegistryEntry, FeedItem, NotebookEntry, Watchlist, WatchlistMembership } from "../../api/types";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { Button, ClearButton, DenseRow, EmptyState, PanelHeader, SearchField } from "../../ui";
import type { FinancialFactForm, FundamentalsForm } from "../../app/useFundamentalsController";
import type {
  MarkdownNoteBodyProps,
  NotebookDateLikeFieldProps,
  NotebookForm,
} from "../../shared/types/notebook";
import type { CompanyWorkspaceTab } from "./companyTypes";
import { CompanyWorkspace } from "./CompanyWorkspace";

type CompanyFieldRefs = MutableRefObject<Record<keyof CompanyForm, HTMLInputElement | null>>;

export type CompaniesScreenProps = {
  watchlists: Watchlist[];
  companyFieldRefs: CompanyFieldRefs;
  companyForm: CompanyForm;
  companyFormRegistryMatches: CompanyRegistryEntry[];
  companyListSearch: string;
  companyWatchlistFilter: string;
  filteredCompanies: Company[];
  companies: Company[];
  selectedCompany: Company | null;
  workspaceAutoFocusId: string | null;
  clearWorkspaceAutoFocus: () => void;
  membershipsByCompany: Record<string, WatchlistMembership[]>;
  selectedCompanyFeedStats: { total: number; unread: number; saved: number };
  companyWorkspaceTab: CompanyWorkspaceTab;
  selectedCompanyFeedItems: FeedItem[];
  selectedCompanyFeedItem: FeedItem | null;
  aiAnalysisJobsByFeedItemId: Record<string, AiAnalysisJob[]>;
  aiAnalysisErrorByFeedItemId: Record<string, string | null>;
  aiAnalysisRequestInFlightByFeedItemId: Record<string, boolean>;
  aiAnalysisProviderConfigured: boolean;
  selectedCompanyNotebookEntries: NotebookEntry[];
  isNotebookComposerOpen: boolean;
  notebookForm: NotebookForm;
  selectedNotebookEntryId: string | null;
  selectedNotebookEntry: NotebookEntry | null;
  notebookEditMode: boolean;
  notebookEditForm: NotebookForm;
  isNotebookEditDirty: boolean;
  notebookError: string | null;
  selectedCompanyClaimEntries: NotebookEntry[];
  selectedClaimEntry: NotebookEntry | null;
  claimStatusDraft: string;
  companiesError: string | null;
  lookupStatus: string | null;
  createCompany: (event: React.FormEvent<HTMLFormElement>) => void;
  updateCompanyForm: (field: keyof CompanyForm, value: string) => void;
  clearCompanyFormField: (field: keyof CompanyForm) => void;
  lookupCompanyIfUseful: () => void;
  lookupCompany: () => void;
  applyRegistryEntryToCompanyForm: (entry: CompanyRegistryEntry) => void;
  setCompanyListSearch: (value: string) => void;
  setCompanyWatchlistFilter: (value: string) => void;
  openWatchlistFromCompanyRow: (watchlistId: string) => void;
  openCompanyWorkspace: (company: Company) => void;
  openCompanyWorkspaceFromKeyboard: (event: React.KeyboardEvent<HTMLElement>, company: Company) => void;
  deleteCompany: (company: Company) => void;
  setCompanyWorkspaceTab: (tab: CompanyWorkspaceTab) => void;
  toggleCompanyFeedItem: (item: FeedItem) => void;
  selectCompanyFeedItemFromKeyboard: (event: React.KeyboardEvent<HTMLElement>, item: FeedItem) => void;
  updateFeedItemState: (item: FeedItem, update: (item: FeedItem) => FeedItem) => void;
  inspectCompanyFeedItem: (item: FeedItem) => void;
  openFeedItemNoteDraft: (item: FeedItem) => void;
  startFeedItemAiAnalysis: (item: FeedItem, promptPresetId?: string, customQuestion?: string) => Promise<void>;
  retryFeedItemAiAnalysis: (jobId: string, itemId: string) => Promise<void>;
  openCompanyInboxFilter: (company: Company) => void;
  setNotebookComposerOpen: Dispatch<SetStateAction<boolean>>;
  updateNotebookForm: (field: keyof NotebookForm, value: string) => void;
  createNotebookEntry: (event: React.FormEvent<HTMLFormElement>) => void;
  setSelectedNotebookEntryId: Dispatch<SetStateAction<string | null>>;
  saveNotebookEntry: (event: React.FormEvent<HTMLFormElement>) => void;
  cancelNotebookEdit: () => void;
  setNotebookEditMode: (value: boolean) => void;
  updateNotebookEditForm: (field: keyof NotebookForm, value: string) => void;
  toggleClaimEntry: (entry: NotebookEntry) => void;
  setClaimStatusDraft: (value: string) => void;
  saveClaimStatus: (entry: NotebookEntry) => void;
  NotebookDateField: React.ComponentType<NotebookDateLikeFieldProps>;
  NotebookQuarterField: React.ComponentType<NotebookDateLikeFieldProps>;
  MarkdownNoteBody: React.ComponentType<MarkdownNoteBodyProps>;
  renderNotebookOrigins: (origins: NotebookEntry["origins"], companyId: string) => React.ReactNode;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  feedItemSummary: (item: FeedItem) => string;
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
  fundamentalsForm: FundamentalsForm;
  financialFactForm: FinancialFactForm;
  selectedFinancialFactId: string | null;
  isFinancialFactEditMode: boolean;
  fundamentalsError: string | null;
  createFinancialPeriod: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  saveFinancialFact: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  deleteFinancialFact: (id: string) => Promise<void>;
  selectFinancialFact: (id: string) => void;
  startEditingFinancialFact: () => void;
  cancelEditingFinancialFact: () => void;
  updateFundamentalsForm: (field: keyof FundamentalsForm, value: string) => void;
  updateFinancialFactForm: (field: keyof FinancialFactForm, value: string) => void;
};

export function CompaniesScreen({
  watchlists,
  companyFieldRefs,
  companyForm,
  companyFormRegistryMatches,
  companyListSearch,
  companyWatchlistFilter,
  filteredCompanies,
  companies,
  selectedCompany,
  workspaceAutoFocusId,
  clearWorkspaceAutoFocus,
  membershipsByCompany,
  selectedCompanyFeedStats,
  companyWorkspaceTab,
  selectedCompanyFeedItems,
  selectedCompanyFeedItem,
  aiAnalysisJobsByFeedItemId,
  aiAnalysisErrorByFeedItemId,
  aiAnalysisRequestInFlightByFeedItemId,
  aiAnalysisProviderConfigured,
  selectedCompanyNotebookEntries,
  isNotebookComposerOpen,
  notebookForm,
  selectedNotebookEntryId,
  selectedNotebookEntry,
  notebookEditMode,
  notebookEditForm,
  isNotebookEditDirty,
  notebookError,
  selectedCompanyClaimEntries,
  selectedClaimEntry,
  claimStatusDraft,
  companiesError,
  lookupStatus,
  createCompany,
  updateCompanyForm,
  clearCompanyFormField,
  lookupCompanyIfUseful,
  lookupCompany,
  applyRegistryEntryToCompanyForm,
  setCompanyListSearch,
  setCompanyWatchlistFilter,
  openWatchlistFromCompanyRow,
  openCompanyWorkspace,
  openCompanyWorkspaceFromKeyboard,
  deleteCompany,
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
  toggleClaimEntry,
  setClaimStatusDraft,
  saveClaimStatus,
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
  createFinancialPeriod,
  saveFinancialFact,
  deleteFinancialFact,
  selectFinancialFact,
  startEditingFinancialFact,
  cancelEditingFinancialFact,
  updateFundamentalsForm,
  updateFinancialFactForm,
}: CompaniesScreenProps) {
  const { t, text } = useLocale();

  return (
    <section className="feed-panel" aria-labelledby="companies-title">
              <PanelHeader
                title={t("companies.title")}
                description={t("companies.description")}
                titleId="companies-title"
              />

              <div className="companies-layout">
                <form className="company-form" onSubmit={createCompany}>
                  <label>
                    {text("Exchange")}
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.exchange = element;
                        }}
                        required
                        value={companyForm.exchange}
                        onChange={(event) => updateCompanyForm("exchange", event.target.value)}
                      />
                      {companyForm.exchange.trim().toUpperCase() !== "GPW" ? (
                        <ClearButton label={text("Clear exchange")} onClick={() => clearCompanyFormField("exchange")} />
                      ) : null}
                    </span>
                  </label>
                  <label>
                    {text("Ticker")}
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.ticker = element;
                        }}
                        required
                        value={companyForm.ticker}
                        onBlur={lookupCompanyIfUseful}
                        onChange={(event) => updateCompanyForm("ticker", event.target.value)}
                        placeholder="CDR"
                      />
                      {companyForm.ticker.trim().length > 0 ? (
                        <ClearButton label={text("Clear ticker")} onClick={() => clearCompanyFormField("ticker")} />
                      ) : null}
                    </span>
                  </label>
                  <label>
                    {text("Name")}
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.displayName = element;
                        }}
                        required
                        value={companyForm.displayName}
                        onBlur={lookupCompanyIfUseful}
                        onChange={(event) => updateCompanyForm("displayName", event.target.value)}
                        placeholder="CD PROJEKT S.A."
                      />
                      {companyForm.displayName.trim().length > 0 ? (
                        <ClearButton label={text("Clear name")} onClick={() => clearCompanyFormField("displayName")} />
                      ) : null}
                    </span>
                  </label>
                  <label>
                    ISIN
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.isin = element;
                        }}
                        value={companyForm.isin}
                        onBlur={lookupCompanyIfUseful}
                        onChange={(event) => updateCompanyForm("isin", event.target.value)}
                        placeholder="PLOPTTC00011"
                      />
                      {companyForm.isin.trim().length > 0 ? (
                        <ClearButton label={text("Clear ISIN")} onClick={() => clearCompanyFormField("isin")} />
                      ) : null}
                    </span>
                  </label>
                  <Button
                    onClick={lookupCompany}
                    onMouseDown={(event) => event.preventDefault()}
                  >
                    <LocateFixed size={16} />
                    {text("Lookup")}
                  </Button>
                  <Button
                    onMouseDown={(event) => event.preventDefault()}
                    type="submit"
                    variant="primary"
                  >
                    <Plus size={16} />
                    {text("Add")}
                  </Button>
                  {companyFormRegistryMatches.length > 0 ? (
                    <div className="company-registry-suggestions" aria-label={text("Company registry suggestions")}>
                      <span>{text("Registry matches")}</span>
                      <div>
                        {companyFormRegistryMatches.map((entry) => (
                          <button
                            className="company-registry-suggestion"
                            key={entry.qualifiedTicker}
                            onClick={() => applyRegistryEntryToCompanyForm(entry)}
                            onMouseDown={(event) => event.preventDefault()}
                            title={`${text("Use")} ${entry.qualifiedTicker}`}
                            type="button"
                          >
                            <strong><TickerLabel value={entry.qualifiedTicker} /></strong>
                            <span>{entry.displayName}</span>
                            <small>{entry.isin ?? text("No ISIN")}</small>
                            {entry.tracked ? <em>{text("Added")}</em> : null}
                          </button>
                        ))}
                      </div>
                    </div>
                  ) : null}
                </form>

                <div className="company-list-toolbar" aria-label={text("Company list search")}>
                  <SearchField
                    ariaLabel={text("Search tracked companies")}
                    className="registry-search-field"
                    clearLabel={text("Clear company search")}
                    onChange={setCompanyListSearch}
                    onClear={() => setCompanyListSearch("")}
                    placeholder={text("Search tracked companies")}
                    type="text"
                    value={companyListSearch}
                  />
                  <label className="company-list-filter">
                    {text("Watchlist")}
                    <select
                      aria-label={text("Company watchlist filter")}
                      onChange={(event) => setCompanyWatchlistFilter(event.target.value)}
                      value={companyWatchlistFilter}
                    >
                      <option value="all">{text("All watchlists")}</option>
                      {watchlists.map((watchlist) => (
                        <option key={watchlist.id} value={watchlist.id}>
                          {watchlist.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <span>
                    {filteredCompanies.length}/{companies.length} {text("companies")}
                  </span>
                </div>

                <div className="company-list" aria-label={text("Companies list")} data-company-list="true">
                  {filteredCompanies.map((company) => (
                    <div className="company-row-block" key={company.id}>
                      <DenseRow
                        aria-label={`${text("Open")} ${company.qualifiedTicker} ${text("workspace")}`}
                        className={[
                          "company-row",
                          selectedCompany?.id === company.id ? "company-row-selected" : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        data-company-id={company.id}
                        data-company-row="true"
                        onClick={() => openCompanyWorkspace(company)}
                        onKeyDown={(event) => openCompanyWorkspaceFromKeyboard(event, company)}
                        role="button"
                        selected={selectedCompany?.id === company.id}
                        tabIndex={0}
                        title={`${text("Open")} ${company.qualifiedTicker} ${text("workspace")}`}
                      >
                        <div className="company-row-main">
                          <h2><TickerLabel value={company.qualifiedTicker} /></h2>
                          <p>{company.displayName}</p>
                        </div>
                        <div className="company-row-context" onClick={(event) => event.stopPropagation()}>
                          <div
                            className="membership-list"
                            aria-label={`${text("Watchlist memberships for")} ${company.qualifiedTicker}`}
                          >
                            {(membershipsByCompany[company.id] ?? []).map((membership) => (
                              <button
                                className="membership-chip membership-link"
                                key={membership.watchlistId}
                                onClick={() => openWatchlistFromCompanyRow(membership.watchlistId)}
                                title={`${text("Open watchlist")} ${membership.watchlistName}`}
                                type="button"
                              >
                                {membership.watchlistName}
                              </button>
                            ))}
                            {(membershipsByCompany[company.id] ?? []).length === 0 ? (
                              <span className="membership-empty">{text("No watchlist")}</span>
                            ) : null}
                          </div>
                          <div className="company-row-actions">
                            <span>{company.isin ?? text("No ISIN")}</span>
                            <Button
                              className="danger-button"
                              onClick={() => deleteCompany(company)}
                              title={`${text("Delete")} ${company.qualifiedTicker}`}
                              variant="icon"
                            >
                              <Trash2 size={16} />
                            </Button>
                          </div>
                        </div>
                      </DenseRow>

                      {selectedCompany?.id === company.id ? (
                        <CompanyWorkspace
                          selectedCompany={selectedCompany}
                          autoFocusOnOpen={workspaceAutoFocusId === selectedCompany.id}
                          onAutoFocusHandled={clearWorkspaceAutoFocus}
                          membershipsByCompany={membershipsByCompany}
                          selectedCompanyFeedStats={selectedCompanyFeedStats}
                          companyWorkspaceTab={companyWorkspaceTab}
                          selectedCompanyFeedItems={selectedCompanyFeedItems}
                          selectedCompanyFeedItem={selectedCompanyFeedItem}
                          aiAnalysisJobsByFeedItemId={aiAnalysisJobsByFeedItemId}
                          aiAnalysisErrorByFeedItemId={aiAnalysisErrorByFeedItemId}
                          aiAnalysisRequestInFlightByFeedItemId={aiAnalysisRequestInFlightByFeedItemId}
                          aiAnalysisProviderConfigured={aiAnalysisProviderConfigured}
                          selectedCompanyNotebookEntries={selectedCompanyNotebookEntries}
                          isNotebookComposerOpen={isNotebookComposerOpen}
                          notebookForm={notebookForm}
                          selectedNotebookEntryId={selectedNotebookEntryId}
                          selectedNotebookEntry={selectedNotebookEntry}
                          notebookEditMode={notebookEditMode}
                          notebookEditForm={notebookEditForm}
                          isNotebookEditDirty={isNotebookEditDirty}
                          notebookError={notebookError}
                          selectedCompanyClaimEntries={selectedCompanyClaimEntries}
                          selectedClaimEntry={selectedClaimEntry}
                          claimStatusDraft={claimStatusDraft}
                          setCompanyWorkspaceTab={setCompanyWorkspaceTab}
                          toggleCompanyFeedItem={toggleCompanyFeedItem}
                          selectCompanyFeedItemFromKeyboard={selectCompanyFeedItemFromKeyboard}
                          updateFeedItemState={updateFeedItemState}
                          inspectCompanyFeedItem={inspectCompanyFeedItem}
                          openFeedItemNoteDraft={openFeedItemNoteDraft}
                          startFeedItemAiAnalysis={startFeedItemAiAnalysis}
                          retryFeedItemAiAnalysis={retryFeedItemAiAnalysis}
                          openCompanyInboxFilter={openCompanyInboxFilter}
                          setNotebookComposerOpen={setNotebookComposerOpen}
                          updateNotebookForm={updateNotebookForm}
                          createNotebookEntry={createNotebookEntry}
                          setSelectedNotebookEntryId={setSelectedNotebookEntryId}
                          saveNotebookEntry={saveNotebookEntry}
                          cancelNotebookEdit={cancelNotebookEdit}
                          setNotebookEditMode={setNotebookEditMode}
                          updateNotebookEditForm={updateNotebookEditForm}
                          toggleClaimEntry={toggleClaimEntry}
                          setClaimStatusDraft={setClaimStatusDraft}
                          saveClaimStatus={saveClaimStatus}
                          NotebookDateField={NotebookDateField}
                          NotebookQuarterField={NotebookQuarterField}
                          MarkdownNoteBody={MarkdownNoteBody}
                          renderNotebookOrigins={renderNotebookOrigins}
                          formatTimestamp={formatTimestamp}
                          feedItemSummary={feedItemSummary}
                          financialPeriods={financialPeriods}
                          financialFacts={financialFacts}
                          kpiDefinitions={kpiDefinitions}
                          fundamentalsForm={fundamentalsForm}
                          financialFactForm={financialFactForm}
                          selectedFinancialFactId={selectedFinancialFactId}
                          isFinancialFactEditMode={isFinancialFactEditMode}
                          fundamentalsError={fundamentalsError}
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
                    </div>
                  ))}
                  {companies.length === 0 ? (
                    <EmptyState>{text("No companies yet.")}</EmptyState>
                  ) : null}
                  {companies.length > 0 && filteredCompanies.length === 0 ? (
                    <EmptyState>{text("No companies match these filters.")}</EmptyState>
                  ) : null}
                </div>

                {companiesError ? (
                  <p className="error-text">{text("Companies command failed")}: {companiesError}</p>
                ) : null}
                {lookupStatus ? <p className="helper-text">{text(lookupStatus)}</p> : null}
              </div>
            </section>
  );
}
