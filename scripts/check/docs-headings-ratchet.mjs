#!/usr/bin/env node
// Canonical-doc heading ratchet (harvest 2026-09-10, #462 / ADR 0110): a
// scripted section replacement in docs/data-model.md once swallowed the four
// `##` sections after the one it meant to retire — docs-drift and
// retired-surface both stayed green because no command or token vanished.
// This gate pins the `##`/`###` heading inventory of the canonical docs in
// docs-headings-baseline.json: a heading may be ADDED freely; a heading that
// disappears reddens until the removal is written into the baseline on
// purpose (`--write`), which makes the deletion a reviewed diff line.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const BASELINE = join(ROOT, "docs-headings-baseline.json");
export const DOCS = [
  "docs/contracts.md",
  "docs/data-model.md",
  "docs/ui-information-architecture.md",
  "docs/product-spec.md",
  "docs/architecture.md",
];

export function headingsOf(markdown) {
  const out = [];
  // CommonMark fence tracking: a fence opens with ≥3 backticks or tildes and
  // closes only with the SAME character and at least the same length, so a
  // ```` block that contains ``` (and a fake heading) stays one fence.
  let fence = null;
  for (const line of markdown.split("\n")) {
    const f = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
    if (f) {
      const [, run, rest] = f;
      if (!fence) {
        fence = run;
        continue;
      }
      // A closing fence carries nothing but whitespace after the delimiter
      // (CommonMark): ```not-a-close inside a block does not close it.
      if (run[0] === fence[0] && run.length >= fence.length && /^[ \t]*$/.test(rest)) {
        fence = null;
        continue;
      }
    }
    if (fence) continue;
    // Optional closing sequence: `## B ##` → "B"; a literal `## C#` keeps its hash.
    const m = /^(##|###) +(.+?)(?: +#+)?\s*$/.exec(line);
    if (m) out.push(`${m[1]} ${m[2]}`);
  }
  return out;
}

export function compare(baseline, current) {
  const missing = {};
  for (const [doc, headings] of Object.entries(baseline)) {
    const have = new Set(current[doc] ?? []);
    const gone = headings.filter((h) => !have.has(h));
    if (gone.length) missing[doc] = gone;
  }
  return missing;
}

function inventory(root) {
  const current = {};
  for (const doc of DOCS) current[doc] = headingsOf(readFileSync(join(root, doc), "utf8"));
  return current;
}

function main(argv) {
  const write = argv.includes("--write");
  const current = inventory(ROOT);
  if (write) {
    writeFileSync(BASELINE, `${JSON.stringify(current, null, 2)}\n`);
    console.log(`docs-headings-ratchet: baseline written (${DOCS.length} docs).`);
    return 0;
  }
  const baseline = JSON.parse(readFileSync(BASELINE, "utf8"));
  const missing = compare(baseline, current);
  const docs = Object.keys(missing);
  if (docs.length === 0) {
    console.log("✓ docs-headings-ratchet: every pinned canonical heading is still present.");
    return 0;
  }
  for (const doc of docs) {
    console.error(`✖ ${doc}: heading(s) gone — ${missing[doc].map((h) => `"${h}"`).join(", ")}`);
  }
  console.error(
    "  A canonical section vanished. If the removal is deliberate (a retirement with its\n" +
      "  pointer in place), run `node scripts/check/docs-headings-ratchet.mjs --write` and\n" +
      "  commit the baseline diff; if not, the edit swallowed a neighbour — restore it.",
  );
  return 1;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exit(main(process.argv.slice(2)));
}
