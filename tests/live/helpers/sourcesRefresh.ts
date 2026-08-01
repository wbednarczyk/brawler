import { expect, type Locator, type Page } from "@playwright/test";
import { en } from "../../../src/shared/locale/resources/en";
import { pl } from "../../../src/shared/locale/resources/pl";

// Driving the real app's manual "Refresh sources" sweep, safely under
// contention (issue #308).
//
// Two failure modes this exists to kill, both observed on the owner's live app:
//
//  1. **The spec demanded an idle app.** A refresh already in flight (prior
//     session activity, or the daily scheduler) renames the button to its
//     in-flight label, so the idle-label locator matches nothing and the spec
//     times out — a contention failure reported as a product failure.
//  2. **A stale hard-coded label made the wait a silent no-op.** The ownership
//     spec waited for `/^(Refreshing|Odświeżam)/`, but the Polish label is
//     "Odświeżanie" — that locator never matched, `toHaveCount(0)` was true
//     instantly, and the spec asserted on ownership values while the sweep was
//     still running. A wait that cannot fail is worse than no wait.
//
// So the labels come from the locale resources themselves rather than being
// retyped here, and `startSourcesRefresh` asserts the sweep actually entered its
// in-flight state — a label that stops matching now fails loudly instead of
// skipping the wait. (`tests/` is outside `tsconfig.json`'s `include`, so a
// renamed key is not a compile error here; `requireLabels` turns it into an
// immediate, named runtime failure instead of an empty locator.)

function requireLabels(key: string, labels: (string | undefined)[]): string[] {
  const present = labels.filter((label): label is string => typeof label === "string" && !!label);
  if (present.length !== labels.length) {
    throw new Error(
      `Locale key \`${key}\` is missing from en/pl resources — tests/live/helpers/sourcesRefresh.ts ` +
        `builds the sources-refresh locators from it. Update the helper with the new key.`,
    );
  }
  return present;
}

const IDLE_LABELS = requireLabels("action.refreshSources", [
  en["action.refreshSources"],
  pl["action.refreshSources"],
]);
const IN_FLIGHT_LABELS = requireLabels("action.refreshing", [
  en["action.refreshing"],
  pl["action.refreshing"],
]);

function anyOf(labels: string[]): RegExp {
  const escaped = labels.map((label) => label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  return new RegExp(`^(${[...new Set(escaped)].join("|")})$`);
}

/** The Sources-screen refresh button while idle (the clickable state). */
export function idleRefreshButton(page: Page): Locator {
  // Two controls carry this label — the shell icon-button and the Sources panel
  // button. The screen-level one is last.
  return page.getByRole("button", { name: anyOf(IDLE_LABELS) }).last();
}

/** The same button while a sweep is running (renamed, disabled). */
export function inFlightRefreshButton(page: Page): Locator {
  return page.getByRole("button", { name: anyOf(IN_FLIGHT_LABELS) });
}

/** Waits until no sources refresh is in flight. */
export async function settleSourcesRefresh(page: Page, timeout: number): Promise<void> {
  await expect(inFlightRefreshButton(page)).toHaveCount(0, { timeout });
}

/**
 * Makes sure a real sources sweep is running, tolerating one already in flight.
 *
 * Returns `"joined"` when a sweep was already running (the same write path a
 * click would have started — the app is the owner's, so demanding an idle app
 * is what made these specs flaky), or `"clicked"` when this call started it.
 * The caller owns the settle wait, because each spec watches a different
 * completion signal.
 */
export async function startSourcesRefresh(page: Page): Promise<"clicked" | "joined"> {
  if ((await inFlightRefreshButton(page).count()) > 0) return "joined";

  const button = idleRefreshButton(page);
  await expect(button).toBeVisible({ timeout: 15_000 });
  await button.click();

  // The punchline guard: the click MUST put the button into its in-flight
  // state. Without it, a label that stopped matching would make every settle
  // wait pass instantly and green-wash the spec.
  await expect(inFlightRefreshButton(page)).not.toHaveCount(0, { timeout: 15_000 });
  return "clicked";
}

/**
 * `startSourcesRefresh` + wait for the sweep to finish. `settleTimeout` must
 * cover a full real sweep: BiznesRadar alone walks ~50 company pages behind a
 * politeness delay.
 */
export async function driveSourcesRefresh(
  page: Page,
  { settleTimeout }: { settleTimeout: number },
): Promise<"clicked" | "joined"> {
  const mode = await startSourcesRefresh(page);
  await settleSourcesRefresh(page, settleTimeout);
  return mode;
}
