import { callCommand } from "./tauri";
import type { CompanySignal } from "./types";
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

// ADR 0084: the deterministic ESPI rule classifier and `signal_dates` are the
// whole classification path; a filing neither can classify lands in an
// explicit unclassified bucket.

// Confirms a proposed derived calendar event onto the calendar, or rejects it (ADR 0036).
export function confirmDerivedEvent(eventId: string, action: "confirm" | "reject") {
  return callCommand<void>("confirm_derived_event", { input: { eventId, action } });
}
