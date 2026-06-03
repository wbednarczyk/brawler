import { describe, it } from "vitest";
import { fireEvent } from "@testing-library/react";
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
    expect(within(settingsRegion).getByRole("heading", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Shortcuts are ignored while typing in fields and editors.")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Open Inbox")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Focus global search")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ctrl+1")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ctrl+K")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("F9")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Shift+F9")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Select next inbox item")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Edit selected notebook entry")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Shortcut key Open Inbox"), {
      target: { value: "I" },
    });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_settings", {
        input: {
          shortcutBindings: expect.objectContaining({
            "app.openInbox": expect.objectContaining({
              key: "I",
              ctrlKey: true,
            }),
          }),
        },
      });
    });
    expect(screen.getByLabelText("Shortcut key Open Inbox")).toHaveValue("I");

    await user.selectOptions(screen.getByLabelText("Settings theme"), "light");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        theme: "light",
      },
    });
    expect(screen.getByLabelText("Settings theme")).toHaveValue("light");

    await user.selectOptions(screen.getByLabelText("Settings locale"), "pl");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        locale: "pl",
      },
    });
    expect(screen.getByLabelText("Język ustawień")).toHaveValue("pl");
    expect(await screen.findByRole("heading", { name: "Ustawienia" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Spółki" })).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Szukaj spółek, informacji, notatek")).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Źródła" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Czyszczenie kanału" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Skróty klawiaturowe" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Otwórz Inbox")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ustaw fokus na wyszukiwaniu globalnym")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Interwał odpytywania źródeł w ustawieniach"), "1800");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        pollIntervalSeconds: 1800,
      },
    });
    expect(screen.getByLabelText("Interwał odpytywania źródeł w ustawieniach")).toHaveValue("1800");

    await user.selectOptions(screen.getByLabelText("Model transkrypcji Gemini"), "gemini-2.5-flash");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        youtubeTranscriptionModel: "gemini-2.5-flash",
      },
    });
    expect(screen.getByLabelText("Model transkrypcji Gemini")).toHaveValue("gemini-2.5-flash");

    await user.selectOptions(screen.getByLabelText("Limit czasu transkrypcji Gemini"), "600");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        youtubeTranscriptionTimeoutSeconds: 600,
      },
    });
    expect(screen.getByLabelText("Limit czasu transkrypcji Gemini")).toHaveValue("600");

    await user.type(screen.getByLabelText("Klucz API Gemini"), "test-gemini-key");
    await user.click(screen.getByRole("button", { name: "Zapisz" }));

    expect(invoke).toHaveBeenCalledWith("set_gemini_transcription_api_key", {
      input: {
        apiKey: "test-gemini-key",
      },
    });
    expect(await within(settingsRegion).findByText("Skonfigurowane")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Pęk kluczy systemu")).toBeInTheDocument();
    expect(screen.getByLabelText("Klucz API Gemini")).toHaveValue("");

    await user.click(screen.getByRole("button", { name: "Wyczyść" }));

    expect(invoke).toHaveBeenCalledWith("clear_gemini_transcription_api_key");
    await waitFor(() => {
      expect(within(settingsRegion).queryByText("OS keychain")).not.toBeInTheDocument();
    });
    expect(within(settingsRegion).getAllByText("Nieskonfigurowane").length).toBeGreaterThanOrEqual(1);

    await user.click(screen.getByRole("button", { name: "Pobierz klucz API Gemini" }));

    expect(openUrl).toHaveBeenCalledWith("https://aistudio.google.com/app/apikey");

    await user.click(screen.getByRole("button", { name: "Źródła" }));
    expect(await screen.findByRole("heading", { name: "Źródła" })).toBeInTheDocument();
    expect(screen.getByText("Odśwież źródła")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Transkrypcje" }));
    expect(await screen.findByRole("heading", { name: "Transkrypcje" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Odśwież zadania" })).toBeInTheDocument();
  });
});
