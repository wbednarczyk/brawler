import { useEffect, useMemo, useState } from "react";
import { ChevronRight, Pencil, Plus, Save, Trash2, X } from "lucide-react";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";
import { useLocale, type LocaleCode } from "../../shared/locale";
import { localizedKpiLabel } from "../../shared/locale/kpiLabels";
import { FACT_FORMS, pluralNoun, type PluralForms } from "../../shared/locale/plural";
import { formatFinancialValue } from "../../shared/format/financialValue";
import { buildFactMatrix } from "./factMatrix";
import { FundamentalsPeriodsSection } from "./FundamentalsPeriodsSection";
import { CompanyAutopilotField } from "../../shared/components/CompanyAutopilotField";
import { CustomKpiManager } from "../../shared/components/CustomKpiManager";
import { DriftDiff, parseDrift } from "../../shared/components/DriftDiff";
import { ActionRow, Button, EmptyState, ErrorText, InfoGrid, InlineConfirm, SectionHeader, SelectField, Sparkline, StatusChip, TextField, TrendChart } from "../../ui";
import { listFactProvenance, type FactProvenance } from "../../api/fundamentalsExtraction";
import { getPriceContext } from "../../api/marketData";
import type { PriceContext } from "../../api/marketData";
import { getAnalystRecommendations } from "../../api/analystRecommendations";
import type { AnalystRecommendationTarget } from "../../api/analystRecommendations";
import { PriceContextSection } from "./PriceContextSection";
import type { FactMatrixRow } from "./factMatrix";
import type {
  FinancialFactForm,
  FundamentalsForm,
} from "../../app/useFundamentalsController";

type FundamentalsPanelProps = {
  companyId: string;
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
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

export function tierLabel(tier: string, text: (value: string) => string): string {
  switch (tier) {
    case "esef":
      return text("ESEF (tagged)");
    case "structured_xhtml":
      return text("Structured HTML");
    case "espi_cover_note":
      return text("ESPI cover note");
    case "pdf":
      return text("Structured read (xHTML)");
    case "agent":
      return text("Agent (MCP)");
    case "html_aggregator":
      return text("Aggregator");
    case "ai_text":
    case "ai":
      return text("AI");
    default:
      return tier;
  }
}

export function FundamentalsPanel({
  companyId,
  financialPeriods,
  financialFacts,
  kpiDefinitions,
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
  const [kpiQuery, setKpiQuery] = useState("");
  const [confirmDeleteFact, setConfirmDeleteFact] = useState(false);
  // U7-A density disclosures (ADR 0076 D6). Collapsed by default so the CSS tier
  // switch only reveals them where the contract folds content: the Autopilot
  // summary row at S, the reporting forms when the pane is short. At tall/wide
  // tiers the container queries ignore the collapsed state and show them inline.
  const [autopilotExpanded, setAutopilotExpanded] = useState(false);
  const [formsExpanded, setFormsExpanded] = useState(false);

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
          financial-facts matrix second, everything else (periods, autopilot,
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
      <section className="fundamentals-section" aria-label={text("Financial facts")}>
        <SectionHeader level="h4" title={text("Financial facts")} />

        <div className="fundamentals-workspace">
          {factMatrix.rows.length > 0 ? (
            /* The KPI × period matrix is DELIBERATE wide content: it scrolls inside
               this bounded wrapper (data-hscroll exempts it from the layout gate). */
            <div className="facts-matrix-scroll" data-hscroll aria-label={text("Financial facts matrix")}>
              <table className="facts-matrix">
                <thead>
                  <tr>
                    <th className="facts-matrix-corner" scope="col">
                      {text("KPI")}
                    </th>
                    {factMatrix.periods.map((period) => (
                      <th key={period.id} scope="col">
                        {period.fiscalYear} {period.periodType.toUpperCase()}
                      </th>
                    ))}
                    <th className="facts-matrix-trend-head" scope="col">
                      {text("Trend")}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {factMatrix.rows.map((row) => (
                    <tr key={row.definition.id}>
                      <th className="facts-matrix-kpi" scope="row">
                        {localizedKpiLabel(row.definition, locale)}
                      </th>
                      {factMatrix.periods.map((period) => {
                        const fact = row.cells[period.id];
                        if (!fact) {
                          return (
                            <td key={period.id} className="facts-matrix-cell-empty">
                              <span aria-hidden="true">—</span>
                            </td>
                          );
                        }
                        return (
                          <td key={period.id}>
                            <button
                              aria-label={`${localizedKpiLabel(row.definition, locale)}, ${period.fiscalYear} ${period.periodType.toUpperCase()}`}
                              className={[
                                "facts-matrix-cell",
                                selectedFinancialFactId === fact.id ? "facts-matrix-cell-selected" : "",
                              ]
                                .filter(Boolean)
                                .join(" ")}
                              onClick={() => selectFinancialFact(fact.id)}
                              type="button"
                            >
                              {formatFinancialValue(
                                {
                                  valueNumeric: fact.valueNumeric,
                                  currency: fact.currency,
                                  asReportedValue: fact.asReportedValue,
                                  asReportedScale: fact.asReportedScale,
                                  valueKind: row.definition.valueKind,
                                  unit: row.definition.unit,
                                  metricKey: row.definition.metricKey,
                                },
                                locale,
                              )}
                              {fact.annotation ? (
                                <span
                                  className="fact-annotation-marker"
                                  title={fact.annotation}
                                  aria-label={`${text("Annotation")}: ${fact.annotation}`}
                                >
                                  *
                                </span>
                              ) : null}
                            </button>
                          </td>
                        );
                      })}
                      <td className="facts-matrix-trend">
                        <Sparkline
                          values={seriesValuesFor(row)}
                          ariaLabel={`${localizedKpiLabel(row.definition, locale)} ${text("trend")}`}
                        />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <EmptyState>{text("No financial facts yet.")}</EmptyState>
          )}

          {/* Financial Fact Detail and Edit */}
          {selectedFact && selectedFactDefinition && selectedFactPeriod ? (
            <form
              className="fact-detail"
              aria-label={text("Financial fact detail")}
              onSubmit={saveFinancialFact}
            >
              {isFinancialFactEditMode ? (
                <>
                  <div className="fact-detail-header">
                    <div>
                      <span className="eyebrow">{text("Editing fact")}</span>
                      <h3>{localizedKpiLabel(selectedFactDefinition, locale)}</h3>
                    </div>
                    <ActionRow className="fact-detail-actions">
                      <Button
                        className="compact-button"
                        onClick={() => deleteFinancialFact(selectedFact.id)}
                        variant="danger"
                      >
                        <Trash2 size={15} />
                        {text("Delete")}
                      </Button>
                      <Button
                        className="compact-button"
                        onClick={cancelEditingFinancialFact}
                      >
                        <X size={15} />
                        {text("Cancel")}
                      </Button>
                      <Button
                        className="compact-button"
                        type="submit"
                        variant="primary"
                      >
                        <Save size={15} />
                        {text("Save")}
                      </Button>
                    </ActionRow>
                  </div>
                  <div className="fact-form-grid">
                    <TextField
                      label={text("Value")}
                      aria-label={text("Numeric value")}
                      type="number"
                      step="any"
                      value={financialFactForm.valueNumeric}
                      onChange={(event) =>
                        updateFinancialFactForm("valueNumeric", event.target.value)
                      }
                    />
                    <TextField
                      label={text("Currency")}
                      aria-label={text("Currency")}
                      value={financialFactForm.currency}
                      onChange={(event) =>
                        updateFinancialFactForm("currency", event.target.value)
                      }
                      placeholder="USD"
                    />
                    <TextField
                      label={text("Annotation")}
                      aria-label={text("Annotation")}
                      value={financialFactForm.annotation}
                      onChange={(event) =>
                        updateFinancialFactForm("annotation", event.target.value)
                      }
                      placeholder={text("One-off event note")}
                    />
                  </div>
                </>
              ) : (
                <>
                  <div className="fact-detail-header">
                    <div>
                      <span className="eyebrow">{text("Financial fact")}</span>
                      <h3>{localizedKpiLabel(selectedFactDefinition, locale)}</h3>
                    </div>
                    {confirmDeleteFact ? (
                      <InlineConfirm
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
                        <Button className="compact-button" onClick={startEditingFinancialFact}>
                          <Pencil size={15} />
                          {text("Edit")}
                        </Button>
                        <Button
                          className="compact-button"
                          onClick={() => setConfirmDeleteFact(true)}
                          variant="danger"
                        >
                          <Trash2 size={15} />
                          {text("Remove")}
                        </Button>
                      </ActionRow>
                    )}
                  </div>
                  <InfoGrid
                    className="fact-detail-grid"
                    items={[
                      {
                        label: text("Period"),
                        value: `${selectedFactPeriod.fiscalYear} ${selectedFactPeriod.periodType.toUpperCase()}`,
                      },
                      {
                        label: text("Value"),
                        value: formatFinancialValue(
                          {
                            valueNumeric: selectedFact.valueNumeric,
                            currency: selectedFact.currency,
                            asReportedValue: selectedFact.asReportedValue,
                            asReportedScale: selectedFact.asReportedScale,
                            valueKind: selectedFactDefinition.valueKind,
                            unit: selectedFactDefinition.unit,
                          },
                          locale,
                        ),
                      },
                      {
                        label: text("As stored"),
                        value: `${selectedFact.valueNumeric}${selectedFact.currency ? ` ${selectedFact.currency}` : ""}`,
                      },
                      ...(selectedFact.annotation
                        ? [
                            {
                              label: text("Annotation"),
                              value: `* ${selectedFact.annotation}`,
                            },
                          ]
                        : []),
                      {
                        label: text("Currency"),
                        value: selectedFact.currency || text("Not set"),
                      },
                      {
                        label: text("Statement basis"),
                        value: selectedFact.statementBasis,
                      },
                      { label: text("Attribution"), value: selectedFact.attribution },
                      { label: text("Variant"), value: selectedFact.variant },
                      {
                        label: text("Data quality"),
                        value: selectedFact.dataQuality,
                      },
                      // Facts are review-free (ADR 0086 dec. 5): `confirmationState`
                      // is a frozen compatibility column, always `confirmed` — it is
                      // not a fact's origin. Origin lives in the Source (tier) +
                      // Validation labels below, so the meaningless row is dropped.
                      ...(selectedProvenance
                        ? [
                            {
                              label: text("Source"),
                              value: (
                                <StatusChip tone="accent">
                                  {tierLabel(selectedProvenance.sourceTier, text)}
                                </StatusChip>
                              ),
                              valueAriaLabel: `${text("Source")}: ${tierLabel(selectedProvenance.sourceTier, text)}`,
                            },
                            {
                              label: text("Validation"),
                              value: (
                                <StatusChip tone={validationTone(selectedProvenance.validationStatus)}>
                                  {validationLabel(selectedProvenance.validationStatus)}
                                </StatusChip>
                              ),
                              valueAriaLabel: `${text("Validation")}: ${validationLabel(selectedProvenance.validationStatus)}`,
                            },
                          ]
                        : []),
                    ]}
                  />
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
                    <section
                      className="fact-detail-drift"
                      aria-label={text("Structure changed")}
                    >
                      <SectionHeader
                        level="h4"
                        title={text("Structure changed")}
                        description={text(
                          "The report layout differs from the confirmed profile — verify before trusting this value.",
                        )}
                      />
                      <DriftDiff drift={selectedDrift} />
                    </section>
                  ) : null}
                </>
              )}
            </form>
          ) : (
            <EmptyState>{text("Select a fact to inspect it.")}</EmptyState>
          )}
        </div>
      </section>

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

      {/* Financial Periods List */}
      <section className="fundamentals-section" aria-label={text("Reporting periods")}>
        <SectionHeader level="h4" title={text("Reporting periods")} />
        {financialPeriods.length > 0 ? (
          <div className="periods-list">
            {financialPeriods.map((period) => (
              <div key={period.id} className="period-item">
                <span className="period-label">
                  {period.fiscalYear} {period.periodType.toUpperCase()}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <EmptyState>{text("No reporting periods yet.")}</EmptyState>
        )}
      </section>

      {/* Autopilot section (U7-A): folds to a summary row + expand at the S tier
          (container query), inline field at M/L. */}
      <div className={`fundamentals-autopilot${autopilotExpanded ? " is-expanded" : ""}`}>
        <button
          type="button"
          className="fundamentals-autopilot-toggle"
          aria-expanded={autopilotExpanded}
          onClick={() => setAutopilotExpanded((value) => !value)}
        >
          <span aria-hidden="true" className="fundamentals-autopilot-chevron">
            <ChevronRight size={15} />
          </span>
          {text("Autopilot")}
        </button>
        <div className="fundamentals-autopilot-body">
          <CompanyAutopilotField companyId={companyId} />
        </div>
      </div>

      <CustomKpiManager companyId={companyId} onDefinitionsChange={setCompanyDefinitions} />

      {/* Reporting forms (U7-A density row): create-period + add-fact side-by-side
          at L, one column at M/S, folded behind a disclosure when the pane is
          short (only the matrix + section headers stay visible). */}
      <div className={`fundamentals-forms${formsExpanded ? " is-expanded" : ""}`}>
        <button
          type="button"
          className="fundamentals-forms-toggle"
          aria-expanded={formsExpanded}
          onClick={() => setFormsExpanded((value) => !value)}
        >
          <span aria-hidden="true" className="fundamentals-forms-chevron">
            <ChevronRight size={15} />
          </span>
          {text("Reporting forms")}
        </button>
        <div className="fundamentals-forms-grid">
          <section className="fundamentals-section" aria-label={text("Create reporting period")}>
            <SectionHeader level="h4" title={text("New reporting period")} />
            <form className="fundamentals-form" onSubmit={createFinancialPeriod}>
              <div className="fundamentals-form-row">
                <TextField
                  label={text("Fiscal year")}
                  aria-label={text("Fiscal year")}
                  type="number"
                  min="1900"
                  max="2100"
                  value={fundamentalsForm.periodFiscalYear}
                  onChange={(event) =>
                    updateFundamentalsForm("periodFiscalYear", event.target.value)
                  }
                  placeholder="2024"
                />
                <SelectField
                  label={text("Period type")}
                  aria-label={text("Period type")}
                  value={fundamentalsForm.periodType}
                  onChange={(event) =>
                    updateFundamentalsForm("periodType", event.target.value)
                  }
                >
                  <option value="annual">{text("Annual")}</option>
                  <option value="q1">{text("Q1")}</option>
                  <option value="q2">{text("Q2")}</option>
                  <option value="q3">{text("Q3")}</option>
                  <option value="q4">{text("Q4")}</option>
                </SelectField>
                <Button
                  className="compact-button"
                  disabled={!fundamentalsForm.periodFiscalYear.trim()}
                  type="submit"
                  variant="primary"
                >
                  <Plus size={15} />
                  {text("Create")}
                </Button>
              </div>
            </form>
          </section>

          {financialPeriods.length > 0 ? (
            <section className="fundamentals-section" aria-label={text("Add financial fact")}>
              <SectionHeader level="h4" title={text("Add financial fact")} />
          <form
            className="fundamentals-form"
            onSubmit={async (event) => {
              await saveFinancialFact(event);
              setKpiQuery("");
            }}
          >
            <div className="fundamentals-form-grid">
              <TextField
                label={text("KPI definition")}
                aria-label={text("KPI definition")}
                list="kpi-definition-options"
                onChange={(event) => {
                  const query = event.target.value;
                  setKpiQuery(query);
                  const match = allDefinitions.find(
                    (definition) =>
                      localizedKpiLabel(definition, locale) === query ||
                      definition.metricKey === query,
                  );
                  updateFinancialFactForm("definitionId", match?.id ?? "");
                }}
                placeholder={text("Search a KPI…")}
                value={kpiQuery}
              />
              <datalist id="kpi-definition-options">
                {allDefinitions.map((definition) => (
                  <option key={definition.id} value={localizedKpiLabel(definition, locale)} />
                ))}
              </datalist>
              <SelectField
                label={text("Reporting period")}
                aria-label={text("Reporting period")}
                value={financialFactForm.periodId}
                onChange={(event) => updateFinancialFactForm("periodId", event.target.value)}
              >
                <option value="">{text("Select a period")}</option>
                {factMatrix.periods.map((period) => (
                  <option key={period.id} value={period.id}>
                    {period.fiscalYear} {period.periodType.toUpperCase()}
                  </option>
                ))}
              </SelectField>
              <TextField
                label={text("Value")}
                aria-label={text("Numeric value")}
                type="number"
                step="any"
                value={financialFactForm.valueNumeric}
                onChange={(event) =>
                  updateFinancialFactForm("valueNumeric", event.target.value)
                }
                placeholder="0"
              />
              <TextField
                label={text("Currency")}
                aria-label={text("Currency")}
                value={financialFactForm.currency}
                onChange={(event) =>
                  updateFinancialFactForm("currency", event.target.value)
                }
                placeholder="USD"
              />
              <Button
                className="compact-button"
                disabled={
                  !financialFactForm.definitionId ||
                  !financialFactForm.periodId ||
                  !financialFactForm.valueNumeric
                }
                type="submit"
                variant="primary"
              >
                <Plus size={15} />
                {text("Add fact")}
              </Button>
            </div>
              </form>
            </section>
          ) : null}
        </div>
      </div>
    </div>
  );
}
