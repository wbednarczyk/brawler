import { callCommand } from "./tauri";
import type {
  FinancialFact,
  FinancialPeriod,
  KpiDefinition,
  ListFinancialFactsInput,
  ListFinancialPeriodsInput,
  ListKpiDefinitionsInput,
  NewFinancialFact,
  NewFinancialPeriod,
  NewKpiDefinition,
  UpdateFinancialFact,
} from "./financialsTypes";

// ============================================================================
// KPI Definition Commands
// ============================================================================

export function listKpiDefinitions(input: ListKpiDefinitionsInput) {
  return callCommand<KpiDefinition[]>("list_kpi_definitions", { input });
}

export function createKpiDefinition(input: NewKpiDefinition) {
  return callCommand<KpiDefinition>("create_kpi_definition", { input });
}

// ============================================================================
// Financial Period Commands
// ============================================================================

export function listFinancialPeriods(input: ListFinancialPeriodsInput) {
  return callCommand<FinancialPeriod[]>("list_financial_periods", { input });
}

export function createFinancialPeriod(input: NewFinancialPeriod) {
  return callCommand<FinancialPeriod>("create_financial_period", { input });
}

// ============================================================================
// Financial Fact Commands
// ============================================================================

export function listFinancialFacts(input: ListFinancialFactsInput) {
  return callCommand<FinancialFact[]>("list_financial_facts", { input });
}

export function createFinancialFact(input: NewFinancialFact) {
  return callCommand<FinancialFact>("create_financial_fact", { input });
}

export function updateFinancialFact(input: UpdateFinancialFact) {
  return callCommand<FinancialFact>("update_financial_fact", { input });
}

export function deleteFinancialFact(id: string) {
  return callCommand<void>("delete_financial_fact", { id });
}
