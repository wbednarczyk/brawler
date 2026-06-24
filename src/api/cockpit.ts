import { callCommand } from "./tauri";
import type { CockpitLayout } from "./generated/CockpitLayout";
import type { SaveCockpitLayoutInput } from "./generated/SaveCockpitLayoutInput";
import type { RenameCockpitLayoutInput } from "./generated/RenameCockpitLayoutInput";

// Types GENERATED from src-tauri/src/storage/cockpit_layouts.rs via ts-rs (ADR 0048).
export type { CockpitLayout } from "./generated/CockpitLayout";
export type { SaveCockpitLayoutInput } from "./generated/SaveCockpitLayoutInput";
export type { RenameCockpitLayoutInput } from "./generated/RenameCockpitLayoutInput";

// Research cockpit saved layouts (ADR 0053, decision 3A): persisted in SQLite,
// not localStorage. `panelsJson` is the source of truth for what is open;
// `layoutJson` is the opaque dockview geometry (may be null).

export function listCockpitLayouts() {
  return callCommand<CockpitLayout[]>("list_cockpit_layouts");
}

export function saveCockpitLayout(input: SaveCockpitLayoutInput) {
  return callCommand<CockpitLayout>("save_cockpit_layout", { input });
}

export function renameCockpitLayout(input: RenameCockpitLayoutInput) {
  return callCommand<CockpitLayout>("rename_cockpit_layout", { input });
}

export function deleteCockpitLayout(layoutId: string) {
  return callCommand<void>("delete_cockpit_layout", { layoutId });
}
