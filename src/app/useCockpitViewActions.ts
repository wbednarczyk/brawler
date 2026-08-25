import { deleteCockpitLayout, saveCockpitLayout, type CockpitLayout } from "../api/cockpit";
import type { Section } from "./navigation";

type UndoableDelete = (input: {
  perform: () => Promise<unknown>;
  restore: () => Promise<unknown>;
  message: string;
  undoLabel: string;
  onPerformed: () => void;
  onRestored: () => void;
}) => void;

/**
 * The saved-cockpit-view lifecycle pair (open + reversible delete, ADR 0076
 * D5), extracted from `AppStateRoot` (F2 — the root stays under its file-size
 * pin by moving cohesive blocks out, not by raising the pin).
 */
export function makeCockpitViewActions(deps: {
  cockpitLayouts: CockpitLayout[];
  activeCockpitLayoutId: string | null;
  setActiveCockpitLayoutId: (id: string | null) => void;
  setCockpitInitialCompanyId: (id: string | null) => void;
  setCockpitInitialPresetId: (id: string | null) => void;
  setActiveSection: (section: Section) => void;
  refreshCockpitLayouts: () => void;
  runUndoableDelete: UndoableDelete;
  text: (key: string) => string;
}) {
  // Opening a saved view clears any company scope so it renders as the pure view.
  function openCockpitView(layoutId: string) {
    deps.setCockpitInitialCompanyId(null);
    deps.setCockpitInitialPresetId(null);
    deps.setActiveCockpitLayoutId(layoutId);
    deps.setActiveSection("Cockpit");
  }

  // Reversible destroy (ADR 0076 D5): a saved view re-creates faithfully via
  // save_cockpit_layout (name + panelsJson + layoutJson), so it deletes
  // immediately with an undo toast rather than a blocking dialog.
  function deleteCockpitView(layoutId: string) {
    const layout = deps.cockpitLayouts.find((entry) => entry.id === layoutId);
    if (!layout) return;
    deps.runUndoableDelete({
      perform: () => deleteCockpitLayout(layoutId),
      restore: () =>
        saveCockpitLayout({
          name: layout.name,
          panelsJson: layout.panelsJson,
          layoutJson: layout.layoutJson,
          dockviewVersion: layout.dockviewVersion,
        }),
      message: deps.text("View deleted"),
      undoLabel: deps.text("Undo"),
      onPerformed: () => {
        if (deps.activeCockpitLayoutId === layoutId) deps.setActiveCockpitLayoutId(null);
        deps.refreshCockpitLayouts();
      },
      onRestored: () => {
        deps.refreshCockpitLayouts();
      },
    });
  }

  return { openCockpitView, deleteCockpitView };
}
