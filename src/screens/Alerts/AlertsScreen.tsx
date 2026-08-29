import { Fragment, useEffect, useMemo, useRef, useState } from "react";

import { type AlertRule, type AlertRuleUpdate, type AttentionEvent, type NewAlertRule } from "../../api/attention";
import type { AttentionController } from "../../app/useAttentionController";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import {
  Button,
  ChipList,
  ErrorText,
  FieldRow,
  Hint,
  PanelHeader,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  TextField,
  useUndoableDelete,
} from "../../ui";
import { AlertRulesSection } from "./AlertRulesSection";
import { FiredAlertsSection } from "./FiredAlertsSection";
import { useAlertsQuery } from "./useAlertsQuery";
import { PRESETS, type ScopeType, parsePrice, priceText } from "./alertLabels";

/**
 * Library screen "Alerts" (ADR 0068 T3; relocated from Settings 2026-07-15, owner
 * decision, v0.54; visual redesign v0.54 to docs/mockups/alerts-library-view.html).
 * The alert-rules manager and fired-alert review, laid out as three cards — create,
 * your rules, fired alerts — with a live plain-language preview of the draft rule.
 * A reference surface like Sources/Watchlists (its own sidebar destination) so it
 * gets the Library `feed-panel` + `PanelHeader` chrome.
 *
 * F4a S4a (ADR 0106): split into this composer/composition + `AlertRulesSection` +
 * `FiredAlertsSection`, data loaded through `useAlertsQuery` (rules/companies/
 * watchlists via `useCommandQuery`; fired events still the shared
 * `AttentionController`, ADR 0097 dec. 6). Pure label/formatting helpers live in
 * `alertLabels.ts`.
 */
export function AlertsScreen({ attention }: { attention: AttentionController }) {
  const { t, text } = useLocale();
  const runUndoableDelete = useUndoableDelete();
  const alerts = useAlertsQuery(attention);

  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Error state stores the raw backend message; known typed errors are mapped
  // to a plain translated sentence at RENDER time (`friendlyError` below), so
  // no effect/callback depends on the locale (react-hooks/exhaustive-deps).
  const friendlyError = (raw: string): string =>
    raw.includes("identical alert rule already exists")
      ? text("An identical alert rule already exists.")
      : raw;

  // New-rule draft.
  const [presetKey, setPresetKey] = useState<string>(PRESETS[0].key);
  const [scopeType, setScopeType] = useState<ScopeType>("watchlist");
  const [scopeRef, setScopeRef] = useState<string>("");
  const [priceMin, setPriceMin] = useState<string>("");
  const [priceMax, setPriceMax] = useState<string>("");

  const preset = useMemo(() => PRESETS.find((p) => p.key === presetKey) ?? PRESETS[0], [presetKey]);
  const companyName = useMemo(
    () => new Map(alerts.companies.map((c) => [c.id, c.qualifiedTicker])),
    [alerts.companies],
  );
  const watchlistName = useMemo(() => new Map(alerts.watchlists.map((w) => [w.id, w.name])), [alerts.watchlists]);

  // Seed the scope target with a sensible default, once, as soon as the
  // library data first arrives — mirrors the previous mount-effect's seeding,
  // now reacting to `useAlertsQuery`'s data instead of owning its own fetch.
  const seededRef = useRef(false);
  useEffect(() => {
    if (seededRef.current) return;
    if (alerts.watchlists.length === 0 && alerts.companies.length === 0) return;
    seededRef.current = true;
    if (alerts.watchlists[0]) {
      setScopeType("watchlist");
      setScopeRef(alerts.watchlists[0].id);
    } else if (alerts.companies[0]) {
      setScopeType("company");
      setScopeRef(alerts.companies[0].id);
    }
  }, [alerts.watchlists, alerts.companies]);

  const scopeOptions = scopeType === "watchlist" ? alerts.watchlists : alerts.companies;
  // When the scope type flips, keep the selection valid.
  useEffect(() => {
    const options = scopeType === "watchlist" ? alerts.watchlists : alerts.companies;
    if (!options.some((option) => option.id === scopeRef)) {
      setScopeRef(options[0]?.id ?? "");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeType]);

  const isPriceRange = preset.triggerType === "price_enters_range";
  const priceMinNum = parsePrice(priceMin);
  const priceMaxNum = parsePrice(priceMax);
  const priceRangeValid = !isPriceRange || (priceMinNum !== null && priceMaxNum !== null && priceMinNum <= priceMaxNum);
  const canAdd = scopeRef.trim() !== "" && priceRangeValid && !busy;

  function addRule() {
    const input: NewAlertRule = {
      triggerType: preset.triggerType,
      signalCategory: preset.signalCategory,
      priceMin: isPriceRange ? priceMinNum : null,
      priceMax: isPriceRange ? priceMaxNum : null,
      scopeType,
      scopeRef,
    };
    setBusy(true);
    setError(null);
    alerts
      .createRule(input)
      .then(() => {
        setPriceMin("");
        setPriceMax("");
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  }

  function toggleRule(rule: AlertRule, enabled: boolean) {
    setError(null);
    alerts.setRuleEnabled(rule.id, enabled).catch((reason) => setError(String(reason)));
  }

  function removeRule(rule: AlertRule) {
    runUndoableDelete({
      perform: () => alerts.removeRule(rule.id),
      restore: () =>
        alerts.createRule({
          triggerType: rule.triggerType,
          signalCategory: rule.signalCategory,
          priceMin: rule.priceMin,
          priceMax: rule.priceMax,
          scopeType: rule.scopeType,
          scopeRef: rule.scopeRef,
        }),
      message: text("Alert rule deleted"),
      undoLabel: text("Undo"),
      onError: (reason) => setError(String(reason)),
    });
  }

  function commitPrice(input: AlertRuleUpdate) {
    setError(null);
    alerts.updateRulePrice(input).catch((reason) => {
      setError(String(reason));
      // The row's edit was local-only until commit; re-read so it shows the
      // value the backend actually holds, not the rejected draft.
      alerts.refetch();
    });
  }

  // The controller re-syncs on a failed mutation; this screen also surfaces it.
  function dismissEvent(event: AttentionEvent) {
    setError(null);
    alerts.dismissEvent(event.id).catch((reason) => setError(String(reason)));
  }

  function markSeen(event: AttentionEvent) {
    alerts.markEventSeen(event.id).catch((reason) => setError(String(reason)));
  }

  // Live preview: a plain-language sentence for the draft rule, with the target
  // and the trigger bolded. Templates carry {target}/{trigger} placeholders so
  // each full sentence stays one translatable unit (grammatical in en + pl); we
  // split on the placeholders to inject the bold nodes.
  const previewTemplate = (): string => {
    if (isPriceRange) {
      return scopeType === "watchlist"
        ? text("I'll tell you when a company on the {target} list enters the {trigger} price range.")
        : text("I'll tell you when {target}'s price enters the {trigger} range.");
    }
    switch (preset.triggerType) {
      case "signal_category":
        if (preset.signalCategory === "insider_transaction") {
          return scopeType === "watchlist"
            ? text("I'll tell you when a company on the {target} list reports {trigger}.")
            : text("I'll tell you when {target} reports {trigger}.");
        }
        return scopeType === "watchlist"
          ? text("I'll tell you when a company on the {target} list publishes {trigger}.")
          : text("I'll tell you when {target} publishes {trigger}.");
      case "price_week52_low":
        return scopeType === "watchlist"
          ? text("I'll tell you when a company on the {target} list hits {trigger}.")
          : text("I'll tell you when {target} hits {trigger}.");
      case "autopilot_run_completed":
        return scopeType === "watchlist"
          ? text("I'll tell you when a company on the {target} list completes {trigger}.")
          : text("I'll tell you when {target} completes {trigger}.");
      default:
        return scopeType === "watchlist"
          ? text("I'll tell you when a company on the {target} list publishes {trigger}.")
          : text("I'll tell you when {target} publishes {trigger}.");
    }
  };

  const previewTriggerNoun = (): string => {
    switch (preset.triggerType) {
      case "signal_category":
        return preset.signalCategory === "insider_transaction"
          ? text("insider transactions")
          : preset.signalCategory === "auditor_opinion"
            ? text("an auditor opinion")
            : preset.signalCategory === "short_position_change"
              ? text("a short position change")
              : text("a profit warning");
      case "price_week52_low":
        return text("a 52-week low");
      case "autopilot_run_completed":
        return text("an autopilot analysis");
      case "price_enters_range":
        return `${priceText(priceMinNum)}–${priceText(priceMaxNum)} zł`;
      default:
        return text("a profit warning");
    }
  };

  const previewTargetTicker = scopeType === "company" ? companyName.get(scopeRef) ?? null : null;
  const previewTargetName =
    scopeType === "watchlist"
      ? watchlistName.get(scopeRef) ?? text("your watchlist")
      : companyName.get(scopeRef) ?? text("this company");

  const renderPreview = () => {
    const template = previewTemplate();
    const trigger = previewTriggerNoun();
    const targetNode = previewTargetTicker ? (
      <TickerLabel value={previewTargetTicker} />
    ) : (
      <strong>{previewTargetName}</strong>
    );
    return template.split(/(\{target\}|\{trigger\})/).map((segment, index) => {
      if (segment === "{target}") return <Fragment key={index}>{targetNode}</Fragment>;
      if (segment === "{trigger}") return <strong key={index}>{trigger}</strong>;
      return <Fragment key={index}>{segment}</Fragment>;
    });
  };

  return (
    <section className="feed-panel" aria-labelledby="alerts-title">
      <PanelHeader
        title={t("alerts.title")}
        description={t("alerts.description")}
        titleId="alerts-title"
      />

      <div className="alerts-layout">
        {/* Card 1 — create a rule: trigger preset → scope → live preview + add. */}
        <div className="alerts-card">
          <SectionHeader className="alerts-card-header" level="h2" title={text("New alert")} />

          <p className="alerts-step">{text("1 · What to watch for")}</p>
          <ChipList ariaLabel={text("Alert presets")} className="alerts-trigger-chips">
            {PRESETS.map((option) => {
              const Icon = option.icon;
              const active = option.key === presetKey;
              return (
                <button
                  key={option.key}
                  type="button"
                  aria-pressed={active}
                  className={["alerts-trigger-chip", active ? "alerts-trigger-chip-active" : ""]
                    .filter(Boolean)
                    .join(" ")}
                  onClick={() => setPresetKey(option.key)}
                >
                  <Icon size={15} aria-hidden={true} />
                  {text(option.label)}
                </button>
              );
            })}
          </ChipList>

          <p className="alerts-step">{text("2 · Where it applies")}</p>
          <FieldRow className="alerts-where-row">
            <div className="alerts-field-group">
              <span className="alerts-field-group-label">{text("Scope")}</span>
              <SegmentedControl ariaLabel={text("Alert scope")}>
                <SegmentedControlOption active={scopeType === "watchlist"} onClick={() => setScopeType("watchlist")}>
                  {text("Watchlist")}
                </SegmentedControlOption>
                <SegmentedControlOption active={scopeType === "company"} onClick={() => setScopeType("company")}>
                  {text("Company")}
                </SegmentedControlOption>
              </SegmentedControl>
            </div>
            <SelectField
              aria-label={text("Alert scope target")}
              label={text("Target")}
              value={scopeRef}
              onChange={(event) => setScopeRef(event.target.value)}
            >
              {scopeOptions.length === 0 ? <option value="">{text("None available")}</option> : null}
              {scopeType === "watchlist"
                ? alerts.watchlists.map((w) => (
                    <option key={w.id} value={w.id}>
                      {w.name}
                    </option>
                  ))
                : alerts.companies.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.qualifiedTicker}
                    </option>
                  ))}
            </SelectField>
            {isPriceRange ? (
              <>
                <TextField
                  aria-label={text("Minimum price")}
                  label={text("Minimum price")}
                  type="number"
                  inputMode="decimal"
                  value={priceMin}
                  onChange={(event) => setPriceMin(event.target.value)}
                />
                <TextField
                  aria-label={text("Maximum price")}
                  label={text("Maximum price")}
                  type="number"
                  inputMode="decimal"
                  value={priceMax}
                  onChange={(event) => setPriceMax(event.target.value)}
                />
              </>
            ) : null}
          </FieldRow>
          {isPriceRange && !priceRangeValid ? (
            <Hint>{text("Enter a minimum and maximum price, with the minimum no higher than the maximum.")}</Hint>
          ) : null}

          <div className="alerts-preview" role="note" aria-label={text("Alert preview")}>
            <p className="alerts-preview-text">{renderPreview()}</p>
            <Button
              className="alerts-preview-add"
              variant="primary"
              disabled={!canAdd}
              onClick={addRule}
              data-ux-primary-action="true"
            >
              {text("Add alert")}
            </Button>
          </div>
        </div>

        <AlertRulesSection
          rules={alerts.rules}
          companyName={companyName}
          watchlistName={watchlistName}
          onToggle={toggleRule}
          onCommitPrice={commitPrice}
          onRemove={removeRule}
        />

        <FiredAlertsSection
          events={alerts.events}
          rules={alerts.rules}
          companyName={companyName}
          eventsError={alerts.eventsError}
          onRetry={() => attention.refresh()}
          onDismiss={dismissEvent}
          onMarkSeen={markSeen}
        />

        {error ? (
          <ErrorText>
            {text("Alert command failed")}: {friendlyError(error)}
          </ErrorText>
        ) : null}
      </div>
    </section>
  );
}
