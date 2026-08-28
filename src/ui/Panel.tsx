import type { ReactNode } from "react";

type PanelProps = {
  ariaLabelledBy?: string;
  children: ReactNode;
  className?: string;
};

type PanelHeaderProps = {
  actions?: ReactNode;
  className?: string;
  description?: ReactNode;
  /** Marks this as the panel's leading title header — tags `.ui-pane-lead-header`
   *  so the shared compact-header rule (ADR 0076 Decision 6, K3 double panel
   *  chrome; `src/styles/ui.css`, scoped to `.spolka-tool`) visually hides the
   *  title (kept in the accessible tree — clip-path, not removal) and drops the
   *  subtitle when the workshop tool frame already shows the name. */
  paneLead?: boolean;
  title: ReactNode;
  titleId?: string;
};

export function Panel({ ariaLabelledBy, children, className }: PanelProps) {
  return (
    <section
      aria-labelledby={ariaLabelledBy}
      className={["ui-panel", className].filter(Boolean).join(" ")}
    >
      {children}
    </section>
  );
}

export function PanelHeader({
  actions,
  className,
  description,
  paneLead = false,
  title,
  titleId,
}: PanelHeaderProps) {
  return (
    <div
      className={["ui-panel-header", paneLead ? "ui-pane-lead-header" : "", className]
        .filter(Boolean)
        .join(" ")}
    >
      <div>
        <h1 id={titleId}>{title}</h1>
        {description ? <p>{description}</p> : null}
      </div>
      {actions ? <div className="ui-panel-header-actions">{actions}</div> : null}
    </div>
  );
}
