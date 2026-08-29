import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";

import type { AlertRule, AlertRuleUpdate } from "../../api/attention";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { pluralNoun } from "../../shared/locale/plural";
import { Button, EmptyState, SectionHeader, StatusChip, TextField } from "../../ui";
import { RULE_FORMS, parsePrice, ruleDescription, ruleTitle, scopeName, triggerIcon } from "./alertLabels";

export type AlertRulesSectionProps = {
  rules: AlertRule[];
  companyName: Map<string, string>;
  watchlistName: Map<string, string>;
  onToggle: (rule: AlertRule, enabled: boolean) => void;
  onCommitPrice: (input: AlertRuleUpdate) => void;
  onRemove: (rule: AlertRule) => void;
};

/**
 * Alerts screen card 2 — the rule list (extracted from `AlertsScreen.tsx`,
 * F4a S4a). Rendered DOM/accessible names unchanged; data now comes from
 * `useAlertsQuery` (ADR 0106) instead of a local effect.
 */
export function AlertRulesSection({
  rules,
  companyName,
  watchlistName,
  onToggle,
  onCommitPrice,
  onRemove,
}: AlertRulesSectionProps) {
  const { text, locale } = useLocale();

  return (
    <div className="alerts-card">
      <SectionHeader
        className="alerts-card-header"
        level="h2"
        title={text("Your alerts")}
        meta={`${rules.length} ${pluralNoun(locale, rules.length, RULE_FORMS)}`}
      />
      {rules.length === 0 ? (
        <EmptyState>{text("No alerts yet — pick what to watch for above.")}</EmptyState>
      ) : (
        <ul className="alerts-list" aria-label={text("Alert rules")}>
          {rules.map((rule) => (
            <AlertRuleRow
              key={rule.id}
              rule={rule}
              companyName={companyName}
              watchlistName={watchlistName}
              onToggle={(enabled) => onToggle(rule, enabled)}
              onCommitPrice={onCommitPrice}
              onRemove={() => onRemove(rule)}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

type AlertRuleRowProps = {
  rule: AlertRule;
  companyName: Map<string, string>;
  watchlistName: Map<string, string>;
  onToggle: (enabled: boolean) => void;
  onCommitPrice: (input: AlertRuleUpdate) => void;
  onRemove: () => void;
};

// One rule row. Keeps its own price-draft state (two-way editing on an
// existing range rule, docs/ui-authoring.md): the input drives local state
// immediately and commits through `update_alert_rule` on blur — decoupled
// from the query's read-only `rules` array (ADR 0106 dec. 4: no cache
// patching, invalidation is refetch-only), so typing is never stomped by a
// mid-edit refetch. `key={rule.id}` on the parent's `.map()` remounts this
// with a fresh draft if the rule identity changes.
function AlertRuleRow({ rule, companyName, watchlistName, onToggle, onCommitPrice, onRemove }: AlertRuleRowProps) {
  const { text } = useLocale();
  const [priceMin, setPriceMin] = useState(rule.priceMin);
  const [priceMax, setPriceMax] = useState(rule.priceMax);

  // Resync from the canonical (query-owned) value on every change — a no-op
  // after a successful commit (typed value === committed value), and the
  // revert path after a REJECTED commit (the parent re-reads on error).
  useEffect(() => {
    setPriceMin(rule.priceMin);
    setPriceMax(rule.priceMax);
  }, [rule.priceMin, rule.priceMax]);

  const description = ruleDescription(rule, text, companyName, watchlistName);
  const Icon = triggerIcon(rule.triggerType, rule.signalCategory);

  const commitPrice = () =>
    onCommitPrice({ id: rule.id, priceMin: priceMin ?? undefined, priceMax: priceMax ?? undefined });

  return (
    <li aria-label={`${text("Alert rule")}: ${description}`} className="alerts-row">
      <span className="alerts-row-icon" aria-hidden="true">
        <Icon size={16} />
      </span>
      <div className="alerts-row-main">
        <div className="alerts-row-title">{ruleTitle(rule, text)}</div>
        <div className="alerts-row-sub">
          <StatusChip>
            {rule.scopeType === "watchlist" ? text("List") : text("Company")}
            {" · "}
            {rule.scopeType === "company" && companyName.get(rule.scopeRef) ? (
              <TickerLabel value={companyName.get(rule.scopeRef)!} />
            ) : (
              scopeName(rule.scopeType, rule.scopeRef, companyName, watchlistName)
            )}
          </StatusChip>
          {rule.triggerType === "price_enters_range" ? (
            <span className="alerts-row-prices">
              <TextField
                aria-label={`${text("Minimum price")} — ${description}`}
                type="number"
                inputMode="decimal"
                value={priceMin ?? ""}
                onChange={(event) => setPriceMin(parsePrice(event.target.value))}
                onBlur={commitPrice}
              />
              <TextField
                aria-label={`${text("Maximum price")} — ${description}`}
                type="number"
                inputMode="decimal"
                value={priceMax ?? ""}
                onChange={(event) => setPriceMax(parsePrice(event.target.value))}
                onBlur={commitPrice}
              />
            </span>
          ) : null}
        </div>
      </div>
      <div className="alerts-row-slots">
        <label className="alerts-enable-control">
          <input
            aria-label={`${text("Enabled")} — ${description}`}
            checked={rule.enabled}
            onChange={(event) => onToggle(event.target.checked)}
            role="switch"
            type="checkbox"
          />
          <span aria-hidden="true" className="alerts-enable-track">
            <span />
          </span>
        </label>
        <Button onClick={onRemove} variant="ghost">
          <Trash2 size={14} />
          {text("Delete")}
        </Button>
      </div>
    </li>
  );
}
