import { Activity, BookOpenText, Bug, Building2, CalendarClock, CalendarDays, Inbox, ListChecks, SearchCheck, Settings, Video } from "lucide-react";
import type { LocaleKey } from "../shared/locale";

export type Section =
  | "Inbox"
  | "ReportSeason"
  | "Companies"
  | "Watchlists"
  | "Research"
  | "Notebooks"
  | "Events"
  | "Transcripts"
  | "Sources"
  | "Diagnostics"
  | "Settings";

export const sections = [
  { label: "Inbox" as const, icon: Inbox, localeKey: "nav.inbox" },
  { label: "ReportSeason" as const, icon: CalendarClock, localeKey: "nav.reportSeason" },
  { label: "Companies" as const, icon: Building2, localeKey: "nav.companies" },
  { label: "Watchlists" as const, icon: ListChecks, localeKey: "nav.watchlists" },
  { label: "Research" as const, icon: SearchCheck, localeKey: "nav.research" },
  { label: "Notebooks" as const, icon: BookOpenText, localeKey: "nav.notebooks" },
  { label: "Events" as const, icon: CalendarDays, localeKey: "nav.events" },
  { label: "Transcripts" as const, icon: Video, localeKey: "nav.transcripts" },
  { label: "Sources" as const, icon: Activity, localeKey: "nav.sources" },
  { label: "Settings" as const, icon: Settings, localeKey: "nav.settings" },
  { label: "Diagnostics" as const, icon: Bug, localeKey: "nav.diagnostics" },
] satisfies Array<{ label: Section; icon: typeof Inbox; localeKey: LocaleKey }>;
