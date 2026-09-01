import { describe, it } from "vitest";
import type { UserEvent } from "@testing-library/user-event";
import userEvent from "@testing-library/user-event";
import {
  appTestState,
  expect,
  initialGeminiCredentialStatus,
  initialTranscriptJobs,
  invoke,
  renderApp,
  screen,
  waitFor,
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
    { name: L(locale, "Settings"), kind: "destination" },
    { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
    { name: L(locale, "Rename"), kind: "rename" },
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

// ---------------------------------------------------------------------------
// Sol R1 finding 2: every reachable substate, table-driven, both locales —
// exact sorted inventory + expectPrimaryMarkerMatchesVariant. Reuses
// openTranscripts's seed (one unresolved/queued "Q2 conference" job) unless a
// state needs different backend data (failed row, key missing).
// ---------------------------------------------------------------------------

const COMPANY_QUERY = "CDR";
const COMPANY_MATCH = /GPW:CDR/;

async function runJobToCompleted(locale: Locale, region: HTMLElement) {
  const user = userEvent.setup();
  await user.click(within(region).getByRole("button", { name: L(locale, "Fetch again") }));
  await waitFor(() => {
    expect(invoke).toHaveBeenCalledWith("run_video_transcript_job", {
      input: { jobId: "transcript_job_unresolved_conference", providerMode: "provider_gemini" },
    });
  });
  await within(region).findByText(L(locale, "Ready"));
  return user;
}

async function expandRow(locale: Locale, region: HTMLElement, user: UserEvent) {
  await user.click(within(region).getByRole("button", { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}` }));
}

async function linkCompany(locale: Locale, region: HTMLElement, user: UserEvent) {
  await user.click(within(region).getByRole("button", { name: L(locale, "Link company") }));
  await user.type(within(region).getByLabelText(L(locale, "Transcript link company lookup")), COMPANY_QUERY);
  const suggestions = await within(region).findByLabelText(L(locale, "Transcript link company suggestions"));
  await user.click(within(suggestions).getByRole("button", { name: COMPANY_MATCH }));
  await waitFor(() => {
    expect(invoke).toHaveBeenCalledWith("resolve_transcript_job_company", {
      input: { jobId: "transcript_job_unresolved_conference", companyId: "company_gpw_cdr" },
    });
  });
}

// The row-collapsed action set common to the default/failed/key-missing-with-
// rows states: unresolved (unlinked) + retryable (queued/failed) job.
function collapsedRowEntries(locale: Locale): ActionInventoryEntry[] {
  return [
    { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
    { name: L(locale, "Rename"), kind: "rename" },
    { name: L(locale, "Link company"), kind: "control" },
    { name: L(locale, "Fetch again"), kind: "fetch" },
    { name: L(locale, "Remove"), kind: "remove" },
  ];
}

describe("Transcripts action inventory — every reachable substate (F4b sol R1 finding 2)", () => {
  it.each(LOCALES)("expanded (no selection): adds the segments disclosure, primary stays `fetch` (%s)", async (locale) => {
    const region = await openTranscripts(locale);
    const user = await runJobToCompleted(locale, region);
    await expandRow(locale, region, user);

    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Fetch transcript"), kind: "fetch" },
        { name: L(locale, "Refresh transcripts"), kind: "refresh" },
        { name: L(locale, "Settings"), kind: "destination" },
        { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
        { name: L(locale, "Rename"), kind: "rename" },
        { name: L(locale, "Link company"), kind: "control" },
        { name: L(locale, "Remove"), kind: "remove" },
        { name: L(locale, "Show segments"), kind: "control" },
        { name: L(locale, "Add to notebook"), kind: "add" },
      ]),
    );
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });

  it.each(LOCALES)("expanded + selection: primary moves to `addToNotebook` (%s)", async (locale) => {
    const region = await openTranscripts(locale);
    const user = await runJobToCompleted(locale, region);
    await expandRow(locale, region, user);
    const segments = await within(region).findByLabelText(L(locale, "Transcript segments"));
    await user.click(within(segments).getAllByRole("checkbox")[0]);

    const inventory = collectActionInventory(region, locale);
    expect(inventory.filter((entry) => entry.kind === "unclassified")).toEqual([]);
    expect(inventory).toEqual(
      sorted([
        { name: L(locale, "Fetch transcript"), kind: "fetch" },
        { name: L(locale, "Refresh transcripts"), kind: "refresh" },
        { name: L(locale, "Settings"), kind: "destination" },
        { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
        { name: L(locale, "Rename"), kind: "rename" },
        { name: L(locale, "Link company"), kind: "control" },
        { name: L(locale, "Remove"), kind: "remove" },
        { name: L(locale, "Show segments"), kind: "control" },
        { name: L(locale, "Add to notebook"), kind: "add" },
      ]),
    );
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
    expect(
      within(region).getByRole("button", { name: L(locale, "Add to notebook") }).getAttribute("data-ux-primary-action"),
    ).toBe("true");
  });

  it.each(LOCALES)("draft open: primary moves to `saveNote`, the draft's Save note / Discard join the inventory (%s)", async (locale) => {
    const region = await openTranscripts(locale);
    const user = await runJobToCompleted(locale, region);
    await expandRow(locale, region, user);
    await linkCompany(locale, region, user);
    const segments = await within(region).findByLabelText(L(locale, "Transcript segments"));
    await user.click(within(segments).getAllByRole("checkbox")[0]);
    await user.click(within(region).getByRole("button", { name: L(locale, "Add to notebook") }));
    await within(region).findByLabelText(L(locale, "Notebook note draft"));

    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Fetch transcript"), kind: "fetch" },
        { name: L(locale, "Refresh transcripts"), kind: "refresh" },
        { name: L(locale, "Settings"), kind: "destination" },
        { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
        { name: L(locale, "Rename"), kind: "rename" },
        { name: L(locale, "Remove"), kind: "remove" },
        { name: L(locale, "Show segments"), kind: "control" },
        { name: L(locale, "Add to notebook"), kind: "add" },
        { name: L(locale, "Discard"), kind: "control" },
        { name: L(locale, "Save note"), kind: "save" },
        // The draft's follow-up quarter field's own popover toggle (shared
        // `NotebookQuarterField`, not a Transcripts-specific control).
        { name: `${L(locale, "Follow-up quarter")} picker`, kind: "control" },
      ]),
    );
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });

  it.each(LOCALES)("quarter picker open: every popover action is classified and localized (%s)", async (locale) => {
    const region = await openTranscripts(locale);
    const user = await runJobToCompleted(locale, region);
    await expandRow(locale, region, user);
    await linkCompany(locale, region, user);
    const segments = await within(region).findByLabelText(L(locale, "Transcript segments"));
    await user.click(within(segments).getAllByRole("checkbox")[0]);
    await user.click(within(region).getByRole("button", { name: L(locale, "Add to notebook") }));
    await within(region).findByLabelText(L(locale, "Notebook note draft"));
    await user.click(within(region).getByRole("button", { name: `${L(locale, "Follow-up quarter")} picker` }));

    const inventory = collectActionInventory(region, locale);
    // Sol R2: the shared quarter picker's popover actions join the sweep —
    // nothing unclassified, and Today/Clear render through text().
    expect(inventory.filter((entry) => entry.kind === "unclassified")).toEqual([]);
    expect(inventory.some((entry) => entry.name === L(locale, "Today"))).toBe(true);
    expect(inventory.some((entry) => entry.name === L(locale, "Clear"))).toBe(true);
  });

  it.each(LOCALES)("link picker open (with suggestions): the suggestion row is classified `control`, never unclassified (%s)", async (locale) => {
    const region = await openTranscripts(locale);
    const user = await runJobToCompleted(locale, region);
    await linkCompanyPickerOpen(locale, region, user);

    // The completed job stays unlinked (no company chosen yet), so the row
    // still carries `Link company`, and expanding it (a side effect of
    // opening the picker) surfaces the loaded segments' `Add to notebook`.
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Fetch transcript"), kind: "fetch" },
        { name: L(locale, "Refresh transcripts"), kind: "refresh" },
        { name: L(locale, "Settings"), kind: "destination" },
        { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
        { name: L(locale, "Rename"), kind: "rename" },
        { name: L(locale, "Link company"), kind: "control" },
        { name: L(locale, "Remove"), kind: "remove" },
        { name: L(locale, "Show segments"), kind: "control" },
        { name: L(locale, "Add to notebook"), kind: "add" },
        { name: L(locale, "Cancel"), kind: "control" },
        // The suggestion row's accessible name concatenates TickerLabel's
        // aria-label with the company name — no separator between them.
        { name: "GPW:CDRCD PROJEKT S.A.", kind: "control" },
      ]),
    );
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });

  it.each(LOCALES)("rename open: Rename swaps for Save / Cancel (%s)", async (locale) => {
    const region = await openTranscripts(locale);
    const user = userEvent.setup();
    await user.click(within(region).getByRole("button", { name: L(locale, "Rename") }));

    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Fetch transcript"), kind: "fetch" },
        { name: L(locale, "Refresh transcripts"), kind: "refresh" },
        { name: L(locale, "Settings"), kind: "destination" },
        { name: `${L(locale, "Open transcript")}: ${JOB_TITLE}`, kind: "control" },
        { name: L(locale, "Save"), kind: "save" },
        { name: L(locale, "Cancel"), kind: "control" },
        { name: L(locale, "Link company"), kind: "control" },
        { name: L(locale, "Fetch again"), kind: "fetch" },
        { name: L(locale, "Remove"), kind: "remove" },
      ]),
    );
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });

  it.each(LOCALES)("failed row: same shape as default, status/reason carry no extra button (%s)", async (locale) => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
    appTestState.geminiCredentialStatusResponse = {
      ...initialGeminiCredentialStatus,
      configured: true,
      storage: "os_keychain",
    };
    appTestState.transcriptJobsResponse = [
      {
        ...initialTranscriptJobs[0],
        status: "failed",
        errorCode: "provider_not_configured",
        error: "Gemini transcription provider is not configured.",
      },
    ];
    renderApp({ section: "Transcripts" });
    const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
    await within(region).findByText(JOB_TITLE);

    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Fetch transcript"), kind: "fetch" },
        { name: L(locale, "Refresh transcripts"), kind: "refresh" },
        { name: L(locale, "Settings"), kind: "destination" },
        ...collapsedRowEntries(locale),
      ]),
    );
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });

  it.each(LOCALES)("key missing (empty): the ONE action is Open settings (%s)", async (locale) => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
    appTestState.geminiCredentialStatusResponse = { ...initialGeminiCredentialStatus, configured: false };
    appTestState.transcriptJobsResponse = [];
    renderApp({ section: "Transcripts" });
    const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
    await within(region).findByText(L(locale, "Gemini key needed first"));

    expect(collectActionInventory(region, locale)).toEqual(
      sorted([{ name: L(locale, "Open settings"), kind: "destination" }]),
    );
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });

  it.each(LOCALES)("key missing (rows present): the invitation joins the (disabled, quiet) composer and the browsable list (%s)", async (locale) => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
    appTestState.geminiCredentialStatusResponse = { ...initialGeminiCredentialStatus, configured: false };
    appTestState.transcriptJobsResponse = [initialTranscriptJobs[0]];
    renderApp({ section: "Transcripts" });
    const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
    await within(region).findByText(JOB_TITLE);

    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Open settings"), kind: "destination" },
        { name: L(locale, "Refresh transcripts"), kind: "refresh" },
        { name: L(locale, "Fetch transcript"), kind: "fetch" },
        ...collapsedRowEntries(locale),
      ]),
    );
    // No "Settings" composer link while the key is missing (dec. 6 source
    // line only renders once configured) — only Open settings is primary.
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
    expect(
      within(region).getByRole("button", { name: L(locale, "Open settings") }).getAttribute("data-ux-primary-action"),
    ).toBe("true");
    expect(within(region).getByRole("button", { name: L(locale, "Fetch transcript") })).toBeDisabled();
  });
});

async function linkCompanyPickerOpen(locale: Locale, region: HTMLElement, user: UserEvent) {
  await user.click(within(region).getByRole("button", { name: L(locale, "Link company") }));
  await user.type(within(region).getByLabelText(L(locale, "Transcript link company lookup")), COMPANY_QUERY);
  await within(region).findByLabelText(L(locale, "Transcript link company suggestions"));
}
