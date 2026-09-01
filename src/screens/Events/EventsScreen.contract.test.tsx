import { describe, it } from "vitest";
import { vi } from "vitest";
import {
  appTestState,
  expect,
  handleAppCommand,
  invoke,
  renderApp,
  screen,
  seedScenario,
  userEvent,
  within,
} from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";
import { plText } from "../../shared/locale/resources/plText";
import type { CompanyEvent } from "../../api/types";
import { addLocalDays, formatLocalDate } from "../../shared/format/datetime";

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];
function L(locale: Locale, en: string): string {
  return locale === "pl" ? (plText[en] ?? en) : en;
}

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

// F4b S3 contract test for the Events redesign (docs/plans/frontend-v2-f4b.md
// § Events, Action inventory + State matrix). Default seed (legacyMinimal):
// week view, current-week range, no active filters — two events this week
// ("Main Market - Corporate actions - Equity - CDR", company CDR, today; and
// the PZU market-making event two days later) — both are visible cards.

const REGION_NAME = { en: "Events", pl: "Wydarzenia" } as const;
const CDR_TITLE = "Main Market - Corporate actions - Equity - CDR";
const PZU_TITLE = "Main Market - End of market making activities - Equity - PZU";

async function openEvents(locale: Locale) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Events" });
  const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
  // Wait for the seeded week data to land (the accessible name carries the
  // raw title; the redesigned card shows the human type label instead —
  // #431/#417).
  await within(region).findByRole("button", { name: `${L(locale, "Open event")}: ${CDR_TITLE}` });
  return region;
}

function defaultInventory(locale: Locale): ActionInventoryEntry[] {
  return sorted([
    { name: locale === "pl" ? "Dodaj wydarzenie" : "Add event", kind: "add" },
    { name: L(locale, "Refresh calendar"), kind: "refresh" },
    { name: L(locale, "Week"), kind: "control" },
    { name: L(locale, "List"), kind: "control" },
    { name: L(locale, "Previous week"), kind: "control" },
    { name: L(locale, "Next week"), kind: "control" },
    { name: L(locale, "Current week"), kind: "control" },
    { name: L(locale, "Filters"), kind: "control" },
    { name: L(locale, "Clear filters"), kind: "control" },
    { name: `${L(locale, "Open event")}: ${CDR_TITLE}`, kind: "control" },
    { name: `${L(locale, "Open event")}: ${PZU_TITLE}`, kind: "control" },
  ]);
}

describe("Events action inventory (F4b contract § Events, Action inventory)", () => {
  for (const locale of LOCALES) {
    it(`the full sorted action inventory matches the contract table, default state (${locale})`, async () => {
      const region = await openEvents(locale);
      expect(collectActionInventory(region, locale)).toEqual(defaultInventory(locale));
    });
  }

  it("no button in the screen root is left unclassified — today every button is (default state)", async () => {
    const region = await openEvents("en");
    const unclassified = collectActionInventory(region, "en").filter(
      (entry) => entry.kind === "unclassified",
    );
    expect(unclassified).toEqual([]);
  });
});

describe("Events primary action per state (F4b contract § Events, decision 5)", () => {
  it("Success (default, no composer, no proposed selection): `addEvent` is the one filled action, marker and variant on the same element", async () => {
    const region = await openEvents("en");
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
    expect(within(region).getByRole("button", { name: "Add event" }).getAttribute("data-ux-primary-action")).toBe(
      "true",
    );
  });

  it("Success (proposed event expanded, list mode): `confirmProposed` is the one filled action", async () => {
    seedScenario("rich");
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: "Events" });
    await userEvent.click(within(region).getByRole("button", { name: "List" }));
    const proposedCard = await within(region).findByRole("button", {
      name: /dividend date \(derived, unconfirmed\)/i,
    });
    await userEvent.click(proposedCard);
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
    const confirmButton = within(region).getByRole("button", { name: "Confirm" });
    expect(confirmButton.getAttribute("data-ux-primary-action")).toBe("true");
  });

  // F4b sol R1 (blocker): a proposed event stayed selected (its detail panel
  // stayed open, "Confirm" stayed hardcoded primary) while the composer also
  // opened — two marked/filled buttons. The enum's own precedence
  // (composerOpen checked before selectedEventStatus) must actually drive
  // every candidate, not just "Add event".
  it("Composer open while a proposed event is still selected: `saveComposer` wins, `Confirm` goes quiet", async () => {
    seedScenario("rich");
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: "Events" });
    await userEvent.click(within(region).getByRole("button", { name: "List" }));
    const proposedCard = await within(region).findByRole("button", {
      name: /dividend date \(derived, unconfirmed\)/i,
    });
    await userEvent.click(proposedCard);
    await within(region).findByRole("button", { name: "Confirm" });

    await userEvent.click(within(region).getByRole("button", { name: "Add event" }));
    await within(region).findByRole("button", { name: "Save" });

    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
    const saveButton = within(region).getByRole("button", { name: "Save" });
    expect(saveButton.getAttribute("data-ux-primary-action")).toBe("true");
    const confirmButton = within(region).getByRole("button", { name: "Confirm" });
    expect(confirmButton.getAttribute("data-ux-primary-action")).toBeNull();
  });
});

describe("Events empty states (F4b contract § Events, State matrix)", () => {
  it.each(LOCALES)(
    "Empty (week has no events, a later match exists): invitation with `Show next week with events` (%s)",
    async (locale) => {
      appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
      const futureDate = formatLocalDate(addLocalDays(new Date(), 21));
      const futureEvent: CompanyEvent = {
        id: "event_future_match",
        companyId: "company_gpw_cdr",
        company: "GPW:CDR",
        companyName: "CD PROJEKT S.A.",
        eventType: "periodic_report",
        title: "Raport za I kwartał",
        eventDate: futureDate,
        eventTime: null,
        status: "confirmed",
        sourceType: "official_calendar",
        sourceAdapterId: "gpw-market-events-rss",
        sourceEventKey: "future-match",
        sourceUrl: null,
        attribution: "GPW",
        fetchedAt: "2026-06-01T08:00:00Z",
        manual: false,
        createdAt: "2026-06-01T08:00:00Z",
        updatedAt: "2026-06-01T08:00:00Z",
      };
      appTestState.companyEventsResponse = [futureEvent];
      renderApp({ section: "Events" });
      const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
      await screen.findByText(L(locale, "Show next week with events"));
      expect(collectEmptyStates(region)).toContain("invitation");
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          { name: locale === "pl" ? "Dodaj wydarzenie" : "Add event", kind: "add" },
          { name: L(locale, "Refresh calendar"), kind: "refresh" },
          { name: L(locale, "Week"), kind: "control" },
          { name: L(locale, "List"), kind: "control" },
          { name: L(locale, "Previous week"), kind: "control" },
          { name: L(locale, "Next week"), kind: "control" },
          { name: L(locale, "Current week"), kind: "control" },
          { name: L(locale, "Filters"), kind: "control" },
          { name: L(locale, "Clear filters"), kind: "control" },
          { name: L(locale, "Show next week with events"), kind: "control" },
        ]),
      );
      expectSinglePrimary(region, 1);
    },
  );

  it("Empty (week empty, active filters, no later match): quiet, `Wyczyść filtry` a control, no primary", async () => {
    appTestState.companyEventsResponse = [];
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: "Events" });
    // No events at all means the Type/Status filters have no options to pick
    // from (they're derived from the loaded events) — the Company filter's
    // options come from the always-populated company list instead.
    const companyFilter = screen.getByLabelText("Event company filter") as HTMLSelectElement;
    await userEvent.selectOptions(companyFilter, companyFilter.options[1].value);
    await screen.findByText("Later there are no events matching the filters");
    expect(collectEmptyStates(region)).toContain("quiet");
    expectSinglePrimary(region, 0);
    expect(within(region).getAllByRole("button", { name: "Clear filters" }).length).toBeGreaterThan(0);
  });

  it("Empty (week empty, no active filters, already refreshed): the invitation's own `Add event` carries the primary — the header's goes quiet", async () => {
    appTestState.companyEventsResponse = [];
    // legacyMinimal's adapters all carry `lastSuccessAt: null` (the
    // "never refreshed" branch) — force one calendar adapter's success
    // timestamp so this test reaches the "already refreshed, no later
    // match, no filters" branch instead (decision 4).
    appTestState.sourceAdaptersResponse = appTestState.sourceAdaptersResponse.map((adapter) =>
      adapter.sourceType === "official_calendar" || adapter.sourceType === "public_calendar"
        ? { ...adapter, lastSuccessAt: "2026-06-05T09:00:00Z" }
        : adapter,
    );
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: "Events" });
    const invitation = await screen.findByText("Bankier and GPW calendars have no later dates for the companies on your lists.");
    const panel = invitation.closest(".event-week-empty-panel") as HTMLElement;
    expect(panel).toBeTruthy();

    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
    const invitationAddEvent = within(panel).getByRole("button", { name: "Add event" });
    expect(invitationAddEvent.getAttribute("data-ux-primary-action")).toBe("true");

    // The header's own "Add event" (outside the invitation panel) must be
    // the ONLY other button with this name, and must NOT be the primary one.
    const allAddEventButtons = within(region).getAllByRole("button", { name: "Add event" });
    expect(allAddEventButtons).toHaveLength(2);
    const headerAddEvent = allAddEventButtons.find((button) => button !== invitationAddEvent);
    expect(headerAddEvent?.getAttribute("data-ux-primary-action")).toBeNull();
  });

  // F4b sol R1 (high): a failed "later match" lookup used to fold into "no
  // match" — the invitation claimed nothing was later when the read simply
  // failed. It must show an error + retry instead, and mark nothing primary.
  it("Empty (week empty, the later-match lookup fails): ErrorText + `Refresh calendar`, no primary — never the 'nothing later' invitation", async () => {
    appTestState.companyEventsResponse = [];
    vi.mocked(invoke).mockImplementation((command, args) => {
      const input = (args as { input?: { mode?: string } } | undefined)?.input;
      if (command === "list_company_events" && input?.mode === "upcoming") {
        return Promise.reject(new Error("network unreachable"));
      }
      return handleAppCommand(command, args);
    });
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: "Events" });
    await within(region).findByText("Failed to check later weeks");
    expect(
      within(region).queryByText("Bankier and GPW calendars have no later dates for the companies on your lists."),
    ).not.toBeInTheDocument();
    expectSinglePrimary(region, 0);
  });

  // F4b sol R1 (high): while the lookup is still in flight, no primary
  // should flash "addEvent" — the empty week says it's checking instead.
  it("Empty (week empty, the later-match lookup is pending): quiet 'Checking later weeks…', no primary", async () => {
    appTestState.companyEventsResponse = [];
    let resolveLookup: (() => void) | undefined;
    vi.mocked(invoke).mockImplementation((command, args) => {
      const input = (args as { input?: { mode?: string } } | undefined)?.input;
      if (command === "list_company_events" && input?.mode === "upcoming") {
        return new Promise((resolve) => {
          resolveLookup = () => resolve([]);
        });
      }
      return handleAppCommand(command, args);
    });
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: "Events" });
    await within(region).findByText("Checking later weeks…");
    expectSinglePrimary(region, 0);
    resolveLookup?.();
  });

  // F4b sol R3 (medium): the error state's `Refresh calendar` must actually
  // re-run the lookup — a failed first read followed by a retry click ends in
  // the jump invitation, not a stuck error.
  it("Empty (lookup failed, then retried): `Refresh calendar` re-runs the lookup and recovers to the jump invitation", async () => {
    const user = userEvent.setup();
    const futureDate = formatLocalDate(addLocalDays(new Date(), 21));
    const futureEvent: CompanyEvent = {
      id: "event_future_match",
      companyId: "company_gpw_cdr",
      company: "GPW:CDR",
      companyName: "CD PROJEKT S.A.",
      eventType: "periodic_report",
      title: "Raport za I kwartał",
      eventDate: futureDate,
      eventTime: null,
      status: "confirmed",
      sourceType: "official_calendar",
      sourceAdapterId: "gpw-market-events-rss",
      sourceEventKey: "future-match",
      sourceUrl: null,
      attribution: "GPW",
      fetchedAt: "2026-06-01T08:00:00Z",
      manual: false,
      createdAt: "2026-06-01T08:00:00Z",
      updatedAt: "2026-06-01T08:00:00Z",
    };
    appTestState.companyEventsResponse = [futureEvent];
    let failedOnce = false;
    vi.mocked(invoke).mockImplementation((command, args) => {
      const input = (args as { input?: { mode?: string } } | undefined)?.input;
      if (command === "list_company_events" && input?.mode === "upcoming" && !failedOnce) {
        failedOnce = true;
        return Promise.reject(new Error("network unreachable"));
      }
      return handleAppCommand(command, args);
    });
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: "Events" });
    const errorLine = await within(region).findByText("Failed to check later weeks");
    const strip = errorLine.closest(".event-week-empty-panel") as HTMLElement;
    await user.click(within(strip).getByRole("button", { name: "Refresh calendar" }));
    await within(region).findByText("Show next week with events");
    expectSinglePrimary(region, 1);
  });
});
