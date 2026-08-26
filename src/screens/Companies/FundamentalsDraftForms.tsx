import { useEffect, useState } from "react";
import { ChevronRight, Plus } from "lucide-react";
import type { FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";
import { useLocale } from "../../shared/locale";
import { localizedKpiLabel } from "../../shared/locale/kpiLabels";
import { Button, SectionHeader, SelectField, TextField } from "../../ui";
import type {
  FinancialFactForm,
  FundamentalsForm,
} from "../../app/useFundamentalsController";
import { useToolHost } from "../../shared/toolHost";

type FundamentalsDraftFormsProps = {
  financialPeriods: FinancialPeriod[];
  matrixPeriods: FinancialPeriod[];
  allDefinitions: KpiDefinition[];
  fundamentalsForm: FundamentalsForm;
  financialFactForm: FinancialFactForm;
  createFinancialPeriod: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  saveFinancialFact: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
  updateFundamentalsForm: (field: keyof FundamentalsForm, value: string) => void;
  updateFinancialFactForm: (field: keyof FinancialFactForm, value: string) => void;
};

/**
 * The create-period / add-fact draft forms (U7-A density row), extracted
 * from FundamentalsPanel (file-size ratchet, ADR 0103): folded behind a
 * disclosure when the pane is short, side-by-side at L, one column at M/S.
 * Owns the drafts' own local state (`kpiQuery`, `formsExpanded`) and their
 * dirty-gate registration with the Spółka workshop (F3a S2/R1, ADR 0107) — a
 * no-op when hosted outside it. "At most one primary action per panel"
 * (plan §9, sol R1 finding 9): once periods exist, "Add fact" is the
 * everyday action and "Create period" demotes to secondary.
 */
export function FundamentalsDraftForms({
  financialPeriods,
  matrixPeriods,
  allDefinitions,
  fundamentalsForm,
  financialFactForm,
  createFinancialPeriod,
  saveFinancialFact,
  updateFundamentalsForm,
  updateFinancialFactForm,
}: FundamentalsDraftFormsProps) {
  const { text, locale } = useLocale();
  const [formsExpanded, setFormsExpanded] = useState(false);
  const [kpiQuery, setKpiQuery] = useState("");

  const { register } = useToolHost();
  useEffect(() => {
    return register({
      isDirty: () =>
        fundamentalsForm.periodFiscalYear.trim() !== "" ||
        fundamentalsForm.periodType !== "annual" ||
        financialFactForm.definitionId !== "" ||
        financialFactForm.valueNumeric.trim() !== "" ||
        financialFactForm.currency.trim() !== "" ||
        financialFactForm.periodId !== "" ||
        kpiQuery.trim() !== "",
      discard: () => {
        updateFundamentalsForm("periodFiscalYear", "");
        updateFundamentalsForm("periodType", "annual");
        updateFinancialFactForm("definitionId", "");
        updateFinancialFactForm("valueNumeric", "");
        updateFinancialFactForm("currency", "");
        updateFinancialFactForm("periodId", "");
        setKpiQuery("");
      },
    });
  }, [
    register,
    fundamentalsForm.periodFiscalYear,
    fundamentalsForm.periodType,
    financialFactForm.definitionId,
    financialFactForm.valueNumeric,
    financialFactForm.currency,
    financialFactForm.periodId,
    kpiQuery,
    updateFundamentalsForm,
    updateFinancialFactForm,
  ]);

  return (
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
        <div role="group" className="fundamentals-section" aria-label={text("Create reporting period")}>
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
                // At most one primary action per panel (plan §9): once
                // periods exist, "Add fact" is the everyday action and
                // "Create period" demotes to secondary.
                variant={financialPeriods.length > 0 ? "secondary" : "primary"}
              >
                <Plus size={15} />
                {text("Create")}
              </Button>
            </div>
          </form>
        </div>

        {financialPeriods.length > 0 ? (
          <div role="group" className="fundamentals-section" aria-label={text("Add financial fact")}>
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
                  {matrixPeriods.map((period) => (
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
          </div>
        ) : null}
      </div>
    </div>
  );
}
