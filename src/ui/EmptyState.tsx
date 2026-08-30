import { Children, useEffect, useRef, type ReactElement, type ReactNode } from "react";

// F4a S1 (ADR 0104 dec. 4 — "an empty state is an invitation"): `kind`
// discriminates the three-beats invitation shape (what this is → where it
// comes from → one action) and the "quiet" shape (a good/expected empty, no
// action — e.g. Alerts' "nothing fired") from the legacy children-only form.
// `kind` omitted keeps every existing call site compiling unchanged while F4
// waves migrate their screens one at a time; `data-empty-kind` lets the
// per-screen contract test (`collectEmptyStates`) assert the migration
// happened. TS enforces "exactly one action" for the invitation shape by
// typing `action` as a single `ReactElement` (not `ReactNode`, which would
// also accept an array/fragment of several controls); a caller that defeats
// TS (an `any`-cast fragment of two buttons) is still caught at runtime — see
// `InvitationAction` below (, sol ).
export type EmptyStateInvitationProps = {
  kind: "invitation";
  title: ReactNode;
  source: ReactNode;
  action: ReactElement;
  className?: string;
};

export type EmptyStateQuietProps = {
  kind: "quiet";
  reason: string;
  /**
   * Optional low-emphasis recovery control (F4b S3, Events "no later match
   * under active filters" — a `Wyczyść filtry` control, never a primary
   * call-to-action; that shape stays "invitation"). Omitted for the plain
   * "nothing here, nothing to do" reading (Alerts' "nothing fired").
   */
  action?: ReactElement;
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

// Focusable-control selector shared with the DOM check below — same set the
// browser-level icon-action/focus-order helpers treat as "an action"
// (tests/browser/helpers/interactionContracts.ts).
const FOCUSABLE_SELECTOR = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

// The invitation's action slot renders through this wrapper so a runtime DOM
// check can enforce "exactly one action" even when a caller defeats the TS
// `ReactElement` typing (e.g. a `fragment as any` holding two buttons) —
// `Children.only` alone would NOT catch that case: a `<>{a}{b}</>` fragment is
// itself one valid React element, so it satisfies `Children.only` while still
// rendering two focusable controls. A dev-time `console.error` (not a throw)
// so a misconfigured empty state degrades to a loud warning rather than
// crashing the screen it appears on.
function InvitationAction({ action }: { action: ReactElement }) {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const focusable = node.querySelectorAll(FOCUSABLE_SELECTOR).length;
    if (focusable !== 1) {
      // Dev-time contract diagnostic (ADR 0104 dec. 4 / ).
      console.error(
        `EmptyState kind="invitation": action slot must render exactly one focusable control, found ${focusable}.`,
      );
    }
  });
  return (
    <span className="empty-state-invitation-action" ref={ref}>
      {action}
    </span>
  );
}

export function EmptyState(props: EmptyStateProps) {
  const className = ["empty-state", props.className].filter(Boolean).join(" ");

  if (props.kind === "invitation") {
    // Defense-in-depth against a non-element value slipping past TS (a
    // `null`/array `action` from an `any`-cast caller) — throws immediately
    // rather than rendering nothing.
    Children.only(props.action);
    return (
      <div className={className} data-empty-kind="invitation">
        <span className="empty-state-invitation-title">{props.title}</span>
        <span className="empty-state-invitation-source">{props.source}</span>
        <InvitationAction action={props.action} />
      </div>
    );
  }

  if (props.kind === "quiet") {
    return (
      <div className={className} data-empty-kind="quiet">
        <span>{props.reason}</span>
        {props.action ? <span className="empty-state-quiet-action">{props.action}</span> : null}
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
