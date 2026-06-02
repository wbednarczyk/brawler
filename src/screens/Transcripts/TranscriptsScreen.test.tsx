import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  initialGeminiCredentialStatus,
  invoke,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

describe("Transcripts screen workflows", () => {
  it("shows transcript jobs and reviews completed transcript segments", async () => {
    const user = userEvent.setup();

    appTestState.geminiCredentialStatusResponse = {
      ...initialGeminiCredentialStatus,
      configured: true,
      storage: "os_keychain",
    };

    renderApp();

    await user.click(screen.getByRole("button", { name: "Transcripts" }));

    const transcriptJobsRegion = await screen.findByLabelText("Transcript jobs");

    expect(screen.getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
    expect(within(transcriptJobsRegion).getByText("Q2 conference")).toBeInTheDocument();
    expect(screen.getByText("Configured")).toBeInTheDocument();

    await user.click(within(transcriptJobsRegion).getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_video_transcript_job", {
        input: {
          jobId: "transcript_job_unresolved_conference",
          providerMode: "provider_gemini",
        },
      });
    });

    await user.click(
      within(transcriptJobsRegion).getByRole("button", {
        name: "Open transcript job: https://www.youtube.com/watch?v=conference",
      }),
    );

    const transcriptSegments = await screen.findByLabelText("Transcript segments");

    expect(within(transcriptSegments).getByText("0:00-0:42")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Search transcript segments"), "margin");

    expect(screen.getByText("1/2")).toBeInTheDocument();
    expect(within(transcriptSegments).getByText("margin").tagName).toBe("MARK");
  });
});
