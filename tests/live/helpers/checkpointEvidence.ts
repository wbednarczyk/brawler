import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "@playwright/test";

// Q6 live UX checkpoint evidence (ADR 0081). Collects the *mechanical* context of
// a real-app exploratory checkpoint — build, platform, viewport, screenshots,
// observations — so a human can answer whether the journey is clear, useful, and
// trustworthy. It deliberately records NO judgment ("UX good" is never emitted by
// automation) and NEVER records the database path or its contents (privacy
// contract): only a caller-supplied non-sensitive dataset LABEL.

export type CheckpointStage = "vertical" | "mid" | "release";

export type CheckpointMeta = {
  /** The served journey under exploration, e.g. "J1 morning review". */
  journey: string;
  /** The active Radicle card (hex7) the checkpoint reports against. */
  card: string;
  /** Which cadence: first vertical slice, mid-milestone, or release dogfood. */
  stage: CheckpointStage;
};

const STAGES: CheckpointStage[] = ["vertical", "mid", "release"];

type EnvMap = Record<string, string | undefined>;

// Access process.env without depending on Node ambient types being present in the
// compilation context (this helper is also imported by a src Vitest test, which
// compiles under the browser tsconfig where `process` is only minimally typed).
const PROCESS_ENV: EnvMap =
  (globalThis as { process?: { env?: EnvMap } }).process?.env ?? {};

/**
 * Read + validate checkpoint metadata from the environment. Throws (refuses) when
 * journey, card, or stage is missing or the stage is not one of the three
 * cadences — a red condition so a checkpoint run cannot silently produce
 * unattributable evidence. Callers that only want to run inside a real checkpoint
 * should gate on `isCheckpointRun()` first.
 */
export function requireCheckpointMeta(env: EnvMap = PROCESS_ENV): CheckpointMeta {
  const journey = env.BRAWLER_UX_JOURNEY?.trim();
  const card = env.BRAWLER_UX_CARD?.trim();
  const stage = env.BRAWLER_UX_STAGE?.trim() as CheckpointStage | undefined;

  const missing: string[] = [];
  if (!journey) missing.push("BRAWLER_UX_JOURNEY");
  if (!card) missing.push("BRAWLER_UX_CARD");
  if (!stage) missing.push("BRAWLER_UX_STAGE");
  if (missing.length) {
    throw new Error(
      `UX checkpoint refuses to run without metadata: set ${missing.join(", ")}. ` +
        `Example: BRAWLER_UX_JOURNEY="J1 morning review" BRAWLER_UX_CARD=a26cc6e ` +
        `BRAWLER_UX_STAGE=vertical make live-cycle LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts`,
    );
  }
  if (!STAGES.includes(stage as CheckpointStage)) {
    throw new Error(
      `BRAWLER_UX_STAGE must be one of ${STAGES.join(" | ")}, got "${stage}".`,
    );
  }
  return { journey: journey as string, card: card as string, stage: stage as CheckpointStage };
}

/** True when any checkpoint metadata is present — i.e. this is a checkpoint run,
 * not an incidental sweep of the full live suite. */
export function isCheckpointRun(env: EnvMap = PROCESS_ENV): boolean {
  return Boolean(env.BRAWLER_UX_STAGE ?? env.BRAWLER_UX_JOURNEY ?? env.BRAWLER_UX_CARD);
}

export type CheckpointManifest = CheckpointMeta & {
  recordedAt: string;
  appVersion: string | null;
  windowsNative: boolean;
  userAgent: string | null;
  viewport: { width: number; height: number; devicePixelRatio: number } | null;
  locale: string | null;
  theme: string | null;
  /** Non-sensitive dataset label supplied by the caller — NEVER the DB path/contents. */
  datasetLabel: string;
  screenshotDir: string;
  observations: string[];
};

const CHECKPOINT_ROOT = "test-results/live/checkpoints";

/**
 * Capture the mechanical context of the live app for a checkpoint. Reads the
 * app version, WebView2 user agent, viewport/DPR, and locale/theme off the real
 * page; the caller passes a non-sensitive dataset label and mechanical
 * observations. Writes `<screenshotDir>/manifest.json` and returns the manifest.
 * Records nothing about the database beyond the label.
 */
export async function recordCheckpoint(
  page: Page,
  meta: CheckpointMeta,
  opts: { datasetLabel: string; nowIso: string; observations: string[] },
): Promise<CheckpointManifest> {
  const dir = join(CHECKPOINT_ROOT, `${meta.stage}-${meta.card}`);
  mkdirSync(dir, { recursive: true });

  const appVersion = await page
    .getByText(/^v\d+\.\d+\.\d+/)
    .first()
    .textContent()
    .catch(() => null);

  const env = await page
    .evaluate(() => ({
      userAgent: navigator.userAgent,
      width: window.innerWidth,
      height: window.innerHeight,
      devicePixelRatio: window.devicePixelRatio,
      locale: document.documentElement.lang || null,
      theme:
        document.documentElement.dataset.theme ||
        document.documentElement.getAttribute("data-theme") ||
        null,
    }))
    .catch(() => null);

  // WebView2 (the Windows desktop runtime) identifies as "Edg/" in its UA; that is
  // the desktop authority per ADR 0066. Absent it, this is not native Windows.
  const windowsNative = env?.userAgent ? /Edg\//.test(env.userAgent) : false;

  const manifest: CheckpointManifest = {
    ...meta,
    recordedAt: opts.nowIso,
    appVersion: appVersion?.trim() ?? null,
    windowsNative,
    userAgent: env?.userAgent ?? null,
    viewport: env
      ? { width: env.width, height: env.height, devicePixelRatio: env.devicePixelRatio }
      : null,
    locale: env?.locale ?? null,
    theme: env?.theme ?? null,
    datasetLabel: opts.datasetLabel,
    screenshotDir: dir,
    observations: opts.observations,
  };

  writeFileSync(join(dir, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  return manifest;
}
