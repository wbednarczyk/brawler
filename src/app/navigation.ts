import {
  Activity,
  Bug,
  Building2,
  Home,
  Inbox,
  ListChecks,
  Settings,
  Video,
} from "lucide-react";
import type { LocaleKey } from "../shared/locale";

// The full set of app sections. Note: several values (ReportSeason, Research,
// Notebooks, Events) are no longer top-level nav destinations — they are hosted
// as panels inside the Cockpit / Company workspace (ADR 0053/0054) — but remain
// valid `activeSection` values because deep links / programmatic navigation
// still use them and AppStateRoot still renders them. "Cockpit" is the same
// kind of deep-link-only value now (ADR 0057 decision 5): there is no
// standalone blank-canvas nav destination — the cockpit is reached only via a
// saved named view (rendered as its own nav item, see AppShell), the "+ New
// view" creator, or opening a company's curated dashboard.
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
//     Company workspace, Compare), followed by the saved named views (a
//     data-driven list, rendered by AppShell from `cockpit_layouts`) and the
//     "+ New view" creator — the composable-views entry point (ADR 0057
//     decision 5) that replaces the old standalone blank "Cockpit" item.
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
      { label: "Companies", icon: Building2, localeKey: "nav.companies" },
      // Compare is hidden from the spine until v0.53 market data gives the mode
      // content (U-Rc, ADR 0076 Resolved) — an empty mode in nav is trust debt.
      // The Section value and CompareScreen stay; restore the entry with:
      // { label: "Compare", icon: Scale, localeKey: "nav.compare" }
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
