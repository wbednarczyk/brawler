import type { ComponentProps } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

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
    sourceStatusSummary: { label: "OK", title: "Sources OK", tone: "ok" },
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
