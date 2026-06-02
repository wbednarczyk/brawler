import { Activity, BookOpenText, Building2, CalendarDays, Inbox, Settings, Video } from "lucide-react";

export type Section = "Inbox" | "Companies" | "Notebooks" | "Events" | "Transcripts" | "Sources" | "Settings";

export const sections = [
  { label: "Inbox" as const, icon: Inbox },
  { label: "Companies" as const, icon: Building2 },
  { label: "Notebooks" as const, icon: BookOpenText },
  { label: "Events" as const, icon: CalendarDays },
  { label: "Transcripts" as const, icon: Video },
  { label: "Sources" as const, icon: Activity },
  { label: "Settings" as const, icon: Settings },
];
