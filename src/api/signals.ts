import { callCommand } from "./tauri";
import type {
  AiEventDerivationSummary,
  AiSignalClassificationSummary,
  CompanySignal,
} from "./types";
import type { ListCompanySignalsInput } from "./generated/ListCompanySignalsInput";

// Input type GENERATED from src-tauri/src/storage/types.rs via ts-rs (ADR 0048).
export type { ListCompanySignalsInput } from "./generated/ListCompanySignalsInput";

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

// Runs the opt-in AI date-extraction fallback, deriving proposed calendar events for
// dividend/general-meeting signals the deterministic parser could not date (ADR 0036).
export function runAiEventDerivation() {
  return callCommand<AiEventDerivationSummary>("run_ai_event_derivation");
}

// Confirms a proposed derived calendar event onto the calendar, or rejects it (ADR 0036).
export function confirmDerivedEvent(eventId: string, action: "confirm" | "reject") {
  return callCommand<void>("confirm_derived_event", { input: { eventId, action } });
}
