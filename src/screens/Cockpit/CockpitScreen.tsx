import { useCallback, useEffect, useRef, useState } from "react";
import { Command } from "lucide-react";
import type { Company, FeedItem } from "../../api/types";
import { useLocale } from "../../shared/locale";
import {
  Button,
  EmptyState,
  SearchField,
  SectionHeader,
  SelectField,
  StatusChip,
  TextField,
} from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatFeedTimestamp } from "../../shared/format/timestamp";
import { CompanyClaimsPanel } from "../../shared/components/CompanyClaimsPanel";
import { CompanyReportDocumentsPanel } from "../../shared/components/CompanyReportDocumentsPanel";
import { QualityPanel } from "../../shared/components/QualityPanel";
import { ReportDiffPanel } from "../Companies/ReportDiffPanel";
import { FundamentalsPanel } from "../Companies/FundamentalsPanel";
import { WatchlistsScreen } from "../Watchlists/WatchlistsScreen";
import { ResearchScreen } from "../Research/ResearchScreen";
import { NotebooksScreen } from "../Notebooks/NotebooksScreen";
import { EventsScreen } from "../Events/EventsScreen";
import { ReportSeasonScreen } from "../ReportSeason/ReportSeasonScreen";
import {
  COCKPIT_LAYOUT_STORAGE_KEY,
  DockLayout,
  DOCKVIEW_VERSION,
  type DockLayoutHandle,
  type DockPanelSpec,
} from "./DockLayout";
import { useCockpitFundamentals } from "./useCockpitFundamentals";
import { CockpitSelectionProvider, useCockpitSelection } from "./CockpitSelectionContext";
import { CommandPalette, type PaletteCommand } from "./CommandPalette";
import {
  deleteCockpitLayout,
  listCockpitLayouts,
  saveCockpitLayout,
  type CockpitLayout,
} from "../../api/cockpit";

// Research cockpit — the full-screen docking shell spike (ADR 0053). It shows
// the LINKED workspace (feed selection drives the inspector + the company's
// claims/diff), a rich Fundamentals panel, a command palette (⌘K) to open any
// panel, and named saved layouts to switch task-shaped workspaces.

type LinkedKind = "feed" | "inspector" | "claims-sel" | "diff-sel";
type PinnedKind = "fundamentals" | "reportDiff" | "claims" | "quality" | "documents";

const LINKED: { id: string; kind: LinkedKind }[] = [
  { id: "feed", kind: "feed" },
  { id: "inspector", kind: "inspector" },
  { id: "claims-sel", kind: "claims-sel" },
  { id: "diff-sel", kind: "diff-sel" },
];

const PINNED_KINDS: PinnedKind[] = ["fundamentals", "reportDiff", "claims", "quality", "documents"];

// Global singleton panels (ADR 0053 phase 4c): full app screens that own their
// own scope/data via contexts (mounted prop-free in AppStateRoot), so the cockpit
// renders them directly. Not company-scoped — opened once from the palette.
type GlobalKind = "watchlists" | "research" | "notebook" | "events" | "reportSeason";

const GLOBAL_KINDS: GlobalKind[] = [
  "watchlists",
  "research",
  "notebook",
  "events",
  "reportSeason",
];

function globalKindLabel(kind: GlobalKind, text: (s: string) => string): string {
  switch (kind) {
    case "watchlists":
      return text("Watchlists");
    case "research":
      return text("Research");
    case "notebook":
      return text("Notebook");
    case "events":
      return text("Events");
    case "reportSeason":
      return text("Report Season");
  }
}

type Pinned = { id: string; companyId: string; kind: PinnedKind };

function pinnedId(companyId: string, kind: PinnedKind): string {
  return `${kind}:${companyId}`;
}

// Per-company dashboard layouts (ADR 0057) are saved cockpit layouts under a
// reserved name so each company remembers its own arrangement; these are filtered
// out of the user's named-view lists.
const DASHBOARD_PREFIX = "dashboard:";
const dashboardLayoutName = (companyId: string) => `${DASHBOARD_PREFIX}${companyId}`;
const isDashboardLayout = (layout: { name: string }) => layout.name.startsWith(DASHBOARD_PREFIX);

// The curated default panels a company dashboard opens with when it has no saved
// layout yet (ADR 0057): fundamentals + claims + quality + documents, plus the feed.
const DASHBOARD_DEFAULT_KINDS: PinnedKind[] = ["fundamentals", "claims", "quality", "documents"];

function pinnedKindLabel(kind: PinnedKind, text: (s: string) => string): string {
  switch (kind) {
    case "fundamentals":
      return text("Fundamentals");
    case "reportDiff":
      return text("Report comparison");
    case "claims":
      return text("Claims");
    case "quality":
      return text("Quality");
    case "documents":
      return text("Report documents");
  }
}

function linkedTitle(kind: LinkedKind, text: (s: string) => string): string {
  switch (kind) {
    case "feed":
      return text("Feed");
    case "inspector":
      return text("Inspector");
    case "claims-sel":
      return text("Claims");
    case "diff-sel":
      return text("Report comparison");
  }
}

// Built-in, task-shaped layout presets (ADR 0053 phase 4d). They compose the
// panel kinds that landed in 4a–4c: `linked` are selection-driven singletons,
// `pinned` open for the active company, `globals` are full-screen singletons.
// Kept under the ≤6 visible-panel ceiling from the terminal-UX research.
type PresetSpec = {
  id: string;
  labelKey: string;
  linked: LinkedKind[];
  pinned: PinnedKind[];
  globals: GlobalKind[];
};

const PRESETS: PresetSpec[] = [
  {
    id: "daily-triage",
    labelKey: "Daily triage",
    linked: ["feed", "inspector", "claims-sel"],
    pinned: [],
    globals: [],
  },
  {
    id: "earnings-season",
    labelKey: "Earnings season",
    linked: ["feed"],
    pinned: ["reportDiff", "fundamentals", "claims"],
    globals: ["reportSeason"],
  },
  {
    id: "deep-dive",
    labelKey: "Deep dive",
    linked: [],
    pinned: ["fundamentals", "reportDiff"],
    globals: ["notebook", "research"],
  },
];

// The app-owned descriptor stored in `panels_json` — which panels are open plus
// the linked selection. The dockview geometry travels separately in
// `layout_json` (decision 3A); this descriptor is the part that survives a
// dockview upgrade and rebuilds the default layout when the geometry is dropped.
type PanelsDescriptor = {
  pinned: Pinned[];
  openGlobals: GlobalKind[];
  closedLinked: string[];
  selectedFeedItemId: string | null;
};

function parsePanels(panelsJson: string): PanelsDescriptor | null {
  try {
    const parsed = JSON.parse(panelsJson) as Partial<PanelsDescriptor>;
    return {
      pinned: Array.isArray(parsed.pinned) ? parsed.pinned : [],
      openGlobals: Array.isArray(parsed.openGlobals)
        ? parsed.openGlobals.filter((kind): kind is GlobalKind => GLOBAL_KINDS.includes(kind))
        : [],
      closedLinked: Array.isArray(parsed.closedLinked) ? parsed.closedLinked : [],
      selectedFeedItemId:
        typeof parsed.selectedFeedItemId === "string" ? parsed.selectedFeedItemId : null,
    };
  } catch {
    return null;
  }
}

export type CockpitScreenProps = {
  companies: Company[];
  feedItems: FeedItem[];
  /** When set, the cockpit (Advanced layout) opens scoped to this company. */
  initialCompanyId?: string | null;
  /** When set, the cockpit opens with this saved layout activated (ADR 0057). */
  initialLayoutId?: string | null;
};

export function CockpitScreen({
  companies,
  feedItems,
  initialCompanyId = null,
  initialLayoutId = null,
}: CockpitScreenProps) {
  return (
    <CockpitSelectionProvider
      companies={companies}
      feedItems={feedItems}
      initialCompanyId={initialCompanyId}
    >
      <CockpitWorkspace
        companies={companies}
        feedItems={feedItems}
        initialLayoutId={initialLayoutId}
        dashboardCompanyId={initialCompanyId}
      />
    </CockpitSelectionProvider>
  );
}

function CockpitWorkspace({
  companies,
  feedItems,
  initialLayoutId = null,
  dashboardCompanyId = null,
}: Omit<CockpitScreenProps, "initialCompanyId"> & { dashboardCompanyId?: string | null }) {
  const { text } = useLocale();
  // Shared selection lives in the cockpit store (decision 6A), not local state.
  const { selection, selectedFeedItem, selectedCompany, selectFeedItem } = useCockpitSelection();
  const [pinned, setPinned] = useState<Pinned[]>([]);
  const [openGlobals, setOpenGlobals] = useState<GlobalKind[]>([]);
  const [closedLinked, setClosedLinked] = useState<Set<string>>(() => new Set());
  const [resetNonce, setResetNonce] = useState(0);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [savedLayouts, setSavedLayouts] = useState<CockpitLayout[]>([]);
  const [layoutName, setLayoutName] = useState("");
  const [pendingGeometry, setPendingGeometry] = useState<unknown>(null);
  const dockRef = useRef<DockLayoutHandle>(null);

  const companyById = new Map(companies.map((company) => [company.id, company]));
  const selectedItem = selectedFeedItem;
  const selectedCompanyId = selectedCompany?.id ?? null;

  const [layoutsLoaded, setLayoutsLoaded] = useState(false);
  // Saved layouts are durable state in SQLite (decision 3A), not localStorage.
  const refreshLayouts = useCallback(() => {
    listCockpitLayouts()
      .then((layouts) => {
        setSavedLayouts(layouts);
        setLayoutsLoaded(true);
      })
      .catch(() => {
        setSavedLayouts([]);
        setLayoutsLoaded(true);
      });
  }, []);
  useEffect(() => {
    refreshLayouts();
  }, [refreshLayouts]);

  // ⌘K / Ctrl+K opens the command palette.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Apply a named layout's geometry after its panel set has been put into state.
  useEffect(() => {
    if (pendingGeometry == null) return;
    dockRef.current?.restore(pendingGeometry as never);
    setPendingGeometry(null);
  }, [pendingGeometry]);

  // Activate the requested saved layout once it has loaded (ADR 0057): a view
  // created via the "+" opens with that layout. Applied once per id.
  const appliedLayoutRef = useRef<string | null>(null);
  useEffect(() => {
    if (!initialLayoutId || appliedLayoutRef.current === initialLayoutId) return;
    const layout = savedLayouts.find((item) => item.id === initialLayoutId);
    if (layout) {
      appliedLayoutRef.current = initialLayoutId;
      applyLayout(layout);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- apply once per id when savedLayouts loads it; applyLayout is a non-memoized component fn intentionally excluded
  }, [initialLayoutId, savedLayouts]);

  // Company dashboard (ADR 0057): when opened scoped to a company (and not opening
  // a specific named view), load that company's saved dashboard layout, else seed
  // the curated default. Applied once per company after layouts have loaded.
  const appliedDashboardRef = useRef<string | null>(null);
  useEffect(() => {
    if (!dashboardCompanyId || initialLayoutId || !layoutsLoaded) return;
    if (appliedDashboardRef.current === dashboardCompanyId) return;
    appliedDashboardRef.current = dashboardCompanyId;
    const saved = savedLayouts.find(
      (layout) => layout.name === dashboardLayoutName(dashboardCompanyId),
    );
    if (saved) applyLayout(saved);
    else seedCompanyDashboard(dashboardCompanyId);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per company after layouts load; non-memoized fns intentionally excluded
  }, [dashboardCompanyId, layoutsLoaded, savedLayouts, initialLayoutId]);

  function seedCompanyDashboard(companyId: string) {
    setPinned(
      DASHBOARD_DEFAULT_KINDS.map((kind) => ({ id: pinnedId(companyId, kind), companyId, kind })),
    );
    setOpenGlobals([]);
    setClosedLinked(new Set(["inspector", "claims-sel", "diff-sel"]));
    setResetNonce((nonce) => nonce + 1);
  }

  function saveDashboard() {
    if (!dashboardCompanyId) return;
    const panels: PanelsDescriptor = {
      pinned,
      openGlobals,
      closedLinked: [...closedLinked],
      selectedFeedItemId: selection.feedItemId,
    };
    const geometry = dockRef.current?.capture() ?? null;
    void saveCockpitLayout({
      name: dashboardLayoutName(dashboardCompanyId),
      panelsJson: JSON.stringify(panels),
      layoutJson: geometry ? JSON.stringify(geometry) : null,
      dockviewVersion: DOCKVIEW_VERSION,
    }).then(refreshLayouts);
  }

  function renderLinked(kind: LinkedKind) {
    switch (kind) {
      case "feed":
        return (
          <FeedPanel
            items={feedItems}
            selectedId={selection.feedItemId}
            onSelect={selectFeedItem}
            text={text}
          />
        );
      case "inspector":
        return <InspectorPanel item={selectedItem} text={text} />;
      case "claims-sel":
        return selectedCompanyId ? (
          <CompanyClaimsPanel key={selectedCompanyId} companyId={selectedCompanyId} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
      case "diff-sel":
        return selectedCompanyId ? (
          <ReportDiffPanel key={selectedCompanyId} companyId={selectedCompanyId} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
    }
  }

  function renderPinned(kind: PinnedKind, companyId: string) {
    switch (kind) {
      case "fundamentals":
        return <CockpitFundamentalsPanel companyId={companyId} />;
      case "reportDiff":
        return <ReportDiffPanel companyId={companyId} />;
      case "claims":
        return <CompanyClaimsPanel companyId={companyId} />;
      case "quality":
        return <QualityPanel companyId={companyId} />;
      case "documents":
        return <CompanyReportDocumentsPanel companyId={companyId} reloadKey={0} />;
    }
  }

  // Global panels render the real, context-driven app screens (no props), so they
  // stay in sync with the rest of the app and need no cockpit-side duplication.
  function renderGlobal(kind: GlobalKind) {
    switch (kind) {
      case "watchlists":
        return <WatchlistsScreen />;
      case "research":
        return <ResearchScreen />;
      case "notebook":
        return <NotebooksScreen />;
      case "events":
        return <EventsScreen />;
      case "reportSeason":
        return <ReportSeasonScreen />;
    }
  }

  const specs: DockPanelSpec[] = [
    ...LINKED.filter((linked) => !closedLinked.has(linked.id)).map((linked) => ({
      id: linked.id,
      // Company-scoped analysis panels (claims/diff) name the selected company in
      // their tab so it is never ambiguous "whose" they are (ADR 0054). The feed
      // and inspector are feed-item-scoped and keep plain titles.
      title:
        (linked.kind === "claims-sel" || linked.kind === "diff-sel") && selectedCompany
          ? `${linkedTitle(linked.kind, text)} · ${selectedCompany.qualifiedTicker}`
          : linkedTitle(linked.kind, text),
      render: () => renderLinked(linked.kind),
    })),
    ...pinned
      .filter((panel) => companyById.has(panel.companyId))
      .map((panel) => {
        const ticker = companyById.get(panel.companyId)?.qualifiedTicker ?? panel.companyId;
        return {
          id: panel.id,
          title: `${ticker} · ${pinnedKindLabel(panel.kind, text)}`,
          render: () => renderPinned(panel.kind, panel.companyId),
        };
      }),
    ...openGlobals.map((kind) => ({
      id: `global:${kind}`,
      title: globalKindLabel(kind, text),
      render: () => renderGlobal(kind),
    })),
  ];

  function handleClose(id: string) {
    if (LINKED.some((linked) => linked.id === id)) {
      setClosedLinked((current) => new Set(current).add(id));
    } else if (id.startsWith("global:")) {
      const kind = id.slice("global:".length) as GlobalKind;
      setOpenGlobals((current) => current.filter((open) => open !== kind));
    } else {
      setPinned((current) => current.filter((panel) => panel.id !== id));
    }
  }

  function openPinned(companyId: string, kind: PinnedKind) {
    const id = pinnedId(companyId, kind);
    setPinned((current) =>
      current.some((panel) => panel.id === id)
        ? current
        : [...current, { id, companyId, kind }],
    );
  }

  function openGlobal(kind: GlobalKind) {
    setOpenGlobals((current) => (current.includes(kind) ? current : [...current, kind]));
  }

  function showLinked(id: string) {
    setClosedLinked((current) => {
      const next = new Set(current);
      next.delete(id);
      return next;
    });
  }

  function resetLayout() {
    setClosedLinked(new Set());
    setPinned([]);
    setOpenGlobals([]);
    selectFeedItem(feedItems[0]?.id ?? null);
    setResetNonce((nonce) => nonce + 1);
  }

  // Built-in presets (phase 4d) compose the panel kinds for a task. Company-scoped
  // panels open for the active company (the linked selection, else the first one);
  // the nonce rebuilds the dock to the preset's default arrangement.
  function applyPreset(preset: PresetSpec) {
    const companyId = selectedCompanyId ?? companies[0]?.id ?? null;
    setClosedLinked(
      new Set(LINKED.filter((linked) => !preset.linked.includes(linked.kind)).map((l) => l.id)),
    );
    setPinned(
      companyId
        ? preset.pinned.map((kind) => ({ id: pinnedId(companyId, kind), companyId, kind }))
        : [],
    );
    setOpenGlobals(preset.globals);
    if (selection.feedItemId == null) {
      selectFeedItem(feedItems[0]?.id ?? null);
    }
    setResetNonce((nonce) => nonce + 1);
  }

  function saveCurrentLayout() {
    const name = layoutName.trim();
    if (!name) return;
    const panels: PanelsDescriptor = {
      pinned,
      openGlobals,
      closedLinked: [...closedLinked],
      selectedFeedItemId: selection.feedItemId,
    };
    const geometry = dockRef.current?.capture() ?? null;
    void saveCockpitLayout({
      name,
      panelsJson: JSON.stringify(panels),
      layoutJson: geometry ? JSON.stringify(geometry) : null,
      dockviewVersion: DOCKVIEW_VERSION,
    })
      .then(() => {
        setLayoutName("");
        refreshLayouts();
      })
      .catch(() => {
        /* failure is surfaced by the runtime; keep the name for a retry */
      });
  }

  function applyLayout(layout: CockpitLayout) {
    const panels = parsePanels(layout.panelsJson);
    if (!panels) return;
    setPinned(panels.pinned);
    setOpenGlobals(panels.openGlobals);
    setClosedLinked(new Set(panels.closedLinked));
    selectFeedItem(panels.selectedFeedItemId);
    // Versioned restore with safe fallback (data-model rule): replay the dockview
    // geometry only when it was produced by the running version; otherwise the
    // panel set rebuilds from panels_json in the default layout.
    if (layout.layoutJson && layout.dockviewVersion === DOCKVIEW_VERSION) {
      try {
        setPendingGeometry(JSON.parse(layout.layoutJson));
      } catch {
        /* corrupt geometry — fall back to the rebuilt default */
      }
    }
  }

  function deleteLayout(id: string) {
    void deleteCockpitLayout(id)
      .then(refreshLayouts)
      .catch(() => {
        /* surfaced by the runtime */
      });
  }

  // Per-company dashboard layouts are reserved; keep them out of the user's named
  // saved-view lists (ADR 0057).
  const namedLayouts = savedLayouts.filter((layout) => !isDashboardLayout(layout));

  // Command palette entries — the launcher: re-show a closed linked panel, open
  // any company panel, load a saved layout, or reset. (Shared with the v0.48.0
  // command-palette epic, which will own the global palette.)
  const commands: PaletteCommand[] = [
    ...PRESETS.map((preset) => ({
      id: `preset:${preset.id}`,
      label: `${text("Apply preset")}: ${text(preset.labelKey)}`,
      run: () => applyPreset(preset),
    })),
    ...LINKED.map((linked) => ({
      id: `show:${linked.id}`,
      label: `${text("Show panel")}: ${linkedTitle(linked.kind, text)}`,
      run: () => showLinked(linked.id),
    })),
    ...companies.flatMap((company) =>
      PINNED_KINDS.map((kind) => ({
        id: `open:${company.id}:${kind}`,
        label: `${text("Open panel")}: ${company.qualifiedTicker} · ${pinnedKindLabel(kind, text)}`,
        run: () => openPinned(company.id, kind),
      })),
    ),
    ...GLOBAL_KINDS.map((kind) => ({
      id: `global:${kind}`,
      label: `${text("Open panel")}: ${globalKindLabel(kind, text)}`,
      run: () => openGlobal(kind),
    })),
    ...namedLayouts.map((layout) => ({
      id: `layout:${layout.id}`,
      label: `${text("Load layout")}: ${layout.name}`,
      run: () => applyLayout(layout),
    })),
    { id: "reset", label: text("Reset layout"), run: resetLayout },
  ];

  const hasLayouts = namedLayouts.length > 0;

  return (
    <section className="cockpit-screen" aria-label={text("Research cockpit")}>
      <div className="cockpit-toolbar">
        <SectionHeader
          level="h3"
          title={text("Research cockpit")}
          actions={
            <div className="cockpit-toolbar-actions">
              <Button onClick={() => setPaletteOpen(true)} variant="secondary" icon={<Command size={15} />}>
                {text("Commands")} (⌘K)
              </Button>
              <Button onClick={resetLayout} variant="secondary">
                {text("Reset layout")}
              </Button>
              {dashboardCompanyId ? (
                <Button onClick={saveDashboard} variant="primary">
                  {text("Save dashboard")}
                </Button>
              ) : null}
            </div>
          }
        />
        <div className="cockpit-add">
          <SelectField
            label={text("Preset")}
            value=""
            onChange={(event) => {
              const preset = PRESETS.find((item) => item.id === event.target.value);
              if (preset) applyPreset(preset);
            }}
          >
            <option value="">{text("Choose a preset…")}</option>
            {PRESETS.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {text(preset.labelKey)}
              </option>
            ))}
          </SelectField>
          <SelectField
            label={text("Saved layout")}
            value=""
            onChange={(event) => {
              const layout = namedLayouts.find((item) => item.id === event.target.value);
              if (layout) applyLayout(layout);
            }}
          >
            <option value="">{hasLayouts ? text("Load a layout…") : text("No saved layouts")}</option>
            {namedLayouts.map((layout) => (
              <option key={layout.id} value={layout.id}>
                {layout.name}
              </option>
            ))}
          </SelectField>
          <TextField
            label={text("Layout name")}
            value={layoutName}
            onChange={(event) => setLayoutName(event.target.value)}
            placeholder={text("e.g. Earnings season")}
          />
          <Button onClick={saveCurrentLayout} variant="primary" disabled={!layoutName.trim()}>
            {text("Save layout")}
          </Button>
          {hasLayouts ? (
            <SelectField
              label={text("Delete layout")}
              value=""
              onChange={(event) => {
                if (event.target.value) deleteLayout(event.target.value);
              }}
            >
              <option value="">{text("Delete a layout…")}</option>
              {namedLayouts.map((layout) => (
                <option key={layout.id} value={layout.id}>
                  {layout.name}
                </option>
              ))}
            </SelectField>
          ) : null}
        </div>
      </div>

      {companies.length === 0 ? (
        <EmptyState>{text("No companies tracked yet.")}</EmptyState>
      ) : (
        <DockLayout
          ref={dockRef}
          panels={specs}
          storageKey={COCKPIT_LAYOUT_STORAGE_KEY}
          resetNonce={resetNonce}
          onClosePanel={handleClose}
        />
      )}

      <CommandPalette
        open={paletteOpen}
        commands={commands}
        onClose={() => setPaletteOpen(false)}
        text={text}
      />
    </section>
  );
}


function FeedPanel({
  items,
  selectedId,
  onSelect,
  text,
}: {
  items: FeedItem[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  text: (s: string) => string;
}) {
  // Cockpit-native feed (decision: clean cockpit panels, not the controller-bound
  // Inbox list). A local title/company filter keeps a long feed scannable in a
  // narrow panel; selection still flows through the shared store (decision 6A).
  const [query, setQuery] = useState("");
  const trimmed = query.trim().toLowerCase();
  const filtered = trimmed
    ? items.filter(
        (item) =>
          item.title.toLowerCase().includes(trimmed) ||
          item.company.toLowerCase().includes(trimmed),
      )
    : items;

  return (
    <section className="cockpit-feed" aria-label={text("Cockpit feed")}>
      <div className="cockpit-feed-filter">
        <SearchField
          ariaLabel={text("Filter feed items")}
          value={query}
          onChange={setQuery}
          onClear={() => setQuery("")}
          clearLabel={text("Clear")}
          placeholder={text("Search the feed…")}
        />
      </div>
      {items.length === 0 ? <EmptyState>{text("No stored feed items.")}</EmptyState> : null}
      {items.length > 0 && filtered.length === 0 ? (
        <EmptyState>{text("No matching feed items.")}</EmptyState>
      ) : null}
      {/* Native <button> per row: DenseRow is an <article>, and role="button" on
          it is invalid ARIA (aria-allowed-role). A real button is keyboard-operable
          and keeps the cockpit axe-clean. */}
      {filtered.map((item) => (
        <button
          type="button"
          key={item.id}
          className={["cockpit-feed-item", item.unread ? "is-unread" : ""].filter(Boolean).join(" ")}
          aria-pressed={item.id === selectedId}
          aria-label={`${text("Inspect feed item")}: ${item.title}`}
          onClick={() => onSelect(item.id)}
        >
          <span className="cockpit-feed-meta">
            <span>{item.type}</span>
            <span>{item.source}</span>
            <TickerLabel value={item.company} />
            <span className="cockpit-feed-time">{formatFeedTimestamp(item.time)}</span>
          </span>
          <span className="cockpit-feed-title">{item.title}</span>
          {item.saved ? <StatusChip tone="accent">{text("Saved")}</StatusChip> : null}
        </button>
      ))}
    </section>
  );
}

function InspectorPanel({
  item,
  text,
}: {
  item: FeedItem | null;
  text: (s: string) => string;
}) {
  if (!item) {
    return <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>;
  }
  // Cockpit-native, read-rich inspector: company/source context, summary, body,
  // and source attachments — no controller-bound actions (AI analysis, mark-read,
  // notes) here; those arrive when the cockpit becomes the host (phase 6).
  const pdfAttachments = item.attachments.filter((attachment) =>
    /\.pdf(?:$|[?#])/i.test(attachment.url),
  );
  return (
    <section className="cockpit-inspector" aria-label={text("Feed item inspector")}>
      <header className="cockpit-inspector-head">
        <TickerLabel value={item.company} />
        <h3 className="cockpit-inspector-title">{item.title}</h3>
        <div className="cockpit-inspector-tags">
          <StatusChip>{item.source}</StatusChip>
          <StatusChip>{item.type}</StatusChip>
          {item.language ? <StatusChip>{item.language.toUpperCase()}</StatusChip> : null}
          <span className="cockpit-inspector-time">{formatFeedTimestamp(item.time)}</span>
        </div>
        {item.attribution ? (
          <p className="cockpit-inspector-attribution">{item.attribution}</p>
        ) : null}
      </header>
      {item.summary ? <p className="cockpit-inspector-summary">{item.summary}</p> : null}
      {item.bodyText ? <p className="cockpit-inspector-body">{item.bodyText}</p> : null}
      {pdfAttachments.length > 0 ? (
        <section className="cockpit-inspector-attachments" aria-label={text("Attachments")}>
          <SectionHeader level="h4" title={text("Attachments")} />
          <ul>
            {pdfAttachments.map((attachment) => (
              <li key={attachment.url}>
                <a href={attachment.url} target="_blank" rel="noreferrer">
                  {attachment.label}
                </a>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      <a className="secondary-button compact-button cockpit-inspector-open" href={item.sourceUrl} target="_blank" rel="noreferrer">
        {text("Open source")}
      </a>
    </section>
  );
}

// The full, editable Fundamentals panel (ADR 0053 phase 4b). It reuses the real
// `FundamentalsPanel` from the Companies screen — the cockpit owns the state via
// `useCockpitFundamentals` (which calls api/financials directly), so editing
// works for any pinned company with no AppStateRoot coupling.
function CockpitFundamentalsPanel({ companyId }: { companyId: string }) {
  const props = useCockpitFundamentals(companyId);
  return <FundamentalsPanel {...props} />;
}
