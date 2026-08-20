#!/usr/bin/env node
// F0.5 (ADR 0104 dec. 2): the production bundle must ship exactly the four
// committed font subsets (2 faces × latin/latin-ext) — no more (accidental
// extra subsets bloat the desktop bundle), no fewer (a missing subset silently
// falls back to system fonts at runtime, where no gate would catch it). Runs
// right after `npm run build` in check-frontend-build.
import { readdirSync } from "node:fs";

const EXPECTED = [
  "schibsted-grotesk-latin-wght-normal",
  "schibsted-grotesk-latin-ext-wght-normal",
  "jetbrains-mono-latin-wght-normal",
  "jetbrains-mono-latin-ext-wght-normal",
];

let files;
try {
  files = readdirSync("dist/assets");
} catch {
  console.error("font-assets: dist/assets missing — run `npm run build` first");
  process.exit(1);
}

const woff2 = files.filter((f) => f.endsWith(".woff2"));
// "latin" is a prefix of "latin-ext" — anchor each prefix to its hash suffix
// so a subset matches exactly one expected name, one-to-one.
const matchesPrefix = (file, prefix) => new RegExp(`^${prefix}-[^.]*\\.woff2$`).test(file);
const missing = EXPECTED.filter((prefix) => woff2.filter((f) => matchesPrefix(f, prefix)).length !== 1);
const unexpected = woff2.filter((f) => !EXPECTED.some((prefix) => matchesPrefix(f, prefix)));

if (missing.length > 0 || unexpected.length > 0 || woff2.length !== EXPECTED.length) {
  console.error(
    `font-assets: FAIL — subsets without exactly one match: [${missing.join(", ")}]; unexpected font files: [${unexpected.join(", ")}]; present ${woff2.length}/${EXPECTED.length}: [${woff2.join(", ")}]`,
  );
  process.exit(1);
}
console.log(`font-assets: OK — ${woff2.length}/4 bundled font subsets present in dist/assets`);
