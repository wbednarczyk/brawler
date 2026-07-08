import type { Dispatch, FormEvent, MutableRefObject, SetStateAction } from "react";
import * as companiesApi from "../api/companies";
import * as watchlistsApi from "../api/watchlists";
import type {
  Company,
  CompanyForm,
  CompanyLookupResult,
  CompanyRegistryEntry,
  Watchlist,
} from "../api/types";

type CompanyControllerInput = {
  companyFieldRefs: MutableRefObject<Record<keyof CompanyForm, HTMLInputElement | null>>;
  companyForm: CompanyForm;
  companyLookupVersionRef: MutableRefObject<number>;
  refreshCompanies: () => Promise<void>;
  refreshCompanyRegistryEntries: () => Promise<void>;
  refreshDatabaseStatus: () => Promise<void>;
  refreshWatchlistMemberships: () => Promise<void>;
  refreshWatchlists: () => Promise<void>;
  setAddingRegistryTicker: Dispatch<SetStateAction<string | null>>;
  setCompaniesError: Dispatch<SetStateAction<string | null>>;
  setCompanyForm: Dispatch<SetStateAction<CompanyForm>>;
  setCompanyListSearch: Dispatch<SetStateAction<string>>;
  setCompanyWatchlistFilter: Dispatch<SetStateAction<string>>;
  setLookupStatus: Dispatch<SetStateAction<string | null>>;
  setSelectedCompanyRegistryTicker: Dispatch<SetStateAction<string | null>>;
  setWatchlistsError: Dispatch<SetStateAction<string | null>>;
  skipNextCompanyLookupRef: MutableRefObject<boolean>;
  resetDeletedWatchlistFilters: (watchlistId: string) => void;
  text: (value: string) => string;
};

export function useCompanyController({
  companyFieldRefs,
  companyForm,
  companyLookupVersionRef,
  refreshCompanies,
  refreshCompanyRegistryEntries,
  refreshDatabaseStatus,
  refreshWatchlistMemberships,
  refreshWatchlists,
  setAddingRegistryTicker,
  setCompaniesError,
  setCompanyForm,
  setCompanyListSearch,
  setCompanyWatchlistFilter,
  setLookupStatus,
  setSelectedCompanyRegistryTicker,
  setWatchlistsError,
  skipNextCompanyLookupRef,
  resetDeletedWatchlistFilters,
  text,
}: CompanyControllerInput) {
  function updateCompanyForm(field: keyof CompanyForm, value: string) {
    companyLookupVersionRef.current += 1;
    setSelectedCompanyRegistryTicker(null);
    setCompanyForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function clearCompanyFormField(field: keyof CompanyForm) {
    companyLookupVersionRef.current += 1;
    skipNextCompanyLookupRef.current = true;
    setSelectedCompanyRegistryTicker(null);
    setLookupStatus(null);
    setCompanyForm((current) => ({
      ...current,
      [field]: field === "exchange" ? "GPW" : "",
    }));
    window.setTimeout(() => {
      companyFieldRefs.current[field]?.focus();
    }, 0);
  }

  function applyLookupResult(result: CompanyLookupResult) {
    setSelectedCompanyRegistryTicker(result.qualifiedTicker);
    setCompanyForm({
      exchange: result.exchange,
      ticker: result.ticker,
      displayName: result.displayName,
      isin: result.isin,
    });
    setLookupStatus(`${text("Filled from")} ${result.source}: ${result.qualifiedTicker}`);
  }

  function applyRegistryEntryToCompanyForm(entry: CompanyRegistryEntry) {
    companyLookupVersionRef.current += 1;
    setSelectedCompanyRegistryTicker(entry.qualifiedTicker);
    setCompanyForm({
      exchange: entry.exchange,
      ticker: entry.ticker,
      displayName: entry.displayName,
      isin: entry.isin ?? "",
    });
    setLookupStatus(`${text("Selected from company directory")}: ${entry.qualifiedTicker}`);
  }

  function lookupCompany() {
    const lookupVersion = companyLookupVersionRef.current;
    setLookupStatus(text("Looking up company directory..."));

    companiesApi.lookupCompany({
      exchange: companyForm.exchange,
      ticker: companyForm.ticker || null,
      displayName: companyForm.displayName || null,
      isin: companyForm.isin || null,
    })
      .then((result) => {
        if (lookupVersion !== companyLookupVersionRef.current) {
          return;
        }

        if (result) {
          applyLookupResult(result);
        } else {
          setLookupStatus(text("No company directory match."));
        }
        setCompaniesError(null);
      })
      .catch((error) => {
        if (lookupVersion !== companyLookupVersionRef.current) {
          return;
        }

        setLookupStatus(null);
        setCompaniesError(String(error));
      });
  }

  function lookupCompanyIfUseful() {
    if (skipNextCompanyLookupRef.current) {
      skipNextCompanyLookupRef.current = false;
      return;
    }

    if (companyForm.ticker || companyForm.displayName.length >= 3 || companyForm.isin) {
      lookupCompany();
    }
  }

  function createCompany(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    companiesApi.createCompany({
      exchange: companyForm.exchange,
      ticker: companyForm.ticker,
      displayName: companyForm.displayName,
      isin: companyForm.isin || null,
      cik: null,
      lei: null,
    })
      .then(() => {
        setCompanyForm({
          exchange: companyForm.exchange.toUpperCase(),
          ticker: "",
          displayName: "",
          isin: "",
        });
        setCompanyListSearch("");
        setCompanyWatchlistFilter("all");
        setCompaniesError(null);
        refreshCompanies();
        refreshDatabaseStatus();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setCompaniesError(String(error));
      });
  }

  function addCompanyFromRegistry(entry: CompanyRegistryEntry) {
    setAddingRegistryTicker(entry.qualifiedTicker);

    companiesApi.createCompany({
      exchange: entry.exchange,
      ticker: entry.ticker,
      displayName: entry.displayName,
      isin: entry.isin,
      cik: null,
      lei: null,
    })
      .then(() => {
        setCompanyListSearch("");
        setCompanyWatchlistFilter("all");
        setCompaniesError(null);
        return Promise.all([
          refreshCompanies(),
          refreshCompanyRegistryEntries(),
          refreshDatabaseStatus(),
          refreshWatchlistMemberships(),
        ]);
      })
      .catch((error) => {
        setCompaniesError(String(error));
      })
      .finally(() => {
        setAddingRegistryTicker(null);
      });
  }

  // Irreversible/cascading (ADR 0076 D5): deleting a tracked company removes its
  // feed, notebook, claims and events; there is no faithful re-create, so the
  // confirm gate lives at the call site as an InlineConfirm — not here.
  function deleteCompany(company: Company) {
    companiesApi.deleteCompany(company.id)
      .then(() => {
        setCompaniesError(null);
        refreshCompanies();
        refreshDatabaseStatus();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setCompaniesError(String(error));
      });
  }

  function createWatchlist(name: string) {
    watchlistsApi.createWatchlist({
      name,
      description: null,
    })
      .then(() => {
        setWatchlistsError(null);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function renameWatchlist(watchlist: Watchlist, name: string) {
    watchlistsApi.renameWatchlist({
      id: watchlist.id,
      name,
      description: watchlist.description,
    })
      .then(() => {
        setWatchlistsError(null);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  // Cascading (ADR 0076 D5): deleting a whole watchlist drops its company
  // memberships and create_watchlist cannot restore them, so the confirm gate is
  // an InlineConfirm at the call site.
  function deleteWatchlist(watchlist: Watchlist) {
    watchlistsApi.deleteWatchlist(watchlist.id)
      .then(() => {
        setWatchlistsError(null);
        resetDeletedWatchlistFilters(watchlist.id);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function addCompanyToWatchlist(watchlist: Watchlist, company: Company) {
    watchlistsApi.addCompanyToWatchlist({
      watchlistId: watchlist.id,
      companyId: company.id,
    })
      .then(() => {
        setWatchlistsError(null);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function removeCompanyFromWatchlist(watchlist: Watchlist, company: Company) {
    watchlistsApi.removeCompanyFromWatchlist({
      watchlistId: watchlist.id,
      companyId: company.id,
    })
      .then(() => {
        setWatchlistsError(null);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  return {
    addCompanyFromRegistry,
    addCompanyToWatchlist,
    applyRegistryEntryToCompanyForm,
    clearCompanyFormField,
    createCompany,
    createWatchlist,
    deleteWatchlist,
    deleteCompany,
    lookupCompany,
    lookupCompanyIfUseful,
    removeCompanyFromWatchlist,
    renameWatchlist,
    updateCompanyForm,
  };
}
