import { callCommand } from "./tauri";
import type { CockpitLayout } from "./generated/CockpitLayout";
import type { SaveCockpitLayoutInput } from "./generated/SaveCockpitLayoutInput";

// Types GENERATED from src-tauri/src/storage/cockpit_layouts.rs via ts-rs (ADR 0048).
export type { CockpitLayout } from "./generated/CockpitLayout";
export type { SaveCockpitLayoutInput } from "./generated/SaveCockpitLayoutInput";

// Research cockpit saved layouts (ADR 0053, decision 3A): persisted in SQLite,
// not localStorage. `panelsJson` is the source of truth for what is open;
// `layoutJson` is the opaque dockview geometry (may be null).

export function listCockpitLayouts() {
  return callCommand<CockpitLayout[]>("list_cockpit_layouts");
}

export function saveCockpitLayout(input: SaveCockpitLayoutInput) {
  return callCommand<CockpitLayout>("save_cockpit_layout", { input });
}

export function deleteCockpitLayout(layoutId: string) {
  return callCommand<void>("delete_cockpit_layout", { layoutId });
}

// Issue #89: in-place rename (id/ordinal preserved). Error codes:
// cockpit_layout_not_found | invalid_cockpit_layout_name |
// duplicate_cockpit_layout_name (save upserts BY NAME, so a duplicate would
// silently fuse two layouts on the next save — the backend rejects it).
export function renameCockpitLayout(layoutId: string, name: string) {
  return callCommand<CockpitLayout>("rename_cockpit_layout", {
    input: { layoutId, name },
  });
}
