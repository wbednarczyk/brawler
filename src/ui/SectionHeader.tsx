import type { ReactNode } from "react";

export type SectionHeaderProps = {
  actions?: ReactNode;
  className?: string;
  description?: ReactNode;
  /** Heading level for the title. Defaults to h2; use h3/h4 for nested sections to preserve document outline. */
  level?: "h2" | "h3" | "h4";
  meta?: ReactNode;
  /** Marks this as the panel's leading title header. Inside a `.cockpit-pane`
   *  the shared compact-header rule (ADR 0076 Decision 6, K3 double panel chrome)
   *  visually hides the title (kept in the accessible tree — clip-path, not
   *  removal — so `aria-labelledby`/heading queries still resolve) and drops the
   *  subtitle, because the dock tab already shows the name. Inert full-screen. */
  paneLead?: boolean;
  title: ReactNode;
  titleId?: string;
  variant?: "plain" | "accent";
};

export function SectionHeader({
  actions,
  className,
  description,
  level: Heading = "h2",
  meta,
  paneLead = false,
  title,
  titleId,
  variant = "plain",
}: SectionHeaderProps) {
  return (
    <div
      className={[
        "ui-section-header",
        variant === "accent" ? "ui-section-header-accent" : "",
        paneLead ? "ui-pane-lead-header" : "",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="ui-section-title">
        <Heading id={titleId}>{title}</Heading>
        {description ? <p>{description}</p> : null}
      </div>
      {meta ? <span className="ui-section-header-meta">{meta}</span> : null}
      {actions ? <div className="ui-section-header-actions">{actions}</div> : null}
    </div>
  );
}
