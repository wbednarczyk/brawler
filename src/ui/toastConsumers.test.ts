import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// Enforcement (ADR 0097 dec. 1/3): a toast is feedback for a DIRECT USER ACTION
// — ambient/system attention lives in the Today stream + the sidebar badge and
// must never surface as a toast again. Deleting the `persistent` variant does
// not make a future system toast inexpressible (any background effect can call
// `toast.show`), so this allowlist pins the production consumers of `useToast`.
// Adding a file here is a deliberate review decision: is the new call site
// feedback for something the user just did?
const ALLOWED_CONSUMERS = new Set([
  // The primitive itself + the reversible-destroy orchestration (ADR 0076 D5).
  "src/ui/Toast.tsx",
  "src/ui/useUndoableDelete.ts",
  // Dev-only gallery preview (not part of the app shell).
  "src/ui/PrimitiveGallery.tsx",
  // User-action feedback call sites.
  "src/app/AppStateRoot.tsx", // manual "Sources refreshed" + view-rename errors
  "src/screens/Settings/ImportExportSettings.tsx", // "Import applied"
  "src/shared/components/CompanyBasicInfoPanel.tsx", // ownership backfill started
  "src/shared/components/CompanyReportDocumentsPanel.tsx", // document actions
]);

/** Every production .ts/.tsx file under src/ (tests and src/test excluded). */
function productionSources(dir: string, acc: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      if (entry === "test" && relative(process.cwd(), path) === join("src", "test")) continue;
      productionSources(path, acc);
      continue;
    }
    if (!/\.(ts|tsx)$/.test(entry) || /\.test\.(ts|tsx)$/.test(entry)) continue;
    acc.push(path);
  }
  return acc;
}

describe("Toast consumer allowlist (ADR 0097)", () => {
  it("only enumerated action-feedback files consume useToast in production code", () => {
    const offenders = productionSources(join(process.cwd(), "src"))
      .filter((path) => /\buseToast\s*\(/.test(readFileSync(path, "utf8")))
      .map((path) => relative(process.cwd(), path).replace(/\\/g, "/"))
      .filter((path) => !ALLOWED_CONSUMERS.has(path));
    expect(
      offenders,
      `New useToast consumer(s) outside the action-feedback allowlist — a toast is feedback ` +
        `for a direct user action, never an ambient/system event (ADR 0097). If this call ` +
        `site is genuine action feedback, add it to ALLOWED_CONSUMERS deliberately: ` +
        offenders.join(", "),
    ).toEqual([]);
  });

  it("keeps the allowlist free of stale entries", () => {
    const consumers = new Set(
      productionSources(join(process.cwd(), "src"))
        .filter((path) => /\buseToast\s*\(/.test(readFileSync(path, "utf8")))
        .map((path) => relative(process.cwd(), path).replace(/\\/g, "/")),
    );
    const stale = [...ALLOWED_CONSUMERS].filter((path) => !consumers.has(path));
    expect(stale, `Allowlisted files no longer consuming useToast: ${stale.join(", ")}`).toEqual(
      [],
    );
  });
});
