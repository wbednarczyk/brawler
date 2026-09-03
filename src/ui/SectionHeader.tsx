import type { ReactNode, Ref } from "react";

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
   *  so the shared compact-header rule (ADR 0076 Decision 6, K3 double panel
   *  chrome; `src/styles/ui.css`, scoped to `.spolka-tool`) visually hides the
   *  title (kept in the accessible tree — clip-path, not removal) and drops the
   *  subtitle when the workshop tool frame already shows the name. */
  paneLead?: boolean;
  title: ReactNode;
  titleId?: string;
  /** Imperative focus target for the title (F3c S1) — the Spółka workshop
   * tool frame focuses its heading on open via this ref, since a heading is
   * not natively focusable. Pair with `titleTabIndex={-1}` (programmatically
   * focusable, out of the Tab order — a heading is not a real tab stop). */
  titleRef?: Ref<HTMLHeadingElement>;
  titleTabIndex?: -1 | 0;
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
  titleRef,
  titleTabIndex,
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
        <Heading id={titleId} ref={titleRef} tabIndex={titleTabIndex}>
          {title}
        </Heading>
        {description ? <p>{description}</p> : null}
      </div>
      {meta ? <span className="ui-section-header-meta">{meta}</span> : null}
      {actions ? <div className="ui-section-header-actions">{actions}</div> : null}
    </div>
  );
}
