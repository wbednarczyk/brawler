export type CompanyEventMode = "upcoming" | "historical" | "all";

export type CompanyEventViewMode = "week" | "list";

export type CompanyEventForm = {
  companyId: string;
  eventType: string;
  title: string;
  eventDate: string;
  eventTime: string;
  status: string;
};
