// Guard (harvest 2026-08-19): every visual case a spec shoots must have a
// tests/browser/visual/catalog.core.mjs entry. The shoot helpers enforce this
// only in contact-sheet mode, so an uncataloged case stays green in ui-smoke
// and detonates the first time someone builds a contact sheet ("basic-info").
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const VISUAL_DIR = path.resolve(fileURLToPath(new URL("../../tests/browser/visual", import.meta.url)));

test("every shot visual case has a catalog entry", async () => {
  const { CATALOG } = await import(path.join(VISUAL_DIR, "catalog.core.mjs"));
  const cataloged = new Set(CATALOG.map((entry) => entry.screen));
  const shot = new Set();
  for (const file of readdirSync(VISUAL_DIR).filter((f) => f.endsWith(".spec.ts"))) {
    const source = readFileSync(path.join(VISUAL_DIR, file), "utf8");
    for (const match of source.matchAll(/shoot(?:Panel|Region)\([^,]+,[^,]+,\s*"([^"]+)"/g)) {
      shot.add(match[1]);
    }
  }
  assert.ok(shot.size > 0, "no shoot calls found — the extraction regex rotted");
  const missing = [...shot].filter((name) => !cataloged.has(name)).sort();
  assert.deepEqual(missing, [], `visual cases shot but not cataloged: ${missing.join(", ")}`);
});
