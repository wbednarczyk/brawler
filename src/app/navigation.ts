import { Activity, BookOpenText, Building2, CalendarDays, Inbox, Settings, Video } from "lucide-react";
import type { LocaleKey } from "../shared/locale";

export type Section = "Inbox" | "Companies" | "Notebooks" | "Events" | "Transcripts" | "Sources" | "Settings";

export const sections = [
  { label: "Inbox" as const, icon: Inbox, localeKey: "nav.inbox" },
  { label: "Companies" as const, icon: Building2, localeKey: "nav.companies" },
  { label: "Notebooks" as const, icon: BookOpenText, localeKey: "nav.notebooks" },
  { label: "Events" as const, icon: CalendarDays, localeKey: "nav.events" },
  { label: "Transcripts" as const, icon: Video, localeKey: "nav.transcripts" },
  { label: "Sources" as const, icon: Activity, localeKey: "nav.sources" },
  { label: "Settings" as const, icon: Settings, localeKey: "nav.settings" },
] satisfies Array<{ label: Section; icon: typeof Inbox; localeKey: LocaleKey }>;
