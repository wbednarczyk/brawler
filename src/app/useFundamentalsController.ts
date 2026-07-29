import type { Dispatch, SetStateAction } from "react";
import * as financialsApi from "../api/financials";
import type {
  FinancialFact,
  FinancialPeriod,
  KpiDefinition,
} from "../api/financialsTypes";

export type FundamentalsForm = {
  periodFiscalYear: string;
  periodType: string;
};

export type FinancialFactForm = {
  definitionId: string;
  valueNumeric: string;
  currency: string;
  periodId: string;
  // One-off note (#156): rendered as a '*' marker next to the value. Empty
  // string on save clears a stored annotation.
  annotation: string;
};

type FundamentalsControllerInput = {
  companyId: string;
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
  fundamentalsForm: FundamentalsForm;
  setFundamentalsForm: Dispatch<SetStateAction<FundamentalsForm>>;
  financialFactForm: FinancialFactForm;
  setFinancialFactForm: Dispatch<SetStateAction<FinancialFactForm>>;
  selectedFinancialFactId: string | null;
  setSelectedFinancialFactId: Dispatch<SetStateAction<string | null>>;
  isFinancialFactEditMode: boolean;
  setIsFinancialFactEditMode: Dispatch<SetStateAction<boolean>>;
  fundamentalsError: string | null;
  setFundamentalsError: Dispatch<SetStateAction<string | null>>;
  refreshFinancialPeriods: () => Promise<void>;
  refreshFinancialFacts: () => Promise<void>;
  refreshKpiDefinitions: () => Promise<void>;
  text: (value: string) => string;
};

export function useFundamentalsController({
  companyId,
  financialPeriods,
  financialFacts,
  fundamentalsForm,
  setFundamentalsForm,
  financialFactForm,
  setFinancialFactForm,
  selectedFinancialFactId,
  setSelectedFinancialFactId,
  isFinancialFactEditMode,
  setIsFinancialFactEditMode,
  setFundamentalsError,
  refreshFinancialPeriods,
  refreshFinancialFacts,
  text,
}: FundamentalsControllerInput) {
  async function createFinancialPeriod(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      setFundamentalsError(null);
      const fiscalYear = parseInt(fundamentalsForm.periodFiscalYear, 10);
      if (Number.isNaN(fiscalYear)) {
        setFundamentalsError(text("Invalid fiscal year"));
        return;
      }

      await financialsApi.createFinancialPeriod({
        companyId,
        fiscalYear,
        periodType: fundamentalsForm.periodType,
      });

      setFundamentalsForm({ periodFiscalYear: "", periodType: "annual" });
      await refreshFinancialPeriods();
    } catch (error) {
      setFundamentalsError(
        error instanceof Error ? error.message : text("Failed to create financial period")
      );
    }
  }

  async function saveFinancialFact(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      setFundamentalsError(null);
      const valueNumeric = parseFloat(financialFactForm.valueNumeric);
      if (Number.isNaN(valueNumeric)) {
        setFundamentalsError(text("Invalid numeric value"));
        return;
      }

      if (isFinancialFactEditMode && selectedFinancialFactId) {
        await financialsApi.updateFinancialFact({
          id: selectedFinancialFactId,
          valueNumeric: financialFactForm.valueNumeric,
          currency: financialFactForm.currency || undefined,
          dataQuality: "final",
          confirmationState: "confirmed",
          // Always sent: the backend treats "" as "clear the annotation".
          annotation: financialFactForm.annotation.trim(),
        });
      } else {
        const selectedPeriod = financialPeriods.find((p) => p.id === financialFactForm.periodId);
        if (!selectedPeriod) {
          setFundamentalsError(text("Please select a reporting period"));
          return;
        }

        await financialsApi.createFinancialFact({
          companyId,
          periodId: selectedPeriod.id,
          definitionId: financialFactForm.definitionId,
          valueNumeric: financialFactForm.valueNumeric,
          currency: financialFactForm.currency || undefined,
          statementBasis: "consolidated",
          attribution: "total",
          variant: "reported",
          measureWindow: "flow",
          dataQuality: "final",
          extractionMethod: "manual",
          confirmationState: "confirmed",
          annotation: financialFactForm.annotation.trim() || undefined,
        });
      }

      setFinancialFactForm({
        definitionId: "",
        valueNumeric: "",
        currency: "",
        periodId: "",
        annotation: "",
      });
      setSelectedFinancialFactId(null);
      setIsFinancialFactEditMode(false);
      await refreshFinancialFacts();
    } catch (error) {
      setFundamentalsError(
        error instanceof Error ? error.message : text("Failed to save financial fact")
      );
    }
  }

  async function deleteFinancialFact(id: string) {
    try {
      setFundamentalsError(null);
      await financialsApi.deleteFinancialFact(id);
      setSelectedFinancialFactId(null);
      setIsFinancialFactEditMode(false);
      await refreshFinancialFacts();
    } catch (error) {
      setFundamentalsError(
        error instanceof Error ? error.message : text("Failed to delete financial fact")
      );
    }
  }

  function selectFinancialFact(factId: string) {
    const fact = financialFacts.find((f) => f.id === factId);
    if (fact) {
      setSelectedFinancialFactId(factId);
      setIsFinancialFactEditMode(false);
      setFinancialFactForm({
        definitionId: fact.definitionId,
        valueNumeric: fact.valueNumeric,
        currency: fact.currency || "",
        periodId: fact.periodId,
        annotation: fact.annotation || "",
      });
    }
  }

  function startEditingFinancialFact() {
    setIsFinancialFactEditMode(true);
  }

  function cancelEditingFinancialFact() {
    setIsFinancialFactEditMode(false);
    setSelectedFinancialFactId(null);
    setFinancialFactForm({
      definitionId: "",
      valueNumeric: "",
      currency: "",
      periodId: "",
      annotation: "",
    });
  }

  function updateFundamentalsForm(field: keyof FundamentalsForm, value: string) {
    setFundamentalsForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function updateFinancialFactForm(field: keyof FinancialFactForm, value: string) {
    setFinancialFactForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  return {
    createFinancialPeriod,
    saveFinancialFact,
    deleteFinancialFact,
    selectFinancialFact,
    startEditingFinancialFact,
    cancelEditingFinancialFact,
    updateFundamentalsForm,
    updateFinancialFactForm,
  };
}
