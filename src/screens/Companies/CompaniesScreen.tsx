import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { CheckCircle2, LocateFixed, Plus, Search, Trash2, X } from "lucide-react";
import type { Company, CompanyForm, CompanyRegistryEntry, FeedItem, NotebookEntry, Watchlist, WatchlistMembership } from "../../api/types";
import type { WatchlistFeedback } from "../../app/appTypes";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { StatusPill } from "../../shared/components/StatusPill";
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
  watchlistName: string;
  watchlistsError: string | null;
  companyFieldRefs: CompanyFieldRefs;
  companyForm: CompanyForm;
  companyFormRegistryMatches: CompanyRegistryEntry[];
  companyListSearch: string;
  filteredCompanies: Company[];
  companies: Company[];
  selectedCompany: Company | null;
  membershipsByCompany: Record<string, WatchlistMembership[]>;
  watchlistAssignments: Record<string, string>;
  watchlistFeedback: WatchlistFeedback | null;
  selectedCompanyFeedStats: { total: number; unread: number; saved: number };
  companyWorkspaceTab: CompanyWorkspaceTab;
  selectedCompanyFeedItems: FeedItem[];
  selectedCompanyFeedItem: FeedItem | null;
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
  setWatchlistName: (value: string) => void;
  createWatchlist: (event: React.FormEvent<HTMLFormElement>) => void;
  createCompany: (event: React.FormEvent<HTMLFormElement>) => void;
  updateCompanyForm: (field: keyof CompanyForm, value: string) => void;
  clearCompanyFormField: (field: keyof CompanyForm) => void;
  lookupCompanyIfUseful: () => void;
  lookupCompany: () => void;
  applyRegistryEntryToCompanyForm: (entry: CompanyRegistryEntry) => void;
  setCompanyListSearch: (value: string) => void;
  openCompanyWorkspace: (company: Company) => void;
  openCompanyWorkspaceFromKeyboard: (event: React.KeyboardEvent<HTMLElement>, company: Company) => void;
  updateWatchlistAssignment: (companyId: string, watchlistId: string) => void;
  addCompanyToWatchlist: (company: Company) => void;
  removeCompanyFromWatchlist: (company: Company) => void;
  deleteCompany: (company: Company) => void;
  setCompanyWorkspaceTab: (tab: CompanyWorkspaceTab) => void;
  toggleCompanyFeedItem: (item: FeedItem) => void;
  selectCompanyFeedItemFromKeyboard: (event: React.KeyboardEvent<HTMLElement>, item: FeedItem) => void;
  updateFeedItemState: (item: FeedItem, update: (item: FeedItem) => FeedItem) => void;
  inspectCompanyFeedItem: (item: FeedItem) => void;
  openFeedItemNoteDraft: (item: FeedItem) => void;
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
};

export function CompaniesScreen({
  watchlists,
  watchlistName,
  watchlistsError,
  companyFieldRefs,
  companyForm,
  companyFormRegistryMatches,
  companyListSearch,
  filteredCompanies,
  companies,
  selectedCompany,
  membershipsByCompany,
  watchlistAssignments,
  watchlistFeedback,
  selectedCompanyFeedStats,
  companyWorkspaceTab,
  selectedCompanyFeedItems,
  selectedCompanyFeedItem,
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
  setWatchlistName,
  createWatchlist,
  createCompany,
  updateCompanyForm,
  clearCompanyFormField,
  lookupCompanyIfUseful,
  lookupCompany,
  applyRegistryEntryToCompanyForm,
  setCompanyListSearch,
  openCompanyWorkspace,
  openCompanyWorkspaceFromKeyboard,
  updateWatchlistAssignment,
  addCompanyToWatchlist,
  removeCompanyFromWatchlist,
  deleteCompany,
  setCompanyWorkspaceTab,
  toggleCompanyFeedItem,
  selectCompanyFeedItemFromKeyboard,
  updateFeedItemState,
  inspectCompanyFeedItem,
  openFeedItemNoteDraft,
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
}: CompaniesScreenProps) {
  return (
    <section className="feed-panel" aria-labelledby="companies-title">
              <div className="panel-header">
                <div>
                  <h1 id="companies-title">Companies</h1>
                  <p>Local company registry backed by SQLite.</p>
                </div>
              </div>

              <div className="companies-layout">
                <section className="watchlist-panel" aria-labelledby="watchlists-title">
                  <div className="subsection-header">
                    <div>
                      <h2 id="watchlists-title">Watchlists</h2>
                      <p>Local groups for companies.</p>
                    </div>
                    <form className="watchlist-form" onSubmit={createWatchlist}>
                      <input
                        aria-label="Watchlist name"
                        placeholder="Main GPW"
                        value={watchlistName}
                        onChange={(event) => setWatchlistName(event.target.value)}
                        required
                      />
                      <Button type="submit" variant="primary">
                        <Plus size={16} />
                        Create
                      </Button>
                    </form>
                  </div>

                  <div className="watchlist-list" aria-label="Watchlist chips">
                    {watchlists.map((watchlist) => (
                      <div className="watchlist-chip" key={watchlist.id}>
                        <span>{watchlist.name}</span>
                        <strong>{watchlist.companyCount}</strong>
                      </div>
                    ))}
                    {watchlists.length === 0 ? (
                      <EmptyState>No watchlists yet.</EmptyState>
                    ) : null}
                  </div>

                  {watchlistsError ? (
                    <p className="error-text">Watchlist command failed: {watchlistsError}</p>
                  ) : null}
                </section>

                <form className="company-form" onSubmit={createCompany}>
                  <label>
                    Exchange
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
                        <button
                          aria-label="Clear exchange"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("exchange")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear exchange"
                          type="button"
                        >
                          <X size={13} />
                        </button>
                      ) : null}
                    </span>
                  </label>
                  <label>
                    Ticker
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
                        <button
                          aria-label="Clear ticker"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("ticker")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear ticker"
                          type="button"
                        >
                          <X size={13} />
                        </button>
                      ) : null}
                    </span>
                  </label>
                  <label>
                    Name
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
                        <button
                          aria-label="Clear name"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("displayName")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear name"
                          type="button"
                        >
                          <X size={13} />
                        </button>
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
                        <button
                          aria-label="Clear ISIN"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("isin")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear ISIN"
                          type="button"
                        >
                          <X size={13} />
                        </button>
                      ) : null}
                    </span>
                  </label>
                  <Button
                    onClick={lookupCompany}
                    onMouseDown={(event) => event.preventDefault()}
                  >
                    <LocateFixed size={16} />
                    Lookup
                  </Button>
                  <Button
                    onMouseDown={(event) => event.preventDefault()}
                    type="submit"
                    variant="primary"
                  >
                    <Plus size={16} />
                    Add
                  </Button>
                  {companyFormRegistryMatches.length > 0 ? (
                    <div className="company-registry-suggestions" aria-label="Company registry suggestions">
                      <span>Registry matches</span>
                      <div>
                        {companyFormRegistryMatches.map((entry) => (
                          <button
                            className="company-registry-suggestion"
                            key={entry.qualifiedTicker}
                            onClick={() => applyRegistryEntryToCompanyForm(entry)}
                            onMouseDown={(event) => event.preventDefault()}
                            title={`Use ${entry.qualifiedTicker}`}
                            type="button"
                          >
                            <strong>{entry.qualifiedTicker}</strong>
                            <span>{entry.displayName}</span>
                            <small>{entry.isin ?? "No ISIN"}</small>
                            {entry.tracked ? <em>Added</em> : null}
                          </button>
                        ))}
                      </div>
                    </div>
                  ) : null}
                </form>

                <div className="company-list-toolbar" aria-label="Company list search">
                  <label className="registry-search-field">
                    <Search size={15} />
                    <input
                      aria-label="Search tracked companies"
                      onChange={(event) => setCompanyListSearch(event.target.value)}
                      placeholder="Search tracked companies"
                      type="text"
                      value={companyListSearch}
                    />
                    {companyListSearch.trim().length > 0 ? (
                      <button
                        aria-label="Clear company search"
                        className="field-clear-button"
                        onClick={() => setCompanyListSearch("")}
                        onMouseDown={(event) => event.preventDefault()}
                        title="Clear company search"
                        type="button"
                      >
                        <X size={13} />
                      </button>
                    ) : null}
                  </label>
                  <span>
                    {filteredCompanies.length}/{companies.length} companies
                  </span>
                </div>

                <div className="company-list" aria-label="Companies list" data-company-list="true">
                  {filteredCompanies.map((company) => (
                    <div className="company-row-block" key={company.id}>
                      <article
                        aria-label={`Open ${company.qualifiedTicker} workspace`}
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
                        tabIndex={0}
                        title={`Open ${company.qualifiedTicker} workspace`}
                      >
                        <div className="company-row-main">
                          <h2>{company.qualifiedTicker}</h2>
                          <p>{company.displayName}</p>
                          <div
                            className="membership-list"
                            aria-label={`Watchlist memberships for ${company.qualifiedTicker}`}
                          >
                            {(membershipsByCompany[company.id] ?? []).map((membership) => (
                              <StatusPill key={membership.watchlistId}>{membership.watchlistName}</StatusPill>
                            ))}
                            {(membershipsByCompany[company.id] ?? []).length === 0 ? (
                              <span className="membership-empty">No watchlist</span>
                            ) : null}
                          </div>
                        </div>
                        <div className="company-row-actions" onClick={(event) => event.stopPropagation()}>
                          <span>{company.isin ?? "No ISIN"}</span>
                          <select
                            aria-label={`Watchlist for ${company.qualifiedTicker}`}
                            disabled={watchlists.length === 0}
                            value={watchlistAssignments[company.id] || watchlists[0]?.id || ""}
                            onChange={(event) =>
                              updateWatchlistAssignment(company.id, event.target.value)
                            }
                          >
                            {watchlists.map((watchlist) => (
                              <option key={watchlist.id} value={watchlist.id}>
                                {watchlist.name}
                              </option>
                            ))}
                          </select>
                          <Button
                            className="compact-button assign-button"
                            disabled={watchlists.length === 0}
                            onClick={() => addCompanyToWatchlist(company)}
                          >
                            <Plus size={15} />
                            Assign
                          </Button>
                          <Button
                            className="compact-button remove-button"
                            disabled={watchlists.length === 0}
                            onClick={() => removeCompanyFromWatchlist(company)}
                          >
                            <X size={15} />
                            Remove
                          </Button>
                          {watchlistFeedback?.companyId === company.id ? (
                            <span
                              aria-label={watchlistFeedback.message}
                              className="inline-success"
                              role="status"
                              title={watchlistFeedback.message}
                            >
                              <CheckCircle2 size={16} />
                            </span>
                          ) : null}
                          <Button
                            onClick={() => deleteCompany(company)}
                            title={`Delete ${company.qualifiedTicker}`}
                            variant="danger"
                          >
                            <Trash2 size={16} />
                          </Button>
                        </div>
                      </article>

                      {selectedCompany?.id === company.id ? (
                        <CompanyWorkspace
                          selectedCompany={selectedCompany}
                          membershipsByCompany={membershipsByCompany}
                          selectedCompanyFeedStats={selectedCompanyFeedStats}
                          companyWorkspaceTab={companyWorkspaceTab}
                          selectedCompanyFeedItems={selectedCompanyFeedItems}
                          selectedCompanyFeedItem={selectedCompanyFeedItem}
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
                        />
                      ) : null}
                    </div>
                  ))}
                  {companies.length === 0 ? (
                    <EmptyState>No companies yet.</EmptyState>
                  ) : null}
                  {companies.length > 0 && filteredCompanies.length === 0 ? (
                    <EmptyState>No companies match this search.</EmptyState>
                  ) : null}
                </div>

                {companiesError ? (
                  <p className="error-text">Companies command failed: {companiesError}</p>
                ) : null}
                {lookupStatus ? <p className="helper-text">{lookupStatus}</p> : null}
              </div>
            </section>
  );
}
