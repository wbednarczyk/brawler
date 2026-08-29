import { Fragment, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { RefreshCw } from "lucide-react";

import { type AlertRule, type AlertRuleUpdate, type AttentionEvent, type NewAlertRule } from "../../api/attention";
import type { AttentionController } from "../../app/useAttentionController";
import { openExternalUrl } from "../../app/openExternalUrl";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import {
  ActionButton,
  ChipList,
  ErrorText,
  FieldRow,
  Hint,
  PanelHeader,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  Skeleton,
  TextField,
  useUndoableDelete,
} from "../../ui";
import { AlertRulesSection } from "./AlertRulesSection";
import { FiredAlertsSection } from "./FiredAlertsSection";
import { useAlertsQuery } from "./useAlertsQuery";
import { PRESETS, type ScopeType, parsePrice, priceText } from "./alertLabels";

// U7-E2 S-tier detection (mirrors Today's `useNarrowPane` / Events'
// `usePaneCompact` — one local copy per screen is the house pattern, same
// 420px S-tier boundary): the composer folds behind `Dodaj alert` only at
// the S tier — M already renders it open (M's own field-wrapping lives in
// `alerts.css`'s separate 640px `@container` rule). jsdom has no
// `ResizeObserver` → stays `false`, i.e. the composer always renders open in
// component tests; the real fold is proven by
// `tests/browser/density-utility.spec.ts`.
function useNarrowPane(ref: RefObject<HTMLElement | null>): boolean {
  const [narrow, setNarrow] = useState(false);
  useEffect(() => {
    const host = ref.current;
    if (!host || typeof ResizeObserver === "undefined") return;
    const pane = (host.closest(".workspace") as HTMLElement | null) ?? host;
    const measure = () => {
      const width = pane.clientWidth;
      setNarrow(width > 0 && width < 420);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(pane);
    return () => observer.disconnect();
  }, [ref]);
  return narrow;
}

/**
 * Library screen "Alerts" (ADR 0068 T3; relocated from Settings 2026-07-15, owner
 * decision, v0.54; visual redesign v0.54 to docs/mockups/alerts-library-view.html).
 * The fired-alert review, the alert-rules manager, and the composer, laid out
 * fired-first (contract § Alerts information hierarchy: fired alerts are the
 * must-see reason the screen exists day to day) with a live plain-language
 * preview of the draft rule.
 *
 * F4a S4a (ADR 0106): split into this composer/composition + `AlertRulesSection` +
 * `FiredAlertsSection`, data loaded through `useAlertsQuery` (rules/companies/
 * watchlists via `useCommandQuery`; fired events still the shared
 * `AttentionController`, ADR 0097 dec. 6). F4a S4b: reordered to fired-first,
 * dictionary verbs (pause/resume/remove), invitation/quiet empty states,
 * per-section partial strips, a dense fired-events cap, and destination
 * navigation for a fired row (mirrors Today's `openAttentionRowAction`).
 * Pure label/formatting helpers live in `alertLabels.ts`.
 */
export function AlertsScreen({
  attention,
  openCompanyWorkspaceById,
  openInbox,
}: {
  attention: AttentionController;
  /** Fired row destination (F4a S4b): the ONE company deep-dive surface
   * since the docking engine's removal (ADR 0108). */
  openCompanyWorkspaceById: (companyId: string) => void;
  /** Fired row fallback destination for a company-less SYSTEM event. */
  openInbox: () => void;
}) {
  const { t, text } = useLocale();
  const runUndoableDelete = useUndoableDelete();
  const alerts = useAlertsQuery(attention);

  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const rootRef = useRef<HTMLElement>(null);
  const composerRef = useRef<HTMLDivElement>(null);
  const narrow = useNarrowPane(rootRef);
  const [composerOpen, setComposerOpen] = useState(false);
  const showComposer = !narrow || composerOpen;

  // Invitation empty state's action + the collapsed S-tier toggle both land
  // here: open the composer and move focus into its first control, so
  // "Dodaj alert" always lands the user somewhere they can act (contract §
  // Alerts, state matrix "Empty (no rules)": "focus composer").
  function focusComposer() {
    setComposerOpen(true);
    requestAnimationFrame(() => {
      composerRef.current?.scrollIntoView({ block: "nearest" });
      composerRef.current?.querySelector<HTMLElement>("button, input")?.focus();
    });
  }

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

  // Fix-C guardrail 8: which "Add alert" button is the one filled element at
  // rest (see the composer's `variant`/`data-ux-primary-action` below).
  const hasRules = alerts.rules.length > 0;

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

  // Fired row destination (F4a S4b, contract § Alerts exit path): marks the
  // event seen, then lands on its target surface — mirrors Today's
  // `openAttentionRowAction` (`src/screens/Today/TodayScreen.tsx`). Folds the
  // old separate "Review"/mark-seen button into the one destination click.
  function openFiredEvent(event: AttentionEvent) {
    void alerts.markEventSeen(event.id).catch(() => {});
    if (event.triggerType === "source_reconciliation" && event.witnessUrl) {
      openExternalUrl(event.witnessUrl);
      return;
    }
    if (event.companyId) {
      openCompanyWorkspaceById(event.companyId);
      return;
    }
    openInbox();
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

  const composer = (
    <div className="alerts-card" ref={composerRef}>
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
              data-action-kind="control"
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
            <SegmentedControlOption
              active={scopeType === "watchlist"}
              data-action-kind="control"
              onClick={() => setScopeType("watchlist")}
            >
              {text("Watchlist")}
            </SegmentedControlOption>
            <SegmentedControlOption
              active={scopeType === "company"}
              data-action-kind="control"
              onClick={() => setScopeType("company")}
            >
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
        <ActionButton
          verb="add"
          className="alerts-preview-add"
          // Fix-C guardrail 8 (sol F4a R1, "one filled element at rest"):
          // with no rules yet, `AlertRulesSection`'s invitation renders its
          // own filled "Add alert" that focuses this composer — this button
          // goes quiet in that state so the two never both render filled at
          // once. Once a rule exists the invitation is gone and this is
          // again the screen's one primary action.
          variant={hasRules ? "primary" : "secondary"}
          disabled={!canAdd}
          onClick={addRule}
          data-ux-primary-action={hasRules ? "true" : undefined}
        >
          {text("Add alert")}
        </ActionButton>
      </div>
    </div>
  );

  return (
    <section className="feed-panel" aria-labelledby="alerts-title" ref={rootRef}>
      <PanelHeader
        title={t("alerts.title")}
        description={t("alerts.description")}
        titleId="alerts-title"
      />

      {alerts.status === "loading" ? (
        <div className="alerts-layout">
          <Skeleton variant="list-row" count={3} label={text("Loading alerts…")} />
        </div>
      ) : alerts.status === "error" ? (
        <div className="alerts-layout">
          <div className="alerts-error-strip" role="alert">
            <ErrorText>{text("Couldn't load alerts.")}</ErrorText>
            <ActionButton kind="control" onClick={alerts.refetch} variant="ghost">
              <RefreshCw aria-hidden="true" size={13} />
              {text("Try again")}
            </ActionButton>
          </div>
        </div>
      ) : (
        <div className="alerts-layout">
          <FiredAlertsSection
            events={alerts.events}
            rules={alerts.rules}
            companyName={companyName}
            eventsError={alerts.eventsError}
            onRetry={() => attention.refresh()}
            onOpen={openFiredEvent}
            onDismiss={dismissEvent}
          />

          <AlertRulesSection
            rules={alerts.rules}
            companyName={companyName}
            watchlistName={watchlistName}
            rulesError={Boolean(alerts.sectionErrors.rules)}
            onRetry={alerts.refetch}
            onToggle={toggleRule}
            onCommitPrice={commitPrice}
            onRemove={removeRule}
            onAddAlert={focusComposer}
          />

          {/* Secondary in the hierarchy (contract § Alerts § 5): folds behind
              its own primary at the S tier so fired alerts + rules stay the
              must-see content above the fold. */}
          {showComposer ? (
            composer
          ) : (
            <div className="alerts-card alerts-composer-collapsed">
              <ActionButton kind="control" variant="secondary" onClick={() => setComposerOpen(true)}>
                {text("Add alert")}
              </ActionButton>
            </div>
          )}

          {error ? (
            <ErrorText>
              {text("Alert command failed")}: {friendlyError(error)}
            </ErrorText>
          ) : null}
        </div>
      )}
    </section>
  );
}
