#!/usr/bin/env node
// Engines-floor gate (guardrail, ADR 0045 — harvested from the jsdom 30 bump).
//
// Class of defect: a dependency raises its `engines.node` floor above the Node
// the Nix devshell provides. CI catches this at `npm ci` (engine-strict=true in
// .npmrc), but only AFTER push — and a host-installed node_modules can mask it
// locally forever ("host green is not a verdict"). This check reddens in Stage
// 1 of check-fast, which runs inside `nix develop`, so the process.version
// being validated IS the flake's Node — the same one CI uses.
//
// Fix direction when this fails: raise the flake's Node (flake.nix devshells,
// currently pkgsUnstable.nodejs_22), never downgrade the dependency to dodge
// the gate.

import { readFileSync } from "node:fs";
import semver from "semver";

const lock = JSON.parse(readFileSync(new URL("../../package-lock.json", import.meta.url), "utf8"));

const offenders = [];
for (const [path, info] of Object.entries(lock.packages ?? {})) {
  const range = info.engines?.node;
  // Optional deps mirror npm's own behavior: an unsatisfied optional engine
  // is a warning (the dep is skipped), not an install failure.
  if (!range || info.optional) continue;
  if (!semver.validRange(range)) continue; // malformed ranges are npm's problem, not ours
  if (!semver.satisfies(process.version, range)) {
    offenders.push({ name: path.replace(/^node_modules\//, "") || "(root)", range });
  }
}

if (offenders.length > 0) {
  console.error(`engines-floor: Node ${process.version} does not satisfy the engines.node of:`);
  for (const { name, range } of offenders) {
    console.error(`  ${name}: requires ${range}`);
  }
  console.error("Raise the flake's Node (flake.nix devshells) — do not downgrade the dependency.");
  process.exit(1);
}

console.log(`engines-floor: Node ${process.version} satisfies every engines.node in package-lock.json.`);
