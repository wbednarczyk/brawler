import { useEffect, useMemo, useRef, useState } from "react";
import { Pencil, Save, Trash2, X } from "lucide-react";
import type { FinancialFact, FinancialPeriod, KpiDefinition, KpiRelevance } from "../../api/financialsTypes";
import { useLocale, type LocaleCode } from "../../shared/locale";
import { localizedKpiLabel } from "../../shared/locale/kpiLabels";
import { AWAITS_NAMING_FORMS, FACT_FORMS, ITEM_FORMS, pluralNoun, type PluralForms } from "../../shared/locale/plural";
import { formatFinancialValue } from "../../shared/format/financialValue";
import { buildFactMatrix, buildStatementTabs, type StatementTabKey } from "./factMatrix";
import { FundamentalsPeriodsSection } from "./FundamentalsPeriodsSection";
import { FundamentalsFactsMatrix } from "./FundamentalsFactsMatrix";
import { factQualityLabel, factQualityTone, tierLabel } from "./factLabels";
import { useVisiblePeriods, naturalCellWidth, cssLengthVar } from "./useVisiblePeriods";
import { CustomKpiManager } from "../../shared/components/CustomKpiManager";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { DriftDiff, parseDrift } from "../../shared/components/DriftDiff";
import {
  ActionRow,
  Button,
  EmptyState,
  ErrorText,
  InfoGrid,
  InlineConfirm,
  Modal,
  SearchField,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  StatusChip,
  TextField,
  TrendChart,
} from "../../ui";
import { listFactProvenance, type FactProvenance } from "../../api/fundamentalsExtraction";
import { getReportDocumentsView } from "../../api/reportDocuments";
import { getPriceContext } from "../../api/marketData";
import type { PriceContext } from "../../api/marketData";
import { getAnalystRecommendations } from "../../api/analystRecommendations";
import type { AnalystRecommendationTarget } from "../../api/analystRecommendations";
import { PriceContextSection } from "./PriceContextSection";
import { FundamentalsDraftForms } from "./FundamentalsDraftForms";
import type { FactMatrixRow } from "./factMatrix";
import type {
  FinancialFactForm,
  FundamentalsForm,
} from "../../app/useFundamentalsController";

type FundamentalsPanelProps = {
  companyId: string;
  // For the fact-detail modal's header ("KPI · GPW:XTB · 2026 H1"). Omitted in
  // a standalone render (e.g. tests) — the header simply drops the ticker line.
  qualifiedTicker?: string;
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
  // Epic #398 "Kluczowe" statement tab: active/primary kpi_relevance rows
  // (ADR 0092). Optional/defaulted so a standalone render (e.g. tests) that
  // doesn't care about the Kluczowe selection can omit it.
  kpiRelevance?: KpiRelevance[];
  fundamentalsForm: FundamentalsForm;
  financialFactForm: FinancialFactForm;
  selectedFinancialFactId: string | null;
  isFinancialFactEditMode: boolean;
  fundamentalsError: string | null;
  fundamentalsLoadError: string | null;
  createFinancialPeriod: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  saveFinancialFact: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  deleteFinancialFact: (id: string) => Promise<void>;
  selectFinancialFact: (id: string) => void;
  startEditingFinancialFact: () => void;
  cancelEditingFinancialFact: () => void;
  updateFundamentalsForm: (field: keyof FundamentalsForm, value: string) => void;
  updateFinancialFactForm: (field: keyof FinancialFactForm, value: string) => void;
  // Cross-panel focus for the "vs target" readout (v0.58 A3, storyboard frame 8):
  // pins/focuses the analyst-recommendations panel. Omitted in standalone renders.
  onOpenRecommendations?: () => void;
};

/**
 * Human-readable source tier (ADR 0061). ESEF is the tagged source of truth;
 * `ai` (Radicle 4fde931) is the AI-confirmed-fact tier the deterministic pool
 * falls through to. A pure module-level function (takes `text` rather than
 * closing over `useLocale`) so it is unit-testable without rendering.
 */
// "Recorded" agrees in gender/number with the fact count in Polish (bug
// e77a1a2 part 3: "40 fakty zapisanych" was wrong — the header used a
// `n === 1 ? … : …` two-way ternary, which cannot express Polish's three
// plural categories). Declined separately from `FACT_FORMS` (the noun) since
// it is this header's own adjective, not reused elsewhere.
const RECORDED_FORMS: PluralForms = {
  en: ["recorded", "recorded"],
  pl: ["zapisany", "zapisane", "zapisanych"],
};

/**
 * The Fundamentals header's "N fact(s) recorded" line, correctly declined in
 * both languages. A pure module-level function (mirrors `tierLabel`) so it is
 * unit-testable without rendering.
 */
export function factsRecordedLabel(count: number, locale: LocaleCode): string {
  return `${count} ${pluralNoun(locale, count, FACT_FORMS)} ${pluralNoun(locale, count, RECORDED_FORMS)}`;
}

// Facts-matrix sticky column widths (dogfooding #6): the KPI column is fixed
// (not auto — a fixed width lets the expander column's CSS `left` be a
// matching static value, since dynamic per-render measurement would require
// inline `style={{…}}`, banned outside AppShell's sidebar-width exception —
// docs/ui-authoring.md § Styling rules). Long labels ellipsize with a native
// `title` tooltip. Mirrored in companies.css `.facts-matrix-kpi`/`-expander`.
// 140px (not the mockup's 96 — real localized labels run longer than "KPI")
// still leaves room for at least one period column at the narrowest S tier
// (measured: a ~320px scroller, ~86px period width — 140+44+86=270 fits with
// margin; the mockup's 96 was measured against a synthetic 10px preview font).

/**
 * Display label for a {@link StatementTabKey} (epic #398 statement switcher):
 * the approved mockup's fixed tab names. A pure module-level function (mirrors
 * `tierLabel`/`factQualityLabel`) so it is unit-testable without rendering.
 */
export function statementTabLabel(key: StatementTabKey, text: (value: string) => string): string {
  switch (key) {
    case "key":
      return text("Key figures");
    case "income":
      return text("Income statement");
    case "balance":
      return text("Balance sheet");
    case "cash_flow":
      return text("Cash flow");
    case "per_share":
      return text("Per share");
    default:
      return text("Operating");
  }
}

/**
 * "N awaiting a catalog name" line (epic #398), rendered as a warn chip in
 * the section header when > 0 (dogfooding #7 — the old completeness bar is
 * gone): the honest, never-silently-absent count of rows the matrix
 * synthesized a placeholder
 * for (`FactMatrixRow.isSynthetic`) because no `kpi_definitions` catalog row
 * matched their metric id yet.
 */
export function uncataloguedPositionsLabel(count: number, locale: LocaleCode): string {
  const noun = pluralNoun(locale, count, ITEM_FORMS);
  const verb = pluralNoun(locale, count, AWAITS_NAMING_FORMS);
  return locale === "pl"
    ? `${count} ${noun} ${verb} na nazwanie`
    : `${count} ${noun} ${verb} a catalog name`;
}

export function FundamentalsPanel({
  companyId,
  qualifiedTicker,
  financialPeriods,
  financialFacts,
  kpiDefinitions,
  kpiRelevance = [],
  fundamentalsForm,
  financialFactForm,
  selectedFinancialFactId,
  isFinancialFactEditMode,
  fundamentalsError,
  fundamentalsLoadError,
  createFinancialPeriod,
  saveFinancialFact,
  deleteFinancialFact,
  selectFinancialFact,
  startEditingFinancialFact,
  cancelEditingFinancialFact,
  updateFundamentalsForm,
  updateFinancialFactForm,
  onOpenRecommendations,
}: FundamentalsPanelProps) {
  const { text, locale } = useLocale();

  // Company-scoped custom KPI definitions are loaded by CustomKpiManager and
  // merged with the global taxonomy so they appear in the matrix and dropdown.
  const [companyDefinitions, setCompanyDefinitions] = useState<KpiDefinition[]>([]);
  const [confirmDeleteFact, setConfirmDeleteFact] = useState(false);

  // Statement switcher (epic #398): one statement in view at a time — replaces
  // the old all-groups-expanded collapsible list. "Kluczowe" is the default
  // view (approved mockup). Switching statements clears the find query so a
  // leftover filter from the previous statement can't hide every row.
  const [activeStatementTab, setActiveStatementTab] = useState<StatementTabKey>("key");
  const [findQuery, setFindQuery] = useState("");

  function selectStatementTab(key: StatementTabKey) {
    setActiveStatementTab(key);
    setFindQuery("");
  }

  // Structured-first provenance (ADR 0061): the source tier + validation verdict
  // the pipeline recorded per fact, badged on the fact detail. Legacy/manual
  // facts have no provenance row (safe: the badges simply don't render).
  const [provenanceById, setProvenanceById] = useState<Record<string, FactProvenance>>({});
  const factIdsKey = financialFacts.map((fact) => fact.id).join(",");
  useEffect(() => {
    const ids = factIdsKey ? factIdsKey.split(",") : [];
    if (ids.length === 0) {
      setProvenanceById({});
      return;
    }
    let cancelled = false;
    listFactProvenance(ids)
      .then((rows) => {
        if (cancelled) return;
        const map: Record<string, FactProvenance> = {};
        for (const row of rows) map[row.factId] = row;
        setProvenanceById(map);
      })
      .catch(() => {
        if (!cancelled) setProvenanceById({});
      });
    return () => {
      cancelled = true;
    };
  }, [factIdsKey]);

  // Report-document titles (card #307): resolves a fact's `sourceDocumentRef`
  // to its stored document's title for the modal's citation block ("source
  // citation · <document name>"). Best-effort — a load failure just leaves the
  // citation block showing the raw citation text with no document name.
  const [documentTitleById, setDocumentTitleById] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    getReportDocumentsView(companyId)
      .then((view) => {
        if (cancelled) return;
        const map: Record<string, string> = {};
        for (const row of view.rows) {
          map[row.document.id] = row.document.title || row.document.url;
        }
        setDocumentTitleById(map);
      })
      .catch(() => {
        if (!cancelled) setDocumentTitleById({});
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  // Price context (v0.53 T5, ADR 0067/0082): latest close/change, 52-week
  // range, and level-0 market ratios, fundamentals-adjacent. A load failure
  // (or a company whose price context is still loading) surfaces an inline
  // error in place of the section rather than silently vanishing — the rest
  // of the panel stays usable either way.
  const [priceContext, setPriceContext] = useState<PriceContext | null>(null);
  const [priceContextError, setPriceContextError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    setPriceContext(null);
    setPriceContextError(null);
    getPriceContext(companyId)
      .then((context) => {
        if (!cancelled) setPriceContext(context);
      })
      .catch((cause) => {
        if (!cancelled) setPriceContextError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  // Newest attributed analyst target (v0.58 A3, ADR 0073) for the "vs target"
  // readout beside the close. Best-effort — a load failure just omits the readout.
  const [analystTarget, setAnalystTarget] = useState<AnalystRecommendationTarget | null>(null);
  useEffect(() => {
    let cancelled = false;
    setAnalystTarget(null);
    getAnalystRecommendations(companyId)
      .then((view) => {
        if (!cancelled) setAnalystTarget(view.latestTarget ?? null);
      })
      .catch(() => {
        if (!cancelled) setAnalystTarget(null);
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  // Reset the delete confirmation whenever the selected fact changes.
  useEffect(() => setConfirmDeleteFact(false), [selectedFinancialFactId]);
  const allDefinitions = useMemo(() => {
    const seen = new Set(kpiDefinitions.map((definition) => definition.id));
    return [...kpiDefinitions, ...companyDefinitions.filter((definition) => !seen.has(definition.id))];
  }, [kpiDefinitions, companyDefinitions]);

  const selectedFact = selectedFinancialFactId
    ? financialFacts.find((f) => f.id === selectedFinancialFactId)
    : null;

  const selectedFactPeriod = selectedFact
    ? financialPeriods.find((p) => p.id === selectedFact.periodId)
    : null;

  const selectedFactDefinition = selectedFact
    ? allDefinitions.find((d) => d.id === selectedFact.definitionId)
    : null;

  const factMatrix = useMemo(
    () => buildFactMatrix(financialPeriods, financialFacts, allDefinitions),
    [financialPeriods, financialFacts, allDefinitions],
  );

  // "Kluczowe" tab selection (ADR 0092): the company's active/primary
  // kpi_relevance rows, by definitionId.
  const keyDefinitionIds = useMemo(
    () =>
      new Set(
        kpiRelevance
          .filter((relevance) => relevance.status === "active" && relevance.rank === "primary")
          .map((relevance) => relevance.definitionId),
      ),
    [kpiRelevance],
  );
  const statementTabs = useMemo(
    () => buildStatementTabs(factMatrix, keyDefinitionIds),
    [factMatrix, keyDefinitionIds],
  );
  const activeTab = statementTabs.find((tab) => tab.key === activeStatementTab) ?? statementTabs[0];

  // Find-a-position (epic #398): filters the ACTIVE statement's rows by
  // localized name — a 150-row statement often has one line the user wants,
  // not the whole tab.
  const matrixFindQuery = findQuery.trim().toLowerCase();
  const visibleMatrixRows = matrixFindQuery
    ? activeTab.rows.filter((row) =>
        localizedKpiLabel(row.definition, locale).toLowerCase().includes(matrixFindQuery),
      )
    : activeTab.rows;

  // Period-expander column (dogfooding #6): the newest MEASURED-capacity
  // periods show by default, oldest hidden behind a full-height clickable
  // column (owner storyboard round 1) — shared logic, this host's own
  // measured period-group width (one <th>) and fixed sticky-column width
  // (KPI column + the expander column itself, both fixed in CSS below).
  const factsScrollRef = useRef<HTMLDivElement | null>(null);
  const factsPeriods = useVisiblePeriods({
    scrollerRef: factsScrollRef,
    total: factMatrix.periods.length,
    measureKey: `${locale}|${factMatrix.periods.map((period) => period.id).join(",")}|${visibleMatrixRows
      .map((row) => row.definition.id)
      .join(",")}`,
    // Widest natural period cell (header label + origin chip, every body
    // value) across ALL periods — the measuring pass renders them all.
    measurePeriodWidth: () => {
      const scroller = factsScrollRef.current;
      if (!scroller) return 0;
      let widest = 0;
      for (const cell of scroller.querySelectorAll<HTMLElement>("[data-period-cell]")) {
        widest = Math.max(widest, naturalCellWidth(cell, cell.querySelector<HTMLElement>(".facts-matrix-cell")));
      }
      return widest;
    },
    // Sticky KPI column as rendered + the trend column's natural width when
    // the tier shows it (its min-width is the floor; it stretches otherwise).
    measureFixedWidth: () => {
      const scroller = factsScrollRef.current;
      if (!scroller) return 0;
      const corner = scroller.querySelector<HTMLElement>(".facts-matrix-corner");
      const trend = scroller.querySelector<HTMLElement>(".facts-matrix-trend-head");
      const trendShown = trend && getComputedStyle(trend).display !== "none";
      const trendWidth = trendShown
        ? Math.max(Number.parseFloat(getComputedStyle(trend).minWidth) || 0, naturalCellWidth(trend))
        : 0;
      return (corner?.offsetWidth ?? 0) + trendWidth;
    },
    measureExpanderWidth: () => cssLengthVar(factsScrollRef.current, "--period-expander-width"),
  });
  const visibleFactPeriods = factMatrix.periods.slice(
    factsPeriods.visibleStart,
    factsPeriods.visibleStart + factsPeriods.visibleCount,
  );
  // The column stays visible while expanded (showing "Collapse earlier") even
  // though `hiddenCount` is then 0 — "no column at all" only applies when
  // there was never anything to hide in the first place.
  const factsShowExpanderColumn = !factsPeriods.measuring && (factsPeriods.expanded || factsPeriods.hiddenCount > 0);
  // Autoscroll to the newest period only on the transition INTO the expanded
  // state (owner decision) — never on mount, never on a later resize.
  useEffect(() => {
    if (!factsPeriods.expanded) return;
    const scroller = factsScrollRef.current;
    if (!scroller) return;
    scroller.scrollLeft = scroller.scrollWidth;
  }, [factsPeriods.expanded]);

  // The never-silently-absent count of rows still awaiting a catalog name
  // (dogfooding #7: moved into the section header as a warn chip, the old
  // completeness bar is gone — it competed with the table for attention and
  // duplicated per-cell provenance).
  const latestMatrixPeriod = factMatrix.periods[factMatrix.periods.length - 1];
  const uncataloguedCount = activeTab.rows.filter((row) => row.isSynthetic).length;

  // Origin chip (epic #398): the source tier of the active statement's
  // current-period facts, when they agree — a mixed statement (e.g. some
  // rows still aggregator-sourced, others issuer-tagged) says so explicitly
  // rather than picking one tier and implying uniform provenance.
  const currentPeriodSourceTiers = latestMatrixPeriod
    ? new Set(
        activeTab.rows
          .map((row) => row.cells[latestMatrixPeriod.id])
          .filter((fact): fact is NonNullable<typeof fact> => Boolean(fact))
          .map((fact) => provenanceById[fact.id]?.sourceTier)
          .filter((tier): tier is string => Boolean(tier)),
      )
    : new Set<string>();
  const originTier = currentPeriodSourceTiers.size === 1 ? [...currentPeriodSourceTiers][0] : null;
  const originIsMixed = currentPeriodSourceTiers.size > 1;

  // Trends must compare like-for-like periods: mixing a full-year figure with
  // quarters distorts the line. When any quarterly/half-year period exists, the
  // trend series uses only those; otherwise it falls back to all periods (e.g.
  // an annual-only history). The matrix table still shows every column.
  const interimPeriods = new Set(["q1", "q2", "q3", "q4", "h1", "h2"]);
  const trendPeriods = factMatrix.periods.some((period) =>
    interimPeriods.has(period.periodType.toLowerCase()),
  )
    ? factMatrix.periods.filter((period) => interimPeriods.has(period.periodType.toLowerCase()))
    : factMatrix.periods;

  // Chronological numeric series for a KPI row (skips periods without a fact).
  const seriesValuesFor = (row: FactMatrixRow): number[] =>
    trendPeriods
      .map((period) => row.cells[period.id])
      .filter((fact): fact is NonNullable<typeof fact> => Boolean(fact))
      .map((fact) => Number(fact.valueNumeric))
      .filter((value) => Number.isFinite(value));

  // Labelled points for the larger per-KPI trend chart.
  const chartPointsFor = (row: FactMatrixRow) =>
    trendPeriods
      .map((period) => ({ period, fact: row.cells[period.id] }))
      .filter((entry) => entry.fact)
      .map((entry) => ({
        label: `${entry.period.fiscalYear} ${entry.period.periodType.toUpperCase()}`,
        value: Number(entry.fact!.valueNumeric),
        display: formatFinancialValue(
          {
            valueNumeric: entry.fact!.valueNumeric,
            currency: entry.fact!.currency,
            asReportedValue: entry.fact!.asReportedValue,
            asReportedScale: entry.fact!.asReportedScale,
            valueKind: row.definition.valueKind,
            unit: row.definition.unit,
          },
          locale,
        ),
      }))
      .filter((point) => Number.isFinite(point.value));

  // The panel's own KPI set drives the N=1 periods×deltas comparison (§A5): the
  // metric keys with facts, in matrix order, and their localized labels.
  const comparisonMetricKeys = useMemo(
    () => factMatrix.rows.map((row) => row.definition.metricKey),
    [factMatrix],
  );
  const kpiLabelByMetricKey = useMemo(() => {
    const map: Record<string, string> = {};
    for (const row of factMatrix.rows) {
      map[row.definition.metricKey] = localizedKpiLabel(row.definition, locale);
    }
    return map;
  }, [factMatrix, locale]);

  const selectedFactRow = selectedFactDefinition
    ? factMatrix.rows.find((row) => row.definition.id === selectedFactDefinition.id)
    : undefined;

  const selectedProvenance = selectedFact ? provenanceById[selectedFact.id] : undefined;
  const selectedDrift = parseDrift(selectedProvenance?.driftJson ?? null);

  const validationLabel = (status: string): string => {
    switch (status) {
      case "passed":
        return text("Validated");
      case "witness_confirmed":
        return text("Witness-confirmed");
      case "unreviewed":
        return text("Unreviewed");
      case "flagged":
        return text("Structure changed");
      default:
        return text("Not validated");
    }
  };
  const validationTone = (status: string): "ok" | "warn" | "danger" | "neutral" => {
    switch (status) {
      case "passed":
      case "witness_confirmed":
        return "ok";
      case "flagged":
        return "danger";
      case "unreviewed":
        return "warn";
      default:
        return "neutral";
    }
  };

  return (
    <div className="company-tab-panel fundamentals-panel" aria-label={text("Company fundamentals")}>
      {/* Compact header (ADR 0076 D6 global rule): no in-panel heading repeating
          the "Fundamentals" pane tab title — just the fact-count caption. */}
      <p className="fundamentals-caption num-tabular">
        {factsRecordedLabel(financialFacts.length, locale)}
      </p>

      {fundamentalsError ? (
        <ErrorText>{text("Fundamentals command failed")}: {fundamentalsError}</ErrorText>
      ) : null}
      {fundamentalsLoadError ? (
        <ErrorText>{text("Failed to load fundamentals data")}: {fundamentalsLoadError}</ErrorText>
      ) : null}

      {/* Section order (owner request 2026-07-14): price context first, the
          financial-facts matrix second, everything else (positions × periods,
          custom KPIs, forms) after. Sector and the IR reports URL live in the
          Basic info panel, not here. */}
      {priceContext ? (
        <PriceContextSection
          data={priceContext}
          className="fundamentals-section"
          analystTarget={analystTarget}
          onFocusRecommendations={onOpenRecommendations}
        />
      ) : priceContextError ? (
        <ErrorText>
          {text("Failed to load price context")}: {priceContextError}
        </ErrorText>
      ) : null}

      {/* Financial Facts List and Detail */}
      <div role="group" className="fundamentals-section" aria-label={text("Financial facts")}>
        <SectionHeader
          level="h4"
          title={text("Financial facts")}
          meta={
            uncataloguedCount > 0 ? (
              <StatusChip tone="warn">{uncataloguedPositionsLabel(uncataloguedCount, locale)}</StatusChip>
            ) : undefined
          }
        />

        <div className="fundamentals-workspace">
          {factMatrix.rows.length > 0 ? (
            <>
              {/* Statement switcher (epic #398, approved mockup): the panel takes the
                  shape of the report — one statement in view at a time, instead of a
                  ~150-row scroll. "Kluczowe" is the default view. */}
              <SegmentedControl ariaLabel={text("Statement")} className="statement-switcher">
                {statementTabs.map((tab) => (
                  <SegmentedControlOption
                    key={tab.key}
                    active={tab.key === activeTab.key}
                    onClick={() => selectStatementTab(tab.key)}
                  >
                    {statementTabLabel(tab.key, text)}
                    <span className="statement-tab-count">{tab.rows.length}</span>
                  </SegmentedControlOption>
                ))}
              </SegmentedControl>

              {/* Find-a-position (epic #398): a 150-row statement often has one line
                  the user wants, not the whole tab. */}
              <SearchField
                ariaLabel={text("Find a position")}
                className="registry-search-field fundamentals-find"
                clearLabel={text("Clear")}
                onChange={setFindQuery}
                onClear={() => setFindQuery("")}
                placeholder={text("Find a position…")}
                type="text"
                value={findQuery}
              />

              <FundamentalsFactsMatrix
                text={text}
                locale={locale}
                visibleMatrixRows={visibleMatrixRows}
                visibleFactPeriods={visibleFactPeriods}
                factsScrollRef={factsScrollRef}
                          factsPeriods={factsPeriods}
                factsShowExpanderColumn={factsShowExpanderColumn}
                selectedFinancialFactId={selectedFinancialFactId}
                selectFinancialFact={selectFinancialFact}
                seriesValuesFor={seriesValuesFor}
                latestPeriodId={latestMatrixPeriod?.id ?? null}
                originTier={originTier}
                originIsMixed={originIsMixed}
              />
            </>
          ) : (
            <EmptyState>{text("No financial facts yet.")}</EmptyState>
          )}

        </div>
      </div>

      {/* Fact detail modal (card #307): replaces the old below-table detail/edit
          section — clicking a matrix cell opens the SAME modal Edytuj switches
          into the edit-form fields. Esc/X/backdrop close via the Modal
          primitive; `cancelEditingFinancialFact` doubles as the close handler
          in both modes (it always clears the selection + edit mode + resets
          the form, a no-op reset when nothing was being edited). */}
      <Modal
        open={Boolean(selectedFact && selectedFactDefinition && selectedFactPeriod)}
        onClose={cancelEditingFinancialFact}
        ariaLabel={text("Financial fact detail")}
        className="fact-detail-modal"
        title={
          selectedFactDefinition && selectedFactPeriod ? (
            <span className="fact-modal-title">
              <span className="fact-modal-title-metric">
                {localizedKpiLabel(selectedFactDefinition, locale)}
              </span>
              <span className="fact-modal-title-period">
                {qualifiedTicker ? <TickerLabel value={qualifiedTicker} /> : null}
                {qualifiedTicker ? " · " : ""}
                {selectedFactPeriod.fiscalYear} {selectedFactPeriod.periodType.toUpperCase()}
                {selectedFactPeriod.periodEndDate
                  ? ` (${text("as of")} ${selectedFactPeriod.periodEndDate})`
                  : ""}
              </span>
            </span>
          ) : (
            ""
          )
        }
        footer={
          selectedFact ? (
            isFinancialFactEditMode ? (
              <ActionRow className="fact-detail-actions">
                <Button
                  className="compact-button"
                  onClick={() => deleteFinancialFact(selectedFact.id)}
                  variant="danger"
                >
                  <Trash2 size={15} />
                  {text("Delete")}
                </Button>
                <Button className="compact-button" onClick={cancelEditingFinancialFact}>
                  <X size={15} />
                  {text("Cancel")}
                </Button>
                <Button
                  className="compact-button"
                  form="fact-edit-form"
                  type="submit"
                  variant="primary"
                >
                  <Save size={15} />
                  {text("Save")}
                </Button>
              </ActionRow>
            ) : confirmDeleteFact ? (
              <InlineConfirm
                            verb="remove"
                cancelLabel={text("Cancel")}
                confirmLabel={text("Remove")}
                onCancel={() => setConfirmDeleteFact(false)}
                onConfirm={() => {
                  void deleteFinancialFact(selectedFact.id);
                  setConfirmDeleteFact(false);
                }}
              >
                {text("Remove this fact?")}
              </InlineConfirm>
            ) : (
              <ActionRow className="fact-detail-actions">
                <Button
                  className="compact-button"
                  onClick={() => setConfirmDeleteFact(true)}
                  variant="danger"
                >
                  <Trash2 size={15} />
                  {text("Remove")}
                </Button>
                <Button className="compact-button" onClick={startEditingFinancialFact}>
                  <Pencil size={15} />
                  {text("Edit")}
                </Button>
                <Button className="compact-button" onClick={cancelEditingFinancialFact}>
                  {text("Close")}
                </Button>
              </ActionRow>
            )
          ) : null
        }
      >
        {selectedFact && selectedFactDefinition && selectedFactPeriod ? (
          isFinancialFactEditMode ? (
            <form id="fact-edit-form" className="fact-form-grid" onSubmit={saveFinancialFact}>
              <TextField
                label={text("Value")}
                aria-label={text("Numeric value")}
                type="number"
                step="any"
                value={financialFactForm.valueNumeric}
                onChange={(event) => updateFinancialFactForm("valueNumeric", event.target.value)}
              />
              <TextField
                label={text("Currency")}
                aria-label={text("Currency")}
                value={financialFactForm.currency}
                onChange={(event) => updateFinancialFactForm("currency", event.target.value)}
                placeholder="USD"
              />
              <TextField
                label={text("Annotation")}
                aria-label={text("Annotation")}
                value={financialFactForm.annotation}
                onChange={(event) => updateFinancialFactForm("annotation", event.target.value)}
                placeholder={text("One-off event note")}
              />
            </form>
          ) : (
            <>
              <div className="fact-modal-value">
                <span className="fact-modal-value-big">
                  {formatFinancialValue(
                    {
                      valueNumeric: selectedFact.valueNumeric,
                      currency: selectedFact.currency,
                      asReportedValue: selectedFact.asReportedValue,
                      asReportedScale: selectedFact.asReportedScale,
                      valueKind: selectedFactDefinition.valueKind,
                      unit: selectedFactDefinition.unit,
                    },
                    locale,
                  )}
                </span>
                {selectedFact.asReportedValue ? (
                  <span className="fact-modal-as-reported">
                    {text("As reported")}: {selectedFact.asReportedValue}
                    {selectedFact.asReportedScale ? ` (${selectedFact.asReportedScale})` : ""}
                  </span>
                ) : null}
              </div>
              <div className="fact-modal-chips">
                <StatusChip tone={factQualityTone(selectedFact.dataQuality)}>
                  {factQualityLabel(selectedFact.dataQuality, text)}
                </StatusChip>
                {selectedProvenance ? (
                  <StatusChip tone="accent">{tierLabel(selectedProvenance.sourceTier, text)}</StatusChip>
                ) : null}
                {selectedProvenance ? (
                  <StatusChip tone={validationTone(selectedProvenance.validationStatus)}>
                    {validationLabel(selectedProvenance.validationStatus)}
                  </StatusChip>
                ) : null}
              </div>
              <InfoGrid
                className="fact-detail-grid"
                items={[
                  {
                    label: text("Basis"),
                    value: `${selectedFact.statementBasis} · ${selectedFact.attribution} · ${selectedFact.variant}`,
                  },
                  { label: text("Extraction method"), value: selectedFact.extractionMethod },
                  { label: text("Created"), value: selectedFact.createdAt },
                  { label: text("Updated"), value: selectedFact.updatedAt },
                  ...(selectedFact.annotation
                    ? [{ label: text("Annotation"), value: `* ${selectedFact.annotation}` }]
                    : []),
                  ...(selectedFact.supersedesId
                    ? [
                        {
                          label: text("Supersession"),
                          value: `${text("Supersedes")} ${factQualityLabel(
                            financialFacts.find((f) => f.id === selectedFact.supersedesId)
                              ?.dataQuality ?? "preliminary",
                            text,
                          ).toLowerCase()}`,
                        },
                      ]
                    : selectedFact.dataQuality !== "final"
                      ? [{ label: text("Supersession"), value: text("Awaiting the final report") }]
                      : []),
                ]}
              />
              {selectedProvenance?.citation ? (
                <div className="fact-citation">
                  <span className="fact-citation-label">
                    {text("Source citation")}
                    {selectedFact.sourceDocumentRef && documentTitleById[selectedFact.sourceDocumentRef]
                      ? ` · ${documentTitleById[selectedFact.sourceDocumentRef]}`
                      : ""}
                  </span>
                  <span>{selectedProvenance.citation}</span>
                </div>
              ) : null}
              {selectedFactRow && chartPointsFor(selectedFactRow).length > 1 ? (
                <div className="fact-detail-chart">
                  <span className="eyebrow">
                    {localizedKpiLabel(selectedFactDefinition, locale)} {text("by period")}
                  </span>
                  <TrendChart
                    ariaLabel={`${localizedKpiLabel(selectedFactDefinition, locale)} ${text("by period")}`}
                    points={chartPointsFor(selectedFactRow)}
                    formatValue={(value) =>
                      formatFinancialValue(
                        {
                          valueNumeric: String(value),
                          currency: selectedFact.currency,
                          valueKind: selectedFactDefinition.valueKind,
                          unit: selectedFactDefinition.unit,
                        },
                        locale,
                      )
                    }
                  />
                </div>
              ) : null}
              {selectedDrift ? (
                <div role="group" className="fact-detail-drift" aria-label={text("Structure changed")}>
                  <SectionHeader
                    level="h4"
                    title={text("Structure changed")}
                    description={text(
                      "The report layout differs from the confirmed profile — verify before trusting this value.",
                    )}
                  />
                  <DriftDiff drift={selectedDrift} />
                </div>
              ) : null}
            </>
          )
        ) : null}
      </Modal>

      {/* Periods × deltas (v0.61 §A5, storyboard surface 2): the N=1 case of the
          comparison read model, complementing the deltas-free matrix above with
          QoQ/YoY per period. Only meaningful once the company has a KPI set. */}
      {comparisonMetricKeys.length > 0 ? (
        <FundamentalsPeriodsSection
          companyId={companyId}
          metricKeys={comparisonMetricKeys}
          kpiLabelByMetricKey={kpiLabelByMetricKey}
          selectedFactId={selectedFinancialFactId}
          onSelectFact={selectFinancialFact}
        />
      ) : null}

      {/* Dogfooding wave 2026-09 (#4): the Reporting periods read-only list
          (restated the matrix headers) and the in-panel Autopilot fold both
          retire — Companies → Manage settings is the only autopilot editor
          now, including the one-company case (ADR 0056 amendment). */}

      <CustomKpiManager companyId={companyId} onDefinitionsChange={setCompanyDefinitions} />

      {/* Reporting forms (U7-A density row): create-period + add-fact side-by-side
          at L, one column at M/S, folded behind a disclosure when the pane is
          short (only the matrix + section headers stay visible). Extracted to
          FundamentalsDraftForms (file-size ratchet, ADR 0103). */}
      <FundamentalsDraftForms
        financialPeriods={financialPeriods}
        matrixPeriods={factMatrix.periods}
        allDefinitions={allDefinitions}
        fundamentalsForm={fundamentalsForm}
        financialFactForm={financialFactForm}
        createFinancialPeriod={createFinancialPeriod}
        saveFinancialFact={saveFinancialFact}
        updateFundamentalsForm={updateFundamentalsForm}
        updateFinancialFactForm={updateFinancialFactForm}
      />
    </div>
  );
}
