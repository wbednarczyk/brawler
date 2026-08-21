import { useCallback, useEffect, useRef, useState } from "react";
import { Command, Plus } from "lucide-react";
import type { Company, FeedItem } from "../../api/types";
import { useLocale } from "../../shared/locale";
import {
  Button,
  EmptyState,
  SearchField,
  SectionHeader,
  SelectField,
  StatusChip,
} from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatListTimestamp } from "../../shared/format/datetime";
import { CompanyBasicInfoPanel } from "../../shared/components/CompanyBasicInfoPanel";
import { CompanyClaimsPanel } from "../../shared/components/CompanyClaimsPanel";
import { CompanyReportDocumentsPanel } from "../../shared/components/CompanyReportDocumentsPanel";
import { CompanyCoveragePanel } from "../../shared/components/CompanyCoveragePanel";
import { QualityPanel } from "../../shared/components/QualityPanel";
import { ReportDiffPanel } from "../Companies/ReportDiffPanel";
import { FundamentalsPanel } from "../Companies/FundamentalsPanel";
import { CockpitCompanyFeedPanel } from "./CockpitCompanyFeedPanel";
import { InspectorPanel } from "./InspectorPanel";
import { CompanyNotebookSection } from "../Companies/CompanyNotebookSection";
import { NotebookDateField } from "../../shared/components/NotebookDateField";
import { NotebookQuarterField } from "../../shared/components/NotebookQuarterField";
import { MarkdownNoteBody } from "../../shared/components/MarkdownNoteBody";
import { WatchlistsScreen } from "../Watchlists/WatchlistsScreen";
import { ResearchScreen } from "../Research/ResearchScreen";
import { useResearchViewModel } from "../../app/state/screenViewModels";
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
import { useCockpitCompanyNotebook } from "./useCockpitCompanyNotebook";
import { useCockpitDecisionJournal } from "./useCockpitDecisionJournal";
import { useCockpitShortPositions } from "./useCockpitShortPositions";
import { useCockpitRedFlags } from "./useCockpitRedFlags";
import { useCockpitAnalystRecommendations } from "./useCockpitAnalystRecommendations";
import { DecisionJournalSection } from "./DecisionJournalSection";
import { ShortPositionsSection } from "./ShortPositionsSection";
import { RedFlagsSection } from "./RedFlagsSection";
import { AnalystRecommendationsSection } from "../../shared/components/AnalystRecommendationsSection";
import { DecisionJournalGlobalPanel } from "./DecisionJournalGlobalPanel";
import { formatDetailTimestamp } from "../../shared/format/datetime";
import { CockpitSelectionProvider, useCockpitSelection } from "./CockpitSelectionContext";
import { CommandPalette, type PaletteCommand } from "../../shared/components/CommandPalette";
import { useCommandPaletteCommands } from "../../app/commandPalette";
import { listCockpitLayouts, saveCockpitLayout, type CockpitLayout } from "../../api/cockpit";

// Research cockpit — the full-screen docking shell spike (ADR 0053). It shows
// the LINKED workspace (feed selection drives the inspector + the company's
// claims/diff), a rich Fundamentals panel, a command palette (⌘K) to open any
// panel, and named saved layouts to switch task-shaped workspaces.

type LinkedKind = "feed" | "inspector" | "claims-sel" | "diff-sel";
type PinnedKind =
  | "basicInfo"
  | "fundamentals"
  | "coverage"
  | "reportDiff"
  | "claims"
  | "quality"
  | "documents"
  | "companyFeed"
  | "companyNotebook"
  | "decisionJournal"
  | "shortPositions"
  | "redFlags"
  | "analystRecommendations";

const LINKED: { id: string; kind: LinkedKind }[] = [
  { id: "feed", kind: "feed" },
  { id: "inspector", kind: "inspector" },
  { id: "claims-sel", kind: "claims-sel" },
  { id: "diff-sel", kind: "diff-sel" },
];

const PINNED_KINDS: PinnedKind[] = [
  "basicInfo",
  "fundamentals",
  "coverage",
  "reportDiff",
  "claims",
  "quality",
  "documents",
  "companyFeed",
  "companyNotebook",
  "decisionJournal",
  "shortPositions",
  "redFlags",
  "analystRecommendations",
];

// Global singleton panels (ADR 0053 phase 4c): full app screens that own their
// own scope/data via contexts (mounted prop-free in AppStateRoot), so the cockpit
// renders them directly. Not company-scoped — opened once from the palette.
type GlobalKind =
  | "watchlists"
  | "research"
  | "notebook"
  | "events"
  | "reportSeason"
  | "decisionJournalGlobal";

const GLOBAL_KINDS: GlobalKind[] = [
  "watchlists",
  "research",
  "notebook",
  "events",
  "reportSeason",
  "decisionJournalGlobal",
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
    case "decisionJournalGlobal":
      return text("Journal (all companies)");
  }
}

// A company-scoped cockpit panel (U-Ra, ADR 0076). `follow` panels track the
// view company and carry no companyId (resolved at render time); `pinned` panels
// freeze a specific company (the pre-U-Ra behavior). Legacy descriptors without a
// `mode` parse as `pinned` (see parsePanels), preserving the old semantics.
type Pinned =
  | { id: string; kind: PinnedKind; mode: "follow" }
  | { id: string; kind: PinnedKind; mode: "pinned"; companyId: string };

function pinnedId(companyId: string, kind: PinnedKind): string {
  return `${kind}:${companyId}`;
}

function followId(kind: PinnedKind): string {
  return `follow:${kind}`;
}

// Per-company dashboard layouts (ADR 0057) are saved cockpit layouts under a
// reserved name so each company remembers its own arrangement; these are filtered
// out of the user's named-view lists.
const DASHBOARD_PREFIX = "dashboard:";
const dashboardLayoutName = (companyId: string) => `${DASHBOARD_PREFIX}${companyId}`;
const isDashboardLayout = (layout: { name: string }) => layout.name.startsWith(DASHBOARD_PREFIX);

// The curated default panels a company dashboard opens with when it has no saved
// layout yet (ADR 0057): fundamentals + claims + quality + documents, plus the feed.
const DASHBOARD_DEFAULT_KINDS: PinnedKind[] = [
  "fundamentals",
  "coverage",
  "companyFeed",
  // 4th anchor slot (DockLayout places specs 0–3 as visible groups, the rest
  // tab into the previous group): Basic info anchors the bottom-right group.
  "basicInfo",
  "claims",
  "quality",
  "documents",
  "redFlags",
  "companyNotebook",
];

// The curated dashboard uses the company-scoped panels only, so every
// selection-driven linked panel (feed/inspector/claims-sel/diff-sel) is closed —
// the company feed is covered by the `companyFeed` panel.
const DASHBOARD_CLOSED_LINKED = ["feed", "inspector", "claims-sel", "diff-sel"];

// The curated dashboard now seeds FOLLOW panels (U-Ra): they track the view
// company (set alongside), so their tab titles are kind-only and switching the
// company retargets them in place.
function dashboardPinned(): Pinned[] {
  return DASHBOARD_DEFAULT_KINDS.map((kind) => ({ id: followId(kind), kind, mode: "follow" }));
}

// The dashboard is already seeded when the panel set is exactly the curated
// follow kinds — used to avoid re-seeding (and flashing) on first mount.
function isDashboardFollowSeeded(pinned: Pinned[]): boolean {
  return (
    pinned.length === DASHBOARD_DEFAULT_KINDS.length &&
    pinned.every((panel) => panel.mode === "follow")
  );
}

function pinnedKindLabel(kind: PinnedKind, text: (s: string) => string): string {
  switch (kind) {
    case "basicInfo":
      return text("Basic info");
    case "fundamentals":
      return text("Fundamentals");
    case "coverage":
      return text("Coverage");
    case "reportDiff":
      return text("Report comparison");
    case "claims":
      return text("Claims");
    case "quality":
      return text("Quality");
    case "documents":
      return text("Report documents");
    case "companyFeed":
      return text("Feed");
    case "companyNotebook":
      return text("Notebook");
    case "decisionJournal":
      return text("Decision journal");
    case "shortPositions":
      return text("Short selling (KNF)");
    case "redFlags":
      return text("Warning signals");
    case "analystRecommendations":
      return text("Analyst recommendations");
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

// Dashboard redesign (epic c793ca1): the cockpit is ONE company-scoped Dashboard;
// presets are panel arrangements that ALL follow the currently-selected view
// company (their company-scoped panels seed as FOLLOW, no frozen companyId —
// see applyPreset). Default preset = "Company overview" (the curated dashboard
// follow set). The retired standalone Research screen becomes the "Evidence /
// Research" preset. Kept under the ≤6 visible-panel ceiling.
const PRESETS: PresetSpec[] = [
  {
    id: "company-overview",
    labelKey: "Company overview",
    linked: [],
    pinned: DASHBOARD_DEFAULT_KINDS,
    globals: [],
  },
  {
    id: "evidence",
    labelKey: "Evidence / Research",
    linked: [],
    pinned: [],
    globals: ["research"],
  },
  {
    id: "earnings-season",
    labelKey: "Earnings season",
    linked: [],
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
// What fills a grid-view cell (ADR 0057): a company-scoped panel or a global
// screen. An unfilled cell renders a "pick a panel" button.
type CellFill =
  | { type: "pinned"; companyId: string; kind: PinnedKind }
  | { type: "follow"; kind: PinnedKind }
  | { type: "global"; kind: GlobalKind };

type GridCell = { id: string; fill: CellFill | null };

type GridSpec = { cols: number; rows: number };

type PanelsDescriptor = {
  pinned: Pinned[];
  openGlobals: GlobalKind[];
  closedLinked: string[];
  selectedFeedItemId: string | null;
  // A composable grid view ("+") carries its grid dimensions and the per-cell
  // fill so it reopens pre-split with each cell's chosen panel (or empty).
  grid: GridSpec | null;
  cells: GridCell[] | null;
  // The view company the cockpit view is scoped to (U-Ra); follow panels resolve
  // their company from it. null ⇒ no view company chosen yet.
  viewCompanyId: string | null;
};

// Normalize a persisted company-scoped panel entry (U-Ra), tolerating legacy
// rows that predate the `mode` field: those had a `companyId` and no `mode`, so
// they parse as `pinned` — exactly the pre-U-Ra behavior. A `follow` entry drops
// any companyId (its company comes from the view company at render time).
function normalizePinned(raw: unknown): Pinned | null {
  if (!raw || typeof raw !== "object") return null;
  const entry = raw as { kind?: unknown; mode?: unknown; companyId?: unknown };
  if (typeof entry.kind !== "string" || !PINNED_KINDS.includes(entry.kind as PinnedKind)) {
    return null;
  }
  const kind = entry.kind as PinnedKind;
  if (entry.mode === "follow") {
    return { id: followId(kind), kind, mode: "follow" };
  }
  // Legacy (no mode) or explicit "pinned": requires a companyId.
  if (typeof entry.companyId !== "string") return null;
  return { id: pinnedId(entry.companyId, kind), kind, mode: "pinned", companyId: entry.companyId };
}

export function parsePanels(panelsJson: string): PanelsDescriptor | null {
  try {
    const parsed = JSON.parse(panelsJson) as Partial<PanelsDescriptor>;
    const grid =
      parsed.grid && typeof parsed.grid.cols === "number" && typeof parsed.grid.rows === "number"
        ? { cols: parsed.grid.cols, rows: parsed.grid.rows }
        : null;
    return {
      pinned: Array.isArray(parsed.pinned)
        ? parsed.pinned.map(normalizePinned).filter((panel): panel is Pinned => panel !== null)
        : [],
      openGlobals: Array.isArray(parsed.openGlobals)
        ? parsed.openGlobals.filter((kind): kind is GlobalKind => GLOBAL_KINDS.includes(kind))
        : [],
      closedLinked: Array.isArray(parsed.closedLinked) ? parsed.closedLinked : [],
      selectedFeedItemId:
        typeof parsed.selectedFeedItemId === "string" ? parsed.selectedFeedItemId : null,
      grid,
      cells: Array.isArray(parsed.cells) ? parsed.cells : null,
      viewCompanyId: typeof parsed.viewCompanyId === "string" ? parsed.viewCompanyId : null,
    };
  } catch {
    return null;
  }
}

// Build the empty cells for a fresh grid view (every cell unfilled).
function emptyGridCells(grid: GridSpec): GridCell[] {
  return Array.from({ length: grid.cols * grid.rows }, (_, index) => ({
    id: `cell:${index}`,
    fill: null,
  }));
}

export type CockpitScreenProps = {
  companies: Company[];
  feedItems: FeedItem[];
  /** When set, the cockpit (Advanced layout) opens scoped to this company. */
  initialCompanyId?: string | null;
  /** When set, the cockpit opens with this saved layout activated (ADR 0057). */
  initialLayoutId?: string | null;
  /** When set, the Dashboard opens on this built-in preset (epic c793ca1). */
  initialPresetId?: string | null;
  /** Notifies the host when saved layouts change (create/save/delete) so the
   * sidebar named-views list (ADR 0057 decision 5) stays in sync. */
  onLayoutsChanged?: () => void;
  /** Highlights + scrolls to this claim in the curated dashboard's pinned Claims panel (Today's `openCompanyClaims` nav seam, F2 S4). */
  highlightClaimId?: string | null;
};

export function CockpitScreen({
  companies,
  feedItems,
  initialCompanyId = null,
  initialLayoutId = null,
  initialPresetId = null,
  onLayoutsChanged, highlightClaimId = null,
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
        initialPresetId={initialPresetId}
        dashboardCompanyId={initialCompanyId} highlightClaimId={highlightClaimId}
        onLayoutsChanged={onLayoutsChanged}
      />
    </CockpitSelectionProvider>
  );
}

function CockpitWorkspace({
  companies,
  feedItems,
  initialLayoutId = null,
  initialPresetId = null,
  dashboardCompanyId = null, highlightClaimId = null,
  onLayoutsChanged,
}: Omit<CockpitScreenProps, "initialCompanyId"> & { dashboardCompanyId?: string | null }) {
  const { text } = useLocale();
  // Shared selection lives in the cockpit store (decision 6A), not local state.
  const { selection, selectedFeedItem, selectedCompany, selectFeedItem } = useCockpitSelection();
  // The research/evidence panel (Dowody preset) reads from the shared research
  // read model; keep a handle to retarget it to the view company below.
  const { setMode: setResearchMode, setSelectedCompanyId: setResearchCompanyId } =
    useResearchViewModel();
  // Seed the dashboard panels synchronously when opening scoped to a company so
  // the very first paint is already the curated dashboard — not the default
  // linked triad that then rebuilds into it (the "flash" on opening a company).
  const [pinned, setPinned] = useState<Pinned[]>(() =>
    dashboardCompanyId ? dashboardPinned() : [],
  );
  // The view company (U-Ra): follow panels resolve to it; the header selector and
  // the ⌘K "switch view company" action set it. Seeded from the opening company;
  // a saved view's persisted value overrides it on apply.
  const [viewCompanyId, setViewCompanyId] = useState<string | null>(dashboardCompanyId ?? null);
  // The active preset (Dashboard redesign, epic c793ca1): the Preset selector
  // reflects it so the user always sees which arrangement is showing. A company
  // dashboard opens on the "Company overview" preset (its seeded follow set);
  // applying another preset or a saved/grid layout updates or clears it.
  const [activePresetId, setActivePresetId] = useState<string | null>(
    initialPresetId ?? (dashboardCompanyId ? "company-overview" : null),
  );
  const [openGlobals, setOpenGlobals] = useState<GlobalKind[]>([]);
  const [closedLinked, setClosedLinked] = useState<Set<string>>(
    () => new Set(dashboardCompanyId ? DASHBOARD_CLOSED_LINKED : []),
  );
  const [resetNonce, setResetNonce] = useState(0);
  // The local palette instance stays for the cell-fill / add-panel / "Commands"
  // toolbar flows (it arms fillTargetRef). The global ⌘K palette (AppShell) is
  // opened by the Ctrl/⌘+K shortcut and is fed the cockpit commands contextually
  // (see useCommandPaletteCommands below).
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [savedLayouts, setSavedLayouts] = useState<CockpitLayout[]>([]);
  const [pendingGeometry, setPendingGeometry] = useState<unknown>(null);
  // Composable grid view (ADR 0057): when active, the view is a fixed cols×rows
  // grid of cells, each rendering its chosen panel or a "pick a panel" button.
  // `gridCells` drives the dock specs instead of pinned/globals/linked.
  const [gridCells, setGridCells] = useState<GridCell[]>([]);
  const [gridDims, setGridDims] = useState<GridSpec | null>(null);
  // Cross-panel invalidation: a fact-producing extraction in the report-documents
  // panel bumps this so the sibling Fundamentals panel(s) refetch. Independent
  // cockpit panels share no read model, so this is the minimal invalidation
  // signal (kept local to the workspace, not a global event bus).
  const [fundamentalsRevision, setFundamentalsRevision] = useState(0);
  const bumpFundamentals = useCallback(() => setFundamentalsRevision((n) => n + 1), []);
  // The cell awaiting a panel pick (set by a cell's button before opening the
  // palette); null means the palette adds/opens normally.
  const fillTargetRef = useRef<string | null>(null);
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
        onLayoutsChanged?.();
      })
      .catch(() => {
        setSavedLayouts([]);
        setLayoutsLoaded(true);
      });
  }, [onLayoutsChanged]);
  useEffect(() => {
    refreshLayouts();
  }, [refreshLayouts]);

  // ⌘K / Ctrl+K is handled globally now (AppShell command palette, v0.50 U6);
  // the cockpit only keeps its local palette for the cell-fill / add-panel flows.

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
    if (!dashboardCompanyId || initialLayoutId || initialPresetId || !layoutsLoaded) return;
    if (appliedDashboardRef.current === dashboardCompanyId) return;
    appliedDashboardRef.current = dashboardCompanyId;
    const saved = savedLayouts.find(
      (layout) => layout.name === dashboardLayoutName(dashboardCompanyId),
    );
    if (saved) {
      // A saved per-company dashboard overrides the seeded default (restores the
      // user's arrangement + geometry).
      applyLayout(saved);
    } else if (!(isDashboardFollowSeeded(pinned) && viewCompanyId === dashboardCompanyId)) {
      // No saved layout: the initializer already seeded this company on first
      // mount (no rebuild → no flash); only re-seed when switching companies
      // without a remount.
      seedCompanyDashboard(dashboardCompanyId);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per company after layouts load; non-memoized fns intentionally excluded
  }, [dashboardCompanyId, layoutsLoaded, savedLayouts, initialLayoutId]);

  // Open on a requested preset (epic c793ca1), applied once per mount after the
  // view company is seeded, so the preset's panels render for that company.
  const appliedPresetRef = useRef(false);
  useEffect(() => {
    if (appliedPresetRef.current || !initialPresetId) return;
    const preset = PRESETS.find((item) => item.id === initialPresetId);
    if (preset) {
      appliedPresetRef.current = true;
      applyPreset(preset);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- apply once per mount; applyPreset is a non-memoized component fn intentionally excluded
  }, [initialPresetId]);

  // The research/evidence cockpit panel (Dowody preset) follows the view company
  // (epic c793ca1): when the view company changes, retarget the research read
  // model to it so the evidence timeline tracks the Dashboard's company like every
  // other follow panel. Guarded on a set view company (null ⇒ leave research state
  // untouched). The setters are stable useState dispatchers, so this only fires on
  // an actual view-company change.
  useEffect(() => {
    if (!viewCompanyId) return;
    setResearchMode("company");
    setResearchCompanyId(viewCompanyId);
  }, [viewCompanyId, setResearchMode, setResearchCompanyId]);

  function seedCompanyDashboard(companyId: string) {
    setGridDims(null);
    setGridCells([]);
    setPinned(dashboardPinned());
    setViewCompanyId(companyId);
    setOpenGlobals([]);
    setClosedLinked(new Set(DASHBOARD_CLOSED_LINKED));
    setActivePresetId("company-overview");
    setResetNonce((nonce) => nonce + 1);
  }

  function saveDashboard() {
    if (!dashboardCompanyId) return;
    const panels: PanelsDescriptor = {
      pinned,
      openGlobals,
      closedLinked: [...closedLinked],
      selectedFeedItemId: selection.feedItemId,
      grid: null,
      cells: null,
      viewCompanyId,
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
      case "basicInfo":
        return <CompanyBasicInfoPanel key={companyId} companyId={companyId} />;
      case "fundamentals":
        return (
          <CockpitFundamentalsPanel
            companyId={companyId}
            qualifiedTicker={companyById.get(companyId)?.qualifiedTicker}
            revision={fundamentalsRevision}
            onOpenRecommendations={() => openPinned(companyId, "analystRecommendations")}
          />
        );
      case "coverage":
        return (
          <CompanyCoveragePanel
            companyId={companyId}
            reloadKey={fundamentalsRevision}
            onOpenDocuments={() => openPinned(companyId, "documents")}
            onHistoryRefreshed={bumpFundamentals}
          />
        );
      case "reportDiff":
        return <ReportDiffPanel companyId={companyId} />;
      case "claims":
        return <CompanyClaimsPanel companyId={companyId} highlightClaimId={companyId === dashboardCompanyId ? highlightClaimId : null} />;
      case "quality":
        return <QualityPanel companyId={companyId} />;
      case "documents":
        return <CompanyReportDocumentsPanel companyId={companyId} onExtracted={bumpFundamentals} />;
      case "companyFeed": {
        const company = companyById.get(companyId);
        return company ? (
          <CockpitCompanyFeedPanel company={company} feedItems={feedItems} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
      }
      case "companyNotebook": {
        const company = companyById.get(companyId);
        return company ? (
          <CockpitCompanyNotebookPanel company={company} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
      }
      case "decisionJournal": {
        const company = companyById.get(companyId);
        return company ? (
          <CockpitDecisionJournalPanel company={company} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
      }
      case "shortPositions": {
        const company = companyById.get(companyId);
        return company ? (
          <CockpitShortPositionsPanel company={company} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
      }
      case "redFlags": {
        const company = companyById.get(companyId);
        return company ? (
          <CockpitRedFlagsPanel company={company} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
      }
      case "analystRecommendations": {
        const company = companyById.get(companyId);
        return company ? (
          <CockpitAnalystRecommendationsPanel company={company} />
        ) : (
          <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>
        );
      }
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
      case "decisionJournalGlobal":
        return <DecisionJournalGlobalPanel companies={companies} />;
    }
  }

  function cellFillTitle(fill: CellFill): string {
    if (fill.type === "pinned") {
      const ticker = companyById.get(fill.companyId)?.qualifiedTicker ?? fill.companyId;
      return `${ticker} · ${pinnedKindLabel(fill.kind, text)}`;
    }
    if (fill.type === "follow") {
      // Follow cells are kind-only (the view company carries the context, U-Ra).
      return pinnedKindLabel(fill.kind, text);
    }
    return globalKindLabel(fill.kind, text);
  }

  function renderCellFill(fill: CellFill) {
    if (fill.type === "pinned") return renderPinned(fill.kind, fill.companyId);
    if (fill.type === "follow") {
      return viewCompanyId ? (
        renderPinned(fill.kind, viewCompanyId)
      ) : (
        <EmptyState>{text("Choose the view company")}</EmptyState>
      );
    }
    return renderGlobal(fill.kind);
  }

  // A cell's center button arms the next palette pick for that cell.
  function pickPanelForCell(cellId: string) {
    fillTargetRef.current = cellId;
    setPaletteOpen(true);
  }

  // Fill the armed cell with the chosen panel — swaps the cell's content in place
  // (same panel id), so dockview keeps it in its grid position (no rebuild).
  function fillCell(cellId: string, fill: CellFill) {
    setGridCells((cells) => cells.map((cell) => (cell.id === cellId ? { ...cell, fill } : cell)));
  }

  // The toolbar "Add panel" action: in a grid view it targets the first empty
  // cell; otherwise it opens the palette to append a panel.
  function openAddPanel() {
    const empty = gridCells.find((cell) => !cell.fill);
    if (empty) {
      pickPanelForCell(empty.id);
      return;
    }
    fillTargetRef.current = null;
    setPaletteOpen(true);
  }

  const isGridView = gridCells.length > 0;

  const gridSpecs: DockPanelSpec[] = gridCells.map((cell) => ({
    id: cell.id,
    title: cell.fill ? cellFillTitle(cell.fill) : text("Empty cell"),
    render: () =>
      cell.fill ? (
        renderCellFill(cell.fill)
      ) : (
        <PickPanelCell onPick={() => pickPanelForCell(cell.id)} text={text} />
      ),
  }));

  const panelSpecs: DockPanelSpec[] = [
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
      .filter((panel) => panel.mode === "follow" || companyById.has(panel.companyId))
      .map((panel) => {
        if (panel.mode === "follow") {
          // Follow panels: kind-only title, content resolved from the view company
          // (or an empty state prompting to choose one, U-Ra / D2 / D4). The tab's
          // pin toggle freezes the current view company onto the panel.
          return {
            id: panel.id,
            title: pinnedKindLabel(panel.kind, text),
            render: () =>
              viewCompanyId ? (
                renderPinned(panel.kind, viewCompanyId)
              ) : (
                <EmptyState>{text("Choose the view company")}</EmptyState>
              ),
            pin: {
              pinned: false,
              disabled: viewCompanyId == null,
              label: text("Pin company"),
              onToggle: () => togglePanelMode(panel),
            },
          };
        }
        const ticker = companyById.get(panel.companyId)?.qualifiedTicker ?? panel.companyId;
        // A pinned panel can rejoin the view company only when no follow panel of
        // its kind already exists (ids are kind-keyed — a collision would clash).
        const followExists = pinned.some(
          (other) => other.mode === "follow" && other.kind === panel.kind,
        );
        return {
          id: panel.id,
          title: `${ticker} · ${pinnedKindLabel(panel.kind, text)}`,
          render: () => renderPinned(panel.kind, panel.companyId),
          pin: {
            pinned: true,
            disabled: followExists,
            label: followExists
              ? text("Another panel already follows the view company")
              : text("Follow view company"),
            onToggle: () => togglePanelMode(panel),
          },
        };
      }),
    ...openGlobals.map((kind) => ({
      id: `global:${kind}`,
      title: globalKindLabel(kind, text),
      render: () => renderGlobal(kind),
    })),
  ];

  const specs: DockPanelSpec[] = isGridView ? gridSpecs : panelSpecs;

  function handleClose(id: string) {
    if (isGridView) {
      // Closing a grid cell empties it back to a "pick a panel" button; the cell
      // itself stays so the grid keeps its shape (rebuild to restore its slot).
      setGridCells((cells) => cells.map((cell) => (cell.id === id ? { ...cell, fill: null } : cell)));
      setResetNonce((nonce) => nonce + 1);
      return;
    }
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
    if (fillTargetRef.current) {
      fillCell(fillTargetRef.current, { type: "pinned", companyId, kind });
      fillTargetRef.current = null;
      setPaletteOpen(false);
      return;
    }
    const id = pinnedId(companyId, kind);
    setPinned((current) =>
      current.some((panel) => panel.id === id)
        ? current
        : [...current, { id, kind, mode: "pinned", companyId }],
    );
  }

  // Open (or reveal) a FOLLOW panel of `kind` (U-Ra): it tracks the view company.
  // Only reachable when a view company is set (the palette gates the entries).
  function openFollow(kind: PinnedKind) {
    if (fillTargetRef.current) {
      fillCell(fillTargetRef.current, { type: "follow", kind });
      fillTargetRef.current = null;
      setPaletteOpen(false);
      return;
    }
    const id = followId(kind);
    setPinned((current) =>
      current.some((panel) => panel.id === id)
        ? current
        : [...current, { id, kind, mode: "follow" }],
    );
  }

  // Toggle a company-scoped panel between following the view company and pinning a
  // frozen company (U-Ra / D5). The panel's id changes, so the stale panel is
  // removed from dockview (geometry loss for that one panel is acceptable) and the
  // add-only reconciler re-adds the new id — the rest of the layout is untouched.
  function togglePanelMode(panel: Pinned) {
    if (panel.mode === "follow") {
      // follow → pinned: freeze the current view company onto this panel.
      if (!viewCompanyId) return;
      const nextId = pinnedId(viewCompanyId, panel.kind);
      dockRef.current?.removePanel(panel.id);
      setPinned((current) => {
        // If a pinned panel for this (kind, company) already exists, just drop the
        // follow panel rather than create a duplicate id.
        if (current.some((other) => other.id === nextId)) {
          return current.filter((other) => other.id !== panel.id);
        }
        return current.map((other) =>
          other.id === panel.id
            ? { id: nextId, kind: panel.kind, mode: "pinned", companyId: viewCompanyId }
            : other,
        );
      });
    } else {
      // pinned → follow: rejoin the view company. Blocked (and the tab button is
      // disabled) when a follow panel of this kind already exists.
      const nextId = followId(panel.kind);
      if (pinned.some((other) => other.id === nextId)) return;
      dockRef.current?.removePanel(panel.id);
      setPinned((current) =>
        current.map((other) =>
          other.id === panel.id ? { id: nextId, kind: panel.kind, mode: "follow" } : other,
        ),
      );
    }
  }

  function openGlobal(kind: GlobalKind) {
    if (fillTargetRef.current) {
      fillCell(fillTargetRef.current, { type: "global", kind });
      fillTargetRef.current = null;
      setPaletteOpen(false);
      return;
    }
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
    if (isGridView && gridDims) {
      // Reset a grid view to all-empty cells (keep its dimensions).
      setGridCells(emptyGridCells(gridDims));
      setResetNonce((nonce) => nonce + 1);
      return;
    }
    setClosedLinked(new Set());
    setPinned([]);
    setOpenGlobals([]);
    selectFeedItem(feedItems[0]?.id ?? null);
    setResetNonce((nonce) => nonce + 1);
  }

  // Built-in presets compose the panel kinds for a task. Company-scoped panels
  // seed as FOLLOW (Dashboard redesign, epic c793ca1): they track the view company
  // and retarget in place when it changes, so every preset follows the currently
  // selected company. The nonce rebuilds the dock to the preset's arrangement.
  function applyPreset(preset: PresetSpec) {
    setGridDims(null);
    setGridCells([]);
    setClosedLinked(
      new Set(LINKED.filter((linked) => !preset.linked.includes(linked.kind)).map((l) => l.id)),
    );
    setPinned(
      preset.pinned.map((kind) => ({ id: followId(kind), kind, mode: "follow" as const })),
    );
    setOpenGlobals(preset.globals);
    setActivePresetId(preset.id);
    if (selection.feedItemId == null) {
      selectFeedItem(feedItems[0]?.id ?? null);
    }
    setResetNonce((nonce) => nonce + 1);
  }

  function applyLayout(layout: CockpitLayout) {
    const panels = parsePanels(layout.panelsJson);
    if (!panels) return;
    // A saved/grid layout is a custom arrangement, not a built-in preset — clear
    // the active-preset reflection so the selector shows no preset selected.
    setActivePresetId(null);
    // A saved view carries its view company (U-Ra); the persisted value wins.
    setViewCompanyId(panels.viewCompanyId);
    if (panels.grid) {
      // A composable grid view: render the fixed cols×rows cell grid (with any
      // saved per-cell fills), not the pinned/global/linked panel set.
      setGridDims(panels.grid);
      setGridCells(panels.cells ?? emptyGridCells(panels.grid));
      setPinned([]);
      setOpenGlobals([]);
      setClosedLinked(new Set(["feed", "inspector", "claims-sel", "diff-sel"]));
      setResetNonce((nonce) => nonce + 1);
      return;
    }
    setGridDims(null);
    setGridCells([]);
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
    // Company-scoped panel TYPES — the add surface lists GENERIC kinds only, and
    // each opens as a FOLLOW panel bound to the current view company (retargets in
    // place when it changes, U-Ra / D2 / card 106f8a7). No entry here enumerates a
    // specific company (owner dogfooding round 2): retargeting the view is the
    // header selector's job, and freezing a company is a panel's own pin toggle.
    ...(viewCompanyId
      ? PINNED_KINDS.map((kind) => ({
          id: `follow:${kind}`,
          label: `${text("Open panel")}: ${pinnedKindLabel(kind, text)}`,
          run: () => openFollow(kind),
        }))
      : []),
    // Global panels — app-wide singletons, not company-scoped (their own group).
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

  // Contribute the cockpit's no-target commands to the global ⌘K palette while the
  // cockpit is mounted (v0.50 U6). Cell-fill arming stays on the local palette.
  useCommandPaletteCommands("cockpit", commands);

  return (
    // Journey-metrics observation point (ADR 0074 pt 3 / ADR 0081 Q3):
    // semantic marker for the company currently shown in the cockpit (null
    // when the view isn't scoped to one), not test-only business state.
    <section className="cockpit-screen" aria-label={text("Research cockpit")} data-company-id={viewCompanyId ?? undefined}>
      <div className="cockpit-toolbar">
        <SectionHeader
          level="h3"
          title={text("Research cockpit")}
          actions={
            <div className="cockpit-toolbar-actions">
              <Button onClick={openAddPanel} variant="primary" icon={<Plus size={15} />}>
                {text("Add panel")}
              </Button>
              <Button
                onClick={() => {
                  fillTargetRef.current = null;
                  setPaletteOpen(true);
                }}
                variant="secondary"
                icon={<Command size={15} />}
              >
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
            label={text("View company")}
            value={viewCompanyId ?? ""}
            onChange={(event) => setViewCompanyId(event.target.value || null)}
          >
            <option value="">—</option>
            {companies.map((company) => (
              <option key={company.id} value={company.id}>
                {company.qualifiedTicker} - {company.displayName}
              </option>
            ))}
          </SelectField>
          <SelectField
            label={text("Preset")}
            value={activePresetId ?? ""}
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
        </div>
      </div>

      {companies.length === 0 ? (
        <EmptyState>{text("No companies tracked yet.")}</EmptyState>
      ) : specs.length === 0 ? (
        <EmptyState className="cockpit-empty-view">
          <strong>{text("This view is empty.")}</strong>
          <p>{text("Add panels to build your view — pick a pre-built panel from the palette.")}</p>
          <Button onClick={openAddPanel} variant="primary" icon={<Plus size={15} />}>
            {text("Add panel")}
          </Button>
        </EmptyState>
      ) : (
        <DockLayout
          ref={dockRef}
          panels={specs}
          grid={isGridView ? gridDims : null}
          storageKey={COCKPIT_LAYOUT_STORAGE_KEY}
          resetNonce={resetNonce}
          onClosePanel={handleClose}
        />
      )}

      <CommandPalette
        open={paletteOpen}
        commands={commands}
        onClose={() => {
          fillTargetRef.current = null;
          setPaletteOpen(false);
        }}
        text={text}
      />
    </section>
  );
}

// The center affordance in an empty grid-view cell (ADR 0057): a single button
// that opens the palette to pick the panel for this cell.
function PickPanelCell({ onPick, text }: { onPick: () => void; text: (s: string) => string }) {
  return (
    <div className="cockpit-cell-empty">
      <Button onClick={onPick} variant="primary" icon={<Plus size={15} />}>
        {text("Pick a panel")}
      </Button>
    </div>
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
  const { locale } = useLocale();
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
    <div role="group" className="cockpit-feed" aria-label={text("Cockpit feed")}>
      <div className="cockpit-feed-filter">
        <SearchField
          ariaLabel={text("Filter feed items")}
          className="search-box"
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
            <span className="cockpit-feed-time num-tabular">{formatListTimestamp(item.time, locale)}</span>
          </span>
          <span className="cockpit-feed-title">{item.title}</span>
          {item.saved ? <StatusChip tone="accent">{text("Saved")}</StatusChip> : null}
        </button>
      ))}
    </div>
  );
}

// The full, editable Fundamentals panel (ADR 0053 phase 4b). It reuses the real
// `FundamentalsPanel` from the Companies screen — the cockpit owns the state via
// `useCockpitFundamentals` (which calls api/financials directly), so editing
// works for any pinned company with no AppStateRoot coupling.
function CockpitFundamentalsPanel({
  companyId,
  qualifiedTicker,
  revision,
  onOpenRecommendations,
}: {
  companyId: string;
  // Card #307: the fact-detail modal's header line. Omitted when the pinned
  // company isn't (yet) resolvable in `companyById`.
  qualifiedTicker?: string;
  // Bumped by a sibling report-documents extraction; forces a facts refetch.
  revision: number;
  // Opens/pins the analyst-recommendations panel from the "vs target" readout.
  onOpenRecommendations?: () => void;
}) {
  const props = useCockpitFundamentals(companyId, revision);
  return (
    <FundamentalsPanel
      {...props}
      qualifiedTicker={qualifiedTicker}
      onOpenRecommendations={onOpenRecommendations}
    />
  );
}

// Company-scoped notebook panel for the curated dashboard (ADR 0057). Reuses the
// real `CompanyNotebookSection` with cockpit-owned state (`useCockpitCompanyNotebook`).
// Origins render read-only (label + external source link) — the cross-screen
// "open origin feed item" nav belongs to the Inbox, not a self-contained panel.
function CockpitCompanyNotebookPanel({ company }: { company: Company }) {
  const { text } = useLocale();
  const notebook = useCockpitCompanyNotebook(company);
  return (
    <CompanyNotebookSection
      company={company}
      notebookEntries={notebook.entries}
      isComposerOpen={notebook.isComposerOpen}
      notebookForm={notebook.notebookForm}
      selectedNotebookEntry={notebook.selectedEntry}
      notebookEditMode={notebook.editMode}
      notebookEditForm={notebook.editForm}
      isNotebookEditDirty={notebook.isEditDirty}
      notebookError={notebook.error}
      setComposerOpen={notebook.setComposerOpen}
      updateNotebookForm={notebook.updateNotebookForm}
      createNotebookEntry={notebook.createNotebookEntry}
      setSelectedNotebookEntryId={notebook.setSelectedEntryId}
      saveNotebookEntry={notebook.saveNotebookEntry}
      cancelNotebookEdit={notebook.cancelNotebookEdit}
      setNotebookEditMode={notebook.setEditMode}
      updateNotebookEditForm={notebook.updateNotebookEditForm}
      NotebookDateField={NotebookDateField}
      NotebookQuarterField={NotebookQuarterField}
      MarkdownNoteBody={MarkdownNoteBody}
      renderNotebookOrigins={(origins) =>
        origins.length === 0 ? (
          <span className="membership-empty">None</span>
        ) : (
          <div className="origin-link-list">
            {origins.map((origin) => (
              <div className="origin-link" key={origin.id}>
                <span>{origin.label ?? origin.sourceType.replace("_", " ")}</span>
                {origin.sourceUrl ? (
                  <a
                    className="secondary-button compact-button"
                    href={origin.sourceUrl}
                    rel="noreferrer"
                    target="_blank"
                  >
                    {text("Source")}
                  </a>
                ) : null}
              </div>
            ))}
          </div>
        )
      }
    />
  );
}

// Company-scoped decision-journal panel (ADR 0071, J3). Reuses the props-driven
// `DecisionJournalSection` with cockpit-owned state (`useCockpitDecisionJournal`),
// mirroring the notebook panel. Not in the curated dashboard defaults — the
// journal is an occasional-entry surface reached via the palette / add-panel.
function CockpitShortPositionsPanel({ company }: { company: Company }) {
  const { view, error } = useCockpitShortPositions(company);
  return <ShortPositionsSection company={company} view={view} error={error} />;
}

function CockpitRedFlagsPanel({ company }: { company: Company }) {
  const { view, error, acknowledge } = useCockpitRedFlags(company);
  const { selectFeedItem } = useCockpitSelection();
  return (
    <RedFlagsSection
      company={company}
      view={view}
      error={error}
      onAcknowledge={acknowledge}
      onOpenEvidence={selectFeedItem}
    />
  );
}

// Palette-only analyst-recommendations panel (v0.58 A3, ADR 0073). Not in the
// curated dashboard defaults — an opt-in quiet read surface (storyboard frame 1).
function CockpitAnalystRecommendationsPanel({ company }: { company: Company }) {
  const { view, error, loading, lastClose, currency, reload } =
    useCockpitAnalystRecommendations(company);
  return (
    <AnalystRecommendationsSection
      company={company}
      view={view}
      error={error}
      loading={loading}
      onRetry={reload}
      lastClose={lastClose}
      currency={currency}
    />
  );
}

function CockpitDecisionJournalPanel({ company }: { company: Company }) {
  const journal = useCockpitDecisionJournal(company);
  return (
    <DecisionJournalSection
      company={company}
      entries={journal.entries}
      isComposerOpen={journal.isComposerOpen}
      form={journal.form}
      supersedingEntry={journal.supersedingEntry}
      selectedEntry={journal.selectedEntry}
      evidenceCandidates={journal.evidenceCandidates}
      linkedEvidenceKeys={journal.linkedEvidenceKeys}
      error={journal.error}
      setComposerOpen={journal.setComposerOpen}
      updateForm={journal.updateForm}
      createEntry={journal.createEntry}
      startSupersede={journal.startSupersede}
      cancelSupersede={journal.cancelSupersede}
      setSelectedEntryId={journal.setSelectedEntryId}
      linkEvidence={journal.linkEvidence}
      formatTimestamp={formatDetailTimestamp}
    />
  );
}
