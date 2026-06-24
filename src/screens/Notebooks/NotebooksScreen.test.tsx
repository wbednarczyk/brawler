import { describe, it } from "vitest";
import { fireEvent } from "@testing-library/react";
import {
  appTestState,
  expect,
  initialGeminiCredentialStatus,
  initialNotebookEntry,
  initialTranscriptJobs,
  invoke,
  renderApp,
  screen,
  userEvent,
  vi,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

describe("Notebook and transcript workflows", () => {
  it("shows the notebooks workspace and transcript job shell", async () => {
    const user = userEvent.setup();

    appTestState.notebookEntriesResponse = [initialNotebookEntry];
    appTestState.geminiCredentialStatusResponse = {
      ...initialGeminiCredentialStatus,
      configured: true,
      storage: "os_keychain",
    };

    renderApp({ section: "Notebooks" });


    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(screen.getByRole("heading", { name: "Notebooks" })).toBeInTheDocument();
    expect(
      await within(notebooksWorkspace).findByRole("button", {
        name: "Open notebook company: GPW:CDR",
      }),
    ).toBeInTheDocument();
    const notebookCompanyButton = within(notebooksWorkspace).getByRole("button", {
      name: "Open notebook company: GPW:CDR",
    });
    expect(notebookCompanyButton).toBeInTheDocument();
    expect(within(notebookCompanyButton).getByText("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(within(notebookCompanyButton).getByLabelText("GPW:CDR")).toBeInTheDocument();
    expect(within(notebooksWorkspace).getByRole("separator", {
      name: "Resize notebook company list",
    })).toBeInTheDocument();
    expect(within(notebooksWorkspace).getByRole("separator", {
      name: "Resize notebook note list",
    })).toBeInTheDocument();
    expect(
      await within(notebooksWorkspace).findByRole("button", {
        name: "Show open claims for GPW:CDR",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", { name: "Show follow-ups for GPW:CDR" }),
    ).toBeInTheDocument();
    const notebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    expect(notebookRow).toBeInTheDocument();
    expect(screen.queryByLabelText("Notebook screen selected body")).not.toBeInTheDocument();

    await user.click(notebookRow);

    expect(screen.getByLabelText("Notebook screen selected body")).toHaveTextContent(
      "Management promised a release milestone in the next two quarters.",
    );

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "New note" }));
    await user.type(screen.getByLabelText("Notebook screen note title"), "Notebook desk note");
    await user.type(screen.getByLabelText("Notebook screen note body"), "Created from the main notebooks pane.");
    await user.type(screen.getByLabelText("Notebook screen note tags"), "desk, workflow");
    await user.selectOptions(screen.getByLabelText("Notebook screen note kind"), "observation");
    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_notebook_entry", {
        input: {
          companyId: "company_gpw_cdr",
          title: "Notebook desk note",
          body: "Created from the main notebooks pane.",
          bodyFormat: "markdown",
          tags: ["desk", "workflow"],
          kind: "observation",
          claimStatus: null,
          eventDate: null,
          followUpAfter: null,
          followUpDate: null,
          origins: [
            {
              sourceType: "manual",
              sourceId: null,
              sourceUrl: null,
              label: "Manual note",
            },
          ],
        },
      });
    });

    const createdNotebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Notebook desk note",
    });
    expect(createdNotebookRow).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Transcripts" }));

    const transcriptJobsRegion = await screen.findByLabelText("Transcript jobs");

    expect(screen.getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_video_transcript_jobs", {
      input: { companyId: null },
    });
    expect(within(transcriptJobsRegion).getByText("Q2 conference")).toBeInTheDocument();
    expect(within(transcriptJobsRegion).getByText("Queued")).toBeInTheDocument();
    expect(screen.getByText("Credentials")).toBeInTheDocument();
    expect(screen.getByText("Configured")).toBeInTheDocument();
    expect(screen.getByText("Timeout")).toBeInTheDocument();
    expect(screen.getByText("300s")).toBeInTheDocument();

    await user.click(within(transcriptJobsRegion).getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_video_transcript_job", {
        input: {
          jobId: "transcript_job_unresolved_conference",
          providerMode: "provider_gemini",
        },
      });
    });
    expect(await within(transcriptJobsRegion).findByText("Completed")).toBeInTheDocument();

    await user.click(
      within(transcriptJobsRegion).getByRole("button", {
        name: "Open transcript job: https://www.youtube.com/watch?v=conference",
      }),
    );

    const transcriptSegments = await screen.findByLabelText("Transcript segments");
    const transcriptDescriptionEditor = await screen.findByLabelText("Transcript description editor");

    await user.clear(within(transcriptDescriptionEditor).getByLabelText("Edit transcript description"));
    await user.type(within(transcriptDescriptionEditor).getByLabelText("Edit transcript description"), "CDR strategy call");
    await user.click(within(transcriptDescriptionEditor).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_video_transcript_job", {
        input: {
          jobId: "transcript_job_unresolved_conference",
          sourceLabel: "CDR strategy call",
        },
      });
    });
    expect(await within(transcriptJobsRegion).findByText("CDR strategy call")).toBeInTheDocument();

    expect(
      within(transcriptSegments).getByText(
        "We expect the second half to be stronger after the release window stabilizes.",
      ),
    ).toBeInTheDocument();
    expect(
      within(transcriptSegments).getByText("Gross margin should normalize over the next two quarters."),
    ).toBeInTheDocument();
    expect(within(transcriptSegments).getByText("0:00-0:42")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Search transcript segments"), "margin");

    expect(
      within(transcriptSegments).queryByText(
        "We expect the second half to be stronger after the release window stabilizes.",
      ),
    ).not.toBeInTheDocument();
    expect(
      within(transcriptSegments).getByText((_, element) =>
        element?.textContent === "Gross margin should normalize over the next two quarters.",
      ),
    ).toBeInTheDocument();
    expect(within(transcriptSegments).getByText("margin").tagName).toBe("MARK");
    expect(screen.getByText("1/2")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear transcript search" }));

    expect(screen.getByLabelText("Search transcript segments")).toHaveValue("");
    expect(screen.getByText("2/2")).toBeInTheDocument();
    expect(
      within(transcriptSegments).getByText(
        "We expect the second half to be stronger after the release window stabilizes.",
      ),
    ).toBeInTheDocument();

    await user.click(
      within(transcriptSegments).getByRole("checkbox", {
        name: "Select transcript segment 0:00-0:42",
      }),
    );
    await user.click(
      within(transcriptSegments).getByRole("checkbox", {
        name: "Select transcript segment 0:43-1:36",
      }),
    );

    expect(within(transcriptJobsRegion).getByText("2")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_transcript_segments", {
      transcriptJobId: "transcript_job_unresolved_conference",
    });

    expect(within(transcriptJobsRegion).getByText("Unlinked")).toBeInTheDocument();
    expect(within(transcriptJobsRegion).getByRole("button", { name: "Create company note draft" })).toBeDisabled();

    await user.type(screen.getByLabelText("Transcript link company lookup"), "CDR");
    const transcriptLinkSuggestions = await screen.findByLabelText("Transcript link company suggestions");
    await user.click(within(transcriptLinkSuggestions).getByRole("button", { name: /GPW:CDR/ }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("resolve_transcript_job_company", {
        input: {
          jobId: "transcript_job_unresolved_conference",
          companyId: "company_gpw_cdr",
        },
      });
    });

    expect(await within(transcriptJobsRegion).findByText("GPW:CDR")).toBeInTheDocument();

    await user.click(within(transcriptJobsRegion).getByRole("button", { name: "Create company note draft" }));

    const transcriptNoteDraft = await screen.findByLabelText("Transcript note draft");

    expect(within(transcriptNoteDraft).getByLabelText("Transcript note body")).toHaveValue(
      "> We expect the second half to be stronger after the release window stabilizes.\n\n> Gross margin should normalize over the next two quarters.",
    );

    await user.clear(within(transcriptNoteDraft).getByLabelText("Transcript note title"));
    await user.type(within(transcriptNoteDraft).getByLabelText("Transcript note title"), "Conference promises");
    await user.selectOptions(within(transcriptNoteDraft).getByLabelText("Transcript note kind"), "claim");
    await user.selectOptions(within(transcriptNoteDraft).getByLabelText("Transcript note status"), "open");
    await user.clear(within(transcriptNoteDraft).getByLabelText("Transcript note tags"));
    await user.type(within(transcriptNoteDraft).getByLabelText("Transcript note tags"), "conference, management-guidance");
    await user.click(within(transcriptNoteDraft).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_note_from_transcript_selection", {
        input: {
          transcriptJobId: "transcript_job_unresolved_conference",
          transcriptSegmentIds: ["transcript_segment_opening", "transcript_segment_margin"],
          noteDraft: {
            title: "Conference promises",
            body: "> We expect the second half to be stronger after the release window stabilizes.\n\n> Gross margin should normalize over the next two quarters.",
            tags: ["conference", "management-guidance"],
            kind: "claim",
            claimStatus: "open",
            eventDate: null,
            followUpAfter: null,
            followUpDate: null,
          },
        },
      });
    });

    await user.type(screen.getByLabelText("Transcript URL"), "https://www.youtube.com/watch?v=newjob");
    await user.type(screen.getByLabelText("Transcript description"), "New job conference");
    await user.type(screen.getByLabelText("Transcript company lookup"), "CDR");
    await user.click(await screen.findByRole("button", { name: /GPW:CDR/ }));
    await user.click(screen.getByRole("button", { name: "Create job" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_video_transcript_job", {
        input: {
          sourceUrl: "https://www.youtube.com/watch?v=newjob",
          companyId: "company_gpw_cdr",
          providerId: "provider_gemini",
          sourceLabel: "New job conference",
          recognizedCompanyCandidates: null,
        },
      });
    });
    expect(await within(transcriptJobsRegion).findByText("New job conference")).toBeInTheDocument();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_video_transcript_job", {
        input: {
          jobId: "transcript_job_created",
          providerMode: "provider_gemini",
        },
      });
    });

    await user.type(screen.getByLabelText("Transcript URL"), "https://www.youtube.com/watch?v=newjob");
    await user.type(screen.getByLabelText("Transcript company lookup"), "CDR");
    await user.click(await screen.findByRole("button", { name: /GPW:CDR/ }));
    await user.click(screen.getByRole("button", { name: "Create job" }));

    await waitFor(() => {
      expect(
        within(transcriptJobsRegion).getAllByRole("button", {
          name: "Open transcript job: https://www.youtube.com/watch?v=newjob",
        }),
      ).toHaveLength(1);
    });

    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    await user.click(
      within(transcriptJobsRegion).getByRole("button", {
        name: "Delete transcript job New job conference",
      }),
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_video_transcript_job", {
        jobId: "transcript_job_created",
      });
    });
    expect(
      within(transcriptJobsRegion).queryByRole("button", {
        name: "Open transcript job: https://www.youtube.com/watch?v=newjob",
      }),
    ).not.toBeInTheDocument();

    confirm.mockRestore();
  });

  it("edits and saves the selected notebook entry with shortcuts", async () => {
    const user = userEvent.setup();

    appTestState.notebookEntriesResponse = [initialNotebookEntry];

    renderApp({ section: "Notebooks" });


    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");
    const notebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    await user.click(notebookRow);
    fireEvent.keyDown(document, { key: "E", code: "KeyE", ctrlKey: true });

    expect(screen.getByLabelText("Notebook screen selected title")).toHaveValue("Release schedule promise");

    await user.clear(screen.getByLabelText("Notebook screen selected body"));
    await user.type(screen.getByLabelText("Notebook screen selected body"), "Shortcut-updated note body.");
    fireEvent.keyDown(screen.getByLabelText("Notebook screen selected body"), {
      key: "S",
      code: "KeyS",
      ctrlKey: true,
    });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "Shortcut-updated note body.",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "open",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-11-30",
        },
      });
    });
  });

  it("deletes the selected notebook entry from the detail toolbar", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    appTestState.notebookEntriesResponse = [initialNotebookEntry];

    renderApp({ section: "Notebooks" });


    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");
    const notebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    await user.click(notebookRow);
    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_notebook_entry", {
        id: "note_company_gpw_cdr_release_schedule",
      });
    });
    expect(confirm).toHaveBeenCalledWith("Delete Release schedule promise?");
    await waitFor(() => {
      expect(
        within(notebooksWorkspace).queryByRole("button", {
          name: "Select notebook screen entry: Release schedule promise",
        }),
      ).not.toBeInTheDocument();
    });
    expect(screen.getByText("Select a note to read or edit it.")).toBeInTheDocument();

    confirm.mockRestore();
  });

  it("shows transcript provider errors in expanded job details", async () => {
    const user = userEvent.setup();

    appTestState.transcriptJobsResponse = [
      {
        ...initialTranscriptJobs[0],
        id: "transcript_job_failed_gemini",
        status: "failed",
        errorCode: "provider_not_configured",
        error: "Gemini transcription provider is not configured.",
      },
    ];

    renderApp();

    await user.click(screen.getByRole("button", { name: "Transcripts" }));

    const transcriptJobsRegion = await screen.findByLabelText("Transcript jobs");

    expect(await within(transcriptJobsRegion).findByText("Failed")).toBeInTheDocument();

    await user.click(
      within(transcriptJobsRegion).getByRole("button", {
        name: "Open transcript job: https://www.youtube.com/watch?v=conference",
      }),
    );

    const errorPanel = await screen.findByLabelText("Transcript job error");

    expect(within(errorPanel).getByText("Provider Not Configured")).toBeInTheDocument();
    expect(within(errorPanel).getByText("Gemini transcription provider is not configured.")).toBeInTheDocument();
    const runButton = within(transcriptJobsRegion).getByRole("button", { name: "Retry" });
    expect(runButton).toBeDisabled();
    expect(runButton).toHaveAttribute(
      "title",
      "Configure Gemini API key in Settings before running transcription",
    );
  });

  it("validates transcript URL before creating a job", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Transcripts" }));

    expect(await screen.findByRole("button", { name: "Create job" })).toBeDisabled();

    await user.type(screen.getByLabelText("Transcript URL"), "https://example.com/video");
    await user.tab();

    expect(screen.getByText("Use a YouTube URL from youtube.com or youtu.be.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create job" })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "Create job" }));

    expect(invoke).not.toHaveBeenCalledWith("create_video_transcript_job", expect.anything());
  });

  it("filters notebook screen entries by kind, status, tag, and follow-up scheduling", async () => {
    const user = userEvent.setup();

    appTestState.notebookEntriesResponse = [
      initialNotebookEntry,
      {
        ...initialNotebookEntry,
        id: "note_company_gpw_cdr_margin_observation",
        title: "Margin observation",
        body: "Desk note about margin pressure after the call.",
        tags: ["desk", "margin"],
        kind: "observation",
        claimStatus: null,
        followUpAfter: null,
        followUpDate: null,
      },
      {
        ...initialNotebookEntry,
        id: "note_company_gpw_cdr_capex_claim",
        title: "Capex delivery claim",
        body: "Management claimed capex should normalize.",
        tags: ["capex", "management-guidance"],
        kind: "claim",
        claimStatus: "delivered",
        followUpAfter: "2026-Q3",
      },
    ];

    renderApp({ section: "Notebooks" });


    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(
      await within(notebooksWorkspace).findByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Open notebook company: GPW:PZU",
      }),
    ).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Notebook watchlist filter"), "watchlist_main_gpw");
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Open notebook company: GPW:PZU",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Show open claims for GPW:CDR" }));

    expect(screen.getByLabelText("Notebook claim status filter")).toHaveValue("open");
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Show follow-ups for GPW:CDR" }));

    expect(screen.getByLabelText("Notebook follow-up filter")).toHaveValue("has_follow_up");
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));

    await user.selectOptions(screen.getByLabelText("Notebook kind filter"), "observation");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.selectOptions(screen.getByLabelText("Notebook claim status filter"), "delivered");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Capex delivery claim",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.type(screen.getByLabelText("Notebook tag filter"), "desk");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Capex delivery claim",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.selectOptions(screen.getByLabelText("Notebook follow-up filter"), "no_follow_up");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();
  });

  it("renders notebook Markdown in read mode", async () => {
    const user = userEvent.setup();

    appTestState.notebookEntriesResponse = [
      {
        ...initialNotebookEntry,
        body: "# Release checklist\n- **Milestone** shipped\n- `Patch` ready",
      },
    ];

    renderApp({ section: "Notebooks" });

    const notebookRow = await screen.findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    expect(within(notebookRow).queryByText("# Release checklist")).not.toBeInTheDocument();

    await user.click(notebookRow);

    const notebookBody = screen.getByLabelText("Notebook screen selected body");

    expect(within(notebookBody).getByRole("heading", { name: "Release checklist" })).toBeInTheDocument();
    expect(within(notebookBody).getByText("Milestone")).toHaveProperty("tagName", "STRONG");
    expect(within(notebookBody).getByText("Patch")).toHaveProperty("tagName", "CODE");

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.clear(screen.getByLabelText("Notebook screen selected follow-up date"));
    await user.type(screen.getByLabelText("Notebook screen selected follow-up date"), "2026-12-15");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "# Release checklist\n- **Milestone** shipped\n- `Patch` ready",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "open",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-12-15",
        },
      });
    });
  });
});
