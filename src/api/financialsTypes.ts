// ============================================================================
// Data Structures (DTOs from storage layer)
//
// Every type below is GENERATED from the Rust source via ts-rs (ADR 0048):
// change the struct in storage/financials.rs and run `make types`. Do not edit
// the shapes here — this barrel only re-exports the generated bindings.
// ============================================================================

export type { FinancialPeriod } from "./generated/FinancialPeriod";
export type { KpiDefinition } from "./generated/KpiDefinition";
export type { KpiRelevance } from "./generated/KpiRelevance";
export type { FinancialFact } from "./generated/FinancialFact";

// Input DTOs (for mutations)
export type { NewFinancialPeriod } from "./generated/NewFinancialPeriod";
export type { UpdateFinancialPeriod } from "./generated/UpdateFinancialPeriod";
export type { NewKpiDefinition } from "./generated/NewKpiDefinition";
export type { NewKpiRelevance } from "./generated/NewKpiRelevance";
export type { UpdateKpiRelevance } from "./generated/UpdateKpiRelevance";
export type { NewFinancialFact } from "./generated/NewFinancialFact";
export type { UpdateFinancialFact } from "./generated/UpdateFinancialFact";

// Query input DTOs
export type { ListKpiDefinitionsInput } from "./generated/ListKpiDefinitionsInput";
export type { ListFinancialPeriodsInput } from "./generated/ListFinancialPeriodsInput";
export type { ListFinancialFactsInput } from "./generated/ListFinancialFactsInput";
