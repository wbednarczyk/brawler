import { useState, type MutableRefObject } from "react";
import { LocateFixed, Plus, SlidersHorizontal, Trash2 } from "lucide-react";
import type { Company, CompanyForm, CompanyRegistryEntry, Watchlist, WatchlistMembership } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { useCompaniesViewModel } from "../../app/state/screenViewModels";
import { Button, ClearButton, DenseRow, EmptyState, ErrorText, InlineConfirm, PanelHeader, SearchField, TextField } from "../../ui";
import { CompanySettingsManager } from "./CompanySettingsManager";

type CompanyFieldRefs = MutableRefObject<Record<keyof CompanyForm, HTMLInputElement | null>>;

// The Companies screen is the company **library + management** surface (ADR 0057):
// browse/search/add companies and manage per-company settings. Opening a company
// (row click) lands the Spółka workshop (ADR 0107) — the deep-dive lives there, not
// in a tabbed panel inside this screen.
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
  membershipsByCompany: Record<string, WatchlistMembership[]>;
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
};

export function CompaniesScreen() {
  const {
    watchlists,
    companyFieldRefs,
    companyForm,
    companyFormRegistryMatches,
    companyListSearch,
    companyWatchlistFilter,
    filteredCompanies,
    companies,
    selectedCompany,
    membershipsByCompany,
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
  } = useCompaniesViewModel();
  const { t, text } = useLocale();
  const [settingsMode, setSettingsMode] = useState(false);
  // Irreversible/cascading (ADR 0076 D5): confirm a company delete in place.
  const [confirmDeleteCompanyId, setConfirmDeleteCompanyId] = useState<string | null>(null);

  return (
    <section className="feed-panel" aria-labelledby="companies-title">
      <PanelHeader
        title={t("companies.title")}
        description={t("companies.description")}
        titleId="companies-title"
        actions={
          <Button
            onClick={() => setSettingsMode((value) => !value)}
            type="button"
            variant={settingsMode ? "primary" : "ghost"}
          >
            <SlidersHorizontal size={14} aria-hidden="true" />
            {settingsMode ? text("Done") : text("Manage settings")}
          </Button>
        }
      />

      {settingsMode ? (
        <CompanySettingsManager
          companies={companies}
          watchlists={watchlists}
          membershipsByCompany={membershipsByCompany}
        />
      ) : (
        <div className="companies-layout">
          <form className="company-form" onSubmit={createCompany}>
            <label>
              {text("Exchange")}
              <span className="field-with-clear">
                <TextField
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
                <TextField
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
                <TextField
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
                <TextField
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
              {/* eslint-disable-next-line no-restricted-syntax -- layout-specific inline filter <select> in the company-list toolbar row */}
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
                  className={[
                    "company-row",
                    selectedCompany?.id === company.id ? "company-row-selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  data-company-id={company.id}
                  interactive={false}
                  selected={selectedCompany?.id === company.id}
                >
                  {/* Primary action is a real <button>, not a role="button" on the
                      row: keeps the row axe-clean (aria-allowed-role) and lets the
                      secondary controls below sit as siblings, not nested
                      interactives (ADR 0076 D9). */}
                  <button
                    type="button"
                    aria-label={`${text("Open")} ${company.qualifiedTicker}`}
                    className="company-row-main"
                    data-company-row="true"
                    onClick={() => openCompanyWorkspace(company)}
                    onKeyDown={(event) => openCompanyWorkspaceFromKeyboard(event, company)}
                    title={`${text("Open")} ${company.qualifiedTicker}`}
                  >
                    <h2><TickerLabel value={company.qualifiedTicker} /></h2>
                    <p>{company.displayName}</p>
                  </button>
                  <div className="company-row-context">
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
                      {confirmDeleteCompanyId === company.id ? (
                        <InlineConfirm
                          cancelLabel={text("Cancel")}
                          confirmLabel={text("Delete")}
                          onCancel={() => setConfirmDeleteCompanyId(null)}
                          onConfirm={() => {
                            setConfirmDeleteCompanyId(null);
                            deleteCompany(company);
                          }}
                        >
                          {`${text("Delete")} ${company.qualifiedTicker} ${text("from tracked companies?")}`}
                        </InlineConfirm>
                      ) : (
                        <Button
                          className="danger-button"
                          onClick={() => setConfirmDeleteCompanyId(company.id)}
                          title={`${text("Delete")} ${company.qualifiedTicker}`}
                          variant="icon"
                        >
                          <Trash2 size={16} />
                        </Button>
                      )}
                    </div>
                  </div>
                </DenseRow>
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
            <ErrorText>{text("Companies command failed")}: {companiesError}</ErrorText>
          ) : null}
          {lookupStatus ? <p className="helper-text">{text(lookupStatus)}</p> : null}
        </div>
      )}
    </section>
  );
}
