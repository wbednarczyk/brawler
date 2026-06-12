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

export function PanelHeader({ actions, className, description, title, titleId }: PanelHeaderProps) {
  return (
    <div className={["ui-panel-header", className].filter(Boolean).join(" ")}>
      <div>
        <h1 id={titleId}>{title}</h1>
        {description ? <p>{description}</p> : null}
      </div>
      {actions ? <div className="ui-panel-header-actions">{actions}</div> : null}
    </div>
  );
}
