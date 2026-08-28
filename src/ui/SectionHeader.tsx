import type { ReactNode } from "react";

export type SectionHeaderProps = {
  actions?: ReactNode;
  className?: string;
  description?: ReactNode;
  /** Small mono/uppercase label above the title (ADR 0104 dec. 2 — the
   *  established eyebrow recipe already used by `DeltaHeader`/`FocusOverlay`),
   *  e.g. a unit qualifier or a context tag. Omit when nothing non-redundant
   *  with the title exists — an eyebrow that repeats the title is noise. */
  eyebrow?: ReactNode;
  /** Heading level for the title. Defaults to h2; use h3/h4 for nested sections to preserve document outline. */
  level?: "h2" | "h3" | "h4";
  meta?: ReactNode;
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
  variant?: "plain" | "accent";
};

export function SectionHeader({
  actions,
  className,
  description,
  eyebrow,
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
        {eyebrow ? <span className="ui-section-eyebrow">{eyebrow}</span> : null}
        <Heading id={titleId}>{title}</Heading>
        {description ? <p>{description}</p> : null}
      </div>
      {meta ? <span className="ui-section-header-meta">{meta}</span> : null}
      {actions ? <div className="ui-section-header-actions">{actions}</div> : null}
    </div>
  );
}
