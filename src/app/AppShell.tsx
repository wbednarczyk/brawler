import {
  useMemo,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import { Activity, CheckCircle2, Moon, RefreshCw, Sun } from "lucide-react";
import type {
  HealthResponse,
  SourceIngestionResult,
  SourceRefreshTrigger,
  AppLocale,
  ShortcutBindingSetting,
  Theme,
} from "../api/types";
import { makeTextTranslator, makeTranslator } from "../shared/locale";
import { useKeyboardShortcuts } from "../shared/shortcuts";
import type { SearchMatch } from "../api/search";
import { GlobalSearch } from "./GlobalSearch";
import type { DbRefreshState, SourceRefreshState } from "./appTypes";
import { sections, type Section } from "./navigation";
import { createAppShortcutDefinitions, type AppShortcutActionMap } from "./shortcuts";

type SourceStatusTone = "ok" | "warn" | "danger";

export type SourceStatusSummary = {
  label: string;
  title: string;
  tone: SourceStatusTone;
};

type AppShellProps = {
  activeSection: Section;
  children: ReactNode;
  dbRefreshState: DbRefreshState;
  effectiveTheme: "dark" | "light";
  health: HealthResponse | null;
  refreshDatabaseBackedViews: () => void;
  refreshSources: (trigger: SourceRefreshTrigger) => void;
  setActiveSection: (section: Section) => void;
  onNavigateToSearchResult: (match: SearchMatch) => void;
  sourceRefreshError: string | null;
  sourceRefreshResult: SourceIngestionResult | null;
  sourceRefreshState: SourceRefreshState;
  sourceStatusSummary: SourceStatusSummary;
  theme: Theme;
  locale: AppLocale;
  shortcutBindings: Record<string, ShortcutBindingSetting>;
  shortcutActions: AppShortcutActionMap;
  totalUnreadFeedItems: number;
  updateTheme: (theme: Theme) => void;
  openSourceStatus: () => void;
  developerMode: boolean;
};

const defaultSidebarWidth = 190;
const minSidebarWidth = 160;
const maxSidebarWidth = 280;

export function AppShell({
  activeSection,
  children,
  dbRefreshState,
  effectiveTheme,
  health,
  refreshDatabaseBackedViews,
  refreshSources,
  setActiveSection,
  onNavigateToSearchResult,
  sourceRefreshError,
  sourceRefreshResult,
  sourceRefreshState,
  sourceStatusSummary,
  theme,
  locale,
  shortcutBindings,
  shortcutActions,
  totalUnreadFeedItems,
  updateTheme,
  openSourceStatus,
  developerMode,
}: AppShellProps) {
  const t = makeTranslator(locale);
  const text = makeTextTranslator(locale);
  const [sidebarWidth, setSidebarWidth] = useState(defaultSidebarWidth);
  const [isSidebarResizing, setIsSidebarResizing] = useState(false);
  const shortcuts = useMemo(() => createAppShortcutDefinitions(
    {
      "app.openInbox": () => setActiveSection("Inbox"),
      "app.openCompanies": () => setActiveSection("Companies"),
      "app.openWatchlists": () => setActiveSection("Watchlists"),
      "app.openNotebooks": () => setActiveSection("Notebooks"),
      "app.openEvents": () => setActiveSection("Events"),
      "app.openTranscripts": () => setActiveSection("Transcripts"),
      "app.openSources": () => setActiveSection("Sources"),
      "app.openSettings": () => setActiveSection("Settings"),
      "app.focusSearch": () => {
        window.setTimeout(() => {
          document.querySelector<HTMLInputElement>("[data-global-search-input]")?.focus();
        }, 0);
      },
      "app.refreshSources": () => {
        if (sourceRefreshState !== "refreshing") {
          void refreshSources("manual");
        }
      },
      "app.refreshDatabase": () => {
        if (dbRefreshState !== "refreshing") {
          refreshDatabaseBackedViews();
        }
      },
      "inbox.nextItem": shortcutActions["inbox.nextItem"],
      "inbox.previousItem": shortcutActions["inbox.previousItem"],
      "inbox.toggleRead": shortcutActions["inbox.toggleRead"],
      "inbox.toggleSaved": shortcutActions["inbox.toggleSaved"],
      "inbox.openSource": shortcutActions["inbox.openSource"],
      "inbox.createNote": shortcutActions["inbox.createNote"],
      "company.nextCompany": shortcutActions["company.nextCompany"],
      "company.previousCompany": shortcutActions["company.previousCompany"],
      "company.nextTab": shortcutActions["company.nextTab"],
      "company.previousTab": shortcutActions["company.previousTab"],
      "notebook.editSelected": shortcutActions["notebook.editSelected"],
      "notebook.saveCurrent": shortcutActions["notebook.saveCurrent"],
    },
    shortcutBindings,
  ), [
    dbRefreshState,
    refreshDatabaseBackedViews,
    refreshSources,
    setActiveSection,
    shortcutActions,
    shortcutBindings,
    sourceRefreshState,
  ]);

  useKeyboardShortcuts(shortcuts);

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

  function clampSidebarWidth(width: number) {
    return Math.min(maxSidebarWidth, Math.max(minSidebarWidth, Math.round(width)));
  }

  function resizeSidebarFromPointer(event: PointerEvent<HTMLDivElement>) {
    setSidebarWidth(clampSidebarWidth(event.clientX));
  }

  function startSidebarResize(event: PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    setIsSidebarResizing(true);
    event.currentTarget.setPointerCapture(event.pointerId);
    resizeSidebarFromPointer(event);
  }

  function resizeSidebar(event: PointerEvent<HTMLDivElement>) {
    if (!isSidebarResizing) {
      return;
    }

    resizeSidebarFromPointer(event);
  }

  function stopSidebarResize(event: PointerEvent<HTMLDivElement>) {
    if (!isSidebarResizing) {
      return;
    }

    setIsSidebarResizing(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function resizeSidebarWithKeyboard(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight" && event.key !== "Home" && event.key !== "End") {
      return;
    }

    event.preventDefault();
    if (event.key === "Home") {
      setSidebarWidth(minSidebarWidth);
      return;
    }
    if (event.key === "End") {
      setSidebarWidth(maxSidebarWidth);
      return;
    }

    const delta = event.key === "ArrowLeft" ? -12 : 12;
    setSidebarWidth((currentWidth) => clampSidebarWidth(currentWidth + delta));
  }

  return (
    <div
      className="app-shell"
      style={{ "--sidebar-width": `${sidebarWidth}px` } as CSSProperties}
    >
      <aside className="sidebar" aria-label={text("Primary navigation")}>
        <div className="brand">
          <div className="brand-mark">B</div>
          <div>
            <div className="brand-title">Brawler</div>
            <div className="brand-subtitle">{t("app.brand.subtitle")}</div>
          </div>
        </div>

        <nav className="nav-list">
          {sections
            .filter((section) => section.label !== "Diagnostics" || developerMode)
            .map((section) => {
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
          <span className="version-label">{health ? `v${health.version}` : "v..."}</span>
        </div>
      </aside>

      <div
        aria-label={text("Resize navigation")}
        aria-orientation="vertical"
        aria-valuemax={maxSidebarWidth}
        aria-valuemin={minSidebarWidth}
        aria-valuenow={sidebarWidth}
        className="sidebar-resizer"
        onKeyDown={resizeSidebarWithKeyboard}
        onPointerDown={startSidebarResize}
        onPointerMove={resizeSidebar}
        onPointerUp={stopSidebarResize}
        role="separator"
        tabIndex={0}
        title={text("Drag to resize navigation")}
      />

      <main className="workspace">
        <header className="topbar">
          <GlobalSearch locale={locale} onNavigate={onNavigateToSearchResult} />
          <div className="topbar-actions">
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
              aria-label={sourceRefreshButtonLabel()}
              className={[
                "icon-button",
                sourceRefreshState === "refreshing" ? "icon-button-spinning" : "",
                sourceRefreshState === "done" ? "topbar-action-success" : "",
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
