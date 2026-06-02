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
import type { WatchlistFeedback } from "./appTypes";

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
  setLookupStatus: Dispatch<SetStateAction<string | null>>;
  setSelectedCompanyRegistryTicker: Dispatch<SetStateAction<string | null>>;
  setWatchlistAssignments: Dispatch<SetStateAction<Record<string, string>>>;
  setWatchlistFeedback: Dispatch<SetStateAction<WatchlistFeedback | null>>;
  setWatchlistName: Dispatch<SetStateAction<string>>;
  setWatchlistsError: Dispatch<SetStateAction<string | null>>;
  skipNextCompanyLookupRef: MutableRefObject<boolean>;
  watchlistAssignments: Record<string, string>;
  watchlistName: string;
  watchlists: Watchlist[];
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
  setLookupStatus,
  setSelectedCompanyRegistryTicker,
  setWatchlistAssignments,
  setWatchlistFeedback,
  setWatchlistName,
  setWatchlistsError,
  skipNextCompanyLookupRef,
  watchlistAssignments,
  watchlistName,
  watchlists,
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
    setLookupStatus(`Filled from ${result.source}: ${result.qualifiedTicker}`);
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
    setLookupStatus(`Selected from GPW registry: ${entry.qualifiedTicker}`);
  }

  function lookupCompany() {
    const lookupVersion = companyLookupVersionRef.current;
    setLookupStatus("Looking up GPW registry...");

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
          setLookupStatus("No GPW registry match.");
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

  function deleteCompany(company: Company) {
    const confirmed = window.confirm(`Delete ${company.qualifiedTicker} from your local registry?`);

    if (!confirmed) {
      return;
    }

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

  function createWatchlist(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    watchlistsApi.createWatchlist({
      name: watchlistName,
      description: null,
    })
      .then(() => {
        setWatchlistName("");
        setWatchlistsError(null);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function updateWatchlistAssignment(companyId: string, watchlistId: string) {
    setWatchlistAssignments((current) => ({
      ...current,
      [companyId]: watchlistId,
    }));
  }

  function showWatchlistFeedback(companyId: string, message: string) {
    setWatchlistFeedback({ companyId, message });
    window.setTimeout(() => {
      setWatchlistFeedback((current) => (current?.companyId === companyId ? null : current));
    }, 1200);
  }

  function addCompanyToWatchlist(company: Company) {
    const watchlistId = watchlistAssignments[company.id] || watchlists[0]?.id;
    const assignedWatchlistName = watchlists.find((watchlist) => watchlist.id === watchlistId)?.name;

    if (!watchlistId) {
      setWatchlistsError("Create a watchlist before assigning companies.");
      return;
    }

    watchlistsApi.addCompanyToWatchlist({
      watchlistId,
      companyId: company.id,
    })
      .then(() => {
        setWatchlistsError(null);
        showWatchlistFeedback(company.id, `Assigned to ${assignedWatchlistName ?? "watchlist"}`);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function removeCompanyFromWatchlist(company: Company) {
    const watchlistId = watchlistAssignments[company.id] || watchlists[0]?.id;
    const assignedWatchlistName = watchlists.find((watchlist) => watchlist.id === watchlistId)?.name;

    if (!watchlistId) {
      setWatchlistsError("Select a watchlist before removing companies.");
      return;
    }

    watchlistsApi.removeCompanyFromWatchlist({
      watchlistId,
      companyId: company.id,
    })
      .then(() => {
        setWatchlistsError(null);
        showWatchlistFeedback(company.id, `Removed from ${assignedWatchlistName ?? "watchlist"}`);
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
    deleteCompany,
    lookupCompany,
    lookupCompanyIfUseful,
    removeCompanyFromWatchlist,
    updateCompanyForm,
    updateWatchlistAssignment,
  };
}
