import { describe, it } from "vitest";
import {
  appTestState,
  currentWeekTestDate,
  expect,
  initialCompanies,
  initialFeedItems,
  initialGeminiCredentialStatus,
  initialNotebookEntry,
  invoke,
  openUrl,
  renderApp,
  screen,
  userEvent,
  vi,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

describe("Settings screen workflows", () => {
  it("shows SQLite-backed settings and persists theme changes", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));

    const settingsRegion = await screen.findByLabelText("Application settings");

    expect(within(settingsRegion).getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Feed Cleanup" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Import And Export" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "AI" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("sqlite")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("night-neon")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("accepted_deferred")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Feed cleanup")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("30 days")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Cleanup interval")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Daily")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Last cleanup")).toBeInTheDocument();
    expect(within(settingsRegion).getAllByText("Not run this session")).toHaveLength(2);
    expect(within(settingsRegion).getByText("Saved")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Gemini")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("provider_gemini")).toBeInTheDocument();
    expect(within(settingsRegion).getAllByText("Gemini 2.5 Flash-Lite").length).toBeGreaterThanOrEqual(1);
    expect(within(settingsRegion).getByText("Cheapest supported")).toBeInTheDocument();
    expect(within(settingsRegion).getAllByText("Not configured").length).toBeGreaterThanOrEqual(1);
    expect(within(settingsRegion).getByText("Credential storage")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("API Key")).toBeInTheDocument();
    expect(
      within(settingsRegion).getByText(
        "Starting a transcript job sends the YouTube URL and video content to Gemini.",
      ),
    ).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Gemini is used only for YouTube transcription.")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("YouTube transcription timeout")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Settings theme"), "light");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        theme: "light",
      },
    });
    expect(screen.getByLabelText("Settings theme")).toHaveValue("light");

    await user.selectOptions(screen.getByLabelText("Settings source poll interval"), "1800");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        pollIntervalSeconds: 1800,
      },
    });
    expect(screen.getByLabelText("Settings source poll interval")).toHaveValue("1800");

    await user.selectOptions(screen.getByLabelText("Gemini transcription model"), "gemini-2.5-flash");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        youtubeTranscriptionModel: "gemini-2.5-flash",
      },
    });
    expect(screen.getByLabelText("Gemini transcription model")).toHaveValue("gemini-2.5-flash");

    await user.selectOptions(screen.getByLabelText("Gemini transcription timeout"), "600");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        youtubeTranscriptionTimeoutSeconds: 600,
      },
    });
    expect(screen.getByLabelText("Gemini transcription timeout")).toHaveValue("600");

    await user.type(screen.getByLabelText("Gemini API key"), "test-gemini-key");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(invoke).toHaveBeenCalledWith("set_gemini_transcription_api_key", {
      input: {
        apiKey: "test-gemini-key",
      },
    });
    expect(await within(settingsRegion).findByText("Configured")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("OS keychain")).toBeInTheDocument();
    expect(screen.getByLabelText("Gemini API key")).toHaveValue("");

    await user.click(screen.getByRole("button", { name: "Clear" }));

    expect(invoke).toHaveBeenCalledWith("clear_gemini_transcription_api_key");
    await waitFor(() => {
      expect(within(settingsRegion).queryByText("OS keychain")).not.toBeInTheDocument();
    });
    expect(within(settingsRegion).getAllByText("Not configured").length).toBeGreaterThanOrEqual(1);

    await user.click(screen.getByRole("button", { name: "Get Gemini API key" }));

    expect(openUrl).toHaveBeenCalledWith("https://aistudio.google.com/app/apikey");
  });
});
