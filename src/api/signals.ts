import { callCommand } from "./tauri";
import type { AiSignalClassificationSummary, CompanySignal } from "./types";

export type ListCompanySignalsInput = {
  companyId: string | null;
  watchlistId: string | null;
  category: string | null;
  status: string | null;
};

export function listCompanySignals(input: ListCompanySignalsInput) {
  return callCommand<CompanySignal[]>("list_company_signals", { input });
}

export function confirmCompanySignal(id: string) {
  return callCommand<CompanySignal>("confirm_company_signal", { input: { id } });
}

export function rejectCompanySignal(id: string) {
  return callCommand<void>("reject_company_signal", { input: { id } });
}

// Runs the opt-in AI classification fallback over unknown official filings.
// A no-op (enabled: false) unless the user enabled espiAiFallbackEnabled.
export function runAiSignalClassification() {
  return callCommand<AiSignalClassificationSummary>("run_ai_signal_classification");
}
