import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import { collectActionInventory, collectEmptyStates, expectSinglePrimary } from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";
import { plText } from "../../shared/locale/resources/plText";
const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];
/** The rendered label for an EN copy key in `locale` (PL via plText, exactly what `text()` renders). */
function L(locale: Locale, en: string, vars: Record<string, string> = {}): string {
  const base = locale === "pl" ? (plText[en] ?? en) : en;
  return Object.entries(vars).reduce((s, [k, v]) => s.replace(`{${k}}`, v), base);
}


// Contract harness for the Companies library (docs/plans/frontend-v2-f4a.md
// § Companies library — contract-exempt: no storyboard/state matrix, but the
// action inventory and the empty-state-as-invitation rules apply).
//
// the per-state action inventory is
// now asserted as a FULL sorted array (`toEqual`, duplicates visible) instead
// of a `Map` lookup of a few named entries — an extra, missing, or
// misclassified button now reddens even if it isn't one of the names this
// file happens to check by name. "No unclassified" is asserted per named
// state, not just the default one. The default ("minimal") scenario's four
// seeded companies (GPW:CDR/PKN/KGH/PZU) and the one seeded watchlist ("Main
// GPW", CDR the only member) are `src/test/scenarios/legacyMinimal.ts`'s
// fixed fixture — the row-level entries below are that fixture's names, not
// re-derived from the screen's own render.

const REGION_NAME = { en: "Companies", pl: "Spółki" } as const;

async function openCompanies(locale: "en" | "pl") {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Companies" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

// The default (minimal) scenario's four seeded tickers, in the DOM row order
// the screen renders them — `Open`/`Remove` are per-row, `qualifiedTicker`-suffixed.
const SEED_TICKERS = ["GPW:CDR", "GPW:KGH", "GPW:PKN", "GPW:PZU"];

function defaultInventory(locale: "en" | "pl"): ActionInventoryEntry[] {
  const open = locale === "pl" ? "Otwórz" : "Open";
  const remove = locale === "pl" ? "Usuń" : "Remove";
  const rowEntries = SEED_TICKERS.flatMap((ticker) => [
    { name: `${open} ${ticker}`, kind: "destination" },
    { name: `${remove} ${ticker}`, kind: "remove" },
  ]);
  const staticEntries: ActionInventoryEntry[] =
    locale === "pl"
      ? [
          { name: "Dodaj", kind: "add" },
          { name: "Otwórz listę obserwowaną Main GPW", kind: "destination" },
          { name: "Wyszukaj", kind: "control" },
          { name: "Zarządzaj ustawieniami", kind: "destination" },
        ]
      : [
          { name: "Add", kind: "add" },
          { name: "Lookup", kind: "control" },
          { name: "Manage settings", kind: "destination" },
          { name: "Open watchlist Main GPW", kind: "destination" },
        ];
  return sorted([...rowEntries, ...staticEntries]);
}

describe("Companies action inventory (F4a contract § Companies library)", () => {
  it("the full sorted action inventory matches the contract table (EN)", async () => {
    const region = await openCompanies("en");
    await within(region).findByText("CD PROJEKT S.A.");
    expect(collectActionInventory(region, "en")).toEqual(defaultInventory("en"));
  });

  it("the full sorted action inventory matches the contract table (PL)", async () => {
    const region = await openCompanies("pl");
    await within(region).findByText("CD PROJEKT S.A.");
    expect(collectActionInventory(region, "pl")).toEqual(defaultInventory("pl"));
  });

  it("Add is the only primary action at rest", async () => {
    const region = await openCompanies("en");
    expectSinglePrimary(region);
  });

  it("a watchlist-membership chip names itself \"Open watchlist …\", not just the list name", async () => {
    const region = await openCompanies("en");
    expect(within(region).queryByRole("button", { name: /^Open watchlist / })).not.toBeNull();
  });
});

describe("Companies empty states are invitations (F4a contract § Companies library)", () => {
  it.each(LOCALES)("Empty (no companies): Lookup + Add + Manage settings + the invitation action, all classified (%s)", async (locale) => {
    appTestState.companiesResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openCompanies(locale);
    await within(region).findByText(L(locale, "No companies in your library yet"));
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Add"), kind: "add" },
        { name: L(locale, "Add your first company"), kind: "control" },
        { name: L(locale, "Lookup"), kind: "control" },
        { name: L(locale, "Manage settings"), kind: "destination" },
      ]),
    );

    const user = userEvent.setup();
    await user.click(within(region).getByRole("button", { name: L(locale, "Add your first company") }));
    expect(within(region).getByLabelText(L(locale, "Ticker"))).toHaveFocus();
  });

  it.each(LOCALES)("Empty (no filter match): Lookup + Add + Manage settings + both clear actions, all classified (%s)", async (locale) => {
    const region = await openCompanies(locale);
    const user = userEvent.setup();
    await user.type(within(region).getByLabelText(L(locale, "Search tracked companies")), "zzz-no-match-zzz");

    await within(region).findByText(L(locale, "No companies match your filters"));
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Add"), kind: "add" },
        { name: L(locale, "Clear company search"), kind: "control" },
        { name: L(locale, "Clear filters"), kind: "control" },
        { name: L(locale, "Lookup"), kind: "control" },
        { name: L(locale, "Manage settings"), kind: "destination" },
      ]),
    );

    await user.click(within(region).getByRole("button", { name: L(locale, "Clear filters") }));
    expect(within(region).getByLabelText(L(locale, "Search tracked companies"))).toHaveValue("");
  });

  it.each(LOCALES)("Empty (registry no-match): the invitation's Add manually is classified and the rest of the inventory is unchanged (%s)", async (locale) => {
    const region = await openCompanies(locale);
    const user = userEvent.setup();
    await user.clear(within(region).getByLabelText(L(locale, "Name")));
    await user.type(within(region).getByLabelText(L(locale, "Name")), "Definitely Not In The Registry Zzz");

    await within(region).findByText(L(locale, "No match in the registry"));
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        ...defaultInventory(locale),
        { name: L(locale, "Add manually"), kind: "control" },
        { name: L(locale, "Clear name"), kind: "control" },
      ]),
    );
  });
});
