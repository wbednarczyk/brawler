import type { ReactNode } from "react";

// A self-contained card for the feed detail rail. It bakes in the containment
// contract the rail needs — a flex column that establishes min-width:0 on its
// whole subtree — so no descendant (a long URL, a filename, a wide input row)
// can grow the section's min-content width and blow out the fixed-width pane.
// New panels render inside it instead of re-deriving containment per element.

export type DetailSectionProps = {
  title: ReactNode;
  icon?: ReactNode;
  description?: ReactNode;
  aside?: ReactNode;
  ariaLabel?: string;
  className?: string;
  children?: ReactNode;
};

export function DetailSection({
  title,
  icon,
  description,
  aside,
  ariaLabel,
  className,
  children,
}: DetailSectionProps) {
  return (
    <section
      aria-label={ariaLabel}
      className={["detail-section", className].filter(Boolean).join(" ")}
    >
      <div className="detail-section-header">
        <div className="detail-section-heading">
          <h3>
            {icon}
            {title}
          </h3>
          {description ? <p>{description}</p> : null}
        </div>
        {aside ? <div className="detail-section-aside">{aside}</div> : null}
      </div>
      {children}
    </section>
  );
}
