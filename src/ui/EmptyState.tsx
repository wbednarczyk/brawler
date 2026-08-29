import type { ReactNode } from "react";

// F4a S1 (ADR 0104 dec. 4 — "an empty state is an invitation"): `kind`
// discriminates the three-beats invitation shape (what this is → where it
// comes from → one action) and the "quiet" shape (a good/expected empty, no
// action — e.g. Alerts' "nothing fired") from the legacy children-only form.
// `kind` omitted keeps every existing call site compiling unchanged while F4
// waves migrate their screens one at a time; `data-empty-kind` lets the
// per-screen contract test (`collectEmptyStates`) assert the migration
// happened. TS enforces "exactly one action" for the invitation shape by
// making it a required singular prop, not an array.
export type EmptyStateInvitationProps = {
  kind: "invitation";
  title: ReactNode;
  source: ReactNode;
  action: ReactNode;
  className?: string;
};

export type EmptyStateQuietProps = {
  kind: "quiet";
  reason: string;
  className?: string;
};

export type EmptyStateLegacyProps = {
  kind?: undefined;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  wrapText?: boolean;
};

export type EmptyStateProps = EmptyStateInvitationProps | EmptyStateQuietProps | EmptyStateLegacyProps;

export function EmptyState(props: EmptyStateProps) {
  const className = ["empty-state", props.className].filter(Boolean).join(" ");

  if (props.kind === "invitation") {
    return (
      <div className={className} data-empty-kind="invitation">
        <span className="empty-state-invitation-title">{props.title}</span>
        <span className="empty-state-invitation-source">{props.source}</span>
        {props.action}
      </div>
    );
  }

  if (props.kind === "quiet") {
    return (
      <div className={className} data-empty-kind="quiet">
        <span>{props.reason}</span>
      </div>
    );
  }

  const { actions, children, wrapText = true } = props;
  return (
    <div className={className} data-empty-kind="legacy">
      {wrapText ? <span>{children}</span> : children}
      {actions}
    </div>
  );
}
