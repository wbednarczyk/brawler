import { callCommand } from "./tauri";
import type { CompanyEvent } from "./types";
import type { ListCompanyEventsInput } from "./generated/ListCompanyEventsInput";
import type { CreateCompanyEventInput } from "./generated/CreateCompanyEventInput";

// Input types GENERATED from src-tauri/src/storage/types.rs via ts-rs (ADR 0048).
export type { ListCompanyEventsInput } from "./generated/ListCompanyEventsInput";
export type { CreateCompanyEventInput } from "./generated/CreateCompanyEventInput";

export function listCompanyEvents(input: ListCompanyEventsInput) {
  return callCommand<CompanyEvent[]>("list_company_events", { input });
}

export function createCompanyEvent(input: CreateCompanyEventInput) {
  return callCommand<CompanyEvent>("create_company_event", { input });
}
