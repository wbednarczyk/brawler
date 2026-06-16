import type { ReactNode } from "react";

export type ListRowProps = {
  // Leading glyph (e.g. a file icon).
  icon?: ReactNode;
  // The primary label; truncates with an ellipsis so long unbroken names never
  // force horizontal scroll.
  title: ReactNode;
  // Full title for the tooltip when the visible label is truncated.
  titleAttr?: string;
  // When set, the title becomes an external link.
  href?: string;
  // Rendered immediately after the title and never truncated (e.g. an external-link glyph).
  adornment?: ReactNode;
  // Trailing muted metadata (e.g. attribution).
  meta?: ReactNode;
  // Trailing affordance pinned to the right (e.g. a status badge or action button).
  trailing?: ReactNode;
  className?: string;
};

// A non-interactive media/list row: leading icon + truncating title (optionally a
// link) + trailing meta and a status/action. Self-contained: it owns the
// `min-width: 0` + ellipsis contract so callers never re-derive truncation.
export function ListRow({
  icon,
  title,
  titleAttr,
  href,
  adornment,
  meta,
  trailing,
  className,
}: ListRowProps) {
  const titleNode = <span className="ui-list-row-title">{title}</span>;
  const main = href ? (
    <a className="ui-list-row-link" href={href} rel="noreferrer" target="_blank" title={titleAttr}>
      {titleNode}
      {adornment}
    </a>
  ) : (
    <span className="ui-list-row-main" title={titleAttr}>
      {titleNode}
      {adornment}
    </span>
  );

  return (
    <li className={["ui-list-row", className].filter(Boolean).join(" ")}>
      {icon ? <span className="ui-list-row-icon">{icon}</span> : null}
      {main}
      {meta ? <span className="ui-list-row-meta">{meta}</span> : null}
      {trailing}
    </li>
  );
}
