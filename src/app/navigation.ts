import {
  Activity,
  BellRing,
  Bug,
  Building2,
  CalendarClock,
  CalendarDays,
  FlaskConical,
  Home,
  Inbox,
  LayoutPanelTop,
  ListChecks,
  Settings,
  Video,
} from "lucide-react";
import type { LocaleKey } from "../shared/locale";

// The full set of app sections. Events, ReportSeason (F4b S4) and Research
// (F4c S3, contract § Decisions #1/#4) are Library nav destinations. The
// Notebooks-global/Journal-global screens retired in F4c S2 (ADR 0108
// amendment) — their `Section` members retired with them (sol fix1 item 7).
export type Section =
  | "Today"
  | "Inbox"
  // The Spółka screen (F3a S1, ADR 0107) — the company deep-dive
  // destination and, since S3, a Modes item (replaces the old Dashboard
  // bridge): opens the last-viewed company.
  | "Spolka"
  | "ReportSeason"
  | "Companies"
  | "Watchlists"
  | "Alerts"
  | "Research"
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
//     Watchlists, Alerts, Events, Report Season, Transcripts, Sources).
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
      // Events + ReportSeason (F4b S4, contract § Decisions #1): join the
      // Library nav; their palette `Open screen: …` entries stay (J4/J7 entry
      // paths, AppShell.tsx SCREEN_PALETTE_ENTRIES).
      { label: "Events", icon: CalendarDays, localeKey: "nav.events" },
      { label: "ReportSeason", icon: CalendarClock, localeKey: "nav.reportSeason" },
      // Research (F4c S3, contract § Decisions #4): joins the Library nav —
      // language pass + labelled actions, not a redesign (owner 28.08).
      { label: "Research", icon: FlaskConical, localeKey: "nav.research" },
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
