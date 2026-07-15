import { Fragment, useEffect, useMemo, useState } from "react";
import { BarChart3, Bot, Coins, FileWarning, TrendingDown, Trash2, Users, X, type LucideIcon } from "lucide-react";

import {
  createAlertRule,
  deleteAlertRule,
  dismissAttentionEvent,
  listAlertRules,
  listAttentionEvents,
  markAttentionEventSeen,
  setAlertRuleEnabled,
  updateAlertRule,
  type AlertRule,
  type AttentionEvent,
  type NewAlertRule,
} from "../../api/attention";
import { listCompanies } from "../../api/companies";
import { listWatchlists } from "../../api/watchlists";
import type { Company, Watchlist } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatListTimestamp } from "../../shared/format/datetime";
import { useLocale } from "../../shared/locale";
import { pluralNoun } from "../../shared/locale/plural";
import {
  Button,
  ChipList,
  EmptyState,
  ErrorText,
  FieldRow,
  Hint,
  PanelHeader,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  StatusChip,
  TextField,
  useUndoableDelete,
} from "../../ui";

type ScopeType = AlertRule["scopeType"];
type TriggerType = AlertRule["triggerType"];
type IconComponent = LucideIcon;

// Preset rule chips (ADR 0068 T3, visual-first per docs/ui-authoring.md): a click
// pre-fills the trigger (and its signal category) so the user only picks a scope.
// The lucide icon rides through to the matching rule row's leading tile so the
// creation choice and the resulting rule read as the same thing.
type Preset = {
  key: string;
  label: string;
  triggerType: TriggerType;
  signalCategory: string | null;
  icon: IconComponent;
};

const PRESETS: readonly Preset[] = [
  { key: "profit_warning", label: "Profit warning", triggerType: "signal_category", signalCategory: "profit_warning", icon: TrendingDown },
  { key: "insider", label: "Insider transactions", triggerType: "signal_category", signalCategory: "insider_transaction", icon: Users },
  { key: "auditor_opinion", label: "Auditor opinion", triggerType: "signal_category", signalCategory: "auditor_opinion", icon: FileWarning },
  { key: "short_position", label: "Short position", triggerType: "signal_category", signalCategory: "short_position_change", icon: TrendingDown },
  { key: "week52_low", label: "52-week low", triggerType: "price_week52_low", signalCategory: null, icon: BarChart3 },
  { key: "price_range", label: "Price range", triggerType: "price_enters_range", signalCategory: null, icon: Coins },
  { key: "autopilot", label: "Autopilot finished", triggerType: "autopilot_run_completed", signalCategory: null, icon: Bot },
];

const RULE_FORMS = { en: ["rule", "rules"], pl: ["reguła", "reguły", "reguł"] } as const;
const NEW_FORMS = { en: ["new", "new"], pl: ["nowa", "nowe", "nowych"] } as const;

function parsePrice(value: string): number | null {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function priceText(value: number | null): string {
  return value === null ? "?" : String(value);
}

// Leading tile / preset icon for a rule's trigger (mirrors the PRESETS icons so a
// created rule keeps the glyph of the choice that made it).
function triggerIcon(triggerType: TriggerType, signalCategory: string | null): IconComponent {
  switch (triggerType) {
    case "signal_category":
      return signalCategory === "insider_transaction"
        ? Users
        : signalCategory === "auditor_opinion"
          ? FileWarning
          : TrendingDown; // profit_warning + short_position_change share the glyph
    case "price_enters_range":
      return Coins;
    case "price_week52_low":
      return BarChart3;
    case "autopilot_run_completed":
      return Bot;
    default:
      return TrendingDown;
  }
}

/**
 * Library screen "Alerts" (ADR 0068 T3; relocated from Settings 2026-07-15, owner
 * decision, v0.54; visual redesign v0.54 to docs/mockups/alerts-library-view.html).
 * The alert-rules manager and fired-alert review, laid out as three cards — create,
 * your rules, fired alerts — with a live plain-language preview of the draft rule.
 * A reference surface like Sources/Watchlists (its own sidebar destination) so it
 * gets the Library `feed-panel` + `PanelHeader` chrome. Stays self-contained: it
 * drives the attention commands (`api/attention`) directly and re-reads on each
 * mutation. The richer Today attention surfaces + persistent toasts are T4.
 */
export function AlertsScreen() {
  const { t, text, locale } = useLocale();
  const runUndoableDelete = useUndoableDelete();

  const [rules, setRules] = useState<AlertRule[]>([]);
  const [events, setEvents] = useState<AttentionEvent[]>([]);
  const [companies, setCompanies] = useState<Company[]>([]);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
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

  const preset = useMemo(
    () => PRESETS.find((p) => p.key === presetKey) ?? PRESETS[0],
    [presetKey],
  );
  const companyName = useMemo(
    () => new Map(companies.map((c) => [c.id, c.qualifiedTicker])),
    [companies],
  );
  const watchlistName = useMemo(() => new Map(watchlists.map((w) => [w.id, w.name])), [watchlists]);

  function refresh() {
    Promise.all([listAlertRules(), listAttentionEvents()])
      .then(([nextRules, nextEvents]) => {
        setRules(nextRules);
        setEvents(nextEvents);
      })
      .catch((reason) => setError(String(reason)));
  }

  useEffect(() => {
    let active = true;
    Promise.all([listAlertRules(), listAttentionEvents(), listCompanies(), listWatchlists()])
      .then(([nextRules, nextEvents, nextCompanies, nextWatchlists]) => {
        if (!active) return;
        setRules(nextRules);
        setEvents(nextEvents);
        setCompanies(nextCompanies);
        setWatchlists(nextWatchlists);
        // Seed the scope target with a sensible default so the form is usable.
        if (nextWatchlists[0]) {
          setScopeType("watchlist");
          setScopeRef(nextWatchlists[0].id);
        } else if (nextCompanies[0]) {
          setScopeType("company");
          setScopeRef(nextCompanies[0].id);
        }
      })
      .catch((reason) => {
        if (active) setError(String(reason));
      });
    return () => {
      active = false;
    };
  }, []);

  const scopeOptions = scopeType === "watchlist" ? watchlists : companies;
  // When the scope type flips, keep the selection valid.
  useEffect(() => {
    const options = scopeType === "watchlist" ? watchlists : companies;
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
    createAlertRule(input)
      .then((rule) => {
        setRules((current) => [...current, rule]);
        setPriceMin("");
        setPriceMax("");
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  }

  function toggleRule(rule: AlertRule, enabled: boolean) {
    setError(null);
    setAlertRuleEnabled(rule.id, enabled)
      .then((updated) => setRules((current) => current.map((r) => (r.id === rule.id ? updated : r))))
      .catch((reason) => setError(String(reason)));
  }

  function removeRule(rule: AlertRule) {
    runUndoableDelete({
      perform: () => deleteAlertRule(rule.id),
      restore: () =>
        createAlertRule({
          triggerType: rule.triggerType,
          signalCategory: rule.signalCategory,
          priceMin: rule.priceMin,
          priceMax: rule.priceMax,
          scopeType: rule.scopeType,
          scopeRef: rule.scopeRef,
        }),
      message: text("Alert rule deleted"),
      undoLabel: text("Undo"),
      onPerformed: () => setRules((current) => current.filter((r) => r.id !== rule.id)),
      onRestored: refresh,
      onError: (reason) => setError(String(reason)),
    });
  }

  // Two-way price editing on an existing range rule (docs/ui-authoring.md): the
  // input drives local state immediately and commits through update_alert_rule.
  function editRulePrice(rule: AlertRule, which: "priceMin" | "priceMax", raw: string) {
    const next = parsePrice(raw);
    setRules((current) => current.map((r) => (r.id === rule.id ? { ...r, [which]: next } : r)));
  }

  function commitRulePrice(rule: AlertRule) {
    setError(null);
    updateAlertRule({ id: rule.id, priceMin: rule.priceMin ?? undefined, priceMax: rule.priceMax ?? undefined })
      .then((updated) => setRules((current) => current.map((r) => (r.id === rule.id ? updated : r))))
      .catch((reason) => setError(String(reason)));
  }

  function dismissEvent(event: AttentionEvent) {
    setError(null);
    dismissAttentionEvent(event.id)
      .then(() => setEvents((current) => current.filter((e) => e.id !== event.id)))
      .catch((reason) => setError(String(reason)));
  }

  function markSeen(event: AttentionEvent) {
    markAttentionEventSeen(event.id)
      .then(() => setEvents((current) => current.map((e) => (e.id === event.id ? { ...e, seen: true } : e))))
      .catch((reason) => setError(String(reason)));
  }

  // Short, human title for a rule's trigger (the rule row's bold first line).
  const ruleTitle = (rule: AlertRule): string => {
    switch (rule.triggerType) {
      case "signal_category":
        return rule.signalCategory === "insider_transaction"
          ? text("Insider transactions")
          : rule.signalCategory === "profit_warning"
            ? text("Profit warning")
            : rule.signalCategory === "auditor_opinion"
              ? text("Auditor opinion")
              : rule.signalCategory === "short_position_change"
                ? text("Short position")
                : `${text("Signal")}: ${rule.signalCategory ?? ""}`;
      case "autopilot_run_completed":
        return text("Autopilot finished");
      case "price_enters_range":
        return text("Price range");
      case "price_week52_low":
        return text("52-week low");
      default:
        return rule.triggerType;
    }
  };

  const triggerLabel = (rule: AlertRule): string => {
    if (rule.triggerType === "price_enters_range") {
      return `${text("Price range")} ${rule.priceMin ?? "?"}–${rule.priceMax ?? "?"}`;
    }
    return ruleTitle(rule);
  };

  const scopeName = (scope: ScopeType, ref: string): string =>
    scope === "watchlist" ? watchlistName.get(ref) ?? ref : companyName.get(ref) ?? ref;

  const ruleDescription = (rule: AlertRule): string =>
    `${triggerLabel(rule)} · ${scopeName(rule.scopeType, rule.scopeRef)}`;

  // Fired-event "what" line, from the trigger type joined onto the event.
  const eventWhat = (event: AttentionEvent): string => {
    switch (event.triggerType) {
      case "signal_category":
        return text("Signal");
      case "autopilot_run_completed":
        return text("Autopilot finished");
      case "price_enters_range":
        return text("Price range");
      case "price_week52_low":
        return text("52-week low");
      default:
        return event.triggerType;
    }
  };

  const eventDescription = (event: AttentionEvent): string =>
    `${companyName.get(event.companyId) ?? event.companyId} · ${event.evidenceType}`;

  // Fired-at renders like every other list timestamp in the app ("today 09:12",
  // "yesterday 14:03", …) — via the shared format layer, per its contract test.
  const formatFiredAt = (iso: string): string => formatListTimestamp(iso, locale, iso);

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

  const previewTargetTicker =
    scopeType === "company" ? companyName.get(scopeRef) ?? null : null;
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

  const unseenCount = events.filter((event) => !event.seen).length;

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
                ? watchlists.map((w) => (
                    <option key={w.id} value={w.id}>
                      {w.name}
                    </option>
                  ))
                : companies.map((c) => (
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

        {/* Card 2 — existing rules. */}
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
              {rules.map((rule) => {
                const description = ruleDescription(rule);
                const Icon = triggerIcon(rule.triggerType, rule.signalCategory);
                return (
                  <li key={rule.id} aria-label={`${text("Alert rule")}: ${description}`} className="alerts-row">
                    <span className="alerts-row-icon" aria-hidden="true">
                      <Icon size={16} />
                    </span>
                    <div className="alerts-row-main">
                      <div className="alerts-row-title">{ruleTitle(rule)}</div>
                      <div className="alerts-row-sub">
                        <StatusChip>
                          {rule.scopeType === "watchlist" ? text("List") : text("Company")}
                          {" · "}
                          {rule.scopeType === "company" && companyName.get(rule.scopeRef) ? (
                            <TickerLabel value={companyName.get(rule.scopeRef)!} />
                          ) : (
                            scopeName(rule.scopeType, rule.scopeRef)
                          )}
                        </StatusChip>
                        {rule.triggerType === "price_enters_range" ? (
                          <span className="alerts-row-prices">
                            <TextField
                              aria-label={`${text("Minimum price")} — ${description}`}
                              type="number"
                              inputMode="decimal"
                              value={rule.priceMin ?? ""}
                              onChange={(event) => editRulePrice(rule, "priceMin", event.target.value)}
                              onBlur={() => commitRulePrice(rule)}
                            />
                            <TextField
                              aria-label={`${text("Maximum price")} — ${description}`}
                              type="number"
                              inputMode="decimal"
                              value={rule.priceMax ?? ""}
                              onChange={(event) => editRulePrice(rule, "priceMax", event.target.value)}
                              onBlur={() => commitRulePrice(rule)}
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
                          onChange={(event) => toggleRule(rule, event.target.checked)}
                          role="switch"
                          type="checkbox"
                        />
                        <span aria-hidden="true" className="alerts-enable-track">
                          <span />
                        </span>
                      </label>
                      <Button onClick={() => removeRule(rule)} variant="ghost">
                        <Trash2 size={14} />
                        {text("Delete")}
                      </Button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {/* Card 3 — fired alerts (review). Richer Today surface + toasts land in T4. */}
        <div className="alerts-card">
          <SectionHeader
            className="alerts-card-header"
            level="h2"
            title={text("Fired alerts")}
            meta={unseenCount > 0 ? `${unseenCount} ${pluralNoun(locale, unseenCount, NEW_FORMS)}` : undefined}
          />
          {events.length === 0 ? (
            <EmptyState>{text("All quiet — nothing has fired. That's the point.")}</EmptyState>
          ) : (
            <ul className="alerts-list" aria-label={text("Fired alerts")}>
              {events.map((event) => {
                const description = eventDescription(event);
                const ruleForEvent = rules.find((r) => r.id === event.ruleId);
                const ticker = companyName.get(event.companyId) ?? event.companyId;
                return (
                  <li key={event.id} aria-label={`${text("Fired alert")}: ${description}`} className="alerts-row alerts-fired">
                    <span
                      aria-hidden="true"
                      className={["alerts-fired-dot", event.seen ? "alerts-fired-dot-seen" : ""]
                        .filter(Boolean)
                        .join(" ")}
                    />
                    <TickerLabel value={ticker} className="alerts-fired-ticker" />
                    <div className="alerts-row-main">
                      <div className="alerts-row-title">{eventWhat(event)}</div>
                      <div className="alerts-row-sub alerts-fired-meta">
                        {formatFiredAt(event.firedAt)}
                        {ruleForEvent ? ` · ${text("Rule")}: ${triggerLabel(ruleForEvent)}` : ""}
                      </div>
                    </div>
                    <div className="alerts-row-slots">
                      {event.seen ? null : (
                        <Button onClick={() => markSeen(event)} variant="ghost">
                          {text("Review")}
                        </Button>
                      )}
                      <Button aria-label={text("Dismiss")} onClick={() => dismissEvent(event)} variant="ghost">
                        <X size={14} aria-hidden={true} />
                      </Button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {error ? (
          <ErrorText>
            {text("Alert command failed")}: {friendlyError(error)}
          </ErrorText>
        ) : null}
      </div>
    </section>
  );
}
