// Contact-sheet screen catalog — plain-JS core (ADR 0081 plan Q5, Radicle
// `81313f0`).
//
// Single source of truth mapping every stable "screen" id shot by the visual
// baseline suite (`tests/browser/visual/*.spec.ts`) to the spec file that
// owns it and the states/tiers that spec shoots for it. `catalog.ts`
// re-exports this module with TypeScript types layered on for the Playwright
// side (`helpers.ts`); `scripts/ux/contact-sheet.mjs` imports it directly.
//
// Deliberately plain JavaScript (no `.ts`): the CLI must run unmodified under
// every Node this repo runs on, including the nix devshell's pinned version,
// which does not enable TypeScript type-stripping by default — importing a
// `.ts` file there throws `ERR_UNKNOWN_FILE_EXTENSION`.

const FULL_TIERS = ["S", "M", "L"];
const M_ONLY = ["M"];

const RAW_CATALOG = [
  { screen: "fundamentals", spec: "visual-companies.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "company-feed", spec: "visual-companies.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "inbox", spec: "visual-inbox-sources.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "sources", spec: "visual-inbox-sources.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "notebook-company", spec: "visual-notebook-claims.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "notebooks-global", spec: "visual-notebook-claims.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "claims", spec: "visual-notebook-claims.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "quality", spec: "visual-quality-docs.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "report-documents", spec: "visual-quality-docs.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "coverage", spec: "visual-quality-docs.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "research", spec: "visual-research-events.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "events", spec: "visual-research-events.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "report-season", spec: "visual-research-events.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "today", spec: "visual-shell-today.spec.ts", states: ["default"], tiers: M_ONLY },
  { screen: "cockpit-shell", spec: "visual-shell-today.spec.ts", states: ["default"], tiers: M_ONLY },
  { screen: "compare", spec: "visual-compare.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "watchlists", spec: "visual-utility.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "transcripts", spec: "visual-utility.spec.ts", states: ["default"], tiers: FULL_TIERS },
  { screen: "settings", spec: "visual-utility.spec.ts", states: ["default"], tiers: FULL_TIERS },
];

/** Validates a catalog list: rejects duplicate or empty screen ids and entries with no states. */
export function validateCatalog(list) {
  const seen = new Set();
  for (const entry of list) {
    if (!entry.screen) {
      throw new Error(`catalog entry missing a screen id (spec: ${entry.spec || "<missing>"})`);
    }
    if (seen.has(entry.screen)) {
      throw new Error(`duplicate catalog screen id "${entry.screen}"`);
    }
    seen.add(entry.screen);
    if (!entry.spec) {
      throw new Error(`catalog entry "${entry.screen}" is missing its owning spec`);
    }
    if (!entry.states || entry.states.length === 0) {
      throw new Error(`catalog entry "${entry.screen}" has no supported states`);
    }
  }
  return list;
}

export const CATALOG = validateCatalog(RAW_CATALOG);

export function findCatalogEntry(screen) {
  return CATALOG.find((entry) => entry.screen === screen);
}

/**
 * A visual case (`shootPanel`/`shootScreen` call) without a matching catalog
 * entry — or a state that entry doesn't declare — is a wiring bug: it would
 * shoot evidence the contact sheet can never place. Throws instead of
 * silently proceeding.
 */
export function assertCataloged(screen, state) {
  const entry = findCatalogEntry(screen);
  if (!entry) {
    throw new Error(
      `visual case "${screen}" has no tests/browser/visual/catalog.ts entry — add one before shooting`,
    );
  }
  if (!entry.states.includes(state)) {
    throw new Error(
      `visual case "${screen}" has no catalog state "${state}" (known: ${entry.states.join(", ")}) ` +
        `— add it to tests/browser/visual/catalog.ts`,
    );
  }
  return entry;
}

// A change under any of these repo-relative prefixes affects every cataloged
// screen: shared UI primitives, global styles, and the app shell.
export const SHARED_PREFIXES = ["src/ui/", "src/styles/", "src/app/"];

/**
 * Read-only mapping from changed file paths (repo-relative, POSIX separators,
 * as `git diff --name-only` emits) to the catalog screens they affect. A file
 * under a shared prefix affects every screen; a file that is itself one of
 * the visual specs affects the screens that spec owns; anything else is
 * reported as `unknown` so the caller can refuse to silently select nothing.
 */
export function resolveChangedFiles(files) {
  const screens = new Set();
  const unknown = [];

  for (const file of files) {
    if (SHARED_PREFIXES.some((prefix) => file.startsWith(prefix))) {
      for (const entry of CATALOG) screens.add(entry.screen);
      continue;
    }

    const specMatch = file.match(/^tests\/browser\/visual\/(visual-[\w-]+\.spec\.ts)$/);
    if (specMatch) {
      const owners = CATALOG.filter((entry) => entry.spec === specMatch[1]);
      if (owners.length === 0) {
        unknown.push(file);
        continue;
      }
      for (const entry of owners) screens.add(entry.screen);
      continue;
    }

    unknown.push(file);
  }

  return { screens: [...screens], unknown };
}

/**
 * Every cell the contact sheet must contain for a given screen selection:
 * dark shoots every declared tier, light shoots M only (mirroring the shoot
 * helpers' `lightPass()` gate) — skipped for a screen whose dark tiers don't
 * include M (none currently, but kept honest rather than assumed). `theme`
 * narrows to just "dark" or "light" cells (matching a `--theme` CLI filter
 * that only ran one project); omitted/undefined expects both.
 *
 * @param {string[]} screens
 * @param {string} [state]
 * @param {"dark" | "light"} [theme]
 */
export function expectedCells(screens, state = "default", theme = undefined) {
  /** @type {{ screen: string, state: string, tier: *, theme: "dark" | "light" }[]} */
  const cells = [];
  for (const screen of screens) {
    const entry = assertCataloged(screen, state);
    if (theme !== "light") {
      for (const tier of entry.tiers) cells.push({ screen, state, tier, theme: "dark" });
    }
    if (theme !== "dark" && entry.tiers.includes("M")) {
      cells.push({ screen, state, tier: "M", theme: "light" });
    }
  }
  return cells;
}
