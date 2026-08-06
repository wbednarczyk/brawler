import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Activity, CheckCircle2, Columns3, Moon, Pencil, PinOff, Plus, RefreshCw, Sun, X } from "lucide-react";
import type {
  HealthResponse,
  SourceIngestionResult,
  SourceRefreshTrigger,
  AppLocale,
  ShortcutBindingSetting,
  Theme,
} from "../api/types";
import { makeTextTranslator, makeTranslator } from "../shared/locale";
import { pluralNoun, type PluralForms } from "../shared/locale/plural";
import { useKeyboardShortcuts } from "../shared/shortcuts";
import type { SearchMatch } from "../api/search";
import { GlobalSearch } from "./GlobalSearch";
import type { DbRefreshState, SourceRefreshState } from "./appTypes";
import { navGroups, type Section } from "./navigation";
import { TextField } from "../ui";
import {
  createAppShortcutDefinitions,
  resolveAppShortcutReferenceItems,
  type AppShortcutActionMap,
} from "./shortcuts";
import {
  CommandPaletteProvider,
  useCommandPalette,
  type PaletteCommand,
} from "./commandPalette";
import { useDeveloperMode } from "./state/SettingsContext";

type SourceStatusTone = "ok" | "warn" | "danger";

export type SourceStatusSummary = {
  label: string;
  title: string;
  tone: SourceStatusTone;
};

/** A company pinned to the sidebar spine (ADR 0054). */
export type PinnedCompany = {
  id: string;
  name: string;
  ticker: string | null;
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
  // Dashboard redesign (epic c793ca1): the "Dashboard" (Cockpit) nav entry opens
  // the one company-scoped Dashboard seeded with a default company — never the
  // blank feed-linked cockpit — so it routes through this handler, not the generic
  // setActiveSection.
  onOpenDashboard: () => void;
  onCreateView: () => void;
  cockpitViews: { id: string; name: string }[];
  activeCockpitViewId: string | null;
  onOpenCockpitView: (viewId: string) => void;
  onDeleteCockpitView: (viewId: string) => void;
  onRenameCockpitView: (viewId: string, name: string) => void;
  onNavigateToSearchResult: (match: SearchMatch) => void;
  pinnedCompanies: PinnedCompany[];
  selectedCompanyId: string | null;
  onOpenCompany: (companyId: string) => void;
  onUnpinCompany: (companyId: string) => void;
  sourceRefreshError: string | null;
  sourceRefreshResult: SourceIngestionResult | null;
  sourceRefreshState: SourceRefreshState;
  sourceStatusSummary: SourceStatusSummary;
  theme: Theme;
  locale: AppLocale;
  shortcutBindings: Record<string, ShortcutBindingSetting>;
  shortcutActions: AppShortcutActionMap;
  totalUnreadFeedItems: number;
  /**
   * Unseen non-routine attention events (ADR 0097 dec. 4) — the Today nav badge,
   * the app's only ambient-attention indicator now that system toasts are gone.
   */
  unseenAttentionCount: number;
  /** True once the attention state hydrated — gates the polite live region so
   * the startup backlog is never replayed as an announcement. */
  attentionHydrated: boolean;
  updateTheme: (theme: Theme) => void;
  openSourceStatus: () => void;
};

// Badge aria-label + polite announcement for unseen attention events (ADR 0097
// dec. 4) — a full noun phrase so the count reads as a sentence in both locales.
const ATTENTION_BADGE_FORMS: PluralForms = {
  en: ["new important item in Today", "new important items in Today"],
  pl: ["nowe ważne zdarzenie w Dziś", "nowe ważne zdarzenia w Dziś", "nowych ważnych zdarzeń w Dziś"],
};

export function AppShell({
  activeSection,
  children,
  dbRefreshState,
  effectiveTheme,
  health,
  refreshDatabaseBackedViews,
  refreshSources,
  setActiveSection,
  onOpenDashboard,
  onCreateView,
  cockpitViews,
  activeCockpitViewId,
  onOpenCockpitView,
  onDeleteCockpitView,
  onRenameCockpitView,
  onNavigateToSearchResult,
  pinnedCompanies,
  selectedCompanyId,
  onOpenCompany,
  onUnpinCompany,
  sourceRefreshError,
  sourceRefreshResult,
  sourceRefreshState,
  sourceStatusSummary,
  theme,
  locale,
  shortcutBindings,
  shortcutActions,
  totalUnreadFeedItems,
  unseenAttentionCount,
  attentionHydrated,
  updateTheme,
  openSourceStatus,
}: AppShellProps) {
  const developerMode = useDeveloperMode();
  const t = makeTranslator(locale);
  const text = makeTextTranslator(locale);
  // Issue #89: inline saved-view rename — the pencil swaps the row label for a
  // TextField; Enter commits, Escape/blur cancels.
  const [renamingViewId, setRenamingViewId] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");

  // ONE coalesced POLITE announcement when the unseen-attention count INCREASES
  // after hydration (ADR 0097 dec. 4): screen-reader users learn something
  // important landed in Today without per-event interrupts — role="alert" left
  // with the persistent toasts. The startup backlog stays badge-only.
  const [attentionAnnouncement, setAttentionAnnouncement] = useState("");
  const previousUnseenRef = useRef<number | null>(null);
  useEffect(() => {
    if (!attentionHydrated) return;
    const previous = previousUnseenRef.current;
    previousUnseenRef.current = unseenAttentionCount;
    // The first post-hydration observation is the backlog — record, don't speak.
    if (previous === null) return;
    if (unseenAttentionCount > previous) {
      setAttentionAnnouncement(
        `${unseenAttentionCount} ${pluralNoun(locale, unseenAttentionCount, ATTENTION_BADGE_FORMS)}`,
      );
    }
  }, [attentionHydrated, unseenAttentionCount, locale]);

  // The command palette's open() lives inside the CommandPaletteProvider (below),
  // which AppShell renders — so it cannot read that context directly. A binder
  // component under the provider wires open() into this ref, which the keyboard
  // shortcut invokes at event time (always after mount).
  const openPaletteRef = useRef<() => void>(() => {});
  const bindPaletteOpen = useCallback((open: () => void) => {
    openPaletteRef.current = open;
  }, []);

  const shortcutActionMap = useMemo<AppShortcutActionMap>(() => ({
    "app.openInbox": () => setActiveSection("Inbox"),
    "app.openCompanies": () => setActiveSection("Companies"),
    "app.openWatchlists": () => setActiveSection("Watchlists"),
    "app.openNotebooks": () => setActiveSection("Notebooks"),
    "app.openEvents": () => setActiveSection("Events"),
    "app.openTranscripts": () => setActiveSection("Transcripts"),
    "app.openSources": () => setActiveSection("Sources"),
    "app.openSettings": () => setActiveSection("Settings"),
    "app.openAlerts": () => setActiveSection("Alerts"),
    "app.commandPalette": () => {
      openPaletteRef.current();
    },
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
  }), [
    dbRefreshState,
    refreshDatabaseBackedViews,
    refreshSources,
    setActiveSection,
    shortcutActions,
    sourceRefreshState,
  ]);

  const shortcuts = useMemo(() => {
    const definitions = createAppShortcutDefinitions(shortcutActionMap, shortcutBindings);
    // Meta+K twin for the palette (macOS ⌘K): the shortcut engine treats ctrl and
    // meta as distinct modifiers, so mirror whatever key the palette resolves to
    // with metaKey. Not a settings row — it tracks the palette binding, no rebind.
    const palette = definitions.find((definition) => definition.id === "app.commandPalette");
    if (palette && palette.binding.ctrlKey) {
      definitions.push({
        ...palette,
        id: "app.commandPalette.meta",
        binding: { ...palette.binding, ctrlKey: false, metaKey: true },
      });
    }
    return definitions;
  }, [shortcutActionMap, shortcutBindings]);

  useKeyboardShortcuts(shortcuts);

  // App-level palette commands: the resolved, enabled shortcuts (minus the palette
  // opener itself) plus jumps to saved views, pinned companies, and view creation.
  const appCommands = useMemo<PaletteCommand[]>(() => {
    const fromShortcuts = resolveAppShortcutReferenceItems(shortcutBindings)
      .filter((item) => !item.disabled && item.id !== "app.commandPalette")
      .map((item) => ({
        id: item.id,
        label: text(item.label),
        run: () => {
          shortcutActionMap[item.id]();
        },
      }));
    const viewCommands = cockpitViews.map((view) => ({
      id: `view:${view.id}`,
      label: `${text("Open view")}: ${view.name}`,
      run: () => onOpenCockpitView(view.id),
    }));
    const companyCommands = pinnedCompanies.map((company) => ({
      id: `company:${company.id}`,
      label: `${text("Open company")}: ${company.ticker ?? company.name}`,
      run: () => onOpenCompany(company.id),
    }));
    return [
      ...fromShortcuts,
      ...viewCommands,
      ...companyCommands,
      { id: "app.createView", label: text("New view"), run: onCreateView },
    ];
    // `text` derives from `locale`; listing `locale` keeps labels re-translating.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    shortcutActionMap,
    shortcutBindings,
    cockpitViews,
    pinnedCompanies,
    onOpenCockpitView,
    onOpenCompany,
    onCreateView,
    locale,
  ]);

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
    <CommandPaletteProvider appCommands={appCommands} text={text}>
      <PaletteShortcutBinder bindOpen={bindPaletteOpen} />
      <div className="app-shell">
        {/* Polite, coalesced ambient-attention announcement (ADR 0097 dec. 4).
            aria-live WITHOUT role="status": announcement behavior is identical,
            but the region never collides with toast queries for role=status. */}
        <div className="visually-hidden" aria-live="polite">
          {attentionAnnouncement}
        </div>
        <aside className="sidebar">
          <div className="brand">
            <div className="brand-mark">B</div>
            <div>
              <div className="brand-heading">
                <div className="brand-title">Brawler</div>
                <span className="brand-version">{health ? `v${health.version}` : "v…"}</span>
              </div>
              <div className="brand-subtitle">{t("app.brand.subtitle")}</div>
            </div>
          </div>

          <nav className="sidebar-nav" aria-label={text("Primary navigation")}>
            {navGroups.map((group) => {
              const items = group.items.filter(
                (item) => item.label !== "Diagnostics" || developerMode,
              );
              if (items.length === 0) {
                return null;
              }
              return (
                <div className="nav-group" key={group.id}>
                  <div className="nav-group-label">{t(group.localeKey)}</div>
                  <div className="nav-list">
                    {items.map((item) => {
                      const Icon = item.icon;
                      const itemLabel = t(item.localeKey);
                      return (
                        <button
                          className={activeSection === item.label ? "nav-item nav-item-active" : "nav-item"}
                          aria-current={activeSection === item.label ? "page" : undefined}
                          key={item.label}
                          onClick={() =>
                            item.label === "Cockpit"
                              ? onOpenDashboard()
                              : setActiveSection(item.label)
                          }
                          type="button"
                          title={itemLabel}
                        >
                          <Icon size={18} aria-hidden="true" />
                          <span>{itemLabel}</span>
                          {item.label === "Inbox" && totalUnreadFeedItems > 0 ? (
                            <span className="nav-badge" aria-label={`${totalUnreadFeedItems} ${text("unread feed item")}`}>
                              {totalUnreadFeedItems}
                            </span>
                          ) : null}
                          {/* Ambient attention (ADR 0097 dec. 4): unseen non-routine
                              attention events; clears when Today marks them seen. */}
                          {item.label === "Today" && unseenAttentionCount > 0 ? (
                            <span
                              className="nav-badge"
                              aria-label={`${unseenAttentionCount} ${pluralNoun(locale, unseenAttentionCount, ATTENTION_BADGE_FORMS)}`}
                            >
                              {unseenAttentionCount}
                            </span>
                          ) : null}
                        </button>
                      );
                    })}
                    {group.id === "modes" ? (
                      <>
                        {cockpitViews.map((view) => {
                          const isActive = activeSection === "Cockpit" && activeCockpitViewId === view.id;
                          const isRenaming = renamingViewId === view.id;
                          return (
                            <div className={isActive ? "pinned-row pinned-row-active" : "pinned-row"} key={view.id}>
                              {isRenaming ? (
                                <TextField
                                  className="nav-view-rename"
                                  aria-label={text("View name")}
                                  value={renameDraft}
                                  autoFocus
                                  onChange={(event) => setRenameDraft(event.target.value)}
                                  onKeyDown={(event) => {
                                    if (event.key === "Enter") {
                                      const next = renameDraft.trim();
                                      setRenamingViewId(null);
                                      if (next && next !== view.name) {
                                        onRenameCockpitView(view.id, next);
                                      }
                                    } else if (event.key === "Escape") {
                                      setRenamingViewId(null);
                                    }
                                  }}
                                  onBlur={() => setRenamingViewId(null)}
                                />
                              ) : (
                                <button
                                  className="nav-item"
                                  aria-current={isActive ? "page" : undefined}
                                  onClick={() => onOpenCockpitView(view.id)}
                                  type="button"
                                  title={view.name}
                                >
                                  <Columns3 size={18} aria-hidden="true" />
                                  <span>{view.name}</span>
                                </button>
                              )}
                              <button
                                className="pinned-unpin icon-button"
                                onClick={() => {
                                  setRenamingViewId(view.id);
                                  setRenameDraft(view.name);
                                }}
                                type="button"
                                aria-label={`${text("Rename view")}: ${view.name}`}
                                title={text("Rename view")}
                              >
                                <Pencil size={14} aria-hidden="true" />
                              </button>
                              <button
                                className="pinned-unpin icon-button"
                                onClick={() => onDeleteCockpitView(view.id)}
                                type="button"
                                aria-label={`${text("Delete view")}: ${view.name}`}
                                title={text("Delete view")}
                              >
                                <X size={14} aria-hidden="true" />
                              </button>
                            </div>
                          );
                        })}
                        <button
                          className="nav-item nav-item-add"
                          onClick={onCreateView}
                          type="button"
                          title={text("New view")}
                        >
                          <Plus size={18} aria-hidden="true" />
                          <span>{text("New view")}</span>
                        </button>
                      </>
                    ) : null}
                  </div>
                </div>
              );
            })}

            {pinnedCompanies.length > 0 ? (
              <div className="nav-group" key="pinned">
                <div className="nav-group-label">{t("nav.group.pinned")}</div>
                <div className="nav-list" aria-label={text("Pinned companies")}>
                  {pinnedCompanies.map((company) => {
                    const isActive = activeSection === "Companies" && selectedCompanyId === company.id;
                    return (
                      <div className={isActive ? "pinned-row pinned-row-active" : "pinned-row"} key={company.id}>
                        <button
                          className="nav-item pinned-company"
                          aria-current={isActive ? "page" : undefined}
                          onClick={() => onOpenCompany(company.id)}
                          type="button"
                          title={text("Open {company} workspace").replace("{company}", company.name)}
                        >
                          <span
                            className="conviction-dot conviction-dot-unknown"
                            aria-hidden="true"
                            title={text("Conviction not yet assessed")}
                          />
                          <span>{company.ticker ? company.ticker : company.name}</span>
                        </button>
                        <button
                          className="pinned-unpin icon-button"
                          onClick={() => onUnpinCompany(company.id)}
                          type="button"
                          aria-label={`${text("Unpin from sidebar")}: ${company.name}`}
                          title={text("Unpin from sidebar")}
                        >
                          <PinOff size={14} aria-hidden="true" />
                        </button>
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : null}
          </nav>
        </aside>

        <div className="app-main">
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
                {/* eslint-disable-next-line no-restricted-syntax -- compact topbar theme switcher: a bare <select> inside an icon label; SelectField's labelled layout does not fit this inline control */}
                <select value={theme} onChange={(event) => updateTheme(event.target.value as Theme)}>
                  <option value="dark">{t("theme.dark")}</option>
                  <option value="light">{t("theme.light")}</option>
                  <option value="system">{t("theme.system")}</option>
                </select>
              </label>
            </div>
          </header>

          {/* Journey-metrics observation point (ADR 0074 pt 3 / ADR 0081 Q3):
              semantic marker for the section currently on screen, not
              test-only business state. */}
          <main className="workspace" data-app-section={activeSection}>
            {children}
          </main>
        </div>
      </div>
    </CommandPaletteProvider>
  );
}

// Bridges the palette's open() (only reachable inside the provider) up to the
// AppShell keyboard-shortcut handler via a stable ref setter.
function PaletteShortcutBinder({ bindOpen }: { bindOpen: (open: () => void) => void }) {
  const { open } = useCommandPalette();
  useEffect(() => {
    bindOpen(open);
  }, [open, bindOpen]);
  return null;
}
