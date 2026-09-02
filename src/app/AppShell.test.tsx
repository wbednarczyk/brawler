import type { ComponentProps } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { AppShell, type PinnedCompany } from "./AppShell";
import { appShortcutReferenceItems, type AppShortcutActionMap } from "./shortcuts";

// A no-op action for every registered shortcut id — built from the runtime
// list (not hand-enumerated) so it never drifts from `AppShortcutId`.
const noopShortcutActions = Object.fromEntries(
  appShortcutReferenceItems.map((item) => [item.id, () => {}]),
) as AppShortcutActionMap;

const CDR: PinnedCompany = { id: "company_gpw_cdr", name: "CD Projekt", ticker: "CDR" };
const PZU: PinnedCompany = { id: "company_gpw_pzu", name: "PZU", ticker: "PZU" };

type AppShellProps = ComponentProps<typeof AppShell>;

function baseProps(overrides: Partial<AppShellProps> = {}): AppShellProps {
  return {
    activeSection: "Today",
    children: null,
    dbRefreshState: "idle",
    effectiveTheme: "dark",
    health: null,
    refreshDatabaseBackedViews: () => {},
    refreshSources: () => {},
    setActiveSection: () => {},
    onOpenSpolkaMode: () => {},
    onNavigateToSearchResult: () => {},
    pinnedCompanies: [],
    trackedCompanies: [CDR, PZU],
    selectedCompanyId: null,
    onOpenCompany: () => {},
    onUnpinCompany: () => {},
    sourceRefreshError: null,
    sourceRefreshResult: null,
    sourceRefreshState: "idle",
    sourceStatusSummary: { kind: "ok", enabled: 2, total: 2 },
    theme: "dark",
    locale: "en",
    shortcutBindings: {},
    shortcutActions: noopShortcutActions,
    totalUnreadFeedItems: 0,
    unseenAttentionCount: 0,
    attentionHydrated: true,
    updateTheme: () => {},
    openSourceStatus: () => {},
    ...overrides,
  };
}

function renderShell(overrides: Partial<AppShellProps> = {}) {
  return render(<AppShell {...baseProps(overrides)} />);
}

// F3a S3 (ADR 0107, consent 3): exactly one `aria-current="page"` across
// modes and pinned rows, for every state the sidebar can be in.
describe("AppShell — exactly one aria-current across modes and pinned rows", () => {
  it("Today", () => {
    renderShell({ activeSection: "Today" });
    const current = document.querySelectorAll('[aria-current="page"]');
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveAccessibleName(/Today/);
  });

  it("Inbox", () => {
    renderShell({ activeSection: "Inbox" });
    const current = document.querySelectorAll('[aria-current="page"]');
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveAccessibleName("Inbox");
  });

  it("Spolka+pinned: the pinned row is current, not the Spółka mode item", () => {
    renderShell({
      activeSection: "Spolka",
      selectedCompanyId: CDR.id,
      pinnedCompanies: [CDR],
    });
    const current = document.querySelectorAll('[aria-current="page"]');
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveTextContent("CDR");
    // The Spółka mode item itself must NOT also carry aria-current.
    expect(screen.getByRole("button", { name: "Company" })).not.toHaveAttribute("aria-current");
  });

  it("Spolka+unpinned: the Spółka mode item is current", () => {
    renderShell({
      activeSection: "Spolka",
      selectedCompanyId: PZU.id,
      pinnedCompanies: [CDR],
    });
    const current = document.querySelectorAll('[aria-current="page"]');
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveAccessibleName("Company");
  });

  it("Companies", () => {
    renderShell({ activeSection: "Companies" });
    const current = document.querySelectorAll('[aria-current="page"]');
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveAccessibleName("Companies");
  });
});

// F4b S4 (contract § Decisions #1): Events and Report Season join the
// Library nav. F4c S1 (docs/plans/f4c-contracts/s1-guardrails.md item 5,
// plan § Decisions 4): Research joins next to Report Season (both "work"
// surfaces; Transcripts/Sources stay last as utilities) — eight destinations
// in the documented order, both locales. `nav.research` = "Research" in BOTH
// locales (plText.ts:521 keeps the English name in PL). RED today — Research
// is not yet a Library nav entry (`navigation.ts:88-92`); S3 adds it.
describe("AppShell — Library group lists eight destinations in order, both locales", () => {
  const LIBRARY_ORDER = {
    en: ["Companies", "Watchlists", "Alerts", "Events", "Report Season", "Research", "Transcripts", "Sources"],
    pl: ["Spółki", "Listy", "Alerty", "Wydarzenia", "Sezon raportów", "Research", "Transkrypcje", "Źródła"],
  } as const;

  it.each(["en", "pl"] as const)("%s", (locale) => {
    renderShell({ locale });
    const group = screen.getByText(locale === "pl" ? "Biblioteka" : "Library").closest(".nav-group");
    if (!group) throw new Error("Library nav group not found");
    const items = Array.from(group.querySelectorAll(".nav-item")).map((node) => node.textContent);
    expect(items).toEqual(LIBRARY_ORDER[locale]);
  });
});

// F4c S3 (sol R2 amendment): Ctrl+4 invokes the Research section through
// AppShell's own shortcut wiring (`useKeyboardShortcuts`, registered on
// `document`), not just the default-binding table.
describe("AppShell — Ctrl+4 invokes the Research section", () => {
  it("calls setActiveSection('Research') on Ctrl+4", () => {
    let active: string | null = null;
    renderShell({ setActiveSection: (section) => { active = section; } });

    fireEvent.keyDown(document, { key: "4", code: "Digit4", ctrlKey: true });

    expect(active).toBe("Research");
  });
});
