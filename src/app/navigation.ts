import {
  Activity,
  BellRing,
  Bug,
  Building2,
  FlaskConical,
  Home,
  Inbox,
  ListChecks,
  Settings,
  Video,
} from "lucide-react";
import type { LocaleKey } from "../shared/locale";

// The full set of app sections. ReportSeason, Research, Notebooks, and Events
// are not top-level nav destinations — they are hosted as panels inside the
// Cockpit / Company workspace (ADR 0053/0054) — but remain valid
// `activeSection` values because deep links / programmatic navigation still
// use them and AppStateRoot still renders them.
export type Section =
  | "Today"
  | "Inbox"
  | "Cockpit"
  // The Spółka screen (F3a S1, ADR 0107) — the company deep-dive
  // destination; not a top-level nav entry (opened by selecting a company),
  // same posture as Cockpit/ReportSeason/etc above.
  | "Spolka"
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

// Mode-based, thesis-centric IA spine (ADR 0054). The left sidebar is grouped:
//   • Modes — the investor's jobs as top-level destinations (Today/Pulse home,
//     Company workspace), followed by the saved named views (a
//     data-driven list, rendered by AppShell from `cockpit_layouts`) and the
//     "+ New view" creator — the composable-views entry point (ADR 0057
//     decision 5).
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
      // Dashboard — the one company-scoped cockpit, entered ONLY from here (epic
      // c793ca1). Two selectors inside: company (scope) + preset (panel
      // arrangement); every preset follows the view company. Amends ADR 0057
      // decision 5: never a blank canvas (seeds a company / resumes layout).
      { label: "Cockpit", icon: FlaskConical, localeKey: "nav.dashboard" },
    ],
  },
  {
    id: "library",
    localeKey: "nav.group.library",
    items: [
      // Companies lives in Library, not Modes (epic c793ca1): the Dashboard is
      // the company workspace; the list is a reference surface.
      { label: "Companies", icon: Building2, localeKey: "nav.companies" },
      { label: "Inbox", icon: Inbox, localeKey: "nav.inbox" },
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
