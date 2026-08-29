import { useEffect, useState } from "react";

import type { AlertRule, AlertRuleUpdate } from "../../api/attention";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { pluralNoun } from "../../shared/locale/plural";
import { ActionButton, EmptyState, ErrorText, Figure, SectionHeader, StatusChip, TextField } from "../../ui";
import { RULE_FORMS, parsePrice, ruleDescription, ruleTitle, scopeName, triggerIcon } from "./alertLabels";

export type AlertRulesSectionProps = {
  rules: AlertRule[];
  companyName: Map<string, string>;
  watchlistName: Map<string, string>;
  /** `sectionErrors.rules === "unavailable"` (F4a S4b Partial state): the
   * rules read failed while the rest of the screen may still be fine — an
   * empty `rules` array here must never be read as "no rules exist". */
  rulesError: boolean;
  onRetry: () => void;
  onToggle: (rule: AlertRule, enabled: boolean) => void;
  onCommitPrice: (input: AlertRuleUpdate) => void;
  onRemove: (rule: AlertRule) => void;
  /** Invitation empty state's action (F4a S4b, contract § Alerts action
   * inventory): opens/focuses the composer rather than duplicating its
   * primary marker — the composer's own `Add alert` stays the ONE primary. */
  onAddAlert: () => void;
};

/**
 * Alerts screen card — the rule list (extracted from `AlertsScreen.tsx`,
 * F4a S4a; language pass + dictionary verbs, F4a S4b). Data comes from
 * `useAlertsQuery` (ADR 0106) instead of a local effect.
 */
export function AlertRulesSection({
  rules,
  companyName,
  watchlistName,
  rulesError,
  onRetry,
  onToggle,
  onCommitPrice,
  onRemove,
  onAddAlert,
}: AlertRulesSectionProps) {
  const { text, locale } = useLocale();

  return (
    <div className="alerts-card">
      <SectionHeader
        className="alerts-card-header"
        level="h2"
        title={text("Your alerts")}
        meta={
          <>
            <Figure value={rules.length} /> {pluralNoun(locale, rules.length, RULE_FORMS)}
          </>
        }
      />
      {rulesError ? (
        <div className="alerts-attention-error">
          <ErrorText>{text("Couldn't load the rules. The rest of the view is up to date.")}</ErrorText>
          <ActionButton kind="control" onClick={onRetry} variant="ghost">
            {text("Try again")}
          </ActionButton>
        </div>
      ) : rules.length === 0 ? (
        <EmptyState
          kind="invitation"
          title={text("You don't have any alerts yet")}
          source={text("A rule says what you want to be told about — for a company or a list.")}
          action={
            <ActionButton verb="add" variant="primary" onClick={onAddAlert}>
              {text("Add alert")}
            </ActionButton>
          }
        />
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
        {rule.enabled ? (
          <ActionButton verb="pause" variant="ghost" onClick={() => onToggle(false)}>
            {text("Pause")}
          </ActionButton>
        ) : (
          <ActionButton verb="resume" variant="ghost" onClick={() => onToggle(true)}>
            {text("Resume")}
          </ActionButton>
        )}
        <ActionButton verb="remove" variant="ghost" onClick={onRemove}>
          {text("Remove")}
        </ActionButton>
      </div>
    </li>
  );
}
