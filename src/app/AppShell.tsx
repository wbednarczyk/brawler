import type { ReactNode } from "react";
import { Activity, CheckCircle2, Moon, RefreshCw, Search, Sun } from "lucide-react";
import type {
  DatabaseStatus,
  HealthResponse,
  SourceIngestionResult,
  SourceRefreshTrigger,
  Theme,
} from "../api/types";
import type { DbRefreshState, SourceRefreshState } from "./appTypes";
import { sections, type Section } from "./navigation";
import { databaseIndicatorClass } from "./theme";

type SourceStatusTone = "ok" | "warn" | "danger";

export type SourceStatusSummary = {
  label: string;
  title: string;
  tone: SourceStatusTone;
};

type AppShellProps = {
  activeSection: Section;
  children: ReactNode;
  databaseError: string | null;
  databaseStatus: DatabaseStatus | null;
  dbRefreshState: DbRefreshState;
  effectiveTheme: "dark" | "light";
  health: HealthResponse | null;
  refreshDatabaseBackedViews: () => void;
  refreshSources: (trigger: SourceRefreshTrigger) => void;
  searchQuery: string;
  setActiveSection: (section: Section) => void;
  setSearchQuery: (query: string) => void;
  sourceRefreshError: string | null;
  sourceRefreshResult: SourceIngestionResult | null;
  sourceRefreshState: SourceRefreshState;
  sourceStatusSummary: SourceStatusSummary;
  theme: Theme;
  totalUnreadFeedItems: number;
  updateTheme: (theme: Theme) => void;
  openSourceStatus: () => void;
};

export function AppShell({
  activeSection,
  children,
  databaseError,
  databaseStatus,
  dbRefreshState,
  effectiveTheme,
  health,
  refreshDatabaseBackedViews,
  refreshSources,
  searchQuery,
  setActiveSection,
  setSearchQuery,
  sourceRefreshError,
  sourceRefreshResult,
  sourceRefreshState,
  sourceStatusSummary,
  theme,
  totalUnreadFeedItems,
  updateTheme,
  openSourceStatus,
}: AppShellProps) {
  function sourceRefreshButtonLabel() {
    if (sourceRefreshState === "refreshing") {
      return "Refreshing sources";
    }

    if (sourceRefreshState === "done") {
      return "Sources refreshed";
    }

    if (sourceRefreshError) {
      return "Source refresh failed";
    }

    return "Refresh sources";
  }

  function sourceRefreshButtonTitle() {
    if (sourceRefreshError) {
      return `Source refresh failed: ${sourceRefreshError}`;
    }

    if (sourceRefreshResult) {
      return `Last refresh: ${sourceRefreshResult.itemsMatched}/${sourceRefreshResult.itemsFetched} matched`;
    }

    return "Fetch GPW ESPI/EBI public listings";
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
                {section.label === "Inbox" && totalUnreadFeedItems > 0 ? (
                  <span className="nav-badge" aria-label={`${totalUnreadFeedItems} unread feed item`}>
                    {totalUnreadFeedItems}
                  </span>
                ) : null}
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
              aria-label="Open source status"
              className={[
                "source-status-pill",
                sourceStatusSummary.tone === "ok" ? "source-status-pill-ok" : "",
                sourceStatusSummary.tone === "warn" ? "source-status-pill-warn" : "",
                sourceStatusSummary.tone === "danger" ? "source-status-pill-danger" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              onClick={openSourceStatus}
              title={sourceStatusSummary.title}
              type="button"
            >
              <Activity size={14} aria-hidden="true" />
              <span>Sources</span>
              <strong>{sourceStatusSummary.label}</strong>
            </button>
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
              aria-label={sourceRefreshButtonLabel()}
              className={[
                "icon-button",
                sourceRefreshState === "refreshing" ? "icon-button-spinning" : "",
                sourceRefreshState === "done" ? "db-status-pill-success" : "",
                sourceRefreshError ? "source-refresh-button-danger" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              disabled={sourceRefreshState === "refreshing"}
              onClick={() => {
                void refreshSources("manual");
              }}
              type="button"
              title={sourceRefreshButtonTitle()}
            >
              {sourceRefreshState === "done" ? <CheckCircle2 size={18} /> : <RefreshCw size={18} />}
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

        {children}
      </main>
    </div>
  );
}
