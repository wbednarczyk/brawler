import type { ReactNode } from "react";

export type SectionHeaderProps = {
  actions?: ReactNode;
  className?: string;
  description?: ReactNode;
  meta?: ReactNode;
  title: ReactNode;
  titleId?: string;
  variant?: "plain" | "accent";
};

export function SectionHeader({
  actions,
  className,
  description,
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
        <h2 id={titleId}>{title}</h2>
        {description ? <p>{description}</p> : null}
      </div>
      {meta ? <span className="ui-section-header-meta">{meta}</span> : null}
      {actions ? <div className="ui-section-header-actions">{actions}</div> : null}
    </div>
  );
}
