import { Plus, Save, Trash2, X } from "lucide-react";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";
import { useLocale } from "../../shared/locale";
import { ActionRow, Button, DenseRow, EmptyState, InfoGrid } from "../../ui";
import type {
  FinancialFactForm,
  FundamentalsForm,
} from "../../app/useFundamentalsController";

type FundamentalsPanelProps = {
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
  fundamentalsForm: FundamentalsForm;
  financialFactForm: FinancialFactForm;
  selectedFinancialFactId: string | null;
  isFinancialFactEditMode: boolean;
  fundamentalsError: string | null;
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
  financialPeriods,
  financialFacts,
  kpiDefinitions,
  fundamentalsForm,
  financialFactForm,
  selectedFinancialFactId,
  isFinancialFactEditMode,
  fundamentalsError,
  createFinancialPeriod,
  saveFinancialFact,
  deleteFinancialFact,
  selectFinancialFact,
  startEditingFinancialFact,
  cancelEditingFinancialFact,
  updateFundamentalsForm,
  updateFinancialFactForm,
}: FundamentalsPanelProps) {
  const { text } = useLocale();

  const selectedFact = selectedFinancialFactId
    ? financialFacts.find((f) => f.id === selectedFinancialFactId)
    : null;

  const selectedFactPeriod = selectedFact
    ? financialPeriods.find((p) => p.id === selectedFact.periodId)
    : null;

  const selectedFactDefinition = selectedFact
    ? kpiDefinitions.find((d) => d.id === selectedFact.definitionId)
    : null;

  return (
    <div className="company-tab-panel fundamentals-panel" aria-label={text("Company fundamentals")}>
      <div className="fundamentals-toolbar">
        <div>
          <h3>{text("Fundamentals")}</h3>
          <p>
            {financialFacts.length} {text(financialFacts.length === 1 ? "fact" : "facts")} {text("recorded")}
          </p>
        </div>
      </div>

      {fundamentalsError ? (
        <p className="error-text">{text("Fundamentals command failed")}: {fundamentalsError}</p>
      ) : null}

      {/* Create Financial Period Section */}
      <section className="fundamentals-section" aria-label={text("Create reporting period")}>
        <div className="section-heading">
          <h4>{text("New reporting period")}</h4>
        </div>
        <form className="fundamentals-form" onSubmit={createFinancialPeriod}>
          <div className="fundamentals-form-row">
            <label>
              {text("Fiscal year")}
              <input
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
            </label>
            <label>
              {text("Period type")}
              <select
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
              </select>
            </label>
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
        <div className="section-heading">
          <h4>{text("Reporting periods")}</h4>
        </div>
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
        <div className="section-heading">
          <h4>{text("Financial facts")}</h4>
        </div>

        <div className="fundamentals-workspace">
          <div className="facts-list" aria-label={text("Financial facts list")}>
            {financialFacts.length > 0 ? (
              financialFacts.map((fact) => {
                const period = financialPeriods.find((p) => p.id === fact.periodId);
                const definition = kpiDefinitions.find((d) => d.id === fact.definitionId);
                return (
                  <DenseRow
                    aria-label={`${text("Financial fact")}: ${definition?.label || fact.definitionId}`}
                    className={[
                      "fact-row",
                      selectedFinancialFactId === fact.id ? "fact-row-selected" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    key={fact.id}
                    onClick={() => selectFinancialFact(fact.id)}
                    role="button"
                    selected={selectedFinancialFactId === fact.id}
                    tabIndex={0}
                  >
                    <div className="fact-row-main">
                      <h4>{definition?.label || fact.definitionId}</h4>
                      <div className="fact-row-meta">
                        <span>{period ? `${period.fiscalYear} ${period.periodType.toUpperCase()}` : text("Unknown period")}</span>
                        <span>{fact.valueNumeric} {fact.currency || ""}</span>
                      </div>
                    </div>
                  </DenseRow>
                );
              })
            ) : (
              <EmptyState>{text("No financial facts yet.")}</EmptyState>
            )}
          </div>

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
                      <h3>{selectedFactDefinition.label}</h3>
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
                    <label>
                      {text("Value")}
                      <input
                        aria-label={text("Numeric value")}
                        type="number"
                        step="any"
                        value={financialFactForm.valueNumeric}
                        onChange={(event) =>
                          updateFinancialFactForm("valueNumeric", event.target.value)
                        }
                      />
                    </label>
                    <label>
                      {text("Currency")}
                      <input
                        aria-label={text("Currency")}
                        value={financialFactForm.currency}
                        onChange={(event) =>
                          updateFinancialFactForm("currency", event.target.value)
                        }
                        placeholder="USD"
                      />
                    </label>
                  </div>
                </>
              ) : (
                <>
                  <div className="fact-detail-header">
                    <div>
                      <span className="eyebrow">{text("Financial fact")}</span>
                      <h3>{selectedFactDefinition.label}</h3>
                    </div>
                    <Button
                      className="compact-button"
                      onClick={startEditingFinancialFact}
                    >
                      <X size={15} />
                      {text("Edit")}
                    </Button>
                  </div>
                  <InfoGrid
                    className="fact-detail-grid"
                    items={[
                      {
                        label: text("Period"),
                        value: `${selectedFactPeriod.fiscalYear} ${selectedFactPeriod.periodType.toUpperCase()}`,
                      },
                      { label: text("Value"), value: selectedFact.valueNumeric },
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
          <div className="section-heading">
            <h4>{text("Add financial fact")}</h4>
          </div>
          <form className="fundamentals-form" onSubmit={saveFinancialFact}>
            <div className="fundamentals-form-grid">
              <label>
                {text("KPI definition")}
                <select
                  aria-label={text("KPI definition")}
                  value={financialFactForm.definitionId}
                  onChange={(event) =>
                    updateFinancialFactForm("definitionId", event.target.value)
                  }
                >
                  <option value="">{text("Select a KPI")}</option>
                  {kpiDefinitions.map((definition) => (
                    <option key={definition.id} value={definition.id}>
                      {definition.label} ({definition.metricKey})
                    </option>
                  ))}
                </select>
              </label>
              <label>
                {text("Value")}
                <input
                  aria-label={text("Numeric value")}
                  type="number"
                  step="any"
                  value={financialFactForm.valueNumeric}
                  onChange={(event) =>
                    updateFinancialFactForm("valueNumeric", event.target.value)
                  }
                  placeholder="0"
                />
              </label>
              <label>
                {text("Currency")}
                <input
                  aria-label={text("Currency")}
                  value={financialFactForm.currency}
                  onChange={(event) =>
                    updateFinancialFactForm("currency", event.target.value)
                  }
                  placeholder="USD"
                />
              </label>
              <Button
                className="compact-button"
                disabled={
                  !financialFactForm.definitionId ||
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
