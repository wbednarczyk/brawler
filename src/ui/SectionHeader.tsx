import type { ReactNode } from "react";

export type SectionHeaderProps = {
  actions?: ReactNode;
  className?: string;
  description?: ReactNode;
  /** Heading level for the title. Defaults to h2; use h3/h4 for nested sections to preserve document outline. */
  level?: "h2" | "h3" | "h4";
  meta?: ReactNode;
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
  title,
  titleId,
  variant = "plain",
}: SectionHeaderProps) {
  return (
    <div
      className={[
        "ui-section-header",
        variant === "accent" ? "ui-section-header-accent" : "",
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
