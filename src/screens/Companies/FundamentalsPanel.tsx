import { useEffect, useMemo, useState } from "react";
import { Pencil, Plus, Save, Trash2, X } from "lucide-react";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";
import { useLocale } from "../../shared/locale";
import { localizedKpiLabel } from "../../shared/locale/kpiLabels";
import { formatFinancialValue } from "../../shared/format/financialValue";
import { buildFactMatrix } from "./factMatrix";
import { CompanyIrReportsUrlField } from "../../shared/components/CompanyIrReportsUrlField";
import { CustomKpiManager } from "../../shared/components/CustomKpiManager";
import { ActionRow, Button, EmptyState, ErrorText, InfoGrid, InlineConfirm, SectionHeader, SelectField, Sparkline, TextField, TrendChart } from "../../ui";
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
};

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
}: FundamentalsPanelProps) {
  const { text, locale } = useLocale();

  // Company-scoped custom KPI definitions are loaded by CustomKpiManager and
  // merged with the global taxonomy so they appear in the matrix and dropdown.
  const [companyDefinitions, setCompanyDefinitions] = useState<KpiDefinition[]>([]);
  const [kpiQuery, setKpiQuery] = useState("");
  const [confirmDeleteFact, setConfirmDeleteFact] = useState(false);

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

  const selectedFactRow = selectedFactDefinition
    ? factMatrix.rows.find((row) => row.definition.id === selectedFactDefinition.id)
    : undefined;

  return (
    <div className="company-tab-panel fundamentals-panel" aria-label={text("Company fundamentals")}>
      <SectionHeader
        level="h3"
        title={text("Fundamentals")}
        description={
          <>
            {financialFacts.length} {text(financialFacts.length === 1 ? "fact" : "facts")} {text("recorded")}
          </>
        }
      />

      {fundamentalsError ? (
        <ErrorText>{text("Fundamentals command failed")}: {fundamentalsError}</ErrorText>
      ) : null}
      {fundamentalsLoadError ? (
        <ErrorText>{text("Failed to load fundamentals data")}: {fundamentalsLoadError}</ErrorText>
      ) : null}

      <CompanyIrReportsUrlField companyId={companyId} />

      <CustomKpiManager companyId={companyId} onDefinitionsChange={setCompanyDefinitions} />

      {/* Create Financial Period Section */}
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

      {/* Financial Facts List and Detail */}
      <section className="fundamentals-section" aria-label={text("Financial facts")}>
        <SectionHeader level="h4" title={text("Financial facts")} />

        <div className="fundamentals-workspace">
          {factMatrix.rows.length > 0 ? (
            <div className="facts-matrix-scroll" aria-label={text("Financial facts matrix")}>
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
                                },
                                locale,
                              )}
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
                      {
                        label: text("Confirmation state"),
                        value: selectedFact.confirmationState,
                      },
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
                </>
              )}
            </form>
          ) : (
            <EmptyState>{text("Select a fact to inspect it.")}</EmptyState>
          )}
        </div>
      </section>

      {/* Add Financial Fact Section */}
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
  );
}
