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
  let fenced = false;
  for (const line of markdown.split("\n")) {
    if (/^\s*(```|~~~)/.test(line)) fenced = !fenced;
    if (fenced) continue;
    const m = /^(##|###) +(.+?)\s*#*\s*$/.exec(line);
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
