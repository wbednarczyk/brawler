#!/usr/bin/env node
// Guard for `visual-update` (ADR 0081 plan Q5 STOP-AND-ASK): a Playwright
// snapshot update must never run silently. Requires SCREEN (the
// tests/browser/visual/catalog.ts screen id this update targets, used as a
// Playwright title grep) and a non-empty REASON; prints both into the run log
// so the change description can cite them, then delegates to Playwright's own
// --update-snapshots against the two visual projects.
import { spawnSync } from "node:child_process";

const screen = process.env.SCREEN?.trim();
const reason = process.env.REASON?.trim();

if (!screen || !reason) {
  console.error('Usage: SCREEN=<name> REASON="why this baseline changed" npm run visual-update');
  process.exit(1);
}

console.log(`visual-update: SCREEN=${screen} REASON=${reason}`);

const result = spawnSync(
  "npx",
  [
    "playwright",
    "test",
    "--project=chromium-visual",
    "--project=chromium-visual-light",
    "--update-snapshots",
    "-g",
    screen,
  ],
  { stdio: "inherit" },
);
process.exit(result.status ?? 1);
