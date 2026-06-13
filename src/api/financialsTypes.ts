// ============================================================================
// Data Structures (DTOs from storage layer)
// ============================================================================

export type FinancialPeriod = {
  id: string;
  companyId: string;
  fiscalYear: number;
  periodType: string;
  periodEndDate: string | null;
  reportEvidenceRef: string | null;
  createdAt: string;
  updatedAt: string;
};

export type KpiDefinition = {
  id: string;
  scope: string;
  companyId: string | null;
  sector: string | null;
  metricKey: string;
  label: string;
  valueKind: string;
  unit: string | null;
  computation: string;
  formula: string | null;
  displayFormat: string | null;
  createdAt: string;
  updatedAt: string;
};

export type KpiRelevance = {
  id: string;
  companyId: string;
  definitionId: string;
  status: string;
  source: string;
  rank: string | null;
  firstSeenPeriod: string | null;
  lastSeenPeriod: string | null;
  createdAt: string;
  updatedAt: string;
};

export type FinancialFact = {
  id: string;
  companyId: string;
  periodId: string;
  definitionId: string;
  valueNumeric: string;
  currency: string | null;
  statementBasis: string;
  attribution: string;
  variant: string;
  measureWindow: string;
  dataQuality: string;
  asReportedValue: string | null;
  asReportedScale: string | null;
  reportingStandard: string | null;
  extractionMethod: string;
  confidence: string | null;
  confirmationState: string;
  supersedesId: string | null;
  sourceDocumentRef: string | null;
  createdAt: string;
  updatedAt: string;
};

// ============================================================================
// Input DTOs (for mutations)
// ============================================================================

export type NewFinancialPeriod = {
  companyId: string;
  fiscalYear: number;
  periodType: string;
  periodEndDate?: string;
  reportEvidenceRef?: string;
};

export type UpdateFinancialPeriod = {
  id: string;
  periodEndDate?: string;
  reportEvidenceRef?: string;
};

export type NewKpiDefinition = {
  scope: string;
  companyId?: string;
  sector?: string;
  metricKey: string;
  label: string;
  valueKind: string;
  unit?: string;
  computation: string;
  formula?: string;
  displayFormat?: string;
};

export type NewKpiRelevance = {
  companyId: string;
  definitionId: string;
  source: string;
  rank?: string;
  firstSeenPeriod?: string;
  lastSeenPeriod?: string;
};

export type UpdateKpiRelevance = {
  id: string;
  status?: string;
  rank?: string;
  firstSeenPeriod?: string;
  lastSeenPeriod?: string;
};

export type NewFinancialFact = {
  companyId: string;
  periodId: string;
  definitionId: string;
  valueNumeric: string;
  currency?: string;
  statementBasis?: string;
  attribution?: string;
  variant?: string;
  measureWindow?: string;
  dataQuality?: string;
  asReportedValue?: string;
  asReportedScale?: string;
  reportingStandard?: string;
  extractionMethod?: string;
  confidence?: string;
  confirmationState?: string;
  supersedesId?: string;
  sourceDocumentRef?: string;
};

export type UpdateFinancialFact = {
  id: string;
  valueNumeric?: string;
  currency?: string;
  dataQuality?: string;
  confirmationState?: string;
  supersedesId?: string;
  sourceDocumentRef?: string;
};

// ============================================================================
// Query Input DTOs
// ============================================================================

export type ListKpiDefinitionsInput = {
  scope?: string;
  sector?: string;
  companyId?: string;
};

export type ListFinancialPeriodsInput = {
  companyId: string;
  fiscalYear?: number;
};

export type ListFinancialFactsInput = {
  companyId?: string;
  periodId?: string;
  definitionId?: string;
};
