import { Check, Edit3, Plus, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { Company, Watchlist, WatchlistMembership } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { pluralNoun } from "../../shared/locale/plural";
import { useWatchlistsViewModel } from "../../app/state/screenViewModels";
import { ActionRow, Button, DenseRow, EmptyState, ErrorText, InlineConfirm, PanelHeader, SearchField, SectionHeader, TextField } from "../../ui";

// Polish needs three plural forms; "{n} companies" must read "18 spółek", not "18 spółki".
const COMPANY_FORMS = { en: ["company", "companies"], pl: ["spółka", "spółki", "spółek"] } as const;

export type WatchlistsScreenProps = {
  companies: Company[];
  watchlists: Watchlist[];
  watchlistMemberships: WatchlistMembership[];
  watchlistsError: string | null;
  selectedWatchlistId: string | null;
  setSelectedWatchlistId: (watchlistId: string | null) => void;
  createWatchlist: (name: string) => void;
  renameWatchlist: (watchlist: Watchlist, name: string) => void;
  deleteWatchlist: (watchlist: Watchlist) => void;
  addCompanyToWatchlist: (watchlist: Watchlist, company: Company) => void;
  removeCompanyFromWatchlist: (watchlist: Watchlist, company: Company) => void;
};

export function WatchlistsScreen() {
  const {
    companies,
    watchlists,
    watchlistMemberships,
    watchlistsError,
    selectedWatchlistId,
    setSelectedWatchlistId,
    createWatchlist,
    renameWatchlist,
    deleteWatchlist,
    addCompanyToWatchlist,
    removeCompanyFromWatchlist,
  } = useWatchlistsViewModel();
  const { t, text, locale } = useLocale();
  const [watchlistName, setWatchlistName] = useState("");
  const [watchlistRenameDraft, setWatchlistRenameDraft] = useState("");
  // Cascading (ADR 0076 D5): confirm a watchlist delete in place.
  const [confirmDeleteWatchlist, setConfirmDeleteWatchlist] = useState(false);
  const [watchlistSearch, setWatchlistSearch] = useState("");
  const [watchlistCompanySearch, setWatchlistCompanySearch] = useState("");
  const [isAddOpen, setAddOpen] = useState(false);
  const [isRenameOpen, setRenameOpen] = useState(false);
  const [selectedAddCompanyIds, setSelectedAddCompanyIds] = useState<Set<string>>(() => new Set());
  const selectedWatchlist = watchlists.find((watchlist) => watchlist.id === selectedWatchlistId) ?? null;
  const normalizedWatchlistSearch = watchlistSearch.trim().toLowerCase();
  const filteredWatchlists = useMemo(
    () =>
      watchlists.filter((watchlist) =>
        normalizedWatchlistSearch
          ? watchlist.name.toLowerCase().includes(normalizedWatchlistSearch)
          : true,
      ),
    [normalizedWatchlistSearch, watchlists],
  );
  const selectedMemberships = useMemo(
    () =>
      selectedWatchlist
        ? watchlistMemberships.filter((membership) => membership.watchlistId === selectedWatchlist.id)
        : [],
    [selectedWatchlist, watchlistMemberships],
  );
  const selectedCompanyIds = useMemo(
    () => new Set(selectedMemberships.map((membership) => membership.companyId)),
    [selectedMemberships],
  );
  const memberCompanies = useMemo(
    () => companies.filter((company) => selectedCompanyIds.has(company.id)),
    [companies, selectedCompanyIds],
  );
  const normalizedSearch = watchlistCompanySearch.trim().toLowerCase();
  const availableCompanies = useMemo(
    () =>
      companies.filter((company) => {
        if (selectedCompanyIds.has(company.id)) {
          return false;
        }

        if (!normalizedSearch) {
          return true;
        }

        return (
          company.qualifiedTicker.toLowerCase().includes(normalizedSearch) ||
          company.displayName.toLowerCase().includes(normalizedSearch) ||
          company.isin?.toLowerCase().includes(normalizedSearch)
        );
      }),
    [companies, normalizedSearch, selectedCompanyIds],
  );

  useEffect(() => {
    if (selectedWatchlist || watchlists.length === 0) {
      return;
    }

    setSelectedWatchlistId(watchlists[0].id);
    setWatchlistRenameDraft(watchlists[0].name);
  }, [selectedWatchlist, watchlists, setSelectedWatchlistId]);

  function submitCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    createWatchlist(watchlistName);
    setWatchlistName("");
  }

  function selectWatchlist(watchlist: Watchlist) {
    setSelectedWatchlistId(watchlist.id);
    setWatchlistRenameDraft(watchlist.name);
    setRenameOpen(false);
    setAddOpen(false);
    setSelectedAddCompanyIds(new Set());
  }

  function submitRename(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (selectedWatchlist) {
      renameWatchlist(selectedWatchlist, watchlistRenameDraft);
    }
    setRenameOpen(false);
  }

  function toggleAddCompany(companyId: string) {
    setSelectedAddCompanyIds((current) => {
      const next = new Set(current);
      if (next.has(companyId)) {
        next.delete(companyId);
      } else {
        next.add(companyId);
      }
      return next;
    });
  }

  function addSelectedCompanies() {
    if (!selectedWatchlist || selectedAddCompanyIds.size === 0) {
      return;
    }

    const selectedCompanies = companies.filter((company) => selectedAddCompanyIds.has(company.id));
    selectedCompanies.forEach((company) => addCompanyToWatchlist(selectedWatchlist, company));
    setSelectedAddCompanyIds(new Set());
    setAddOpen(false);
    setWatchlistCompanySearch("");
  }

  return (
    <section className="feed-panel" aria-labelledby="watchlists-title">
      <PanelHeader
        paneLead
        title={t("watchlists.title")}
        description={t("watchlists.description")}
        titleId="watchlists-title"
        actions={
          <form className="watchlist-form" onSubmit={submitCreate}>
            <TextField
              aria-label={text("Watchlist name")}
              placeholder="Main GPW"
              value={watchlistName}
              onChange={(event) => setWatchlistName(event.target.value)}
              required
            />
            <Button type="submit" variant="primary">
              <Plus size={16} />
              {text("Create")}
            </Button>
          </form>
        }
      />

      <div className="watchlists-workspace">
        <div className="watchlists-sidebar">
          <SearchField
            ariaLabel={text("Search watchlists")}
            className="registry-search-field"
            clearLabel={text("Clear watchlist search")}
            onChange={setWatchlistSearch}
            onClear={() => setWatchlistSearch("")}
            placeholder={text("Search watchlists")}
            type="text"
            value={watchlistSearch}
          />
          <div className="watchlist-list" aria-label={text("Watchlists")}>
            {filteredWatchlists.map((watchlist) => (
              <button
                className={
                  selectedWatchlist?.id === watchlist.id
                    ? "watchlist-row watchlist-row-selected"
                    : "watchlist-row"
                }
                key={watchlist.id}
                data-watchlist-id={watchlist.id}
                onClick={() => selectWatchlist(watchlist)}
                type="button"
              >
                <span>{watchlist.name}</span>
                <strong>{watchlist.companyCount}</strong>
              </button>
            ))}
            {watchlists.length === 0 ? (
              <EmptyState>{text("No watchlists yet.")}</EmptyState>
            ) : null}
            {watchlists.length > 0 && filteredWatchlists.length === 0 ? (
              <EmptyState>{text("No watchlists match this search.")}</EmptyState>
            ) : null}
          </div>
        </div>

        <div className="watchlist-detail" aria-label={text("Selected watchlist")}>
          {selectedWatchlist ? (
            <>
              <div className="watchlist-detail-header">
                {isRenameOpen ? (
                  <form className="watchlist-rename-form" onSubmit={submitRename}>
                    <TextField
                      aria-label={text("Rename watchlist")}
                      onChange={(event) => setWatchlistRenameDraft(event.target.value)}
                      required
                      value={watchlistRenameDraft}
                    />
                    <Button type="submit" variant="secondary">
                      <Check size={14} />
                      {text("Save")}
                    </Button>
                    <Button onClick={() => setRenameOpen(false)} type="button" variant="ghost">
                      <X size={14} />
                    </Button>
                  </form>
                ) : (
                  <div>
                    <span className="eyebrow">{text("Selected watchlist")}</span>
                    <h3>{selectedWatchlist.name}</h3>
                    <p>
                      {memberCompanies.length}{" "}
                      {pluralNoun(locale, memberCompanies.length, COMPANY_FORMS)}
                    </p>
                  </div>
                )}
                {!isRenameOpen ? (
                  <ActionRow className="watchlist-detail-actions">
                    <Button onClick={() => setAddOpen((current) => !current)} type="button" variant="primary">
                      <Plus size={14} />
                      {text("Add companies")}
                    </Button>
                    <Button
                      onClick={() => {
                        setWatchlistRenameDraft(selectedWatchlist.name);
                        setRenameOpen(true);
                      }}
                      type="button"
                      variant="secondary"
                    >
                      <Edit3 size={14} />
                      {text("Rename")}
                    </Button>
                    {confirmDeleteWatchlist ? (
                      <InlineConfirm
                        cancelLabel={text("Cancel")}
                        confirmLabel={text("Delete")}
                        onCancel={() => setConfirmDeleteWatchlist(false)}
                        onConfirm={() => {
                          setConfirmDeleteWatchlist(false);
                          deleteWatchlist(selectedWatchlist);
                        }}
                      >
                        {`${text("Delete")} ${selectedWatchlist.name}?`}
                      </InlineConfirm>
                    ) : (
                      <Button
                        onClick={() => setConfirmDeleteWatchlist(true)}
                        type="button"
                        variant="danger"
                      >
                        <Trash2 size={14} />
                        {text("Delete")}
                      </Button>
                    )}
                  </ActionRow>
                ) : null}
              </div>

              {isAddOpen ? (
                <section className="watchlist-add-panel" aria-label={text("Add companies")}>
                  <SectionHeader
                    className="watchlist-add-panel-header"
                    level="h4"
                    title={text("Add companies")}
                    actions={
                      <>
                        <Button
                          disabled={selectedAddCompanyIds.size === 0}
                          onClick={addSelectedCompanies}
                          type="button"
                          variant="primary"
                        >
                          <Check size={14} />
                          {text("Add selected")}
                        </Button>
                        <Button
                          onClick={() => {
                            setAddOpen(false);
                            setSelectedAddCompanyIds(new Set());
                          }}
                          type="button"
                          variant="ghost"
                        >
                          <X size={14} />
                        </Button>
                      </>
                    }
                  />
                  <SearchField
                    ariaLabel={text("Search tracked companies to add")}
                    className="registry-search-field"
                    clearLabel={text("Clear company search")}
                    onChange={setWatchlistCompanySearch}
                    onClear={() => setWatchlistCompanySearch("")}
                    placeholder={text("Search tracked companies")}
                    type="text"
                    value={watchlistCompanySearch}
                  />
                  <div className="watchlist-picker-list">
                    {availableCompanies.map((company) => (
                      <button
                        aria-pressed={selectedAddCompanyIds.has(company.id)}
                        className={
                          selectedAddCompanyIds.has(company.id)
                            ? "watchlist-picker-row watchlist-picker-row-selected"
                            : "watchlist-picker-row"
                        }
                        key={company.id}
                        onClick={() => toggleAddCompany(company.id)}
                        type="button"
                      >
                        <TickerLabel value={company.qualifiedTicker} />
                        <span>{company.displayName}</span>
                        {selectedAddCompanyIds.has(company.id) ? <Check size={14} /> : <Plus size={14} />}
                      </button>
                    ))}
                    {availableCompanies.length === 0 ? (
                      <EmptyState>{text("No tracked companies to add.")}</EmptyState>
                    ) : null}
                  </div>
                </section>
              ) : null}

              <section className="watchlist-members-section" aria-label={text("Companies in watchlist")}>
                <SectionHeader
                  className="watchlist-table-header"
                  level="h4"
                  title={text("In this watchlist")}
                  meta={
                    <>
                      {memberCompanies.length}{" "}
                      {pluralNoun(locale, memberCompanies.length, COMPANY_FORMS)}
                    </>
                  }
                />
                <div className="watchlist-member-table">
                  {memberCompanies.map((company) => (
                    <DenseRow className="watchlist-member-row" interactive={false} key={company.id}>
                      <TickerLabel value={company.qualifiedTicker} />
                      <span>{company.displayName}</span>
                      <span>{company.isin ?? text("No ISIN")}</span>
                      <Button onClick={() => removeCompanyFromWatchlist(selectedWatchlist, company)} type="button" variant="ghost">
                        <X size={14} />
                        {text("Remove")}
                      </Button>
                    </DenseRow>
                  ))}
                  {memberCompanies.length === 0 ? (
                    <EmptyState>{text("No companies in this watchlist.")}</EmptyState>
                  ) : null}
                </div>
              </section>
            </>
          ) : (
            <EmptyState>{text("Select or create a watchlist.")}</EmptyState>
          )}
        </div>
      </div>

      {watchlistsError ? (
        <ErrorText>{text("Watchlist command failed")}: {watchlistsError}</ErrorText>
      ) : null}
    </section>
  );
}
