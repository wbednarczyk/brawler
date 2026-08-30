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

    const transcriptJobsRegion = await screen.findByRole("region", { name: "Transcripts" });

    expect(screen.getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
    expect(within(transcriptJobsRegion).getByText("Q2 conference")).toBeInTheDocument();

    await user.click(within(transcriptJobsRegion).getByRole("button", { name: "Fetch again" }));

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
        name: "Open transcript: Q2 conference",
      }),
    );

    const transcriptSegments = await screen.findByLabelText("Transcript segments");

    expect(within(transcriptSegments).getByText("0:00-0:42")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Search transcript segments"), "margin");

    // F4b S2: the header counter tracks selection, not the search-match count.
    expect(document.querySelector(".transcript-search-count")?.textContent).toBe("0 selected of 2");
    expect(within(transcriptSegments).getByText("margin").tagName).toBe("MARK");
  });

  // U7-E2 density contract (ADR 0076 D6): Transcripts — S: list; segments fold.
  // The expanded job detail carries a "Segments" disclosure (collapsed by
  // default) so at the S/short tiers the segment review folds behind it while
  // staying reachable; at M/L the segments render inline and the toggle is
  // CSS-hidden. jsdom has no container queries, so here we assert the disclosure
  // affordance + state; the visual tier switch is asserted in the browser spec.
  it("folds transcript segments behind a disclosure (S density contract)", async () => {
    const user = userEvent.setup();

    appTestState.geminiCredentialStatusResponse = {
      ...initialGeminiCredentialStatus,
      configured: true,
      storage: "os_keychain",
    };

    renderApp();

    await user.click(screen.getByRole("button", { name: "Transcripts" }));
    const transcriptJobsRegion = await screen.findByRole("region", { name: "Transcripts" });

    // Run the job so it completes with stored segments (matches the review flow),
    // then open it to reach the segment disclosure.
    await user.click(within(transcriptJobsRegion).getByRole("button", { name: "Fetch again" }));
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
        name: "Open transcript: Q2 conference",
      }),
    );

    const segmentsToggle = await screen.findByRole("button", { name: "Show segments" });
    expect(segmentsToggle).toHaveAttribute("aria-expanded", "false");
    // The segment review stays in the DOM — the fold is reachable, never lost.
    expect(await screen.findByLabelText("Transcript segments")).toBeInTheDocument();

    await user.click(segmentsToggle);
    expect(segmentsToggle).toHaveAttribute("aria-expanded", "true");
  });
});
