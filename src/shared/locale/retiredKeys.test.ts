import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { en } from "./resources/en";
import { pl } from "./resources/pl";
import { plText } from "./resources/plText";

// F4b S2 (docs/plans/f4b-contracts/s2-transcripts.md item 6): the Transcripts
// redesign retires the "transcript job" vocabulary. This pins the retirement
// shut — both directions: the keys are gone from every resource table AND no
// `src/**` call site still names them (a stray `text("Create job")` would
// otherwise silently render as its own English fallback, invisible to any
// other gate).
const RETIRED_T_KEYS = ["action.refreshJobs", "empty.noTranscriptJobs", "error.transcriptJobsUnavailable"];

const RETIRED_PLTEXT_KEYS = [
  "Create transcript job",
  "New transcript job",
  "URL is required. Description and company are optional.",
  "Create job",
  "Transcript URL",
  "Transcript description",
  "Open transcript job",
  "Untitled transcript",
  "Unlinked transcript",
  "Transcript job metadata for",
  "Retry Gemini transcription",
  "Delete transcript job",
  "Stored transcript segments for this job will also be removed.",
  "Transcript job details",
  "Transcript description editor",
  "Edit transcript description",
  "Selected",
  "Transcript job error",
  "Transcript job failed",
  "No detailed provider error was stored.",
  "Optional company link",
  "Keep this transcript unlinked, or link it when selected segments should become a company notebook note.",
  "Create a company notebook draft from selected segments",
  "Link a company before saving selected segments as a company notebook note",
  "Create company note draft",
  "Transcript note draft",
  "Transcript segments will be available after the job completes.",
  "No transcript segments stored for this job.",
  "Search transcript",
  "Clear transcript search",
  "Transcript engine settings",
  "Transcript jobs",
  "Starting a transcript job sends the YouTube URL and video content to Gemini.",
];

// Dogfooding wave 2026-09, S1. Commit 1 (#6): the Pozycje × okresy "Show all
// periods" / "Show fewer periods" disclosure button is replaced by the
// shared period-expander column (useVisiblePeriods.ts). Commit 2 (#4): the
// read-only Reporting periods list (restated the matrix headers) and the
// Fundamentals Autopilot fold both retire — Companies → Manage settings is
// the only autopilot editor now (ADR 0056 amendment); its description text
// was exclusive to the retired `CompanyAutopilotField` (the option-label
// keys "Off — manual" etc. stay — CompanySettingsManager.tsx still uses
// them). All plText-only keys.
const RETIRED_PLTEXT_KEYS_WAVE_S1 = [
  "Show all periods",
  "Show fewer periods",
  "Reporting periods",
  "No reporting periods yet.",
  "Automatically process this company's new reports on the next source refresh. Off keeps everything manual; Assist auto-fetches and extracts but you confirm each value; Autopilot also auto-confirms extracted values as unreviewed (cited and reversible).",
  // Wave S2/S3 (dogfooding #1b, #12): the Activity destination is "Open in
  // documents"; the KPI ticket's control is the thread button named
  // "Open source: …" — both old keys lost their last call site.
  "Open document",
  "Open source document",
];

const SCAN_ROOT = "src";
const SCAN_EXTENSIONS = [".ts", ".tsx"];
// This file and the resource tables carry the retired tokens as data (the
// list above, and — during the migration window — nothing at all); excluding
// them keeps the scan honest about LIVE call sites only.
const SCAN_EXCLUDE_SUFFIXES = [
  "shared/locale/retiredKeys.test.ts",
  "shared/locale/resources/plText.ts",
  "shared/locale/resources/en.ts",
  "shared/locale/resources/pl.ts",
];

function listSourceFiles(root: string, dir: string, out: string[]): string[] {
  for (const entry of readdirSync(join(root, dir))) {
    const rel = dir ? `${dir}/${entry}` : entry;
    const full = join(root, rel);
    if (statSync(full).isDirectory()) {
      listSourceFiles(root, rel, out);
    } else if (SCAN_EXTENSIONS.some((ext) => rel.endsWith(ext))) {
      if (!SCAN_EXCLUDE_SUFFIXES.some((suffix) => rel.endsWith(suffix))) out.push(rel);
    }
  }
  return out;
}

// F4c S1 (docs/plans/f4c-contracts/s1-guardrails.md item 3, plan §
// Decisions 1, 7b): pins the Notebooks-global screen's key family (retired by
// S2 — decision 1 deletes `src/screens/Notebooks/**`) and the Research
// tooltip-only labels the dec. 3/4 relabel retires (retired by S3). Both
// families are RED today — the keys/call sites still exist; S2/S3 make this
// green. `notebooks.title`/`notebooks.description` are typed keys
// (`en.ts:68-69`, `pl.ts:70-71`); the rest are `plText` keys.
const RETIRED_T_KEYS_F4C = ["notebooks.title", "notebooks.description"];

const RETIRED_PLTEXT_KEYS_F4C = [
  // plText.ts:829-872 — the Notebooks-global-screen-exclusive keys: every
  // `Notebooks workspace` / `Notebook companies` / `Notebook screen …` /
  // `Notebook *filter*` key in that block (verified against src/** call
  // sites: each name below resolves ONLY inside `src/screens/Notebooks/**`
  // today).
  "Notebooks workspace",
  "Notebook companies",
  "Notebook screen entries",
  "Notebook watchlist filter",
  "Notebook filter reset",
  "Notebook filters",
  "Notebook kind filter",
  "Notebook claim status filter",
  "Notebook tag filter",
  "Notebook follow-up filter",
  "Notebook screen note title",
  "Notebook screen note kind",
  "Notebook screen note tags",
  "Notebook screen note claim status",
  "Notebook screen note event date",
  "Notebook screen note follow-up quarter",
  "Notebook screen note follow-up date",
  "Notebook screen note body",
  "Select notebook screen entry",
  "Notebook screen entry detail",
  "Notebook screen selected title",
  "Notebook screen selected body",
  "Notebook screen selected kind",
  "Notebook screen selected claim status",
  "Notebook screen selected tags",
  "Notebook screen selected event date",
  "Notebook screen selected follow-up quarter",
  "Notebook screen selected follow-up date",
  // plText.ts:1115, :1131-1134 — the "Open Notebooks" shortcut/palette entry
  // and the notebook-entry-editor shortcut copy (`app.openNotebooks`
  // retires; decision 3: Ctrl+4 moves to Research).
  "Open Notebooks",
  "Open notebook entry editor",
  "Save notebook edit",
  "Notebooks",
  // F4c S2 (contract "Notes from S1"): the non-pattern-named keys that were
  // ALSO exclusive to the deleted Notebooks-global screen — verified with
  // `grep -rn 'text("<key>")\|t("<key>")' src` after S2's deletions: zero
  // live call sites for any of these.
  "Open notebook company",
  "notebook entries for",
  "Show open claims for",
  "Show follow-ups for",
  "Open company workspace",
  "Add companies before using notebooks.",
  "No notebook companies match this watchlist.",
  "No company selected",
  "visible note",
  "visible notes",
  "Notebook follow-up summary",
  "Tag",
  "tag",
  "Has follow-up",
  "No follow-up",
  "No notes for",
  // Research tooltip-only labels the dec. 3/4 relabel retires (S3):
  // `Mark reviewed` → `Mark as reviewed`; `Delete reminder`/`Delete research
  // question` → `Remove reminder`/`Remove question`; `Snooze reminder` →
  // `Snooze`; `Complete reminder` → `Mark as done`; `Reopen reminder` stays
  // `Reopen` but drops the tooltip-only "reminder" suffix once it carries a
  // visible label; `Link evidence`/`Open evidence`/`Open source URL` →
  // `Link to question`/`Open`/`Open source`.
  "Mark reviewed",
  "Delete reminder",
  "Delete research question",
  "Snooze reminder",
  "Complete reminder",
  "Reopen reminder",
  "Link evidence",
  "Open evidence",
  "Open source URL",
];

// F4c S4 (docs/plans/f4c-contracts/s4-settings-pass-banner.md, sol R2
// amendment): `AI workers` was a dead `plText` key — no live `text()` call
// site (ADR 0084 already retired the AI-routing UI it once labeled) — with
// only a negative assertion in `SettingsScreen.test.tsx` proving its absence.
// Pinned here so it can't quietly reappear.
const RETIRED_PLTEXT_KEYS_S4 = ["AI workers"];

// wave2 S1 (#454): SPOLKA_TOOL_COMMANDS labels move to the "Open tool:
// <Name>" family — the flat "Open fundamentals"-style labels retire. "Open
// notebook" is EXCLUDED: TranscriptJobRow.tsx still renders it for its own
// unrelated "open the linked note" destination (verified — the only other
// live call site of any of these labels).
const RETIRED_PLTEXT_KEYS_WAVE2_S1 = [
  "Open overview",
  "Open fundamentals",
  "Open feed",
  "Open coverage",
  "Open recommendations",
  "Open claims",
  "Open decision journal",
  "Open quality",
  "Open report diff",
  "Open research",
  "Open ownership",
  "Open signals",
  "Open documents",
  "Open events",
];

describe("retired Fundamentals vocabulary stays retired (dogfooding wave 2026-09, S1)", () => {
  it("is absent from the plText resource table", () => {
    const hits = RETIRED_PLTEXT_KEYS_WAVE_S1.filter((key) => key in plText).map((key) => `plText.ts: ${key}`);
    expect(hits, `Retired keys still present:\n${hits.join("\n")}`).toEqual([]);
  });

  it("is absent from every src/** call site", () => {
    const files = listSourceFiles(process.cwd(), SCAN_ROOT, []);
    const needles = RETIRED_PLTEXT_KEYS_WAVE_S1.flatMap((token) => [
      `text("${token}")`,
      `t("${token}")`,
      `text('${token}')`,
      `t('${token}')`,
    ]);
    const hits: string[] = [];
    for (const rel of files) {
      const content = readFileSync(join(process.cwd(), rel), "utf8");
      for (const needle of needles) {
        if (content.includes(needle)) hits.push(`${rel}: ${needle}`);
      }
    }
    expect(hits, `Retired keys still referenced:\n${hits.join("\n")}`).toEqual([]);
  });
});

describe("retired 'AI workers' vocabulary stays retired (F4c S4)", () => {
  it("is absent from the plText resource table", () => {
    const hits = RETIRED_PLTEXT_KEYS_S4.filter((key) => key in plText).map((key) => `plText.ts: ${key}`);
    expect(hits, `Retired keys still present:\n${hits.join("\n")}`).toEqual([]);
  });

  it("is absent from every src/** call site", () => {
    const files = listSourceFiles(process.cwd(), SCAN_ROOT, []);
    const needles = RETIRED_PLTEXT_KEYS_S4.flatMap((token) => [
      `text("${token}")`,
      `t("${token}")`,
      `text('${token}')`,
      `t('${token}')`,
    ]);
    const hits: string[] = [];
    for (const rel of files) {
      const content = readFileSync(join(process.cwd(), rel), "utf8");
      for (const needle of needles) {
        if (content.includes(needle)) hits.push(`${rel}: ${needle}`);
      }
    }
    expect(hits, `Retired keys still referenced:\n${hits.join("\n")}`).toEqual([]);
  });
});

describe("retired Notebooks-global-screen + Research tooltip-only vocabulary (F4c S1)", () => {
  it("is absent from every locale resource table", () => {
    const hits = [
      ...RETIRED_T_KEYS_F4C.filter((key) => key in en).map((key) => `en.ts: ${key}`),
      ...RETIRED_T_KEYS_F4C.filter((key) => key in pl).map((key) => `pl.ts: ${key}`),
      ...RETIRED_PLTEXT_KEYS_F4C.filter((key) => key in plText).map((key) => `plText.ts: ${key}`),
    ];
    expect(hits, `Retired keys still present:\n${hits.join("\n")}`).toEqual([]);
  });

  it("is absent from every src/** call site", () => {
    const files = listSourceFiles(process.cwd(), SCAN_ROOT, []);
    const retiredTokens = [...RETIRED_T_KEYS_F4C, ...RETIRED_PLTEXT_KEYS_F4C];
    const needles = retiredTokens.flatMap((token) => [
      `text("${token}")`,
      `t("${token}")`,
      `text('${token}')`,
      `t('${token}')`,
    ]);
    const hits: string[] = [];
    for (const rel of files) {
      const content = readFileSync(join(process.cwd(), rel), "utf8");
      for (const needle of needles) {
        if (content.includes(needle)) hits.push(`${rel}: ${needle}`);
      }
    }
    expect(hits, `Retired keys still referenced:\n${hits.join("\n")}`).toEqual([]);
  });
});

describe("retired SPOLKA_TOOL_COMMANDS flat labels stay retired (wave2 S1, #454)", () => {
  it("is absent from the plText resource table", () => {
    const hits = RETIRED_PLTEXT_KEYS_WAVE2_S1.filter((key) => key in plText).map((key) => `plText.ts: ${key}`);
    expect(hits, `Retired keys still present:\n${hits.join("\n")}`).toEqual([]);
  });

  it("is absent from every src/** call site", () => {
    const files = listSourceFiles(process.cwd(), SCAN_ROOT, []);
    const needles = RETIRED_PLTEXT_KEYS_WAVE2_S1.flatMap((token) => [
      `text("${token}")`,
      `t("${token}")`,
      `text('${token}')`,
      `t('${token}')`,
    ]);
    const hits: string[] = [];
    for (const rel of files) {
      const content = readFileSync(join(process.cwd(), rel), "utf8");
      for (const needle of needles) {
        if (content.includes(needle)) hits.push(`${rel}: ${needle}`);
      }
    }
    expect(hits, `Retired keys still referenced:\n${hits.join("\n")}`).toEqual([]);
  });
});

describe("retired Transcripts 'job' vocabulary stays retired (F4b S2)", () => {
  it("is absent from every locale resource table", () => {
    const hits = [
      ...RETIRED_T_KEYS.filter((key) => key in en).map((key) => `en.ts: ${key}`),
      ...RETIRED_T_KEYS.filter((key) => key in pl).map((key) => `pl.ts: ${key}`),
      ...RETIRED_PLTEXT_KEYS.filter((key) => key in plText).map((key) => `plText.ts: ${key}`),
    ];
    expect(hits, `Retired keys still present:\n${hits.join("\n")}`).toEqual([]);
  });

  it("is absent from every src/** call site", () => {
    const files = listSourceFiles(process.cwd(), SCAN_ROOT, []);
    const retiredTokens = [...RETIRED_T_KEYS, ...RETIRED_PLTEXT_KEYS];
    // Match the `text("<token>")` / `t("<token>")` call shape only — a bare
    // quoted-literal match would also flag unrelated identifiers (e.g.
    // "Selected" inside `dataSelected`) and prose comments that merely quote
    // the retired string while explaining something else.
    const needles = retiredTokens.flatMap((token) => [
      `text("${token}")`,
      `t("${token}")`,
      `text('${token}')`,
      `t('${token}')`,
    ]);
    const hits: string[] = [];
    for (const rel of files) {
      const content = readFileSync(join(process.cwd(), rel), "utf8");
      for (const needle of needles) {
        if (content.includes(needle)) hits.push(`${rel}: ${needle}`);
      }
    }
    expect(hits, `Retired keys still referenced:\n${hits.join("\n")}`).toEqual([]);
  });
});
