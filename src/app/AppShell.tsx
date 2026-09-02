import { Fragment, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Activity, CheckCircle2, Moon, PinOff, RefreshCw, Sun } from "lucide-react";
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

// Typed, not pre-worded: the view model reports the state, the shell words it
// in the user's language (the "3 issues" chip read English inside the Polish
// UI — found on the live app, F4c; ADR 0104 dec. 3).
export type SourceStatusSummary =
  | { kind: "error"; message: string }
  | { kind: "none" }
  | { kind: "issues"; count: number }
  | { kind: "ok"; enabled: number; total: number };

const SOURCE_ISSUE_FORMS: PluralForms = {
  en: ["issue", "issues"],
  pl: ["problem", "problemy", "problemów"],
};
const SOURCE_READY_FORMS: PluralForms = {
  en: ["enabled source ready", "enabled sources ready"],
  pl: ["włączone źródło gotowe", "włączone źródła gotowe", "włączonych źródeł gotowych"],
};

function sourceStatusPill(
  summary: SourceStatusSummary,
  locale: "en" | "pl",
  text: (value: string) => string,
): { label: string; title: string; tone: "ok" | "warn" | "danger" } {
  switch (summary.kind) {
    case "error":
      return { label: text("error"), title: `${text("Source refresh failed")}: ${summary.message}`, tone: "danger" };
    case "none":
      return { label: text("0 sources"), title: text("No sources configured"), tone: "warn" };
    case "issues":
      return {
        label: `${summary.count} ${pluralNoun(locale, summary.count, SOURCE_ISSUE_FORMS)}`,
        title: `${summary.count} ${pluralNoun(locale, summary.count, SOURCE_ISSUE_FORMS)} — ${text("open Sources")}`,
        tone: "danger",
      };
    case "ok":
      return {
        label: `${summary.enabled}/${summary.total}`,
        title: `${summary.enabled} ${pluralNoun(locale, summary.enabled, SOURCE_READY_FORMS)}`,
        tone: "ok",
      };
  }
}

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
  // Spółka mode (F3a S3, ADR 0107 amendment): opens the last-viewed company
  // (selectedCompanyId), else the first pinned, else the first tracked
  // company — never a blank screen. Replaces the old Dashboard bridge.
  onOpenSpolkaMode: () => void;
  onNavigateToSearchResult: (match: SearchMatch) => void;
  pinnedCompanies: PinnedCompany[];
  /** Every tracked company — feeds the palette's `Open company: TICKER`
   * entries (F3a S3, plan §7); pinned companies are a subset. */
  trackedCompanies: PinnedCompany[];
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

// Global-surface palette entries (F3a S3, plan "Trasy powierzchni globalnych"
// po F3a): entries for screens that also have a nav item keep the palette
// path used by J4/J7 (F4b S4, contract § Decisions #1 — Events and Report
// Season joined the Library nav but their `Open screen: …` command stays);
// the rest still have no top-level nav item and need this as their only
// entry point.
const SCREEN_PALETTE_ENTRIES: ReadonlyArray<{ section: Section; labelText: string; actionKey: string }> = [
  { section: "Research", labelText: "Research", actionKey: "screen.open.research" },
  { section: "Events", labelText: "Events", actionKey: "screen.open.events" },
  { section: "ReportSeason", labelText: "Report Season", actionKey: "screen.open.reportSeason" },
];

/** Builds AppShell's app-level palette commands — a pure function (no React)
 * so the copy gate (paletteCopy.test.ts) can exercise it directly with a rich
 * fixture, in both locales, without rendering. */
export function buildAppCommands(input: {
  shortcutBindings: Record<string, ShortcutBindingSetting>;
  shortcutActionMap: AppShortcutActionMap;
  trackedCompanies: PinnedCompany[];
  onOpenCompany: (companyId: string) => void;
  setActiveSection: (section: Section) => void;
  text: (s: string) => string;
}): PaletteCommand[] {
  const { text } = input;
  const fromShortcuts = resolveAppShortcutReferenceItems(input.shortcutBindings)
    .filter((item) => !item.disabled && item.id !== "app.commandPalette")
    .map((item) => ({
      id: item.id,
      label: text(item.label),
      verb: item.verb,
      actionKey: `shortcut.${item.id}`,
      run: () => {
        input.shortcutActionMap[item.id]();
      },
    }));
  // Every tracked company (F3a S3, plan §7 "Otwórz spółkę:") — pinned
  // companies are a subset, so this alone covers the previously-separate
  // "pinned only" list too.
  const companyCommands = input.trackedCompanies.map((company) => ({
    id: `company:${company.id}`,
    label: `${text("Open company")}: ${company.ticker ?? company.name}`,
    verb: "open" as const,
    actionKey: `company.open.${company.id}`,
    run: () => input.onOpenCompany(company.id),
  }));
  const screenCommands = SCREEN_PALETTE_ENTRIES.map(({ section, labelText, actionKey }) => ({
    id: `screen:${section}`,
    label: `${text("Open screen")}: ${text(labelText)}`,
    verb: "open" as const,
    actionKey,
    run: () => input.setActiveSection(section),
  }));
  return [...fromShortcuts, ...companyCommands, ...screenCommands];
}

export function AppShell({
  activeSection,
  children,
  dbRefreshState,
  effectiveTheme,
  health,
  refreshDatabaseBackedViews,
  refreshSources,
  setActiveSection,
  onOpenSpolkaMode,
  onNavigateToSearchResult,
  pinnedCompanies,
  trackedCompanies,
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
  const sourcePill = sourceStatusPill(sourceStatusSummary, locale, text);

  // ONE coalesced POLITE announcement when the unseen-attention count INCREASES
  // after hydration (ADR 0097 dec. 4): screen-reader users learn something
  // important landed in Today without per-event interrupts — role="alert" left
  // with the persistent toasts. The startup backlog stays badge-only. The
  // region is CLEARED on a decrease (visiting Today) so a later identical
  // count produces a real DOM mutation — aria-live only announces changes, and
  // React skips a set to the same string, so 0→1→0→1 would otherwise go
  // silent the second time.
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
    } else if (unseenAttentionCount < previous) {
      setAttentionAnnouncement("");
    }
    // `locale` is deliberately not a dependency: a locale switch must neither
    // re-announce nor leave the previous locale's sentence behind — the effect
    // below clears the region instead.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attentionHydrated, unseenAttentionCount]);
  useEffect(() => {
    setAttentionAnnouncement("");
  }, [locale]);

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
    "app.openResearch": () => setActiveSection("Research"),
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

  // App-level palette commands: the resolved, enabled shortcuts (minus the
  // palette opener itself), every tracked company, and the global screens
  // (F3a S3, plan "Trasy powierzchni globalnych"). Building is a pure
  // function (`buildAppCommands`, below) so the copy gate (paletteCopy.test.ts)
  // can exercise it directly, with a rich fixture, in both locales — without
  // rendering.
  const appCommands = useMemo<PaletteCommand[]>(
    () =>
      buildAppCommands({
        shortcutBindings,
        shortcutActionMap,
        trackedCompanies,
        onOpenCompany,
        setActiveSection,
        text,
      }),
    // `text` derives from `locale`; listing `locale` keeps labels re-translating.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      shortcutActionMap,
      shortcutBindings,
      trackedCompanies,
      onOpenCompany,
      setActiveSection,
      locale,
    ],
  );

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
                <Fragment key={group.id}>
                  <div className="nav-group">
                    <div className="nav-group-label">{t(group.localeKey)}</div>
                    <div className="nav-list">
                      {items.map((item) => {
                        const Icon = item.icon;
                        const itemLabel = t(item.localeKey);
                        // Spółka (F3a S3, ADR 0107 amendment): exactly one
                        // aria-current across modes + pinned rows — a pinned,
                        // selected company's OWN row is current instead of this
                        // mode item.
                        const isSpolkaMode = item.label === "Spolka";
                        const isActive = isSpolkaMode
                          ? activeSection === "Spolka" &&
                            !pinnedCompanies.some((company) => company.id === selectedCompanyId)
                          : activeSection === item.label;
                        return (
                          <button
                            className={isActive ? "nav-item nav-item-active" : "nav-item"}
                            aria-current={isActive ? "page" : undefined}
                            key={item.label}
                            onClick={() => (isSpolkaMode ? onOpenSpolkaMode() : setActiveSection(item.label))}
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
                    </div>
                  </div>
                </Fragment>
              );
            })}

            {pinnedCompanies.length > 0 ? (
              <div className="nav-group" key="pinned">
                <div className="nav-group-label">{t("nav.group.pinned")}</div>
                <div className="nav-list" aria-label={text("Pinned companies")}>
                  {pinnedCompanies.map((company) => {
                    const isActive = activeSection === "Spolka" && selectedCompanyId === company.id;
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
                className={["source-status-pill", `source-status-pill-${sourcePill.tone}`].join(" ")}
                onClick={openSourceStatus}
                title={sourcePill.title}
                type="button"
              >
                <Activity size={14} aria-hidden="true" />
                <span>{t("app.sources.label")}</span>
                <strong>{sourcePill.label}</strong>
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
