#!/usr/bin/env node
// Disk-space guard (ADR 0045 guardrail-harvest, 2026-07-11).
//
// The failure this closes: on 2026-07-11 the Windows C: drive filled up and
// killed the dev session (Claude Code process + three in-flight agents) MID-WORK.
// The trap is WSL-specific and invisible from inside: WSL2 stores the whole
// filesystem in a grow-only ext4.vhdx on the host drive, so `df /` inside WSL
// showed ~790 GiB free while writes were failing because C: (backing the vhdx)
// had 0. A guard that only checks the WSL root would never fire.
//
// So this guard checks BOTH levels and fails the gate BEFORE the session-killer:
//   1. the repo's own filesystem (works everywhere, incl. CI), and
//   2. on WSL, every mounted host drive backing the environment (/mnt/c).
//
// Remedy it prints: `make disk-clean` (safe caches) / `make disk-clean-deep`
// (adds cargo target + full nix GC) + the host-side vhdx compaction note —
// freeing space INSIDE WSL does not shrink the vhdx on the host by itself.
//
// Thresholds (GiB, env-overridable for tests):
//   BRAWLER_DISK_FAIL_GIB (default 10) — hard-fail the gate
//   BRAWLER_DISK_WARN_GIB (default 40) — loud warning + remedy
//
// Instant: statfs only, no directory walks. No dependencies.

import { statfsSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

// CI skip (ADR 0090): GitHub-hosted runners are ephemeral and set CI=true. The
// guard exists to stop a full HOST drive from killing a long-lived WSL dev
// session mid-work; on a throwaway runner it only false-fails on the small
// preinstalled-toolchain footprint (the disk-reclaim step handles real space).
if (process.env.CI === "true") {
  console.log("✓ disk-guard: skipped under CI (ephemeral runner; disk reclaim handled in the workflow).");
  process.exit(0);
}

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const FAIL_GIB = Number(process.env.BRAWLER_DISK_FAIL_GIB ?? 10);
const WARN_GIB = Number(process.env.BRAWLER_DISK_WARN_GIB ?? 40);
const GIB = 1024 ** 3;

function freeGib(path) {
  const s = statfsSync(path);
  // bavail: blocks available to unprivileged users — what a build can actually use.
  return (s.bavail * s.bsize) / GIB;
}

const checks = [{ label: "repo filesystem (WSL root)", path: repoRoot }];
// Host drives backing a WSL environment. /mnt/c is the standard drvfs mount of
// the drive that holds ext4.vhdx; skip silently when absent (CI, bare Linux).
for (const drive of ["/mnt/c"]) {
  if (existsSync(drive)) checks.push({ label: `host drive ${drive} (backs the WSL vhdx)`, path: drive });
}

const remedy = [
  "  → make disk-clean       safe: package-manager caches, mutants artifacts, old nix generations, journal, fstrim",
  "  → make disk-clean-deep  adds: cargo target/ (~tens of GiB, forces a full rebuild) + full nix GC",
  "  → WSL only: freeing space inside WSL does NOT shrink the vhdx on the host —",
  "    from Windows PowerShell (admin): wsl --shutdown; wsl --manage <distro> --set-sparse true",
].join("\n");

let failed = false;
let warned = false;
for (const { label, path } of checks) {
  let free;
  try {
    free = freeGib(path);
  } catch {
    continue; // unreadable mount (e.g. detached drive) — not this guard's failure
  }
  const pretty = `${free.toFixed(1)} GiB free on ${label}`;
  if (free < FAIL_GIB) {
    console.error(`✖ disk-guard: ${pretty} — below the ${FAIL_GIB} GiB hard floor. A full disk kills sessions mid-work; clean up before continuing.\n${remedy}`);
    failed = true;
  } else if (free < WARN_GIB) {
    console.warn(`⚠ disk-guard: ${pretty} — below the ${WARN_GIB} GiB warning line. Clean up soon:\n${remedy}`);
    warned = true;
  }
}

if (failed) process.exit(1);
if (!warned) console.log("✓ disk-guard: free space healthy on all checked filesystems.");
