import { callCommand } from "./tauri";
import type { CompanyEvent } from "./types";

export type ListCompanyEventsInput = {
  mode: string;
  companyId: string | null;
  watchlistId: string | null;
  eventType: string | null;
  status: string | null;
  dateFrom: string | null;
  dateTo: string | null;
};

export type CreateCompanyEventInput = {
  companyId: string;
  eventType: string;
  title: string;
  eventDate: string;
  eventTime: string | null;
  status: string;
  sourceType: string;
  sourceAdapterId: string | null;
  sourceEventKey: string | null;
  sourceUrl: string | null;
  attribution: string | null;
  fetchedAt: string | null;
};

export function listCompanyEvents(input: ListCompanyEventsInput) {
  return callCommand<CompanyEvent[]>("list_company_events", { input });
}

export function createCompanyEvent(input: CreateCompanyEventInput) {
  return callCommand<CompanyEvent>("create_company_event", { input });
}
