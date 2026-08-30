import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  initialGeminiCredentialStatus,
  renderApp,
  screen,
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

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];
/** The rendered label for an EN copy key in `locale` (PL via plText, exactly what `text()` renders). */
function L(locale: Locale, en: string): string {
  return locale === "pl" ? (plText[en] ?? en) : en;
}

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

// F4b S1 red contract skeleton for the Transcripts redesign
// (docs/plans/frontend-v2-f4b.md § Transcripts, Action inventory). S2
// implements the redesign; until then EVERY action here renders through the
// legacy `<Button className="compact-button">` shape with no
// `data-action-kind`, so `collectActionInventory` reports every entry as
// "unclassified" and every assertion below is expected red — do NOT make
// these green (S2 does).
//
// Default seed (`src/test/scenarios/scenarios.ts` populated scenario): one
// transcript job ("Q2 conference", id `transcript_job_unresolved_conference`)
// with no linked company and a status that renders today's "Retry" — the
// contract's redesigned row would show `Fetch again` + `Link company`
// alongside `Open transcript: Q2 conference` and `Remove`.

const REGION_NAME = { en: "Transcripts", pl: "Transkrypcje" } as const;
const JOB_TITLE = "Q2 conference";

async function openTranscripts(locale: Locale) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  appTestState.geminiCredentialStatusResponse = {
    ...initialGeminiCredentialStatus,
    configured: true,
    storage: "os_keychain",
  };
  renderApp({ section: "Transcripts" });
  const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
  await within(region).findByText(JOB_TITLE);
  return region;
}

function defaultInventory(locale: Locale): ActionInventoryEntry[] {
  return sorted([
    { name: L(locale, "Fetch transcript"), kind: "fetch" },
    { name: L(locale, "Refresh transcripts"), kind: "refresh" },
    { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
    { name: L(locale, "Link company"), kind: "control" },
    { name: L(locale, "Fetch again"), kind: "fetch" },
    { name: L(locale, "Remove"), kind: "remove" },
  ]);
}

describe("Transcripts action inventory (F4b contract § Transcripts, Action inventory)", () => {
  for (const locale of LOCALES) {
    it(`the full sorted action inventory matches the contract table, default state (${locale})`, async () => {
      const region = await openTranscripts(locale);
      expect(collectActionInventory(region, locale)).toEqual(defaultInventory(locale));
    });
  }

  it("no button in the screen root is left unclassified — today every button is (default state)", async () => {
    const region = await openTranscripts("en");
    const unclassified = collectActionInventory(region, "en").filter(
      (entry) => entry.kind === "unclassified",
    );
    expect(unclassified).toEqual([]);
  });
});

describe("Transcripts primary action per state (F4b contract § Transcripts, decision 6)", () => {
  it("Success (no selection, no draft): `fetch` is the one filled action, marker and variant on the same element", async () => {
    const region = await openTranscripts("en");
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });
});

describe("Transcripts status exclusivity (F4b contract § Transcripts decision 3: one status per row, never two chips)", () => {
  it("exactly one [data-transcript-status] per row — today the tag does not exist at all", async () => {
    const region = await openTranscripts("en");
    const rows = region.querySelectorAll(".transcript-row, [data-transcript-job-id]");
    expect(rows.length).toBeGreaterThan(0);
    for (const row of Array.from(rows)) {
      expect(row.querySelectorAll("[data-transcript-status]").length).toBe(1);
    }
  });
});

describe("Transcripts empty states (F4b contract § Transcripts, State matrix)", () => {
  it.each(LOCALES)("Empty (no transcripts): an invitation with the composer's Fetch transcript action (%s)", async (locale) => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
    appTestState.geminiCredentialStatusResponse = {
      ...initialGeminiCredentialStatus,
      configured: true,
      storage: "os_keychain",
    };
    appTestState.transcriptJobsResponse = [];
    renderApp({ section: "Transcripts" });
    const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([{ name: L(locale, "Fetch transcript"), kind: "fetch" }]),
    );
  });
});
