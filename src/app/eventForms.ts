import { formatLocalDate } from "../shared/format/datetime";
import type { CompanyEventForm } from "../shared/types/events";

export const companyEventTypeOptions = [
  "periodic_report",
  "corporate_action",
  "dividend",
  "shareholder_meeting",
  "conference_call",
  "investor_conference",
  "market_making",
  "listing_change",
  "other_market_event",
  "custom",
];

export const companyEventStatusOptions = [
  "scheduled",
  "confirmed",
  "tentative",
  "changed",
  "cancelled",
  "completed",
];

export function emptyCompanyEventForm(): CompanyEventForm {
  return {
    companyId: "",
    eventType: "custom",
    title: "",
    eventDate: formatLocalDate(new Date()),
    eventTime: "",
    status: "scheduled",
  };
}
