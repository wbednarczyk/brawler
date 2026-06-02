import type { ReactNode } from "react";
import { Activity, CheckCircle2, Moon, RefreshCw, Search, Sun } from "lucide-react";
import type {
  DatabaseStatus,
  HealthResponse,
  SourceIngestionResult,
  SourceRefreshTrigger,
  AppLocale,
  Theme,
} from "../api/types";
import { makeTextTranslator, makeTranslator } from "../shared/locale";
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
  locale: AppLocale;
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
  locale,
  totalUnreadFeedItems,
  updateTheme,
  openSourceStatus,
}: AppShellProps) {
  const t = makeTranslator(locale);
  const text = makeTextTranslator(locale);

  function sourceRefreshButtonLabel() {
    if (sourceRefreshState === "refreshing") {
      return t("app.sources.refreshing");
    }

    if (sourceRefreshState === "done") {
      return t("app.sources.refreshed");
    }

    if (sourceRefreshError) {
      return t("app.sources.failed");
    }

    return t("app.sources.refresh");
  }

  function sourceRefreshButtonTitle() {
    if (sourceRefreshError) {
      return `${text("Source refresh failed")}: ${sourceRefreshError}`;
    }

    if (sourceRefreshResult) {
      return `${text("Last refresh")}: ${sourceRefreshResult.itemsMatched}/${sourceRefreshResult.itemsFetched} ${text("matched")}`;
    }

    return text("Fetch GPW ESPI/EBI public listings");
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label={text("Primary navigation")}>
        <div className="brand">
          <div className="brand-mark">B</div>
          <div>
            <div className="brand-title">Brawler</div>
            <div className="brand-subtitle">{t("app.brand.codename")}</div>
          </div>
        </div>

        <nav className="nav-list">
          {sections.map((section) => {
            const Icon = section.icon;
            const sectionLabel = t(section.localeKey);
            return (
              <button
                className={activeSection === section.label ? "nav-item nav-item-active" : "nav-item"}
                key={section.label}
                onClick={() => setActiveSection(section.label)}
                type="button"
                title={sectionLabel}
              >
                <Icon size={18} aria-hidden="true" />
                <span>{sectionLabel}</span>
                {section.label === "Inbox" && totalUnreadFeedItems > 0 ? (
                  <span className="nav-badge" aria-label={`${totalUnreadFeedItems} ${text("unread feed item")}`}>
                    {totalUnreadFeedItems}
                  </span>
                ) : null}
              </button>
            );
          })}
        </nav>

        <div className="sidebar-footer">
          <div className="status-pill" title={text("Rust command boundary health")}>
            <span className={health ? "status-dot status-ok" : "status-dot status-warn"} />
            {health ? `${health.status} ${health.version}` : t("app.health.pending")}
          </div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="search-box">
            <Search size={18} aria-hidden="true" />
            <input
              aria-label={t("app.search.ariaLabel")}
              placeholder={t("app.search.placeholder")}
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
            />
          </div>
          <div className="topbar-actions">
            <div className="ai-mode-pill" title={t("app.aiMode.title")}>
              <span>{t("app.aiMode.label")}</span>
              <strong>{t("app.aiMode.value")}</strong>
            </div>
            <button
              aria-label={t("app.sources.openStatus")}
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
              <span>{t("app.sources.label")}</span>
              <strong>{sourceStatusSummary.label}</strong>
            </button>
            <button
              aria-label={
                dbRefreshState === "refreshing"
                  ? t("app.database.refreshing")
                  : t("app.database.refresh")
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
                  ? t("app.database.refreshing")
                  : dbRefreshState === "done"
                    ? t("app.database.refreshed")
                    : databaseStatus
                      ? `${t("app.database.active")}: ${databaseStatus.appliedMigrations} ${text("migration")}, ${databaseStatus.sourceAdapters} ${text("source")}, ${databaseStatus.settings} ${text("settings")}`
                      : databaseError
                        ? `Database error: ${databaseError}`
                        : t("app.database.pending")
              }
            >
              <span
                aria-label={
                  databaseError
                    ? t("app.database.failed")
                    : databaseStatus
                      ? t("app.database.active")
                      : t("app.database.pending")
                }
                className={databaseIndicatorClass(databaseStatus, databaseError)}
                role="status"
              />
              <span>{t("app.database.label")}</span>
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
            <label className="theme-control" title={t("settings.appearance.theme")}>
              {effectiveTheme === "dark" ? <Moon size={16} /> : <Sun size={16} />}
              <select value={theme} onChange={(event) => updateTheme(event.target.value as Theme)}>
                <option value="dark">{t("theme.dark")}</option>
                <option value="light">{t("theme.light")}</option>
                <option value="system">{t("theme.system")}</option>
              </select>
            </label>
          </div>
        </header>

        {children}
      </main>
    </div>
  );
}
