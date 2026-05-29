import {
  type CSSProperties,
  type FormEvent,
  type KeyboardEvent,
  type PointerEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  BookOpenText,
  Building2,
  CheckCircle2,
  FileText,
  Inbox,
  LocateFixed,
  Moon,
  Plus,
  RefreshCw,
  Save,
  Search,
  Settings,
  Sun,
  Trash2,
  Mail,
  MailOpen,
  Video,
} from "lucide-react";

type Theme = "dark" | "light" | "system";

type HealthResponse = {
  status: string;
  version: string;
};

type DatabaseStatus = {
  appliedMigrations: number;
  companies: number;
  sourceAdapters: number;
  settings: number;
};

type Section = "Inbox" | "Companies" | "Notebooks" | "Transcripts" | "Sources" | "Settings";

type InboxStatusFilter = "all" | "unread" | "saved";
type DbRefreshState = "idle" | "refreshing" | "done";

type FeedItem = {
  id: string;
  company: string;
  type: string;
  source: string;
  time: string;
  title: string;
  unread: boolean;
  saved: boolean;
  sourceUrl: string;
  language: string;
  publishedAt: string;
  fetchedAt: string;
  attribution: string;
  summary: string;
};

type Company = {
  id: string;
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string | null;
  cik: string | null;
  lei: string | null;
};

type CompanyForm = {
  exchange: string;
  ticker: string;
  displayName: string;
  isin: string;
};

type CompanyLookupResult = {
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string;
  source: string;
};

type Watchlist = {
  id: string;
  name: string;
  description: string | null;
  companyCount: number;
};

type WatchlistMembership = {
  watchlistId: string;
  watchlistName: string;
  companyId: string;
};

type WatchlistFeedback = {
  companyId: string;
  message: string;
};

type SourceAdapter = {
  id: string;
  displayName: string;
  sourceType: string;
  fetchMode: string;
  enabled: boolean;
  defaultPollIntervalSeconds: number;
  lastSuccessAt: string | null;
  lastErrorAt: string | null;
  lastError: string | null;
  markets: string[];
};

type UserSettings = {
  theme: Theme;
  accentPalette: string;
  pollIntervalSeconds: number;
  settingsSource: string;
  settingsImportExportFormat: string;
  yamlImportExportStatus: string;
  aiProviders: {
    youtubeTranscriptionProvider: string;
    generalAnalysisProvider: string | null;
  };
  aiAnalysisMode: string;
};

function databaseIndicatorClass(status: DatabaseStatus | null, error: string | null) {
  if (error) {
    return "status-dot status-danger";
  }

  if (status) {
    return "status-dot status-ok";
  }

  return "status-dot status-warn";
}

const sections = [
  { label: "Inbox" as const, icon: Inbox },
  { label: "Companies" as const, icon: Building2 },
  { label: "Notebooks" as const, icon: BookOpenText },
  { label: "Transcripts" as const, icon: Video },
  { label: "Sources" as const, icon: Activity },
  { label: "Settings" as const, icon: Settings },
];

const detailPaneMinWidth = 300;
const detailPaneMaxWidth = 620;
const detailPaneDefaultWidth = 360;

function resolveTheme(theme: Theme) {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }

  return theme;
}

export function App() {
  const contentGridRef = useRef<HTMLElement | null>(null);
  const [activeSection, setActiveSection] = useState<Section>("Inbox");
  const [theme, setTheme] = useState<Theme>("dark");
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [databaseStatus, setDatabaseStatus] = useState<DatabaseStatus | null>(null);
  const [databaseError, setDatabaseError] = useState<string | null>(null);
  const [companies, setCompanies] = useState<Company[]>([]);
  const [companiesError, setCompaniesError] = useState<string | null>(null);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
  const [watchlistMemberships, setWatchlistMemberships] = useState<WatchlistMembership[]>([]);
  const [watchlistsError, setWatchlistsError] = useState<string | null>(null);
  const [watchlistName, setWatchlistName] = useState("");
  const [watchlistAssignments, setWatchlistAssignments] = useState<Record<string, string>>({});
  const [watchlistFeedback, setWatchlistFeedback] = useState<WatchlistFeedback | null>(null);
  const [inboxWatchlistFilter, setInboxWatchlistFilter] = useState("all");
  const [inboxCompanyFilter, setInboxCompanyFilter] = useState("all");
  const [inboxTypeFilter, setInboxTypeFilter] = useState("all");
  const [inboxSourceFilter, setInboxSourceFilter] = useState("all");
  const [inboxStatusFilter, setInboxStatusFilter] = useState<InboxStatusFilter>("all");
  const [feedState, setFeedState] = useState<FeedItem[]>([]);
  const [feedError, setFeedError] = useState<string | null>(null);
  const [sourceAdapters, setSourceAdapters] = useState<SourceAdapter[]>([]);
  const [sourceAdaptersError, setSourceAdaptersError] = useState<string | null>(null);
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [selectedFeedItemId, setSelectedFeedItemId] = useState<string | null>(null);
  const [detailPaneWidth, setDetailPaneWidth] = useState(detailPaneDefaultWidth);
  const [dbRefreshState, setDbRefreshState] = useState<DbRefreshState>("idle");
  const [searchQuery, setSearchQuery] = useState("");
  const [lookupStatus, setLookupStatus] = useState<string | null>(null);
  const [companyForm, setCompanyForm] = useState<CompanyForm>({
    exchange: "GPW",
    ticker: "",
    displayName: "",
    isin: "",
  });

  const effectiveTheme = useMemo(() => resolveTheme(theme), [theme]);
  const membershipsByCompany = useMemo(() => {
    return watchlistMemberships.reduce<Record<string, WatchlistMembership[]>>(
      (grouped, membership) => {
        grouped[membership.companyId] = grouped[membership.companyId] ?? [];
        grouped[membership.companyId].push(membership);
        return grouped;
      },
      {},
    );
  }, [watchlistMemberships]);
  const companiesById = useMemo(() => {
    return companies.reduce<Record<string, Company>>((indexed, company) => {
      indexed[company.id] = company;
      return indexed;
    }, {});
  }, [companies]);
  const feedTypes = useMemo(() => {
    return Array.from(new Set(feedState.map((item) => item.type))).sort();
  }, [feedState]);
  const feedSources = useMemo(() => {
    return Array.from(new Set(feedState.map((item) => item.source))).sort();
  }, [feedState]);
  const filteredFeedItems = useMemo(() => {
    const normalizedSearch = searchQuery.trim().toLowerCase();
    const allowedTickers =
      inboxWatchlistFilter === "all"
        ? null
        : new Set(
            watchlistMemberships
              .filter((membership) => membership.watchlistId === inboxWatchlistFilter)
              .map((membership) => companiesById[membership.companyId]?.qualifiedTicker)
              .filter((qualifiedTicker): qualifiedTicker is string => Boolean(qualifiedTicker)),
          );

    return feedState.filter((item) => {
      const watchlistMatches = allowedTickers ? allowedTickers.has(item.company) : true;
      const companyMatches = inboxCompanyFilter === "all" || item.company === inboxCompanyFilter;
      const typeMatches = inboxTypeFilter === "all" || item.type === inboxTypeFilter;
      const sourceMatches = inboxSourceFilter === "all" || item.source === inboxSourceFilter;
      const statusMatches =
        inboxStatusFilter === "all" ||
        (inboxStatusFilter === "unread" && item.unread) ||
        (inboxStatusFilter === "saved" && item.saved);
      const searchMatches =
        normalizedSearch.length === 0 ||
        [item.company, item.title, item.source, item.type, item.summary]
          .join(" ")
          .toLowerCase()
          .includes(normalizedSearch);

      return (
        watchlistMatches &&
        companyMatches &&
        typeMatches &&
        sourceMatches &&
        statusMatches &&
        searchMatches
      );
    });
  }, [
    companiesById,
    feedState,
    inboxCompanyFilter,
    inboxSourceFilter,
    inboxStatusFilter,
    inboxTypeFilter,
    inboxWatchlistFilter,
    searchQuery,
    watchlistMemberships,
  ]);
  const selectedFeedItem =
    filteredFeedItems.find((item) => item.id === selectedFeedItemId) ?? filteredFeedItems[0] ?? null;
  const hasActiveInboxFilters =
    searchQuery.trim().length > 0 ||
    inboxWatchlistFilter !== "all" ||
    inboxCompanyFilter !== "all" ||
    inboxTypeFilter !== "all" ||
    inboxSourceFilter !== "all" ||
    inboxStatusFilter !== "all";

  useEffect(() => {
    document.documentElement.dataset.theme = effectiveTheme;
  }, [effectiveTheme]);

  useEffect(() => {
    invoke<HealthResponse>("health")
      .then((response) => {
        setHealth(response);
        setHealthError(null);
      })
      .catch((error) => {
        setHealth(null);
        setHealthError(String(error));
      });
  }, []);

  useEffect(() => {
    invoke<DatabaseStatus>("database_status")
      .then((response) => {
        setDatabaseStatus(response);
        setDatabaseError(null);
      })
      .catch((error) => {
        setDatabaseStatus(null);
        setDatabaseError(String(error));
      });
  }, []);

  function refreshDatabaseStatus() {
    return invoke<DatabaseStatus>("database_status")
      .then((response) => {
        setDatabaseStatus(response);
        setDatabaseError(null);
      })
      .catch((error) => {
        setDatabaseStatus(null);
        setDatabaseError(String(error));
      });
  }

  function refreshCompanies() {
    return invoke<Company[]>("list_companies")
      .then((response) => {
        setCompanies(response);
        setCompaniesError(null);
      })
      .catch((error) => {
        setCompanies([]);
        setCompaniesError(String(error));
      });
  }

  function refreshWatchlists() {
    return invoke<Watchlist[]>("list_watchlists")
      .then((response) => {
        setWatchlists(response);
        setWatchlistsError(null);
        setWatchlistAssignments((current) => {
          const fallback = response[0]?.id ?? "";
          const next = { ...current };

          for (const company of companies) {
            if (!next[company.id]) {
              next[company.id] = fallback;
            }
          }

          return next;
        });
      })
      .catch((error) => {
        setWatchlists([]);
        setWatchlistsError(String(error));
      });
  }

  function refreshWatchlistMemberships() {
    return invoke<WatchlistMembership[]>("list_watchlist_memberships")
      .then((response) => {
        setWatchlistMemberships(response);
        setWatchlistsError(null);
      })
      .catch((error) => {
        setWatchlistMemberships([]);
        setWatchlistsError(String(error));
      });
  }

  function refreshFeedItems() {
    return invoke<FeedItem[]>("list_feed_items")
      .then((response) => {
        setFeedState(response);
        setFeedError(null);
        setSelectedFeedItemId((current) => {
          if (current && response.some((item) => item.id === current)) {
            return current;
          }

          return response[0]?.id ?? null;
        });
      })
      .catch((error) => {
        setFeedState([]);
        setFeedError(String(error));
      });
  }

  function refreshSourceAdapters() {
    return invoke<SourceAdapter[]>("list_source_adapters")
      .then((response) => {
        setSourceAdapters(response);
        setSourceAdaptersError(null);
      })
      .catch((error) => {
        setSourceAdapters([]);
        setSourceAdaptersError(String(error));
      });
  }

  function refreshSettings() {
    return invoke<UserSettings>("get_settings")
      .then((response) => {
        setSettings(response);
        setTheme(response.theme);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettings(null);
        setSettingsError(String(error));
      });
  }

  function refreshDatabaseBackedViews() {
    setDbRefreshState("refreshing");

    Promise.all([
      refreshDatabaseStatus(),
      refreshCompanies(),
      refreshWatchlists(),
      refreshWatchlistMemberships(),
      refreshFeedItems(),
      refreshSourceAdapters(),
      refreshSettings(),
    ]).then(() => {
      setDbRefreshState("done");
      window.setTimeout(() => {
        setDbRefreshState("idle");
      }, 900);
    });
  }

  useEffect(() => {
    refreshCompanies();
    refreshWatchlists();
    refreshWatchlistMemberships();
    refreshFeedItems();
    refreshSourceAdapters();
    refreshSettings();
  }, []);

  function updateTheme(nextTheme: Theme) {
    setTheme(nextTheme);

    invoke<UserSettings>("update_settings", {
      input: {
        theme: nextTheme,
      },
    })
      .then((response) => {
        setSettings(response);
        setTheme(response.theme);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function updateCompanyForm(field: keyof CompanyForm, value: string) {
    setCompanyForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function applyLookupResult(result: CompanyLookupResult) {
    setCompanyForm({
      exchange: result.exchange,
      ticker: result.ticker,
      displayName: result.displayName,
      isin: result.isin,
    });
    setLookupStatus(`Filled from ${result.source}: ${result.qualifiedTicker}`);
  }

  function lookupCompany() {
    setLookupStatus("Looking up local fixtures...");

    invoke<CompanyLookupResult | null>("lookup_company", {
      input: {
        exchange: companyForm.exchange,
        ticker: companyForm.ticker || null,
        displayName: companyForm.displayName || null,
        isin: companyForm.isin || null,
      },
    })
      .then((result) => {
        if (result) {
          applyLookupResult(result);
        } else {
          setLookupStatus("No local fixture match.");
        }
        setCompaniesError(null);
      })
      .catch((error) => {
        setLookupStatus(null);
        setCompaniesError(String(error));
      });
  }

  function lookupCompanyIfUseful() {
    if (companyForm.ticker || companyForm.displayName.length >= 3 || companyForm.isin) {
      lookupCompany();
    }
  }

  function createCompany(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    invoke<Company>("create_company", {
      input: {
        exchange: companyForm.exchange,
        ticker: companyForm.ticker,
        displayName: companyForm.displayName,
        isin: companyForm.isin || null,
        cik: null,
        lei: null,
      },
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

  function deleteCompany(company: Company) {
    const confirmed = window.confirm(`Delete ${company.qualifiedTicker} from your local registry?`);

    if (!confirmed) {
      return;
    }

    invoke<void>("delete_company", { companyId: company.id })
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

    invoke<Watchlist>("create_watchlist", {
      input: {
        name: watchlistName,
        description: null,
      },
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

  function addCompanyToWatchlist(company: Company) {
    const watchlistId = watchlistAssignments[company.id] || watchlists[0]?.id;
    const watchlistName = watchlists.find((watchlist) => watchlist.id === watchlistId)?.name;

    if (!watchlistId) {
      setWatchlistsError("Create a watchlist before assigning companies.");
      return;
    }

    invoke<void>("add_company_to_watchlist", {
      input: {
        watchlistId,
        companyId: company.id,
      },
    })
      .then(() => {
        setWatchlistsError(null);
        showWatchlistFeedback(company.id, `Assigned to ${watchlistName ?? "watchlist"}`);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function removeCompanyFromWatchlist(company: Company) {
    const watchlistId = watchlistAssignments[company.id] || watchlists[0]?.id;
    const watchlistName = watchlists.find((watchlist) => watchlist.id === watchlistId)?.name;

    if (!watchlistId) {
      setWatchlistsError("Select a watchlist before removing companies.");
      return;
    }

    invoke<void>("remove_company_from_watchlist", {
      input: {
        watchlistId,
        companyId: company.id,
      },
    })
      .then(() => {
        setWatchlistsError(null);
        showWatchlistFeedback(company.id, `Removed from ${watchlistName ?? "watchlist"}`);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function showWatchlistFeedback(companyId: string, message: string) {
    setWatchlistFeedback({ companyId, message });
    window.setTimeout(() => {
      setWatchlistFeedback((current) => (current?.companyId === companyId ? null : current));
    }, 1200);
  }

  function updateSelectedFeedItem(update: (item: FeedItem) => FeedItem) {
    if (!selectedFeedItem) {
      return;
    }

    const nextItem = update(selectedFeedItem);

    invoke<FeedItem>("update_feed_item_state", {
      input: {
        id: nextItem.id,
        read: !nextItem.unread,
        saved: nextItem.saved,
      },
    })
      .then((response) => {
        setFeedState((current) =>
          current.map((item) => (item.id === response.id ? response : item)),
        );
        setFeedError(null);
      })
      .catch((error) => {
        setFeedError(String(error));
      });
  }

  function toggleFeedItemReadState(item: FeedItem) {
    invoke<FeedItem>("update_feed_item_state", {
      input: {
        id: item.id,
        read: item.unread,
        saved: item.saved,
      },
    })
      .then((response) => {
        setFeedState((current) =>
          current.map((feedItem) => (feedItem.id === response.id ? response : feedItem)),
        );
        setFeedError(null);
      })
      .catch((error) => {
        setFeedError(String(error));
      });
  }

  function clearInboxFilters() {
    setSearchQuery("");
    setInboxWatchlistFilter("all");
    setInboxCompanyFilter("all");
    setInboxTypeFilter("all");
    setInboxSourceFilter("all");
    setInboxStatusFilter("all");
  }

  function clampDetailPaneWidth(width: number) {
    const gridWidth = contentGridRef.current?.getBoundingClientRect().width ?? 0;
    const responsiveMaxWidth = gridWidth > 0 ? Math.min(detailPaneMaxWidth, gridWidth * 0.55) : detailPaneMaxWidth;

    return Math.round(Math.min(Math.max(width, detailPaneMinWidth), responsiveMaxWidth));
  }

  function resizeDetailPaneFromPointer(clientX: number) {
    const gridBounds = contentGridRef.current?.getBoundingClientRect();

    if (!gridBounds) {
      return;
    }

    setDetailPaneWidth(clampDetailPaneWidth(gridBounds.right - clientX));
  }

  function startDetailPaneResize(event: PointerEvent<HTMLDivElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    resizeDetailPaneFromPointer(event.clientX);
  }

  function resizeDetailPane(event: PointerEvent<HTMLDivElement>) {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) {
      return;
    }

    resizeDetailPaneFromPointer(event.clientX);
  }

  function stopDetailPaneResize(event: PointerEvent<HTMLDivElement>) {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function resizeDetailPaneWithKeyboard(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      setDetailPaneWidth((current) => clampDetailPaneWidth(current + 24));
    }

    if (event.key === "ArrowRight") {
      event.preventDefault();
      setDetailPaneWidth((current) => clampDetailPaneWidth(current - 24));
    }
  }

  function formatPollInterval(seconds: number) {
    if (seconds % 60 === 0) {
      return `${seconds / 60} min`;
    }

    return `${seconds}s`;
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">B</div>
          <div>
            <div className="brand-title">Brawler</div>
            <div className="brand-subtitle">codename</div>
          </div>
        </div>

        <nav className="nav-list">
          {sections.map((section) => {
            const Icon = section.icon;
            return (
              <button
                className={activeSection === section.label ? "nav-item nav-item-active" : "nav-item"}
                key={section.label}
                onClick={() => setActiveSection(section.label)}
                type="button"
                title={section.label}
              >
                <Icon size={18} aria-hidden="true" />
                <span>{section.label}</span>
              </button>
            );
          })}
        </nav>

        <div className="sidebar-footer">
          <div className="status-pill" title="Rust command boundary health">
            <span className={health ? "status-dot status-ok" : "status-dot status-warn"} />
            {health ? `${health.status} ${health.version}` : "health pending"}
          </div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="search-box">
            <Search size={18} aria-hidden="true" />
            <input
              aria-label="Search feed"
              placeholder="Search companies, feed, notes"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
            />
          </div>
          <div className="topbar-actions">
            <div className="ai-mode-pill" title="AI mode: source-grounded decision support">
              <span>AI</span>
              <strong>source</strong>
            </div>
            <button
              aria-label={
                dbRefreshState === "refreshing"
                  ? "Refreshing database-backed views"
                  : "Refresh database-backed views"
              }
              className={[
                "db-status-pill",
                dbRefreshState === "refreshing" ? "icon-button-spinning" : "",
                dbRefreshState === "done" ? "db-status-pill-success" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              disabled={dbRefreshState === "refreshing"}
              onClick={refreshDatabaseBackedViews}
              type="button"
              title={
                dbRefreshState === "refreshing"
                  ? "Refreshing database-backed views"
                  : dbRefreshState === "done"
                    ? "Database-backed views refreshed"
                    : databaseStatus
                  ? `Database active: ${databaseStatus.appliedMigrations} migration, ${databaseStatus.sourceAdapters} source, ${databaseStatus.settings} settings`
                  : databaseError
                    ? `Database error: ${databaseError}`
                    : "Database pending"
              }
            >
              <span
                aria-label={
                  databaseError
                    ? "Database connection failed"
                    : databaseStatus
                      ? "Database connection active"
                      : "Database connection pending"
                }
                className={databaseIndicatorClass(databaseStatus, databaseError)}
                role="status"
              />
              <span>DB</span>
              {dbRefreshState === "done" ? (
                <CheckCircle2 size={14} aria-hidden="true" />
              ) : (
                <RefreshCw size={14} aria-hidden="true" />
              )}
            </button>
            <button
              aria-label="Refresh sources unavailable"
              className="icon-button"
              disabled
              type="button"
              title="Source refresh will pull latest remote feeds after source ingestion is wired"
            >
              <RefreshCw size={18} />
            </button>
            <label className="theme-control" title="Theme">
              {effectiveTheme === "dark" ? <Moon size={16} /> : <Sun size={16} />}
              <select value={theme} onChange={(event) => updateTheme(event.target.value as Theme)}>
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">System</option>
              </select>
            </label>
          </div>
        </header>

        <section
          className={activeSection === "Inbox" ? "content-grid" : "content-grid content-grid-single"}
          ref={contentGridRef}
          style={
            activeSection === "Inbox"
              ? ({ "--detail-pane-width": `${detailPaneWidth}px` } as CSSProperties)
              : undefined
          }
        >
          {activeSection === "Inbox" ? (
            <section className="feed-panel" aria-labelledby="inbox-title">
              <div className="panel-header">
                <div>
                  <h1 id="inbox-title">Inbox</h1>
                  <p>Stored feed items filtered by local companies and watchlists.</p>
                </div>
                <div className="segmented-control" aria-label="Feed status filter">
                  <button
                    type="button"
                    className={inboxStatusFilter === "all" ? "segment-active" : undefined}
                    onClick={() => setInboxStatusFilter("all")}
                  >
                    All
                  </button>
                  <button
                    type="button"
                    className={inboxStatusFilter === "unread" ? "segment-active" : undefined}
                    onClick={() => setInboxStatusFilter("unread")}
                  >
                    Unread
                  </button>
                  <button
                    type="button"
                    className={inboxStatusFilter === "saved" ? "segment-active" : undefined}
                    onClick={() => setInboxStatusFilter("saved")}
                  >
                    Saved
                  </button>
                </div>
              </div>

              <div className="filter-reset-row" aria-label="Inbox filter reset">
                <button
                  className="secondary-button compact-button"
                  disabled={!hasActiveInboxFilters}
                  onClick={clearInboxFilters}
                  type="button"
                >
                  Clear filters
                </button>
              </div>

              <div className="filter-toolbar" aria-label="Inbox filters">
                <label>
                  Watchlist
                  <select
                    aria-label="Inbox watchlist"
                    value={inboxWatchlistFilter}
                    onChange={(event) => setInboxWatchlistFilter(event.target.value)}
                  >
                    <option value="all">All watchlists</option>
                    {watchlists.map((watchlist) => (
                      <option key={watchlist.id} value={watchlist.id}>
                        {watchlist.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Company
                  <select
                    aria-label="Inbox company"
                    value={inboxCompanyFilter}
                    onChange={(event) => setInboxCompanyFilter(event.target.value)}
                  >
                    <option value="all">All companies</option>
                    {companies.map((company) => (
                      <option key={company.id} value={company.qualifiedTicker}>
                        {company.qualifiedTicker}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Type
                  <select
                    aria-label="Inbox type"
                    value={inboxTypeFilter}
                    onChange={(event) => setInboxTypeFilter(event.target.value)}
                  >
                    <option value="all">All types</option>
                    {feedTypes.map((type) => (
                      <option key={type} value={type}>
                        {type}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Source
                  <select
                    aria-label="Inbox source"
                    value={inboxSourceFilter}
                    onChange={(event) => setInboxSourceFilter(event.target.value)}
                  >
                    <option value="all">All sources</option>
                    {feedSources.map((source) => (
                      <option key={source} value={source}>
                        {source}
                      </option>
                    ))}
                  </select>
                </label>
              </div>

              <div className="feed-list" aria-label="Feed items">
                {filteredFeedItems.map((item) => (
                  <article
                    className={[
                      "feed-row",
                      item.unread ? "unread" : "",
                      selectedFeedItem?.id === item.id ? "feed-row-selected" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    key={item.id}
                    onClick={() => setSelectedFeedItemId(item.id)}
                    onDoubleClick={() => toggleFeedItemReadState(item)}
                  >
                    <div className="feed-row-main">
                      <div className="feed-meta">
                        <span>{item.company}</span>
                        <span>{item.type}</span>
                        <span>{item.source}</span>
                        <span>{item.time}</span>
                      </div>
                      <h2>{item.title}</h2>
                    </div>
                    {item.saved ? <span className="saved-pill">Saved</span> : null}
                    {item.unread ? <span className="unread-dot" title="Unread" /> : null}
                  </article>
                ))}
                {filteredFeedItems.length === 0 ? (
                  <div className="empty-state">
                    <span>No feed items for selected filters.</span>
                    {hasActiveInboxFilters ? (
                      <button
                        className="secondary-button compact-button"
                        onClick={clearInboxFilters}
                        type="button"
                      >
                        Clear filters
                      </button>
                    ) : null}
                  </div>
                ) : null}
                {feedError ? <p className="error-text">Feed command failed: {feedError}</p> : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Companies" ? (
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
                      <button className="primary-button" type="submit">
                        <Plus size={16} />
                        Create
                      </button>
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
                      <div className="empty-state">No watchlists yet.</div>
                    ) : null}
                  </div>

                  {watchlistsError ? (
                    <p className="error-text">Watchlist command failed: {watchlistsError}</p>
                  ) : null}
                </section>

                <form className="company-form" onSubmit={createCompany}>
                  <label>
                    Exchange
                    <input
                      required
                      value={companyForm.exchange}
                      onChange={(event) => updateCompanyForm("exchange", event.target.value)}
                    />
                  </label>
                  <label>
                    Ticker
                    <input
                      required
                      value={companyForm.ticker}
                      onBlur={lookupCompanyIfUseful}
                      onChange={(event) => updateCompanyForm("ticker", event.target.value)}
                      placeholder="CDR"
                    />
                  </label>
                  <label>
                    Name
                    <input
                      required
                      value={companyForm.displayName}
                      onBlur={lookupCompanyIfUseful}
                      onChange={(event) => updateCompanyForm("displayName", event.target.value)}
                      placeholder="CD PROJEKT S.A."
                    />
                  </label>
                  <label>
                    ISIN
                    <input
                      value={companyForm.isin}
                      onBlur={lookupCompanyIfUseful}
                      onChange={(event) => updateCompanyForm("isin", event.target.value)}
                      placeholder="PLOPTTC00011"
                    />
                  </label>
                  <button className="secondary-button" onClick={lookupCompany} type="button">
                    <LocateFixed size={16} />
                    Lookup
                  </button>
                  <button className="primary-button" type="submit">
                    <Plus size={16} />
                    Add
                  </button>
                </form>

                <div className="company-list" aria-label="Companies list">
                  {companies.map((company) => (
                    <article className="company-row" key={company.id}>
                      <div className="company-row-main">
                        <h2>{company.qualifiedTicker}</h2>
                        <p>{company.displayName}</p>
                        <div
                          className="membership-list"
                          aria-label={`Watchlist memberships for ${company.qualifiedTicker}`}
                        >
                          {(membershipsByCompany[company.id] ?? []).map((membership) => (
                            <span className="membership-chip" key={membership.watchlistId}>
                              {membership.watchlistName}
                            </span>
                          ))}
                          {(membershipsByCompany[company.id] ?? []).length === 0 ? (
                            <span className="membership-empty">No watchlist</span>
                          ) : null}
                        </div>
                      </div>
                      <div className="company-row-actions">
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
                        <button
                          className="secondary-button compact-button assign-button"
                          disabled={watchlists.length === 0}
                          onClick={() => addCompanyToWatchlist(company)}
                          type="button"
                        >
                          Assign
                        </button>
                        <button
                          className="secondary-button compact-button remove-button"
                          disabled={watchlists.length === 0}
                          onClick={() => removeCompanyFromWatchlist(company)}
                          type="button"
                        >
                          Remove
                        </button>
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
                        <button
                          className="icon-button danger-button"
                          onClick={() => deleteCompany(company)}
                          title={`Delete ${company.qualifiedTicker}`}
                          type="button"
                        >
                          <Trash2 size={16} />
                        </button>
                      </div>
                    </article>
                  ))}
                  {companies.length === 0 ? (
                    <div className="empty-state">No companies yet.</div>
                  ) : null}
                </div>

                {companiesError ? (
                  <p className="error-text">Companies command failed: {companiesError}</p>
                ) : null}
                {lookupStatus ? <p className="helper-text">{lookupStatus}</p> : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Sources" ? (
            <section className="feed-panel" aria-labelledby="sources-title">
              <div className="panel-header">
                <div>
                  <h1 id="sources-title">Sources</h1>
                  <p>Local source adapter status before remote ingestion is wired.</p>
                </div>
              </div>

              <div className="sources-layout" aria-label="Source adapters">
                {sourceAdapters.map((adapter) => (
                  <article className="source-row" key={adapter.id}>
                    <div className="source-row-main">
                      <div className="source-title-line">
                        <span
                          className={adapter.enabled ? "status-dot status-ok" : "status-dot status-warn"}
                          title={adapter.enabled ? "Enabled" : "Disabled"}
                        />
                        <h2>{adapter.displayName}</h2>
                        <span className="source-id">{adapter.id}</span>
                      </div>
                      <p>
                        {adapter.sourceType} · {adapter.fetchMode}
                      </p>
                      <div className="source-chip-list" aria-label={`Markets for ${adapter.displayName}`}>
                        {adapter.markets.map((market) => (
                          <span className="membership-chip" key={market}>
                            {market}
                          </span>
                        ))}
                        {adapter.markets.length === 0 ? (
                          <span className="membership-empty">No markets</span>
                        ) : null}
                      </div>
                    </div>
                    <dl className="source-status-grid">
                      <div>
                        <dt>Poll</dt>
                        <dd>{formatPollInterval(adapter.defaultPollIntervalSeconds)}</dd>
                      </div>
                      <div>
                        <dt>Last success</dt>
                        <dd>{adapter.lastSuccessAt ?? "Never"}</dd>
                      </div>
                      <div>
                        <dt>Last error</dt>
                        <dd>{adapter.lastErrorAt ?? "None"}</dd>
                      </div>
                      <div>
                        <dt>Status</dt>
                        <dd>{adapter.lastError ?? (adapter.enabled ? "Ready" : "Disabled")}</dd>
                      </div>
                    </dl>
                  </article>
                ))}
                {sourceAdapters.length === 0 ? (
                  <div className="empty-state">No source adapters configured.</div>
                ) : null}
                {sourceAdaptersError ? (
                  <p className="error-text">Source command failed: {sourceAdaptersError}</p>
                ) : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Settings" ? (
            <section className="feed-panel" aria-labelledby="settings-title">
              <div className="panel-header">
                <div>
                  <h1 id="settings-title">Settings</h1>
                  <p>SQLite-backed local runtime settings.</p>
                </div>
              </div>

              <div className="settings-layout" aria-label="Application settings">
                <div className="settings-row">
                  <label>
                    Theme
                    <select
                      aria-label="Settings theme"
                      value={theme}
                      onChange={(event) => updateTheme(event.target.value as Theme)}
                    >
                      <option value="dark">Dark</option>
                      <option value="light">Light</option>
                      <option value="system">System</option>
                    </select>
                  </label>
                  <div className="settings-summary">
                    <span>Source</span>
                    <strong>{settings?.settingsSource ?? "sqlite"}</strong>
                  </div>
                  <div className="settings-summary">
                    <span>Palette</span>
                    <strong>{settings?.accentPalette ?? "night-neon"}</strong>
                  </div>
                  <div className="settings-summary">
                    <span>Poll</span>
                    <strong>{formatPollInterval(settings?.pollIntervalSeconds ?? 900)}</strong>
                  </div>
                </div>

                <dl className="settings-grid">
                  <div>
                    <dt>YAML import/export</dt>
                    <dd>{settings?.yamlImportExportStatus ?? "accepted_deferred"}</dd>
                  </div>
                  <div>
                    <dt>Settings format</dt>
                    <dd>{settings?.settingsImportExportFormat ?? "yaml"}</dd>
                  </div>
                  <div>
                    <dt>YouTube transcription</dt>
                    <dd>{settings?.aiProviders.youtubeTranscriptionProvider ?? "gemini"}</dd>
                  </div>
                  <div>
                    <dt>General AI provider</dt>
                    <dd>{settings?.aiProviders.generalAnalysisProvider ?? "Not configured"}</dd>
                  </div>
                  <div>
                    <dt>AI analysis mode</dt>
                    <dd>{settings?.aiAnalysisMode ?? "source_grounded"}</dd>
                  </div>
                </dl>

                {settingsError ? (
                  <p className="error-text">Settings command failed: {settingsError}</p>
                ) : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Inbox" ? (
            <div
              aria-label="Resize feed details"
              aria-orientation="vertical"
              aria-valuemax={detailPaneMaxWidth}
              aria-valuemin={detailPaneMinWidth}
              aria-valuenow={detailPaneWidth}
              className="pane-resizer"
              onKeyDown={resizeDetailPaneWithKeyboard}
              onPointerDown={startDetailPaneResize}
              onPointerMove={resizeDetailPane}
              onPointerUp={stopDetailPaneResize}
              role="separator"
              tabIndex={0}
              title="Drag to resize feed details"
            />
          ) : null}

          {activeSection === "Inbox" ? (
            <aside className="detail-pane" aria-label="Feed item details">
              <div className="detail-icon">
                <FileText size={24} />
              </div>
              {selectedFeedItem ? (
                <>
                  <h2>{selectedFeedItem.title}</h2>
                  <p>{selectedFeedItem.summary}</p>
                  <div className="detail-actions" aria-label="Feed item actions">
                    <button
                      className="secondary-button compact-button"
                      onClick={() =>
                        updateSelectedFeedItem((item) => ({
                          ...item,
                          unread: !item.unread,
                        }))
                      }
                      type="button"
                    >
                      {selectedFeedItem.unread ? <MailOpen size={15} /> : <Mail size={15} />}
                      {selectedFeedItem.unread ? "Mark read" : "Mark unread"}
                    </button>
                    <button
                      className="secondary-button compact-button"
                      onClick={() =>
                        updateSelectedFeedItem((item) => ({
                          ...item,
                          saved: !item.saved,
                        }))
                      }
                      type="button"
                    >
                      <Save size={15} />
                      {selectedFeedItem.saved ? "Unsave" : "Save"}
                    </button>
                    <a
                      className="secondary-button compact-button"
                      href={selectedFeedItem.sourceUrl}
                      rel="noreferrer"
                      target="_blank"
                    >
                      Open source
                    </a>
                  </div>
                  <dl>
                    <div>
                      <dt>Company</dt>
                      <dd>{selectedFeedItem.company}</dd>
                    </div>
                    <div>
                      <dt>Source</dt>
                      <dd>{selectedFeedItem.source}</dd>
                    </div>
                    <div>
                      <dt>Source URL</dt>
                      <dd>
                        <a href={selectedFeedItem.sourceUrl} rel="noreferrer" target="_blank">
                          {selectedFeedItem.sourceUrl}
                        </a>
                      </dd>
                    </div>
                    <div>
                      <dt>Published</dt>
                      <dd>{selectedFeedItem.publishedAt}</dd>
                    </div>
                    <div>
                      <dt>Fetched</dt>
                      <dd>{selectedFeedItem.fetchedAt}</dd>
                    </div>
                    <div>
                      <dt>Attribution</dt>
                      <dd>{selectedFeedItem.attribution}</dd>
                    </div>
                  </dl>
                </>
              ) : (
                <>
                  <h2>No item selected</h2>
                  <p>Select a feed item to inspect source details and provenance.</p>
                </>
              )}
              {healthError ? <p className="error-text">Health command failed: {healthError}</p> : null}
              {databaseError ? (
                <p className="error-text">Database command failed: {databaseError}</p>
              ) : null}
            </aside>
          ) : null}
        </section>
      </main>
    </div>
  );
}
