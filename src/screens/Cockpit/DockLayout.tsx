import { createContext, forwardRef, useContext, useEffect, useImperativeHandle, useRef, useState } from "react";
import type { ReactNode } from "react";
import { ExternalLink, Maximize2, PictureInPicture2 } from "lucide-react";
import { DockviewReact } from "dockview";
import { useLocale } from "../../shared/locale";
import type {
  DockviewApi,
  DockviewGroupPanel,
  DockviewReadyEvent,
  IDockviewPanelProps,
  IDockviewPanelHeaderProps,
  IDockviewHeaderActionsProps,
  SerializedDockview,
} from "dockview";
import "dockview/dist/styles/dockview.css";

/** The installed dockview version, stamped onto a saved layout's geometry so a
 *  future geometry-format change can be migrated or safely discarded on restore
 *  (ADR 0053, decision 3A). Bump this when upgrading the `dockview` dependency. */
export const DOCKVIEW_VERSION = "6.6.1";

/** localStorage key for the live dockview geometry (distinct from the named
 *  SQLite layouts). Exported so a recovery path can clear a geometry that crashes
 *  the dock on restore. */
export const COCKPIT_LAYOUT_STORAGE_KEY = "cockpit.layout.v2";

/** Drop the persisted live geometry so the cockpit rebuilds its default layout —
 *  the escape hatch when a stored layout makes the dock throw. */
export function clearCockpitLayoutStorage() {
  try {
    window.localStorage.removeItem(COCKPIT_LAYOUT_STORAGE_KEY);
  } catch {
    /* storage unavailable — nothing to clear */
  }
}

// DockLayout — the dockview adapter for the research-cockpit shell (ADR 0053).
// dockview owns only pane *arrangement*; everything inside a pane stays
// primitive-first (ADR 0037). This is the ONLY module allowed to import dockview
// (enforced by eslint no-restricted-imports, scoped to src/screens/Cockpit/).
//
// What this adds over the rejected inline pilot:
//  - a full-screen canvas host (the inline-list host was the wrong gospodarz);
//  - ACCESSIBLE tabs: a custom tab component with role="tab" + aria-selected +
//    keyboard activation, closing the a11y gap dockview's default div-tabs left;
//  - best-effort layout persistence to localStorage (the open question the pilot
//    could not answer) with a reset path;
//  - dynamic add/remove of panels reconciled from React state.

export type DockPanelSpec = {
  /** Stable panel id. */
  id: string;
  /** Tab strip label. */
  title: string;
  /** Pane body. Re-evaluated on every render via context — never stale. */
  render: () => ReactNode;
};

/**
 * Pop a panel group out to a separate OS window, degrading gracefully to an
 * in-app floating group when an OS window cannot be opened (ADR 0053 decision 2A:
 * floating works regardless; OS-window pop-out is the gated capability).
 *
 * dockview's `addPopoutGroup` opens the window via `window.open` and resolves
 * `false` when that is blocked or unavailable — which is exactly the case in a
 * browser/jsdom and, until validated, inside Tauri (Tauri routes `window.open`
 * through its `WebviewWindow` bridge, requiring `core:webview:allow-create-webview-window`).
 * Either way the panel must not be lost: on a `false` result or a throw we fall
 * back to a floating group, so the action always does something useful. The
 * real OS-window path must still be VALIDATED on native Windows (Faza 5 sub-spike);
 * this code makes the bridge possible and safe, not verified.
 */
export async function popOutOrFloat(
  api: Pick<DockviewApi, "addPopoutGroup" | "addFloatingGroup">,
  group: DockviewGroupPanel,
): Promise<"popout" | "float"> {
  try {
    if (await api.addPopoutGroup(group)) return "popout";
  } catch {
    /* OS window blocked (browser/jsdom, or un-bridged Tauri) — float instead */
  }
  api.addFloatingGroup(group);
  return "float";
}

const PanelContentContext = createContext<Map<string, () => ReactNode>>(new Map());

function DockPanel(props: IDockviewPanelProps) {
  const contents = useContext(PanelContentContext);
  const render = contents.get(props.api.id);
  return <div className="cockpit-pane">{render ? render() : null}</div>;
}

// Accessible tab — vs dockview's default div tab (no role, no state, mouse-only).
// This is the a11y fix the pilot flagged as mandatory before adoption. Two
// SIBLING native <button>s (activate + close): real keyboard operation and
// screen-reader semantics, while avoiding both `aria-required-parent` (role="tab"
// needs a tablist parent dockview does not provide) and `nested-interactive`
// (a close control nested inside an interactive tab). aria-pressed carries the
// active state.
function AccessibleTab(props: IDockviewPanelHeaderProps) {
  const { api } = props;
  const [active, setActive] = useState(api.isActive);
  const [title, setTitle] = useState(api.title);
  useEffect(() => {
    const a = api.onDidActiveChange(() => setActive(api.isActive));
    const t = api.onDidTitleChange(() => setTitle(api.title));
    return () => {
      a.dispose();
      t.dispose();
    };
  }, [api]);
  return (
    <span className="cockpit-tab">
      <button
        type="button"
        className="cockpit-tab-activate"
        aria-pressed={active}
        onClick={() => api.setActive()}
      >
        {title}
      </button>
      <button
        type="button"
        className="cockpit-tab-close"
        aria-label={`Close ${title}`}
        title={`Close ${title}`}
        onClick={() => api.close()}
      >
        ×
      </button>
    </span>
  );
}

// Per-group header actions — maximize, float, and pop out to a separate OS
// window. The pop-out is the cockpit's answer to the ultrawide / FancyZones
// use case (dockview opens a real window for the group). All accessible buttons.
function GroupActions(props: IDockviewHeaderActionsProps) {
  const { containerApi, group, activePanel } = props;
  const { text } = useLocale();
  // `title` gives the visible hover tooltip these tiny icon buttons were missing;
  // `aria-label` keeps the same text for screen readers.
  const maximizeLabel = text("Maximize panel group");
  const floatLabel = text("Float panel group");
  const popoutLabel = text("Pop out panel group to a window");
  return (
    <div className="cockpit-group-actions">
      <button
        type="button"
        aria-label={maximizeLabel}
        title={maximizeLabel}
        onClick={() => {
          if (containerApi.hasMaximizedGroup()) containerApi.exitMaximizedGroup();
          else if (activePanel) containerApi.maximizeGroup(activePanel);
        }}
      >
        <Maximize2 size={13} />
      </button>
      <button
        type="button"
        aria-label={floatLabel}
        title={floatLabel}
        onClick={() => containerApi.addFloatingGroup(group)}
      >
        <PictureInPicture2 size={13} />
      </button>
      <button
        type="button"
        aria-label={popoutLabel}
        title={popoutLabel}
        onClick={() => void popOutOrFloat(containerApi, group)}
      >
        <ExternalLink size={13} />
      </button>
    </div>
  );
}

const DOCK_COMPONENTS = { default: DockPanel };
const TAB_COMPONENTS = { default: AccessibleTab };

type DockLayoutProps = {
  panels: DockPanelSpec[];
  /** localStorage key for the serialized geometry (spike-grade persistence). */
  storageKey: string;
  /** Bumped by the caller to force a default-layout rebuild (Reset action). */
  resetNonce: number;
  /** Fired when the user closes a panel, so the owner drops it from state
   *  (otherwise reconciliation would immediately re-add it). */
  onClosePanel?: (id: string) => void;
};

// Imperative handle for named-layout save/restore (the geometry — splits/tabs —
// lives in dockview, not React state, so the owner captures/restores it here).
export type DockLayoutHandle = {
  capture: () => SerializedDockview | null;
  restore: (layout: SerializedDockview) => void;
};

export const DockLayout = forwardRef<DockLayoutHandle, DockLayoutProps>(function DockLayout(
  { panels, storageKey, resetNonce, onClosePanel },
  ref,
) {
  const contents = new Map(panels.map((panel) => [panel.id, panel.render]));
  const apiRef = useRef<DockviewApi | null>(null);
  const panelsRef = useRef(panels);
  panelsRef.current = panels;
  const onCloseRef = useRef(onClosePanel);
  onCloseRef.current = onClosePanel;
  // True while WE rebuild (reset / restore) so programmatic removals are not
  // mistaken for user closes.
  const rebuildingRef = useRef(false);
  const containerRef = useRef<HTMLDivElement | null>(null);

  // Keyboard model — operate the panel set without a mouse (a hard requirement
  // before the cockpit can become the default shell, ADR 0053). Tabs and group
  // actions are already focusable native buttons; these Alt shortcuts add the
  // cross-panel moves that tabbing alone is clumsy for. Listening on the
  // container catches keydown bubbling from whichever pane/tab has focus.
  //   Alt+Left / Alt+Right  move focus to the previous / next panel
  //   Alt+W                 close the active panel
  //   Alt+M                 toggle maximize of the active group
  //   Alt+P                 pop the active group out to a window (floats if the
  //                         OS window is blocked — popOutOrFloat / decision 2A)
  // W/M/P are suppressed while typing in a field so they never eat input or act
  // mid-edit; the navigation arrows stay live everywhere.
  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    function isEditableTarget(target: EventTarget | null): boolean {
      const el = target as HTMLElement | null;
      if (!el) return false;
      const tag = el.tagName;
      return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
    }
    function onKeyDown(event: KeyboardEvent) {
      const api = apiRef.current;
      if (!api || !event.altKey || event.ctrlKey || event.metaKey) return;
      switch (event.key) {
        case "ArrowRight":
          event.preventDefault();
          api.moveToNext({ includePanel: true });
          api.activePanel?.focus();
          break;
        case "ArrowLeft":
          event.preventDefault();
          api.moveToPrevious({ includePanel: true });
          api.activePanel?.focus();
          break;
        case "w":
        case "W":
          if (isEditableTarget(event.target)) return;
          event.preventDefault();
          api.activePanel?.api.close();
          break;
        case "m":
        case "M":
          if (isEditableTarget(event.target)) return;
          event.preventDefault();
          if (api.hasMaximizedGroup()) api.exitMaximizedGroup();
          else if (api.activePanel) api.maximizeGroup(api.activePanel);
          break;
        case "p":
        case "P":
          if (isEditableTarget(event.target)) return;
          event.preventDefault();
          if (api.activeGroup) void popOutOrFloat(api, api.activeGroup);
          break;
      }
    }
    node.addEventListener("keydown", onKeyDown);
    return () => node.removeEventListener("keydown", onKeyDown);
  }, []);

  function addPanelDefault(api: DockviewApi, spec: DockPanelSpec, referenceId?: string) {
    // Adding a duplicate id throws in dockview. After a clear, floating/pop-out
    // groups can survive, so guard before adding to avoid blanking the dock.
    if (api.panels.some((panel) => panel.id === spec.id)) return;
    api.addPanel({
      id: spec.id,
      component: "default",
      tabComponent: "default",
      title: spec.title,
      position: referenceId ? { referencePanel: referenceId, direction: "within" } : undefined,
    });
  }

  function addAt(api: DockviewApi, spec: DockPanelSpec, referencePanel: string, direction: "right" | "below") {
    if (api.panels.some((panel) => panel.id === spec.id)) return;
    api.addPanel({
      id: spec.id,
      component: "default",
      tabComponent: "default",
      title: spec.title,
      position: { referencePanel, direction },
    });
  }

  function buildDefault(api: DockviewApi) {
    api.clear();
    const specs = panelsRef.current;
    // Dense 2×2 research-cockpit grid: top-left, top-right, bottom-left,
    // bottom-right — several views (incl. cross-company) visible at once.
    specs.forEach((spec, index) => {
      if (index === 0) addPanelDefault(api, spec);
      else if (index === 1) addAt(api, spec, specs[0].id, "right");
      else if (index === 2) addAt(api, spec, specs[0].id, "below");
      else if (index === 3) addAt(api, spec, specs[1].id, "below");
      else addPanelDefault(api, spec, specs[index - 1].id); // extras tab in
    });
  }

  const onReady = (event: DockviewReadyEvent) => {
    apiRef.current = event.api;
    rebuildingRef.current = true;
    const saved = readLayout(storageKey);
    const specIds = new Set(panelsRef.current.map((p) => p.id));
    const savedUsable =
      saved &&
      Array.isArray(saved.panelIds) &&
      saved.panelIds.length > 0 &&
      saved.panelIds.every((id: string) => specIds.has(id));
    if (savedUsable) {
      try {
        event.api.fromJSON(saved.layout);
      } catch {
        buildDefault(event.api);
      }
    } else {
      buildDefault(event.api);
    }
    rebuildingRef.current = false;
    // Tell the owner about user closes so it drops the panel from state.
    event.api.onDidRemovePanel((panel) => {
      if (!rebuildingRef.current) onCloseRef.current?.(panel.id);
    });
    // Persist geometry whenever the user drags/splits/closes.
    event.api.onDidLayoutChange(() => {
      const api = apiRef.current;
      if (!api) return;
      writeLayout(storageKey, {
        panelIds: api.panels.map((p) => p.id),
        layout: api.toJSON(),
      });
    });
  };

  // Rebuild to the default layout when the caller resets.
  const firstResetRef = useRef(resetNonce);
  useEffect(() => {
    if (resetNonce === firstResetRef.current) return;
    firstResetRef.current = resetNonce;
    const api = apiRef.current;
    if (api) {
      clearLayout(storageKey);
      rebuildingRef.current = true;
      try {
        buildDefault(api);
      } catch {
        // A rebuild failure must never blank the cockpit: clear everything and
        // try once more from a clean slate.
        try {
          api.clear();
          buildDefault(api);
        } catch {
          /* swallow — the ErrorBoundary recovery path is the last resort */
        }
      } finally {
        rebuildingRef.current = false;
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- buildDefault reads refs
  }, [resetNonce, storageKey]);

  // Reconcile dockview with the spec list — ADD-ONLY. Removals are driven by the
  // owner (via onClosePanel) so a user close is never re-added.
  useEffect(() => {
    const api = apiRef.current;
    if (!api) return;
    const liveIds = new Set(api.panels.map((p) => p.id));
    let prevId = api.panels[api.panels.length - 1]?.id;
    for (const spec of panels) {
      if (!liveIds.has(spec.id)) {
        addPanelDefault(api, spec, prevId);
        prevId = spec.id;
      } else {
        // Keep the tab title current when a company-scoped panel's selection
        // changes (e.g. "Claims · GPW:CDR" → "Claims · GPW:DNP"). The panel is
        // created once, so its title must be updated in place, not via re-add.
        const panel = api.getPanel(spec.id);
        if (panel && panel.title !== spec.title) {
          panel.setTitle(spec.title);
        }
      }
    }
  }, [panels]);

  // Named-layout save/restore: the geometry lives in dockview, not React state.
  useImperativeHandle(ref, () => ({
    capture: () => apiRef.current?.toJSON() ?? null,
    restore: (layout) => {
      const api = apiRef.current;
      if (!api) return;
      rebuildingRef.current = true;
      try {
        api.fromJSON(layout);
      } catch {
        /* incompatible snapshot — leave the current layout intact */
      } finally {
        rebuildingRef.current = false;
      }
    },
  }));

  return (
    <PanelContentContext.Provider value={contents}>
      <div ref={containerRef} className="cockpit-dock dockview-theme-brawler">
        <DockviewReact
          components={DOCK_COMPONENTS}
          tabComponents={TAB_COMPONENTS}
          defaultTabComponent={AccessibleTab}
          rightHeaderActionsComponent={GroupActions}
          onReady={onReady}
        />
      </div>
    </PanelContentContext.Provider>
  );
});

type StoredLayout = { panelIds: string[]; layout: SerializedDockview };

function readLayout(key: string): StoredLayout | null {
  try {
    const raw = window.localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as StoredLayout) : null;
  } catch {
    return null;
  }
}

function writeLayout(key: string, value: StoredLayout) {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage unavailable — ephemeral fallback, fine for the spike */
  }
}

function clearLayout(key: string) {
  try {
    window.localStorage.removeItem(key);
  } catch {
    /* ignore */
  }
}
