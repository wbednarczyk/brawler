import { callCommand } from "./tauri";
import type {
  FinancialFact,
  FinancialPeriod,
  KpiDefinition,
  KpiRelevance,
  ListFinancialFactsInput,
  ListFinancialPeriodsInput,
  ListKpiDefinitionsInput,
  NewFinancialFact,
  NewFinancialPeriod,
  NewKpiDefinition,
  NewKpiRelevance,
  UpdateFinancialFact,
  UpdateFinancialPeriod,
  UpdateKpiRelevance,
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

export function updateFinancialPeriod(input: UpdateFinancialPeriod) {
  return callCommand<FinancialPeriod>("update_financial_period", { input });
}

export function deleteFinancialPeriod(id: string) {
  return callCommand<void>("delete_financial_period", { id });
}

// ============================================================================
// KPI Relevance Commands
// ============================================================================

export function listKpiRelevance(companyId: string) {
  return callCommand<KpiRelevance[]>("list_kpi_relevance", { companyId });
}

export function createKpiRelevance(input: NewKpiRelevance) {
  return callCommand<KpiRelevance>("create_kpi_relevance", { input });
}

export function updateKpiRelevance(input: UpdateKpiRelevance) {
  return callCommand<KpiRelevance>("update_kpi_relevance", { input });
}

export function deleteKpiRelevance(id: string) {
  return callCommand<void>("delete_kpi_relevance", { id });
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
