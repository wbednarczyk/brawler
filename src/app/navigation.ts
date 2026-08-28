import {
  Activity,
  BellRing,
  Bug,
  Building2,
  Home,
  Inbox,
  LayoutPanelTop,
  ListChecks,
  Settings,
  Video,
} from "lucide-react";
import type { LocaleKey } from "../shared/locale";

// The full set of app sections. ReportSeason, Research, Notebooks, and Events
// are not top-level nav destinations — they are reached via the palette or
// deep links, but remain valid `activeSection` values because AppStateRoot
// still renders them.
export type Section =
  | "Today"
  | "Inbox"
  // The Spółka screen (F3a S1, ADR 0107) — the company deep-dive
  // destination and, since S3, a Modes item (replaces the old Dashboard
  // bridge): opens the last-viewed company.
  | "Spolka"
  // Decision journal, all companies (F3a S3, ADR 0107) — a standalone screen
  // route (palette-only entry point; not a nav item).
  | "Journal"
  | "ReportSeason"
  | "Companies"
  | "Watchlists"
  | "Alerts"
  | "Research"
  | "Notebooks"
  | "Events"
  | "Transcripts"
  | "Sources"
  | "Diagnostics"
  | "Settings";

export type NavItem = {
  label: Section;
  icon: typeof Inbox;
  localeKey: LocaleKey;
};

export type NavGroup = {
  id: "modes" | "library" | "utilities";
  localeKey: LocaleKey;
  items: NavItem[];
};

// Mode-based, thesis-centric IA spine (ADR 0054, amended F3a S3 / ADR 0107,
// ADR 0108 — no docking engine, no "Widoki" group). The left sidebar is
// grouped:
//   • Modes — the investor's jobs as top-level destinations: Today, Inbox,
//     Spółka (opens the last-viewed company).
//   • Library — the named reference surfaces the modes draw on (Companies,
//     Watchlists, Alerts, Transcripts, Sources).
//   • Utilities — Settings + Diagnostics (developer-gated).
// Pinned/favorite companies render as a data-driven group between Modes
// and Library (built at runtime from `UserSettings.pinnedCompanyIds`).
export const navGroups: NavGroup[] = [
  {
    id: "modes",
    localeKey: "nav.group.modes",
    items: [
      { label: "Today", icon: Home, localeKey: "nav.today" },
      { label: "Inbox", icon: Inbox, localeKey: "nav.inbox" },
      // Spółka (F3a S3, ADR 0107 amendment) replaces the old Dashboard bridge:
      // opens the last-viewed company, else the first pinned, else the first
      // tracked company — never a blank screen.
      { label: "Spolka", icon: LayoutPanelTop, localeKey: "nav.spolka" },
    ],
  },
  {
    id: "library",
    localeKey: "nav.group.library",
    items: [
      { label: "Companies", icon: Building2, localeKey: "nav.companies" },
      { label: "Watchlists", icon: ListChecks, localeKey: "nav.watchlists" },
      // Alerts (ADR 0068 T3): a Library destination — the alert-rule manager +
      // fired-alerts review are a reference surface, not a preference.
      { label: "Alerts", icon: BellRing, localeKey: "nav.alerts" },
      { label: "Transcripts", icon: Video, localeKey: "nav.transcripts" },
      { label: "Sources", icon: Activity, localeKey: "nav.sources" },
    ],
  },
  {
    id: "utilities",
    localeKey: "nav.group.utilities",
    items: [
      { label: "Settings", icon: Settings, localeKey: "nav.settings" },
      { label: "Diagnostics", icon: Bug, localeKey: "nav.diagnostics" },
    ],
  },
];

// Flat list of nav items (every destination across all groups), kept for
// callers that only need the membership/order without the grouping.
export const sections: NavItem[] = navGroups.flatMap((group) => group.items);
