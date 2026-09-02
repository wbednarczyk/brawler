import { useState } from "react";
import { AlertTriangle } from "lucide-react";

import { ActionButton, ClearButton } from "../../ui";
import { useLocale } from "../../shared/locale";

/**
 * A config-state condition (ADR 0087 dec. 5): a property of the app itself — a
 * source unreachable, a provider misconfigured — stated ONCE as a banner above
 * the stream, never repeated per row. Copy is pre-translated by the producer
 * (typed, product-facing), never a raw error string. The one wired condition's
 * action opens Sources (F4c dec. 6: it was labeled "Diagnostics" but always
 * opened Sources — every banner action here navigates elsewhere, so the
 * button it renders is classified `kind="destination"` in this shared spot.
 */
export type ConfigCondition = {
  id: string;
  /** The already-translated headline (e.g. "Source X unreachable for 2 days"). */
  message: string;
  /** Optional trailing clause. */
  detail?: string;
  /** Optional action (e.g. open Diagnostics). */
  action?: { label: string; onClick: () => void };
};

/**
 * The dismissible config-state banner bar(s) above the Today stream. Renders one
 * bar per active condition; dismissing hides it for the session. Ships with no
 * live condition wired yet — TODO(D3a): source-health / provider-config
 * conditions arrive on the stream/attention payloads and are mapped to typed
 * `ConfigCondition`s here (ADR 0087 dec. 5 + consequences).
 */
export function ConfigBanner({ conditions }: { conditions: ConfigCondition[] }) {
  const { text } = useLocale();
  const [dismissed, setDismissed] = useState<Set<string>>(() => new Set());
  const active = conditions.filter((condition) => !dismissed.has(condition.id));
  if (active.length === 0) return null;
  return (
    <div className="today-config-banners" role="status">
      {active.map((condition) => (
        <div key={condition.id} className="today-config-banner">
          <AlertTriangle aria-hidden="true" size={15} className="today-config-banner-icon" />
          <span className="today-config-banner-message">{condition.message}</span>
          {condition.detail ? (
            <span className="today-config-banner-detail">{condition.detail}</span>
          ) : null}
          {condition.action ? (
            <ActionButton
              kind="destination"
              className="today-config-banner-action"
              onClick={condition.action.onClick}
              type="button"
              variant="ghost"
            >
              {condition.action.label}
            </ActionButton>
          ) : null}
          <ClearButton
            label={text("Dismiss")}
            onClick={() => setDismissed((prior) => new Set(prior).add(condition.id))}
          />
        </div>
      ))}
    </div>
  );
}
