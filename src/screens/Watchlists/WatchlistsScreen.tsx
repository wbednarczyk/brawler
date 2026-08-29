import { X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Company, Watchlist, WatchlistMembership } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatCount } from "../../shared/format/financialValue";
import { useLocale } from "../../shared/locale";
import { COMPANY_FORMS, pluralNoun, type PluralForms } from "../../shared/locale/plural";
import { useWatchlistsViewModel } from "../../app/state/screenViewModels";
import {
  ActionButton,
  ActionRow,
  DenseRow,
  EmptyState,
  ErrorText,
  Figure,
  InlineConfirm,
  PanelHeader,
  SearchField,
  SectionHeader,
  TextField,
} from "../../ui";

// "N tracked" adjective agreement for the 0-members invitation's library-size
// hint (F4a S3 redesign) — Polish needs the three-category form the other
// PluralForms constants use (mirrors UNSEEN_FORMS' adjective precedent in
// shared/locale/plural.ts; kept local since only this screen needs it).
const TRACKED_FORMS: PluralForms = { en: ["tracked", "tracked"], pl: ["śledzona", "śledzone", "śledzonych"] };

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
  openCompanyWorkspaceById: (companyId: string) => void;
};

/**
 * Library screen "Watchlists" (F4a S3 redesign, docs/plans/frontend-v2-f4a.md
 * § Watchlists; approved mockup docs/mockups/frontend-v2-f4/watchlists.html):
 * two panes — watchlist names + counts, and the selected list's members — with
 * exactly one filled action at rest ("Add companies"). Member rows carry no
 * action column: a real "Open company" destination plus a ghost "Remove"
 * action, both real focusable buttons (ADR 0104 dec. 3 amendment).
 */
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
    openCompanyWorkspaceById,
  } = useWatchlistsViewModel();
  const { t, text, locale } = useLocale();
  const nameFieldRef = useRef<HTMLInputElement>(null);
  const [watchlistName, setWatchlistName] = useState("");
  const [watchlistRenameDraft, setWatchlistRenameDraft] = useState("");
  // Cascading (ADR 0076 D5): confirm a watchlist delete in place.
  const [confirmDeleteWatchlist, setConfirmDeleteWatchlist] = useState(false);
  const [watchlistSearch, setWatchlistSearch] = useState("");
  const [watchlistCompanySearch, setWatchlistCompanySearch] = useState("");
  const [isAddOpen, setAddOpen] = useState(false);
  const [isRenameOpen, setRenameOpen] = useState(false);
  // S (<420px pane) / short (<480px tall): activating a list opens the detail
  // AS a stacked view in place of the names list (F4a Fix-B). The tier switch
  // itself is CSS-only (container queries on `.watchlists-workspace-detail-open`,
  // see watchlists.css) — this flag only tracks whether the user asked to see
  // the detail; on M/L it renders unused (both panes always show there).
  const [detailOpenAtS, setDetailOpenAtS] = useState(false);
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
  // A search that excludes every list takes over the sidebar with the
  // "no watchlist by that name" invitation and hides the (now stale) detail
  // pane, keeping exactly one filled action on screen (state matrix, F4a S3).
  const searchHasNoMatch = watchlists.length > 0 && normalizedWatchlistSearch !== "" && filteredWatchlists.length === 0;
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
  // The add-companies picker shows the WHOLE library (not just what's
  // missing): a company already on the list renders as a disabled,
  // "already on the list" row instead of disappearing, so the picker reads as
  // "everything you track" rather than a shrinking list (F4a S3 redesign).
  const normalizedSearch = watchlistCompanySearch.trim().toLowerCase();
  const filteredLibraryCompanies = useMemo(
    () =>
      companies.filter((company) => {
        if (!normalizedSearch) {
          return true;
        }

        return (
          company.qualifiedTicker.toLowerCase().includes(normalizedSearch) ||
          company.displayName.toLowerCase().includes(normalizedSearch) ||
          company.isin?.toLowerCase().includes(normalizedSearch)
        );
      }),
    [companies, normalizedSearch],
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
    setDetailOpenAtS(true);
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

  const addSelectedLabel =
    selectedAddCompanyIds.size > 0 ? (
      <>
        {text("Add selected")} · <Figure value={selectedAddCompanyIds.size} />
      </>
    ) : (
      text("Add selected")
    );

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
              ref={nameFieldRef}
              aria-label={text("Watchlist name")}
              placeholder="Main GPW"
              value={watchlistName}
              onChange={(event) => setWatchlistName(event.target.value)}
              required
            />
            <ActionButton verb="create" type="submit" variant="secondary">
              {text("Create")}
            </ActionButton>
          </form>
        }
      />

      {watchlists.length === 0 ? (
        <EmptyState
          kind="invitation"
          title={text("No watchlists yet.")}
          source={text(
            "A watchlist is a group of companies from your library. Today, Inbox and Report Season will only show what's on it.",
          )}
          action={
            <ActionButton
              verb="create"
              variant="primary"
              data-ux-primary-action="true"
              onClick={() => nameFieldRef.current?.focus()}
            >
              {text("Create your first watchlist")}
            </ActionButton>
          }
        />
      ) : (
        <div
          className={
            detailOpenAtS ? "watchlists-workspace watchlists-workspace-detail-open" : "watchlists-workspace"
          }
        >
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
                <DenseRow
                  as="button"
                  className="watchlist-row"
                  data-action-kind="destination"
                  data-watchlist-id={watchlist.id}
                  key={watchlist.id}
                  onClick={() => selectWatchlist(watchlist)}
                  selected={selectedWatchlist?.id === watchlist.id}
                >
                  <span>{watchlist.name}</span>
                  <Figure value={watchlist.companyCount} />
                </DenseRow>
              ))}
              {searchHasNoMatch ? (
                <EmptyState
                  kind="invitation"
                  title={text("No watchlists match this search.")}
                  source={text("Check the spelling, or create a new watchlist with this name.")}
                  action={
                    <ActionButton
                      verb="create"
                      variant="primary"
                      data-ux-primary-action="true"
                      onClick={() => createWatchlist(watchlistSearch.trim())}
                    >
                      {text("Create watchlist \"{name}\"").replace("{name}", watchlistSearch.trim())}
                    </ActionButton>
                  }
                />
              ) : null}
            </div>
          </div>

          <div className="watchlist-detail" aria-label={text("Selected watchlist")}>
            {detailOpenAtS ? (
              <ActionButton
                className="watchlist-back-to-lists"
                kind="control"
                variant="ghost"
                onClick={() => setDetailOpenAtS(false)}
              >
                {text("Back to lists")}
              </ActionButton>
            ) : null}
            {searchHasNoMatch ? null : selectedWatchlist ? (
              <>
                {isRenameOpen ? (
                  <form className="watchlist-rename-form" onSubmit={submitRename}>
                    <TextField
                      aria-label={text("Rename watchlist")}
                      onChange={(event) => setWatchlistRenameDraft(event.target.value)}
                      required
                      value={watchlistRenameDraft}
                    />
                    <ActionButton verb="save" type="submit" variant="secondary">
                      {text("Save")}
                    </ActionButton>
                    <ActionButton kind="control" onClick={() => setRenameOpen(false)} type="button" variant="ghost">
                      {text("Cancel")}
                    </ActionButton>
                  </form>
                ) : (
                  <SectionHeader
                    className="watchlist-detail-header"
                    level="h4"
                    eyebrow={text("Selected list")}
                    title={selectedWatchlist.name}
                    meta={
                      <>
                        <Figure value={memberCompanies.length} /> {pluralNoun(locale, memberCompanies.length, COMPANY_FORMS)} ·{" "}
                        {text("used by Today, Inbox and Report Season")}
                      </>
                    }
                    actions={
                      <ActionRow className="watchlist-detail-actions">
                        <ActionButton
                          verb="rename"
                          variant="secondary"
                          onClick={() => {
                            setWatchlistRenameDraft(selectedWatchlist.name);
                            setRenameOpen(true);
                          }}
                        >
                          {text("Rename")}
                        </ActionButton>
                        {confirmDeleteWatchlist ? (
                          <InlineConfirm
                            cancelLabel={text("Cancel")}
                            confirmLabel={text("Remove")}
                            onCancel={() => setConfirmDeleteWatchlist(false)}
                            onConfirm={() => {
                              setConfirmDeleteWatchlist(false);
                              deleteWatchlist(selectedWatchlist);
                            }}
                          >
                            {`${text("Remove")} ${selectedWatchlist.name}?`}
                          </InlineConfirm>
                        ) : (
                          <ActionButton verb="remove" variant="danger" onClick={() => setConfirmDeleteWatchlist(true)}>
                            {text("Remove")}
                          </ActionButton>
                        )}
                        {memberCompanies.length > 0 && !isAddOpen ? (
                          <ActionButton
                            verb="add"
                            variant="primary"
                            data-ux-primary-action="true"
                            onClick={() => setAddOpen(true)}
                          >
                            {text("Add companies")}
                          </ActionButton>
                        ) : null}
                      </ActionRow>
                    }
                  />
                )}

                {isAddOpen ? (
                  <section className="watchlist-add-panel" aria-label={text("Add companies")}>
                    <SectionHeader
                      className="watchlist-add-panel-header"
                      level="h4"
                      title={text("Add companies")}
                      actions={
                        <>
                          <ActionButton
                            verb="add"
                            variant="primary"
                            data-ux-primary-action="true"
                            disabled={selectedAddCompanyIds.size === 0}
                            onClick={addSelectedCompanies}
                          >
                            {addSelectedLabel}
                          </ActionButton>
                          <ActionButton
                            kind="control"
                            variant="ghost"
                            onClick={() => {
                              setAddOpen(false);
                              setSelectedAddCompanyIds(new Set());
                            }}
                          >
                            {text("Cancel")}
                          </ActionButton>
                        </>
                      }
                    />
                    <SearchField
                      ariaLabel={text("Search tracked companies to add")}
                      className="registry-search-field"
                      clearLabel={text("Clear company search")}
                      onChange={setWatchlistCompanySearch}
                      onClear={() => setWatchlistCompanySearch("")}
                      placeholder={`${text("Search the library")} (${formatCount(companies.length)})`}
                      type="text"
                      value={watchlistCompanySearch}
                    />
                    <div className="watchlist-picker-list">
                      {filteredLibraryCompanies.map((company) => {
                        const isMember = selectedCompanyIds.has(company.id);
                        return (
                          <label
                            className={
                              isMember ? "watchlist-picker-row watchlist-picker-row-disabled" : "watchlist-picker-row"
                            }
                            key={company.id}
                          >
                            <input
                              checked={isMember || selectedAddCompanyIds.has(company.id)}
                              disabled={isMember}
                              onChange={() => toggleAddCompany(company.id)}
                              type="checkbox"
                            />
                            <TickerLabel value={company.qualifiedTicker} />
                            <span>
                              {company.displayName}
                              {isMember ? ` · ${text("already on the list")}` : ""}
                            </span>
                          </label>
                        );
                      })}
                      {filteredLibraryCompanies.length === 0 ? (
                        <EmptyState>{text("No tracked companies to add.")}</EmptyState>
                      ) : null}
                    </div>
                  </section>
                ) : null}

                <section className="watchlist-members-section" aria-label={text("Companies in watchlist")}>
                  {memberCompanies.length > 0 ? (
                    <>
                      <SectionHeader
                        className="watchlist-table-header"
                        level="h4"
                        title={text("In this watchlist")}
                        meta={
                          <>
                            {memberCompanies.length} {pluralNoun(locale, memberCompanies.length, COMPANY_FORMS)}
                          </>
                        }
                      />
                      <div className="watchlist-member-table">
                        {memberCompanies.map((company) => (
                          <DenseRow className="watchlist-member-row" interactive={false} key={company.id}>
                            <TickerLabel value={company.qualifiedTicker} />
                            <span className="watchlist-member-name">
                              <span className="watchlist-member-name-text">{company.displayName}</span>
                              {company.isin ? (
                                <span className="watchlist-member-isin">{company.isin}</span>
                              ) : null}
                            </span>
                            <ActionRow className="watchlist-member-actions">
                              <ActionButton
                                kind="destination"
                                variant="secondary"
                                onClick={() => openCompanyWorkspaceById(company.id)}
                              >
                                {text("Open company")}
                              </ActionButton>
                              <ActionButton
                                verb="remove"
                                variant="ghost"
                                aria-label={text("Remove from list")}
                                title={text("Remove from list")}
                                onClick={() => removeCompanyFromWatchlist(selectedWatchlist, company)}
                              >
                                <X aria-hidden="true" size={14} />
                                <span className="watchlist-member-remove-label">{text("Remove from list")}</span>
                              </ActionButton>
                            </ActionRow>
                          </DenseRow>
                        ))}
                      </div>
                    </>
                  ) : isAddOpen ? null : (
                    <EmptyState
                      kind="invitation"
                      title={text("No companies in this watchlist.")}
                      source={text(
                        "Add companies from your library ({count} {tracked}). A company can be on several watchlists.",
                      )
                        .replace("{count}", formatCount(companies.length))
                        .replace("{tracked}", pluralNoun(locale, companies.length, TRACKED_FORMS))}
                      action={
                        <ActionButton
                          verb="add"
                          variant="primary"
                          data-ux-primary-action="true"
                          onClick={() => setAddOpen(true)}
                        >
                          {text("Add companies")}
                        </ActionButton>
                      }
                    />
                  )}
                </section>
              </>
            ) : (
              <EmptyState>{text("Select or create a watchlist.")}</EmptyState>
            )}
          </div>
        </div>
      )}

      {watchlistsError ? (
        <ErrorText>{text("Watchlist command failed")}: {watchlistsError}</ErrorText>
      ) : null}
    </section>
  );
}
