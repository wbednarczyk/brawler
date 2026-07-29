import { useCallback, useEffect, useState } from "react";
import {
  listFinancialFacts,
  listFinancialPeriods,
  listKpiDefinitions,
} from "../../api/financials";
import type {
  FinancialFact,
  FinancialPeriod,
  KpiDefinition,
} from "../../api/financialsTypes";
import { useLocale } from "../../shared/locale";
import {
  useFundamentalsController,
  type FinancialFactForm,
  type FundamentalsForm,
} from "../../app/useFundamentalsController";

const EMPTY_FUNDAMENTALS_FORM: FundamentalsForm = { periodFiscalYear: "", periodType: "annual" };
const EMPTY_FACT_FORM: FinancialFactForm = {
  definitionId: "",
  valueNumeric: "",
  currency: "",
  periodId: "",
  annotation: "",
};

// Cockpit-native fundamentals state + actions for one company. It owns the
// periods/facts/KPI state and the two forms (the part AppStateRoot owns for the
// Companies screen), then composes the shared `useFundamentalsController`
// (decision: clean cockpit panels reuse the real editable panel via api/* —
// no AppStateRoot coupling). Lets the full, editable `FundamentalsPanel` render
// for any company a panel pins (ADR 0053 phase 4b).
// `revision` bumps when a sibling panel writes facts for this company (a
// report-documents extraction); it forces the facts/periods refetch below so the
// panel never shows stale data after a successful extraction.
export function useCockpitFundamentals(companyId: string, revision = 0) {
  const { text } = useLocale();
  const [financialPeriods, setFinancialPeriods] = useState<FinancialPeriod[]>([]);
  const [financialFacts, setFinancialFacts] = useState<FinancialFact[]>([]);
  const [kpiDefinitions, setKpiDefinitions] = useState<KpiDefinition[]>([]);
  const [fundamentalsForm, setFundamentalsForm] = useState<FundamentalsForm>(EMPTY_FUNDAMENTALS_FORM);
  const [financialFactForm, setFinancialFactForm] = useState<FinancialFactForm>(EMPTY_FACT_FORM);
  const [selectedFinancialFactId, setSelectedFinancialFactId] = useState<string | null>(null);
  const [isFinancialFactEditMode, setIsFinancialFactEditMode] = useState(false);
  const [fundamentalsError, setFundamentalsError] = useState<string | null>(null);
  const [fundamentalsLoadError, setFundamentalsLoadError] = useState<string | null>(null);

  const refreshFinancialPeriods = useCallback(async () => {
    setFinancialPeriods(await listFinancialPeriods({ companyId }));
  }, [companyId]);
  const refreshFinancialFacts = useCallback(async () => {
    setFinancialFacts(await listFinancialFacts({ companyId }));
  }, [companyId]);
  const refreshKpiDefinitions = useCallback(async () => {
    setKpiDefinitions(await listKpiDefinitions({ companyId }));
  }, [companyId]);

  // Reload everything when the pinned company changes; a load failure surfaces
  // in the panel without crashing the cockpit.
  useEffect(() => {
    let cancelled = false;
    setFundamentalsLoadError(null);
    Promise.all([
      listFinancialPeriods({ companyId }),
      listFinancialFacts({ companyId }),
      listKpiDefinitions({ companyId }),
    ])
      .then(([periods, facts, definitions]) => {
        if (cancelled) return;
        setFinancialPeriods(periods);
        setFinancialFacts(facts);
        setKpiDefinitions(definitions);
      })
      .catch((reason) => {
        if (!cancelled) {
          setFundamentalsLoadError(
            reason instanceof Error ? reason.message : text("Failed to load fundamentals"),
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, revision, text]);

  const actions = useFundamentalsController({
    companyId,
    financialPeriods,
    financialFacts,
    kpiDefinitions,
    fundamentalsForm,
    setFundamentalsForm,
    financialFactForm,
    setFinancialFactForm,
    selectedFinancialFactId,
    setSelectedFinancialFactId,
    isFinancialFactEditMode,
    setIsFinancialFactEditMode,
    fundamentalsError,
    setFundamentalsError,
    refreshFinancialPeriods,
    refreshFinancialFacts,
    refreshKpiDefinitions,
    text,
  });

  return {
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
    ...actions,
  };
}
