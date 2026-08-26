import { useState } from "react";
import { Columns3, FlaskConical, Pencil } from "lucide-react";
import type { Company } from "../api/types";
import type { CockpitLayout } from "../api/generated/CockpitLayout";
import { TextField } from "../ui";
import type { Section } from "./navigation";

export type LegacyDashboardRow = { id: string; companyId: string; ticker: string | null };

const DASHBOARD_PREFIX = "dashboard:";

/** "Dawny dashboard · TICKER" Widoki rows (F3a S3, consent 2): every
 * `cockpit_layouts` row using the reserved `dashboard:` name prefix
 * (CockpitScreen's own `DASHBOARD_PREFIX`) — the ticker resolves via the live
 * company list, falling back to the raw id if the company no longer exists. */
export function buildLegacyDashboardRows(
  cockpitLayouts: CockpitLayout[],
  companiesById: Record<string, Company>,
): LegacyDashboardRow[] {
  return cockpitLayouts
    .filter((layout) => layout.name.startsWith(DASHBOARD_PREFIX))
    .map((layout) => {
      const companyId = layout.name.slice(DASHBOARD_PREFIX.length);
      return { id: layout.id, companyId, ticker: companiesById[companyId]?.ticker ?? null };
    });
}

type SidebarViewsGroupProps = {
  activeSection: Section;
  groupLabel: string;
  cockpitViews: { id: string; name: string }[];
  activeCockpitViewId: string | null;
  cockpitInitialCompanyId: string | null;
  legacyDashboardLayouts: LegacyDashboardRow[];
  onOpenCockpitView: (viewId: string) => void;
  /** Wired through by the host (AppStateRoot) but no longer rendered here (F3a
   * S3/R1 finding 4, ADR 0107 decision 5): deleting a saved view is a
   * structure mutation the freeze removes — rename (metadata) stays. Kept in
   * the prop type so the host's existing wiring needs no change. */
  onDeleteCockpitView: (viewId: string) => void;
  onRenameCockpitView: (viewId: string, name: string) => void;
  onOpenLegacyDashboard: (companyId: string) => void;
  text: (s: string) => string;
};

// "Widoki" (F3a S3, ADR 0107 amendment): the frozen cockpit's named views
// plus every legacy per-company dashboard (`dashboard:` layout rows), read-only
// structure — no "+ New view" here (consent 1). Extracted from AppShell
// (file-size ratchet) since it carries its own inline-rename state.
export function SidebarViewsGroup({
  activeSection,
  groupLabel,
  cockpitViews,
  activeCockpitViewId,
  cockpitInitialCompanyId,
  legacyDashboardLayouts,
  onOpenCockpitView,
  onRenameCockpitView,
  onOpenLegacyDashboard,
  text,
}: SidebarViewsGroupProps) {
  const [renamingViewId, setRenamingViewId] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");

  if (cockpitViews.length === 0 && legacyDashboardLayouts.length === 0) {
    return null;
  }

  return (
    <div className="nav-group">
      <div className="nav-group-label">{groupLabel}</div>
      <div className="nav-list">
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
            </div>
          );
        })}
        {legacyDashboardLayouts.map((row) => {
          const isActive = activeSection === "Cockpit" && cockpitInitialCompanyId === row.companyId;
          const label = `${text("Legacy dashboard")} · ${row.ticker ?? row.companyId}`;
          return (
            <button
              className={isActive ? "nav-item nav-item-active" : "nav-item"}
              aria-current={isActive ? "page" : undefined}
              key={row.id}
              onClick={() => onOpenLegacyDashboard(row.companyId)}
              type="button"
              title={label}
            >
              <FlaskConical size={18} aria-hidden="true" />
              <span>{label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
