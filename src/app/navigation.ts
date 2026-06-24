import {
  Activity,
  Bug,
  Building2,
  Columns3,
  Home,
  Inbox,
  ListChecks,
  Scale,
  Settings,
  Video,
} from "lucide-react";
import type { LocaleKey } from "../shared/locale";

// The full set of app sections. Note: several values (ReportSeason, Research,
// Notebooks, Events) are no longer top-level nav destinations — they are hosted
// as panels inside the Cockpit / Company workspace (ADR 0053/0054) — but remain
// valid `activeSection` values because deep links / programmatic navigation
// still use them and AppStateRoot still renders them.
export type Section =
  | "Today"
  | "Inbox"
  | "Cockpit"
  | "Compare"
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

// Mode-based, thesis-centric IA spine (ADR 0054). The left sidebar is grouped:
//   • Modes — the investor's jobs as top-level destinations (Today/Pulse home,
//     Company workspace, Compare). Cockpit rides along as the interim advanced
//     workspace until it folds into the Company workspace mode (ADR 0054 task 3).
//   • Library — the named reference surfaces the modes draw on (Inbox bulk
//     triage, Watchlists, Transcripts, Sources).
//   • Utilities — Settings + Diagnostics (developer-gated).
// Pinned/favorite companies render as a data-driven group between Modes and
// Library (built at runtime from `UserSettings.pinnedCompanyIds`).
export const navGroups: NavGroup[] = [
  {
    id: "modes",
    localeKey: "nav.group.modes",
    items: [
      { label: "Today", icon: Home, localeKey: "nav.today" },
      { label: "Cockpit", icon: Columns3, localeKey: "nav.cockpit" },
      { label: "Companies", icon: Building2, localeKey: "nav.companies" },
      { label: "Compare", icon: Scale, localeKey: "nav.compare" },
    ],
  },
  {
    id: "library",
    localeKey: "nav.group.library",
    items: [
      { label: "Inbox", icon: Inbox, localeKey: "nav.inbox" },
      { label: "Watchlists", icon: ListChecks, localeKey: "nav.watchlists" },
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
