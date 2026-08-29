import { forwardRef } from "react";
import { Button, type ButtonProps } from "./Button";
import type { Verb } from "../shared/verbs";

// The verb-dictionary action wrapper (ADR 0104 dec. 3 amendment, F4a S1):
// every screen action is either a dictionary VERB (a state-changing command —
// create/rename/pause/resume/add/remove/…) or a `kind` of "destination"
// (navigates elsewhere, noun-labeled per dec. 3) / "control" (a filter,
// search, or toggle — not a command at all). The two are mutually exclusive so
// a caller can't tag an action with both. `ActionButton` carries no styling of
// its own — it renders `Button` unchanged plus the classification metadata
// that the per-screen action-inventory contract test reads
// (`src/test/uxContracts.tsx` `collectActionInventory`).
export type ActionButtonProps = ButtonProps &
  ({ verb: Verb; kind?: never } | { kind: "destination" | "control"; verb?: never });

export const ActionButton = forwardRef<HTMLButtonElement, ActionButtonProps>(function ActionButton(
  { verb, kind, ...buttonProps },
  ref,
) {
  const actionKind = verb ?? kind;
  return (
    <Button
      ref={ref}
      data-action-kind={actionKind}
      data-action-verb={verb}
      {...buttonProps}
    />
  );
});
