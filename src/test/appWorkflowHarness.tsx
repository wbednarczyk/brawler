import { render } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { downloadDir, join } from "@tauri-apps/api/path";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { openUrl } from "@tauri-apps/plugin-opener";
import { beforeEach, vi } from "vitest";
import { App } from "../App";
import { handleAppCommand } from "./workflowHarness/commands";
import { appTestState as workflowAppTestState, resetAppTestState } from "./workflowHarness/state";

export { screen, waitFor, within } from "@testing-library/react";
export { default as userEvent } from "@testing-library/user-event";
export { invoke } from "@tauri-apps/api/core";
export { downloadDir, join } from "@tauri-apps/api/path";
export { save } from "@tauri-apps/plugin-dialog";
export { writeTextFile } from "@tauri-apps/plugin-fs";
export { openUrl } from "@tauri-apps/plugin-opener";
export { expect } from "vitest";
export { vi };
export const appTestState = workflowAppTestState;
export {
  currentWeekTestDate,
  initialCompanies,
  initialFeedItems,
  initialGeminiCredentialStatus,
  initialNotebookEntry,
  initialTranscriptJobs,
  invalidLicenseStatus,
  missingLicenseStatus,
} from "./workflowHarness/testData";

export function renderApp() {
  return render(<App initialLicenseStatus={appTestState.licenseStatusResponse} />);
}

// Tauri module mocks now live in src/test/setup.ts (a configured setupFile);
// vitest 3+ only honors vi.mock from the test file or a setup file. This harness
// imports the mocked members above and resets them here per test.
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
