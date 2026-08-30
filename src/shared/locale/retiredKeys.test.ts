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
