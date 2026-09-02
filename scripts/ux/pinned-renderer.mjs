// Detects whether the process is running inside the official Playwright
// docker image pinned to the installed @playwright/test version — the only
// place pixel baselines may be generated/compared (#448).
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";

const installedVersion = createRequire(import.meta.url)("@playwright/test/package.json").version;

const PINNED_INFO_PATH = "/ms-playwright/.docker-info";

export function pinnedImage(version = installedVersion) {
  return `mcr.microsoft.com/playwright:v${version}-noble`;
}

export function pinnedRenderer({ infoPath = PINNED_INFO_PATH, version = installedVersion } = {}) {
  try {
    return JSON.parse(readFileSync(infoPath, "utf8"))?.dockerImageName === pinnedImage(version);
  } catch {
    return false;
  }
}
