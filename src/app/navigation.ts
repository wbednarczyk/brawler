import {
  Activity,
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

// The full set of app sections. Note: several values (ReportSeason, Research,
// Notebooks, Events) are no longer top-level nav destinations — they are hosted
// as panels inside the Cockpit / Company workspace (ADR 0053/0054) — but remain
// valid `activeSection` values because deep links / programmatic navigation
// still use them and AppStateRoot still renders them. ADR 0057 decision 5
// removed the standalone *blank-canvas* "Cockpit" nav item; it is otherwise
// reached via a saved named view (rendered as its own nav item, see AppShell),
// the "+ New view" creator, or a company's curated dashboard. Amended
// 2026-07-13 (owner, card 47d5fbb): a single "Research" cockpit entry is
// restored in Modes — but it is never blank (the cockpit seeds a first
// company/feed item and resumes the last layout), so ADR 0057's "no empty
// mode" rationale is preserved rather than reversed.
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
      // Dashboard — the one company-scoped cockpit, entered ONLY from here (owner
      // redesign 2026-07-13, epic c793ca1). Two selectors inside: company (scope) +
      // preset (panel arrangement); every preset follows the view company. The
      // standalone Research screen is retired into a Dashboard preset. Amends ADR
      // 0057 decision 5: never a blank canvas (seeds a company / resumes layout).
      { label: "Cockpit", icon: FlaskConical, localeKey: "nav.dashboard" },
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
      // Companies dropped from Modes into Library (owner 2026-07-13, epic c793ca1):
      // the Dashboard is the company workspace; the list is a reference surface.
      { label: "Companies", icon: Building2, localeKey: "nav.companies" },
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
