import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";
import {
  COMPANY_SPECS,
  makeResearchReminder,
  makeResearchQuestion,
  makeResearchEvidenceItem,
  makeWatchlist,
} from "../../test/scenarios/entities";
import type { ResearchEvidenceItem, ResearchQuestion, ResearchReminder } from "../../api/researchTypes";

// F4c S1 (docs/plans/f4c-contracts/s1-guardrails.md item 4, plan §
// Decisions 4): RED contract skeleton for the Research language pass — S3
// makes every control an `ActionButton` carrying the labels/kinds this table
// names; this file pins the target shape. Shape mirrors
// SourcesScreen.contract.test.tsx (F4b S4). Four states cover the contract's
// named substates: `rest` (reminders + questions + evidence all seeded — the
// FIRST question auto-selects, `useResearchController.ts:191-195`, so both
// the "Active question" panel's status actions AND the evidence row's "Link
// to question" render without any click; `Reopen reminder`, which needs a
// non-open reminder, is the one icon action out of this skeleton's coverage),
// `no-reminders` / `no-questions` (each empties one company-mode collection —
// an `invitation` empty state per plan dec. 4; emptying questions also drops
// the selection, so no "Link to question" there), `watchlist-mode-empty` (a
// watchlist with no member companies and no evidence — the `quiet` empty
// state). Every state fails TODAY on both axes: every button's
// `data-action-kind` is `"unclassified"` (no `ActionButton` yet) and the
// icon-only tooltip actions still carry their pre-F4c labels (`Mark
// reviewed`, `Complete/Snooze/Delete reminder`, `Delete research question`,
// `Link/Open evidence`, `Open source URL`) instead of the dec. 4 relabel.

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];

const REGION_NAME = "Research"; // plText.ts:521 — English name kept in PL too (dec. 4).

const cdr = COMPANY_SPECS.find((spec) => spec.key === "cdr")!;
const reminderFixture: ResearchReminder = makeResearchReminder(cdr);
const questionFixture: ResearchQuestion = makeResearchQuestion(cdr);
const evidenceFixture: ResearchEvidenceItem = makeResearchEvidenceItem(cdr);

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

const LABELS = {
  en: {
    refresh: "Refresh",
    markReviewed: "Mark as reviewed",
    company: "Company",
    watchlist: "Watchlist",
    feedItems: "Feed items",
    notes: "Notes",
    claims: "Claims",
    events: "Events",
    transcripts: "Transcripts",
    signals: "Signals",
    reviewQueueFold: "Review queue",
    questionsFold: "Research questions",
    addReminder: "Add reminder",
    addQuestion: "Add question",
    markAsDone: "Mark as done",
    snooze: "Snooze",
    removeReminder: "Remove reminder",
    removeQuestion: "Remove question",
    statusOpen: "Status open",
    open: "Open",
    openSource: "Open source",
    markAsAnswered: "Mark as answered",
    markAsClosed: "Mark as closed",
    reopenQuestion: "Reopen",
    clearSelection: "Clear selection",
    linkToQuestion: "Link to question",
  },
  pl: {
    refresh: "Odśwież",
    markReviewed: "Oznacz jako przejrzane",
    company: "Spółka",
    watchlist: "Lista obserwowanych",
    feedItems: "Elementy kanału",
    notes: "Notatki",
    claims: "Tezy",
    events: "Wydarzenia",
    transcripts: "Transkrypcje",
    signals: "Sygnały",
    reviewQueueFold: "Do przeglądu",
    questionsFold: "Pytania badawcze",
    addReminder: "Dodaj przypomnienie",
    addQuestion: "Dodaj pytanie",
    markAsDone: "Oznacz jako zrobione",
    snooze: "Odłóż",
    removeReminder: "Usuń przypomnienie",
    // ASSUMPTION: the plan gives no PL text for this one — "remove" verb +
    // object noun, same pattern as "Remove reminder"/"Usuń przypomnienie".
    // S3 owns the final call.
    removeQuestion: "Usuń pytanie",
    statusOpen: "Otwarte",
    open: "Otwórz",
    openSource: "Otwórz źródło",
    // ASSUMPTIONS (not given verbatim by the plan): "Mark as" + the existing
    // "Answered"/"Close" adjective forms already in plText.ts:563-564.
    markAsAnswered: "Oznacz jako odpowiedziane",
    markAsClosed: "Oznacz jako zamknięte",
    // Unchanged (plText.ts:566) — only the `kind` (unclassified → resume) changes.
    reopenQuestion: "Wznów",
    // Unchanged (plText.ts:567) — only the `kind` (unclassified → control) changes.
    clearSelection: "Wyczyść wybór",
    linkToQuestion: "Powiąż z pytaniem",
  },
} as const;

// No space in the DOM: the fold chip's label/count `<span>`s are adjacent
// with no whitespace text node between them (verified against today's render).
function foldName(label: string, count: number): string {
  return `${label}${count}`;
}

// The first question auto-selects (`useResearchController.ts:191-195`), so
// the "Active question" panel's status actions render whenever >=1 question
// is seeded — not only when a state deliberately clicks a row.
function questionActiveInventory(locale: Locale): ActionInventoryEntry[] {
  const t = LABELS[locale];
  return [
    { name: t.markAsAnswered, kind: "markAs" },
    { name: t.markAsClosed, kind: "markAs" },
    { name: t.reopenQuestion, kind: "resume" },
    { name: t.clearSelection, kind: "control" },
  ];
}

// No space (same adjacent-`<span>` DOM shape as the fold chip).
function questionRowName(locale: Locale): string {
  return `${questionFixture.title}${LABELS[locale].statusOpen}`;
}

function headerInventory(locale: Locale): ActionInventoryEntry[] {
  const t = LABELS[locale];
  return [
    { name: t.refresh, kind: "refresh" },
    { name: t.markReviewed, kind: "markAs" },
  ];
}

function scopeBarInventory(locale: Locale): ActionInventoryEntry[] {
  const t = LABELS[locale];
  return [
    { name: t.company, kind: "control" },
    { name: t.watchlist, kind: "control" },
    { name: t.feedItems, kind: "control" },
    { name: t.notes, kind: "control" },
    { name: t.claims, kind: "control" },
    { name: t.events, kind: "control" },
    { name: t.transcripts, kind: "control" },
    { name: t.signals, kind: "control" },
  ];
}

function reminderRowInventory(locale: Locale): ActionInventoryEntry[] {
  const t = LABELS[locale];
  return [
    { name: t.markAsDone, kind: "markAs" },
    { name: t.snooze, kind: "snooze" },
    { name: t.removeReminder, kind: "remove" },
  ];
}

function questionRowInventory(locale: Locale): ActionInventoryEntry[] {
  const t = LABELS[locale];
  return [
    { name: questionRowName(locale), kind: "control" },
    { name: t.removeQuestion, kind: "remove" },
  ];
}

// `withLink`: the evidence row's Link action only renders when a question is
// selected (`canLink` in ResearchEvidencePanel.tsx) — true whenever the
// company-mode question list is non-empty (the first question auto-selects),
// false once the question list is empty or in watchlist mode.
function evidenceRowInventory(locale: Locale, withLink: boolean): ActionInventoryEntry[] {
  const t = LABELS[locale];
  return [
    ...(withLink ? [{ name: t.linkToQuestion, kind: "link" }] : []),
    { name: t.open, kind: "open" },
    { name: t.openSource, kind: "open" },
  ];
}

async function openResearch(
  locale: Locale,
  overrides: { reminders?: ResearchReminder[]; questions?: ResearchQuestion[]; evidence?: ResearchEvidenceItem[] } = {},
) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  appTestState.researchRemindersResponse = overrides.reminders ?? [reminderFixture];
  appTestState.researchQuestionsResponse = overrides.questions ?? [questionFixture];
  appTestState.researchEvidenceItemsResponse = overrides.evidence ?? [evidenceFixture];
  // No pre-seeded evidence link matches this question (filtered by
  // endpointId already), but clearing it keeps the state deterministic
  // rather than relying on that incidental non-match.
  appTestState.evidenceLinksResponse = [];
  renderApp({ section: "Research" });
  const region = await screen.findByRole("region", { name: REGION_NAME });
  // Anchor the wait on the evidence row, not the question title — the
  // question title renders TWICE once a question is seeded (the row + the
  // auto-selected "Active question" panel), which `findByText` on a single
  // string flags as ambiguous.
  await within(region).findByText(evidenceFixture.title);
  return region;
}

async function openWatchlistModeEmpty(locale: Locale, user: ReturnType<typeof userEvent.setup>) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  appTestState.watchlistsResponse = [makeWatchlist("watchlist_empty_contract", "Empty Watch", 0)];
  appTestState.watchlistMembershipsResponse = [];
  appTestState.researchRemindersResponse = [];
  appTestState.researchQuestionsResponse = [];
  appTestState.researchEvidenceItemsResponse = [];
  renderApp({ section: "Research" });
  const region = await screen.findByRole("region", { name: REGION_NAME });
  await user.click(within(region).getByRole("button", { name: LABELS[locale].watchlist }));
  await within(region).findByText(
    locale === "pl" ? "Wybrana lista obserwowana nie ma spółek." : "Selected watchlist has no companies.",
  );
  return region;
}

describe("Research action inventory (F4c contract § Research, plan dec. 4)", () => {
  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, rest state (%s)",
    async (locale) => {
      const region = await openResearch(locale);
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...headerInventory(locale),
          ...scopeBarInventory(locale),
          { name: foldName(LABELS[locale].reviewQueueFold, 1), kind: "control" },
          { name: foldName(LABELS[locale].questionsFold, 1), kind: "control" },
          { name: LABELS[locale].addReminder, kind: "add" },
          ...reminderRowInventory(locale),
          { name: LABELS[locale].addQuestion, kind: "add" },
          ...questionRowInventory(locale),
          ...questionActiveInventory(locale),
          ...evidenceRowInventory(locale, true),
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(LABELS[locale].markReviewed);
    },
  );

  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, no-reminders state (%s)",
    async (locale) => {
      const region = await openResearch(locale, { reminders: [] });
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...headerInventory(locale),
          ...scopeBarInventory(locale),
          { name: foldName(LABELS[locale].reviewQueueFold, 0), kind: "control" },
          { name: foldName(LABELS[locale].questionsFold, 1), kind: "control" },
          { name: LABELS[locale].addReminder, kind: "add" },
          { name: LABELS[locale].addQuestion, kind: "add" },
          ...questionRowInventory(locale),
          ...questionActiveInventory(locale),
          ...evidenceRowInventory(locale, true),
        ]),
      );
      expect(collectEmptyStates(region)).toContain("invitation");
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 0);
    },
  );

  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, no-questions state (%s)",
    async (locale) => {
      const region = await openResearch(locale, { questions: [] });
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...headerInventory(locale),
          ...scopeBarInventory(locale),
          { name: foldName(LABELS[locale].reviewQueueFold, 1), kind: "control" },
          { name: foldName(LABELS[locale].questionsFold, 0), kind: "control" },
          { name: LABELS[locale].addReminder, kind: "add" },
          ...reminderRowInventory(locale),
          { name: LABELS[locale].addQuestion, kind: "add" },
          ...evidenceRowInventory(locale, false),
        ]),
      );
      expect(collectEmptyStates(region)).toContain("invitation");
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(LABELS[locale].markReviewed);
    },
  );

  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, watchlist-mode-empty state (%s)",
    async (locale) => {
      const user = userEvent.setup();
      const region = await openWatchlistModeEmpty(locale, user);
      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...headerInventory(locale),
          ...scopeBarInventory(locale),
          { name: foldName(LABELS[locale].reviewQueueFold, 0), kind: "control" },
          { name: LABELS[locale].addReminder, kind: "add" },
        ]),
      );
      expect(collectEmptyStates(region)).toContain("quiet");
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 0);
    },
  );

  it("no button in the screen root is left unclassified — every action is now classified", async () => {
    const region = await openResearch("en");
    const unclassified = collectActionInventory(region, "en").filter(
      (entry) => entry.kind === "unclassified",
    );
    expect(unclassified).toEqual([]);
  });
});
