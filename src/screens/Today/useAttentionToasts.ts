import { useEffect } from "react";

import type { AlertRule, AttentionEvent } from "../../api/attention";
import type { Company } from "../../api/types";
import { useToast } from "../../ui";
import { useLocale } from "../../shared/locale";
import { attentionEventTitleText } from "./attentionEventLabels";

// Fired-alert ids already raised as a persistent toast this app session (ADR 0068
// T4). Module scope, not component state: Today unmounts whenever the user leaves
// the section, so a per-render Set would re-toast on every re-entry. Resets only
// on a full app reload — "session" per the T4 contract.
const toastedAttentionEventIds = new Set<string>();

/**
 * Toast wiring for fired attention events (ADR 0068 T4, toast policy v2 —
 * ADR 0087 dec. 3). A toast is a POINTER, never the store: the Today stream is
 * the system of record. Routing is by the event's typed `severity` (computed
 * once in `storage::severity`, never re-inferred here):
 *
 *   - `urgent`  → a PERSISTENT `negative` toast (the only level that persists),
 *     click-through Review + Dismiss synced to the stream row.
 *   - `notable` → a TRANSIENT (auto-dismissing) `caution` toast, announcing the
 *     landing; the row remains the place to act.
 *   - `routine` → NO toast (stream only).
 *
 * Either way an event toasts at most once per app session (the module-scoped
 * dedup Set); dismissing the row dismisses the toast and vice versa (a single
 * seen/dismissed state).
 */
export function useAttentionToasts({
  companies,
  events,
  companyById,
  attentionRulesById,
  onReview,
  onDismiss,
}: {
  companies: Company[];
  /** The same company-grouped, newest-first events the stream renders. */
  events: AttentionEvent[];
  companyById: Map<string, Company>;
  attentionRulesById: Map<string, AlertRule>;
  onReview: (event: AttentionEvent) => void;
  onDismiss: (id: string) => void;
}) {
  const { text } = useLocale();
  const toast = useToast();

  useEffect(() => {
    // The events fetch and the company list are independent round trips. If the
    // former settles first, `companyById` is still empty and the toast would bake
    // the raw company id into its message — never retried (the dedup Set). Wait
    // for at least one company; `events` (built off `companyById`) changes
    // reference once companies resolve, re-running this effect with real tickers.
    if (companies.length === 0) return;
    for (const event of events) {
      if (event.seen || event.dismissed) continue;
      // Routine events are stream-only (ADR 0087 dec. 3) — no toast, and NOT
      // marked toasted (nothing was shown to dedup against).
      if (event.severity === "routine") continue;
      if (toastedAttentionEventIds.has(event.id)) continue;
      toastedAttentionEventIds.add(event.id);
      const rule = event.ruleId ? attentionRulesById.get(event.ruleId) : undefined;
      const ticker = companyById.get(event.companyId)?.qualifiedTicker ?? event.companyId;
      const detail = attentionEventTitleText(event, rule, text);
      if (event.severity === "urgent") {
        // Urgent: a persistent pointer to the top of the stream, synced to the row.
        toast.show({
          message: `${ticker} — ${detail}`,
          tone: "negative",
          persistent: true,
          dismissLabel: text("Dismiss"),
          actionLabel: text("Review"),
          onAction: () => onReview(event),
          onDismiss: () => onDismiss(event.id),
        });
      } else {
        // Notable: a transient announcement (auto-dismisses); the row stays the
        // place to act, so no persistent dismiss/onDismiss sync is needed.
        toast.show({
          message: `${ticker} — ${detail}`,
          tone: "caution",
          actionLabel: text("Review"),
          onAction: () => onReview(event),
        });
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [events, companies.length]);
}
