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
   *  for a shared compact-header rule (ADR 0076 Decision 6, K3 double panel
   *  chrome) that visually hid the title (kept in the accessible tree —
   *  clip-path, not removal) and dropped the subtitle when a dock tab already
   *  showed the name. Currently inert everywhere: the rule was scoped to the
   *  retired `.cockpit-pane` host (ADR 0108) and has no replacement yet —
   *  the Spółka workshop tool header (`.spolka-tool-header`) duplicates this
   *  title today. Kept on the prop for the existing call sites; re-scope or
   *  drop as a follow-up. */
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
