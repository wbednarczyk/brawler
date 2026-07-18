import type { Page } from "@playwright/test";

// Typed Playwright bridge onto `window.__brawlerMock` (ADR 0081 Q2, Radicle
// a9992e2 / src/test/browserSmokeRuntime.ts). Every call crosses via
// `page.evaluate`, so arguments/results are JSON-serialized — no class
// instances (e.g. `Error`), only plain data.

export type ScenarioName = "empty" | "minimal" | "rich";

export type ScenarioOverlayName =
  | "hostile-content"
  | "dense-history"
  | "partial-data"
  | "stale-processing"
  | "conflicting-statuses"
  | "mixed-locale"
  | "attention-overflow";

export type ScenarioSpec = {
  base: ScenarioName;
  overlays?: readonly ScenarioOverlayName[];
};

export type InvocationPhase = "before-handler" | "after-handler";

export type InvocationMatch = {
  command: string;
  args?: Record<string, unknown>;
  phase?: InvocationPhase;
};

export type PendingInvocation = {
  id: string;
  command: string;
  args: Record<string, unknown> | undefined;
  phase: InvocationPhase;
};

export type MockCommandError = { code: string; message: string };

interface BrawlerMockBridge {
  reset(spec?: ScenarioName | ScenarioSpec): void;
  hold(match: InvocationMatch): string;
  pending(): readonly PendingInvocation[];
  release(id: string): void;
  reject(id: string, error: MockCommandError): void;
  releaseAll(): void;
}

declare global {
  interface Window {
    __brawlerMock?: BrawlerMockBridge;
  }
}

/** Re-seed the mock store (optionally with Q2 overlays). Setup order inside
 * the browser runtime is always base -> projection -> overlays. */
export async function resetMockScenario(page: Page, spec?: ScenarioName | ScenarioSpec): Promise<void> {
  await page.evaluate((s) => window.__brawlerMock?.reset(s), spec);
}

/** Register interest in the NEXT matching command/job invocation; returns the
 * id it will be held under once a real call matches. */
export async function holdInvocation(page: Page, match: InvocationMatch): Promise<string> {
  const id = await page.evaluate((m) => window.__brawlerMock?.hold(m), match);
  if (!id) {
    throw new Error("window.__brawlerMock is not installed — is this a mock-runtime browser page?");
  }
  return id;
}

export async function pendingInvocations(page: Page): Promise<readonly PendingInvocation[]> {
  return (await page.evaluate(() => window.__brawlerMock?.pending())) ?? [];
}

/** Let a held invocation proceed/complete normally. */
export async function releaseInvocation(page: Page, id: string): Promise<void> {
  await page.evaluate((invocationId) => window.__brawlerMock?.release(invocationId), id);
}

/** Settle a held invocation with the shared ADR 0070 CommandError envelope
 * (delegates to the Deliverable A seam — see controlledAsync.ts). */
export async function rejectInvocation(page: Page, id: string, error: MockCommandError): Promise<void> {
  await page.evaluate(([invocationId, err]) => window.__brawlerMock?.reject(invocationId, err), [id, error] as const);
}

export async function releaseAllInvocations(page: Page): Promise<void> {
  await page.evaluate(() => window.__brawlerMock?.releaseAll());
}
