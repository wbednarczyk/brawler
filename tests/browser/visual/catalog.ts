// Contact-sheet screen catalog (ADR 0081 plan Q5, Radicle `81313f0`).
//
// Typed re-export of the plain-JS catalog core (`catalog.core.mjs`), which is
// the single source of truth mapping every stable "screen" id shot by the
// visual baseline suite (`tests/browser/visual/*.spec.ts`) to the spec file
// that owns it and the states/tiers that spec shoots for it. Kept as a
// separate `.mjs` module (not authored here as `.ts`) so plain Node — the nix
// devshell's pinned version included — can import it from
// `scripts/ux/contact-sheet.mjs` without relying on TypeScript type-stripping
// support, which isn't available on every Node this repo runs on.
//
// `helpers.ts` validates every `shootPanel`/`shootScreen` call against this
// catalog (a call for an uncataloged screen/state throws instead of silently
// shooting unreviewed evidence); `scripts/ux/contact-sheet.mjs` uses the core
// module to resolve `--screens`/`--changed` into the spec files to run and
// the cells the emitted HTML must contain.

import * as core from "./catalog.core.mjs";

export type Tier = "S" | "M" | "L";

export interface CatalogEntry {
  /** Stable screen id, e.g. "fundamentals" — the first arg to shootPanel/shootScreen. */
  screen: string;
  /** Spec filename (basename only) under tests/browser/visual/ that owns this screen. */
  spec: string;
  /** States this screen is shot in. Every current spec shoots a single "default" state. */
  states: string[];
  /**
   * Dark-project tiers this screen produces (light always shoots M only, per
   * the shoot helpers). Most screens are forced across S/M/L; a bare
   * `shootScreen` call with no forced tiers (today, spolka-rest,
   * spolka-tool-claims) is M-only.
   */
  tiers: Tier[];
}

export interface ChangedResolution {
  screens: string[];
  unknown: string[];
}

export interface ExpectedCell {
  screen: string;
  state: string;
  tier: Tier;
  theme: "dark" | "light";
}

export const CATALOG: CatalogEntry[] = core.CATALOG;
export const SHARED_PREFIXES: string[] = core.SHARED_PREFIXES;

export const validateCatalog: (list: CatalogEntry[]) => CatalogEntry[] = core.validateCatalog;
export const findCatalogEntry: (screen: string) => CatalogEntry | undefined = core.findCatalogEntry;
export const assertCataloged: (screen: string, state: string) => CatalogEntry = core.assertCataloged;
export const resolveChangedFiles: (files: string[]) => ChangedResolution = core.resolveChangedFiles;
export const expectedCells: (
  screens: string[],
  state?: string,
  theme?: "dark" | "light",
) => ExpectedCell[] = core.expectedCells;
