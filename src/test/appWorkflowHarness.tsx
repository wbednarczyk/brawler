import { render } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { downloadDir, join } from "@tauri-apps/api/path";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { openUrl } from "@tauri-apps/plugin-opener";
import { beforeEach, vi } from "vitest";
import { App } from "../App";
import type { Section } from "../app/navigation";
import {
  currentWeekTestDate,
  legacyInvalidLicenseStatus,
  legacyMissingLicenseStatus,
  legacyNotebookEntry,
} from "./scenarios/legacyMinimal";
import { createMockRuntime } from "./scenarios/runtime";

export { currentWeekTestDate };

export { screen, waitFor, within } from "@testing-library/react";
export { default as userEvent } from "@testing-library/user-event";
export { invoke } from "@tauri-apps/api/core";
export { downloadDir, join } from "@tauri-apps/api/path";
export { save } from "@tauri-apps/plugin-dialog";
export { writeTextFile } from "@tauri-apps/plugin-fs";
export { openUrl } from "@tauri-apps/plugin-opener";
export { expect } from "vitest";
export { vi };

// ONE canonical mock runtime backs the whole Vitest layer (ADR 0048 Keystone
// B/C). `appTestState` is a thin VIEW over its store — every `*Response` field
// is a getter/setter over a `ScenarioData` collection, so a test that assigns
// `appTestState.feedItemsResponse = [...]` mutates the same store the command
// router reads. A few fields (`searchResponse`, `aiAnalysisJobsResponse`,
// `refreshSourcesError`) are canned command-output overrides the old harness
// exposed; they live as plain slots honored by `handleAppCommand`.
const runtime = createMockRuntime("minimal");

interface CannedOverrides {
  searchResponse: unknown | null;
  aiAnalysisJobsResponse: Record<string, unknown>;
  refreshSourcesError: string | null;
}

const canned: CannedOverrides = {
  searchResponse: null,
  aiAnalysisJobsResponse: {},
  refreshSourcesError: null,
};

type Data = typeof runtime.data;
// Clone on assignment so a test that injects a shared seed object (e.g.
// `initialFeedItems[0]`) is never corrupted by the runtime's in-place mutations.
const field = <K extends keyof Data>(key: K) => ({
  get: () => runtime.data[key],
  set: (value: Data[K]) => {
    runtime.data[key] = structuredClone(value);
  },
  enumerable: true,
});

export interface AppTestState {
  companiesResponse: Data["companies"];
  feedItemsResponse: Data["feedItems"];
  researchEvidenceItemsResponse: Data["researchEvidence"];
  researchQuestionsResponse: Data["researchQuestions"];
  evidenceLinksResponse: Data["evidenceLinks"];
  researchBriefJobsResponse: Data["researchBriefJobs"];
  researchDigestJobsResponse: Data["researchDigestJobs"];
  researchRemindersResponse: Data["researchReminders"];
  companyEventsResponse: Data["events"];
  companySignalsResponse: Data["signals"];
  transcriptJobsResponse: Data["transcriptJobs"];
  transcriptSegmentsResponse: Data["transcriptSegments"];
  companyRegistryEntriesResponse: Data["registry"];
  watchlistsResponse: Data["watchlists"];
  watchlistMembershipsResponse: Data["watchlistMemberships"];
  notebookEntriesResponse: Data["notebookEntries"];
  managementClaimsResponse: Data["managementClaims"];
  settingsResponse: Data["settings"];
  localMetricsSnapshotResponse: Data["metricsSnapshot"];
  licenseStatusResponse: Data["licenseStatus"];
  sourceAdaptersResponse: Data["sourceAdapters"];
  backupStatusResponse: Data["backupStatus"];
  autopilotRunsResponse: Data["autopilotRuns"];
  companyAutopilotModesResponse: Data["autopilotModes"];
  financialFactsResponse: Data["financialFacts"];
  claimsToVerifyResponse: Data["claimsToVerify"];
  reportSeasonUpcomingResponse: Data["reportSeasonUpcoming"];
  researchReviewCheckpointResponse: Data["researchReviewCheckpoints"][number] | null;
  geminiCredentialStatusResponse: Data["credentialStatuses"][number] | null;
  searchResponse: unknown | null;
  aiAnalysisJobsResponse: Record<string, unknown>;
  refreshSourcesError: string | null;
}

export const appTestState = Object.defineProperties(
  {} as AppTestState,
  {
    companiesResponse: field("companies"),
    feedItemsResponse: field("feedItems"),
    researchEvidenceItemsResponse: field("researchEvidence"),
    researchQuestionsResponse: field("researchQuestions"),
    evidenceLinksResponse: field("evidenceLinks"),
    researchBriefJobsResponse: field("researchBriefJobs"),
    researchDigestJobsResponse: field("researchDigestJobs"),
    researchRemindersResponse: field("researchReminders"),
    companyEventsResponse: field("events"),
    companySignalsResponse: field("signals"),
    transcriptJobsResponse: field("transcriptJobs"),
    transcriptSegmentsResponse: field("transcriptSegments"),
    companyRegistryEntriesResponse: field("registry"),
    watchlistsResponse: field("watchlists"),
    watchlistMembershipsResponse: field("watchlistMemberships"),
    notebookEntriesResponse: field("notebookEntries"),
    managementClaimsResponse: field("managementClaims"),
    settingsResponse: field("settings"),
    localMetricsSnapshotResponse: field("metricsSnapshot"),
    licenseStatusResponse: field("licenseStatus"),
    sourceAdaptersResponse: field("sourceAdapters"),
    backupStatusResponse: field("backupStatus"),
    autopilotRunsResponse: field("autopilotRuns"),
    companyAutopilotModesResponse: field("autopilotModes"),
    financialFactsResponse: field("financialFacts"),
    claimsToVerifyResponse: field("claimsToVerify"),
    reportSeasonUpcomingResponse: field("reportSeasonUpcoming"),
    // Single-row views over collection stores.
    researchReviewCheckpointResponse: {
      get: () => runtime.data.researchReviewCheckpoints[0] ?? null,
      set: (value: unknown) => {
        runtime.data.researchReviewCheckpoints = value
          ? [structuredClone(value) as Data["researchReviewCheckpoints"][number]]
          : [];
      },
      enumerable: true,
    },
    geminiCredentialStatusResponse: {
      get: () => runtime.data.credentialStatuses.find((c) => c.providerId === "provider_gemini") ?? null,
      set: (value: unknown) => {
        const next = runtime.data.credentialStatuses.filter((c) => c.providerId !== "provider_gemini");
        if (value) next.unshift(structuredClone(value) as Data["credentialStatuses"][number]);
        runtime.data.credentialStatuses = next;
      },
      enumerable: true,
    },
    // Canned command-output overrides.
    searchResponse: {
      get: () => canned.searchResponse,
      set: (value: unknown) => {
        canned.searchResponse = value;
      },
      enumerable: true,
    },
    aiAnalysisJobsResponse: {
      get: () => canned.aiAnalysisJobsResponse,
      set: (value: Record<string, unknown>) => {
        canned.aiAnalysisJobsResponse = value;
      },
      enumerable: true,
    },
    refreshSourcesError: {
      get: () => canned.refreshSourcesError,
      set: (value: string | null) => {
        canned.refreshSourcesError = value;
      },
      enumerable: true,
    },
  },
);

function feedItemIdOf(args: unknown): string | undefined {
  const a = args as { input?: { feedItemId?: string }; feedItemId?: string } | undefined;
  return a?.input?.feedItemId ?? a?.feedItemId;
}

/** Route a command, honoring the canned overrides the old harness supported. */
export function handleAppCommand(command: string, args?: unknown): Promise<unknown> {
  if (command === "search" && canned.searchResponse) return Promise.resolve(canned.searchResponse);
  if (command === "list_ai_analysis") {
    const feedItemId = feedItemIdOf(args);
    if (feedItemId && feedItemId in canned.aiAnalysisJobsResponse) {
      return Promise.resolve(canned.aiAnalysisJobsResponse[feedItemId]);
    }
  }
  if ((command === "refresh_sources" || command === "refresh_source") && canned.refreshSourcesError) {
    return Promise.reject(new Error(canned.refreshSourcesError));
  }
  return runtime.invoke(command, args as Record<string, unknown> | undefined);
}

export function resetAppTestState() {
  runtime.reset("minimal");
  canned.searchResponse = null;
  canned.aiAnalysisJobsResponse = {};
  canned.refreshSourcesError = null;
}

// --- Seed values consumed by individual tests (now sourced from the canonical
// scenario so there is a single source of truth) ---
const SEED = runtime.data;
export const initialCompanies = SEED.companies;
export const initialFeedItems = SEED.feedItems;
export const initialGeminiCredentialStatus =
  SEED.credentialStatuses.find((c) => c.providerId === "provider_gemini") ?? SEED.credentialStatuses[0];
export const initialNotebookEntry = legacyNotebookEntry;
export const initialTranscriptJobs = SEED.transcriptJobs;
export const invalidLicenseStatus = legacyInvalidLicenseStatus;
export const missingLicenseStatus = legacyMissingLicenseStatus;

// Most workflow tests were written against the historical landing screen (Inbox),
// so the harness defaults there to keep them focused on their subject. The
// PRODUCTION default is Today/Pulse (ADR 0054) — asserted directly in
// App.test. Pass `section` to land on a specific screen (e.g. one the
// sidebar spine reaches but a focused test wants to bypass).
export function renderApp(options?: { section?: Section }) {
  return render(
    <App
      initialLicenseStatus={appTestState.licenseStatusResponse as never}
      initialSection={options?.section ?? "Inbox"}
    />,
  );
}

// Renders the app with NO section override, so the production default shell
// (Today/Pulse) decides the landing screen. Use this to assert the default.
export function renderAppDefaultShell() {
  return render(<App initialLicenseStatus={appTestState.licenseStatusResponse as never} />);
}

// Tauri module mocks live in src/test/setup.ts (a configured setupFile); vitest
// 3+ only honors vi.mock from the test file or a setup file. This harness resets
// them per test and re-points invoke at the runtime router.
beforeEach(() => {
  resetAppTestState();
  vi.mocked(invoke).mockClear();
  vi.mocked(downloadDir).mockClear();
  vi.mocked(join).mockClear();
  vi.mocked(openUrl).mockClear();
  vi.mocked(save).mockClear();
  vi.mocked(writeTextFile).mockClear();
  vi.mocked(invoke).mockImplementation(handleAppCommand);
});
