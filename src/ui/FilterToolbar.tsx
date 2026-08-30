import { useState, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";

import { useLocale } from "../shared/locale";

export type FilterToolbarProps = {
  ariaLabel: string;
  children: ReactNode;
  className?: string;
  /**
   * Always-visible leading slot (typically a search field) that stays outside
   * the S-tier disclosure collapse (ADR 0076 D6). Optional: toolbars without a
   * search simply fold all of `children` behind the disclosure.
   */
  search?: ReactNode;
};

// Panel density contract (ADR 0076 D6): at the S width tier (@container pane
// max-width 419px, see ui.css) the toolbar collapses to a single row — the
// `search` slot stays visible and the remaining controls fold behind a
// "Filtry" disclosure. The disclosure button + rotating chevron are rendered
// here so every consumer inherits the behavior; the tier switch itself is
// CSS-only (container query), so above S the disclosure is hidden and the
// controls always show regardless of the toggle state.
export function FilterToolbar({ ariaLabel, children, className, search }: FilterToolbarProps) {
  const { text } = useLocale();
  const [expanded, setExpanded] = useState(false);

  return (
    // data-hscroll: at extreme narrow panes a control strip scrolls rather than
    // clipping its controls (responsive.css lets the toolbar scroll instead of
    // starving the list) — a DELIBERATE horizontal scroller, exempt from the
    // panel-overflow layout gate (ADR 0045).
    <div
      className={["filter-toolbar", className].filter(Boolean).join(" ")}
      data-hscroll
      aria-label={ariaLabel}
    >
      {search != null ? <div className="filter-toolbar-search">{search}</div> : null}
      <button
        type="button"
        className="filter-toolbar-disclosure"
        data-action-kind="control"
        aria-expanded={expanded}
        onClick={() => setExpanded((open) => !open)}
      >
        <span aria-hidden="true" className="filter-toolbar-chevron">
          <ChevronRight size={15} />
        </span>
        {text("Filters")}
      </button>
      <div className="filter-toolbar-controls" data-collapsed={!expanded}>
        {children}
      </div>
    </div>
  );
}
