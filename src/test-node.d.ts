declare const process: {
  cwd(): string;
};

declare module "node:fs" {
  export function existsSync(path: string): boolean;
  export function readFileSync(path: string, encoding: "utf8"): string;
  export function mkdirSync(path: string, options?: { recursive?: boolean }): void;
  export function writeFileSync(path: string, data: string, encoding?: "utf8"): void;
  export function readdirSync(path: string): string[];
  export function statSync(path: string): { isDirectory(): boolean };
}

declare module "node:path" {
  export function join(...paths: string[]): string;
  export function relative(from: string, to: string): string;
}

// Plain-JS modules shared with node scripts (visual-update guard); typed here
// so src tests can exercise them under the app tsconfig (no @types/node).
declare module "*/catalog.core.mjs" {
  export interface VisualCatalogEntry {
    screen: string;
    spec: string;
    states: string[];
    tiers: ("S" | "M" | "L")[];
  }
  export interface VisualExpectedCell {
    screen: string;
    state: string;
    tier: "S" | "M" | "L";
    theme: "dark" | "light";
  }
  export const CATALOG: VisualCatalogEntry[];
  export function expectedCells(
    screens?: string[],
    state?: string,
    theme?: "dark" | "light",
  ): VisualExpectedCell[];
}

declare module "*/visual-update-core.mjs" {
  import type { VisualExpectedCell } from "*/catalog.core.mjs";

  export function resolveScreen(screenId: string): { spec: string; cells: VisualExpectedCell[] };
  export function specSnapshotDir(spec: string): string;
  export function cellFileName(cell: VisualExpectedCell): string;
  export function allExpectedCells(): VisualExpectedCell[];
  export function diffSnapshots(
    preMap: Record<string, string>,
    postMap: Record<string, string>,
    targetFiles: string[],
  ): { added: string[]; removed: string[]; changedSiblings: string[]; missingTarget: string[] };
}
