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
  join,
  openUrl,
  renderApp,
  save,
  screen,
  userEvent,
  vi,
  waitFor,
  writeTextFile,
  within,
} from "../../test/appWorkflowHarness";

describe("Settings screen workflows", () => {
  it("shows local settings and persists theme changes", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));

    const settingsRegion = await screen.findByLabelText("Application settings");

    expect(within(settingsRegion).getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    expect(screen.getByLabelText("Settings palette")).toHaveValue("night-neon");
    expect(within(settingsRegion).getByRole("button", { name: "Sources" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("button", { name: "Import And Export" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("button", { name: "Keyboard shortcuts" })).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Settings palette"), "midnight-horizon");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        accentPalette: "midnight-horizon",
      },
    });
    expect(screen.getByLabelText("Settings palette")).toHaveValue("midnight-horizon");

    await user.click(within(settingsRegion).getByRole("button", { name: "Sources" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Feed Cleanup" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Feed cleanup")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("30 days")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Cleanup interval")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Daily")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Last cleanup")).toBeInTheDocument();
    expect(within(settingsRegion).getAllByText("Not run this session")).toHaveLength(2);
    expect(within(settingsRegion).getByText("Saved")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "AI" }));

    expect(within(settingsRegion).getByRole("heading", { name: "AI" })).toBeInTheDocument();
    expect(screen.getByLabelText("Gemini transcription model")).toHaveValue("gemini-2.5-flash");
    expect(screen.getByLabelText("Gemini transcription timeout")).toHaveValue("300");
    expect(screen.getByLabelText("General AI provider")).toHaveValue("");
    expect(screen.getByLabelText("General AI model")).toHaveValue("gemini-2.5-flash");
    expect(screen.getByLabelText("General AI timeout")).toHaveValue("90");
    expect(within(settingsRegion).queryByText("provider_gemini")).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("Cheapest supported")).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("Source-grounded feed analysis")).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("YouTube transcription provider ID")).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("General AI disclosure")).not.toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Credentials" }));

    expect(within(settingsRegion).getByText("Credential kind")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("API Key")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Keyboard shortcuts" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Shortcuts are ignored while typing in fields and editors.")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Open Inbox")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Focus Inbox search")).toBeInTheDocument();
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

    await user.click(within(settingsRegion).getByRole("button", { name: "Appearance" }));

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
    expect(screen.queryByPlaceholderText("Szukaj elementów kanału")).not.toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Źródła" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Źródła" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Czyszczenie kanału" })).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Import i eksport" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Import i eksport" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Dane badawcze")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Skróty klawiaturowe" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Skróty klawiaturowe" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Otwórz Inbox")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ustaw fokus na wyszukiwaniu inboxu")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Źródła" }));

    await user.selectOptions(screen.getByLabelText("Interwał odpytywania źródeł w ustawieniach"), "1800");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        pollIntervalSeconds: 1800,
      },
    });
    expect(screen.getByLabelText("Interwał odpytywania źródeł w ustawieniach")).toHaveValue("1800");

    await user.click(within(settingsRegion).getByRole("button", { name: "AI" }));

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

    await user.selectOptions(screen.getByLabelText("Ogólny dostawca AI"), "provider_gemini");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        generalAnalysisProvider: "provider_gemini",
      },
    });
    expect(screen.getByLabelText("Ogólny dostawca AI")).toHaveValue("provider_gemini");

    await user.selectOptions(screen.getByLabelText("Ogólny model AI"), "gemini-3.5-flash");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        generalAnalysisModel: "gemini-3.5-flash",
      },
    });
    expect(screen.getByLabelText("Ogólny model AI")).toHaveValue("gemini-3.5-flash");

    await user.selectOptions(screen.getByLabelText("Limit czasu ogólnego AI"), "180");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        generalAnalysisTimeoutSeconds: 180,
      },
    });
    expect(screen.getByLabelText("Limit czasu ogólnego AI")).toHaveValue("180");

    await user.click(within(settingsRegion).getByRole("button", { name: "Poświadczenia" }));

    await user.type(screen.getByLabelText("Klucz API Gemini"), "test-gemini-key");
    await user.click(screen.getByRole("button", { name: "Zapisz" }));

    expect(invoke).toHaveBeenCalledWith("set_gemini_transcription_api_key", {
      input: {
        apiKey: "test-gemini-key",
      },
    });
    expect(await within(settingsRegion).findByText("Skonfigurowane")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Rodzaj poświadczenia")).toBeInTheDocument();
    expect(screen.getByLabelText("Klucz API Gemini")).toHaveValue("");

    await user.click(screen.getByRole("button", { name: "Wyczyść" }));

    expect(invoke).toHaveBeenCalledWith("clear_gemini_transcription_api_key");
    await waitFor(() => {
      expect(within(settingsRegion).queryByText("Skonfigurowane")).not.toBeInTheDocument();
    });
    expect(within(settingsRegion).getAllByText("Nieskonfigurowane").length).toBeGreaterThanOrEqual(1);

    await user.click(screen.getByRole("button", { name: "Pobierz klucz API Gemini" }));

    expect(openUrl).toHaveBeenCalledWith("https://aistudio.google.com/app/apikey");

    const primaryNavigation = screen.getByLabelText("Nawigacja główna");

    await user.click(within(primaryNavigation).getByRole("button", { name: "Źródła" }));
    expect(await screen.findByRole("heading", { name: "Źródła" })).toBeInTheDocument();
    expect(screen.getByText("Odśwież źródła")).toBeInTheDocument();

    await user.click(within(primaryNavigation).getByRole("button", { name: "Transkrypcje" }));
    expect(await screen.findByRole("heading", { name: "Transkrypcje" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Odśwież zadania" })).toBeInTheDocument();
  });

  it("previews and applies import/export workflows", async () => {
    const user = userEvent.setup();
    vi.mocked(save)
      .mockResolvedValueOnce("/tmp/research-export")
      .mockResolvedValueOnce("/tmp/settings-export");

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "Import And Export" }));
    const researchPanel = within(settingsRegion).getByLabelText("Research data");
    const settingsPanel = within(settingsRegion).getByLabelText("Settings");

    await user.click(within(researchPanel).getByRole("button", { name: "Export" }));

    expect(invoke).toHaveBeenCalledWith("export_research_data");
    expect(join).toHaveBeenCalledWith("/home/test/Downloads", "brawler-research-data-2026-06-05.json");
    expect(save).toHaveBeenCalledWith({
      title: "Export file",
      defaultPath: "/home/test/Downloads/brawler-research-data-2026-06-05.json",
      filters: [{ name: "Research data", extensions: ["json"] }],
      canCreateDirectories: true,
    });
    expect(writeTextFile).toHaveBeenCalledWith("/tmp/research-export.json", "{\"schemaVersion\":1}");

    await user.upload(
      within(researchPanel).getByLabelText("Choose research data file"),
      new File(["{\"schemaVersion\":1}"], "research.json", { type: "application/json" }),
    );
    expect(within(researchPanel).getByLabelText("Choose research data file")).toHaveAttribute(
      "accept",
      ".json",
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("preview_research_import", {
        input: {
          contents: "{\"schemaVersion\":1}",
        },
      });
    });
    expect(within(researchPanel).getByLabelText("Import preview")).toBeInTheDocument();
    expect(within(researchPanel).getByText("Companies created")).toBeInTheDocument();

    await user.click(within(researchPanel).getByRole("button", { name: "Apply import" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("apply_research_import", {
        input: {
          contents: "{\"schemaVersion\":1}",
        },
      });
    });
    expect(within(researchPanel).getByLabelText("Import result")).toBeInTheDocument();

    await user.click(within(settingsPanel).getByRole("button", { name: "Export" }));

    expect(invoke).toHaveBeenCalledWith("export_settings_data");
    expect(join).toHaveBeenCalledWith("/home/test/Downloads", "brawler-settings-2026-06-05.yaml");
    expect(save).toHaveBeenCalledWith({
      title: "Export file",
      defaultPath: "/home/test/Downloads/brawler-settings-2026-06-05.yaml",
      filters: [{ name: "Settings", extensions: ["yaml", "yml"] }],
      canCreateDirectories: true,
    });
    expect(writeTextFile).toHaveBeenCalledWith(
      "/tmp/settings-export.yaml",
      "schemaVersion: 1\nsettings:\n  theme: dark\n",
    );

    await user.upload(
      within(settingsPanel).getByLabelText("Choose settings file"),
      new File(["schemaVersion: 1\nsettings:\n  theme: light\n"], "settings.yaml", {
        type: "application/x-yaml",
      }),
    );
    expect(within(settingsPanel).getByLabelText("Choose settings file")).toHaveAttribute(
      "accept",
      ".yaml,.yml",
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("preview_settings_import", {
        input: {
          contents: "schemaVersion: 1\nsettings:\n  theme: light\n",
        },
      });
    });

    await user.click(within(settingsPanel).getByRole("button", { name: "Apply import" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("apply_settings_import", {
        input: {
          contents: "schemaVersion: 1\nsettings:\n  theme: light\n",
        },
      });
    });
  });
});
