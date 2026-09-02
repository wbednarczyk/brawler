// Detects whether the process is running inside the official Playwright
// docker image pinned to the installed @playwright/test version — the only
// place pixel baselines may be generated/compared (#448).
import { readFileSync } from "node:fs";

const PINNED_INFO_PATH = "/ms-playwright/.docker-info";

// The locked @playwright/test version — from package-lock.json, the same source
// the Makefile derives the image tag from, and readable without node_modules
// (the docs-gates job runs these scripts on a bare checkout).
function lockedVersion() {
  const lock = JSON.parse(readFileSync(new URL("../../package-lock.json", import.meta.url), "utf8"));
  return lock.packages["node_modules/@playwright/test"].version;
}

export function pinnedImage(version = lockedVersion()) {
  return `mcr.microsoft.com/playwright:v${version}-noble`;
}

export function pinnedRenderer({ infoPath = PINNED_INFO_PATH, version = lockedVersion() } = {}) {
  try {
    return JSON.parse(readFileSync(infoPath, "utf8"))?.dockerImageName === pinnedImage(version);
  } catch {
    return false;
  }
}
