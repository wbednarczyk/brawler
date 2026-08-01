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
  // ADR 0093 dec. 2: `final | preliminary | estimated`. Defaults to "final"
  // for a NEW fact (the manual-entry path stays highest on the trust ladder).
  // Editing an EXISTING fact must resend its own current value, never a
  // hardcoded "final" — the backend treats `data_quality` as an immutable
  // uniqueness-slot dimension on update and rejects any requested CHANGE with
  // a typed `FinancialFactDataQualityLocked` conflict (a resend of the
  // current value is a no-op it passes through). `selectFinancialFact`
  // populates this from the selected fact so a save always resends the row's
  // own quality.
  dataQuality: string;
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
          // The row's OWN existing quality (ADR 0093 dec. 2), never a
          // hardcoded "final" — see the `FinancialFactForm.dataQuality` doc
          // comment. A resend of the current value is a no-op the backend
          // passes through; a hardcoded literal would attempt an illegal
          // flip on every edit of a preliminary/estimated fact.
          dataQuality: financialFactForm.dataQuality,
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
          dataQuality: financialFactForm.dataQuality,
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
        dataQuality: "final",
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
        // Preserved verbatim so a save resends the row's own quality — see
        // the `FinancialFactForm.dataQuality` doc comment (ADR 0093 dec. 2).
        dataQuality: fact.dataQuality,
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
      dataQuality: "final",
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
