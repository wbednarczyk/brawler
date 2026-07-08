import { describe, it } from "vitest";
import { fireEvent } from "@testing-library/react";
import {
  expect,
  handleAppCommand,
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

function updateSettingsCallCount(): number {
  return vi.mocked(invoke).mock.calls.filter(([command]) => command === "update_settings").length;
}

describe("Settings screen workflows", () => {
  // Walks five settings sections plus a locale flip through the full app render —
  // dozens of userEvent round-trips that legitimately exceed the 5s default when
  // the whole suite transforms in parallel (flaked 3× at 50xx/5000ms under the
  // full gate, always green in isolation — card b6b866f). Explicit budget, not a
  // weaker assertion.
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
    // With no provider selected the model list is empty (catalog-driven, ADR 0028).
    expect(screen.getByLabelText("General AI model")).toHaveValue("");
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
    expect(within(settingsRegion).getByText("Open command palette")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ctrl+1")).toBeInTheDocument();
    // The command palette now owns Ctrl+K; Focus Inbox search moved to Ctrl+F.
    expect(within(settingsRegion).getByText("Ctrl+K")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ctrl+F")).toBeInTheDocument();
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

    expect(invoke).toHaveBeenCalledWith("set_provider_api_key", {
      input: {
        providerId: "provider_gemini",
        apiKey: "test-gemini-key",
      },
    });
    expect(await within(settingsRegion).findByText("Skonfigurowane")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Rodzaj poświadczenia")).toBeInTheDocument();
    expect(screen.getByLabelText("Klucz API Gemini")).toHaveValue("");

    await user.click(screen.getByRole("button", { name: "Wyczyść" }));

    expect(invoke).toHaveBeenCalledWith("clear_provider_api_key", {
      input: {
        providerId: "provider_gemini",
      },
    });
    await waitFor(() => {
      expect(within(settingsRegion).queryByText("Skonfigurowane")).not.toBeInTheDocument();
    });
    expect(within(settingsRegion).getAllByText("Nieskonfigurowane").length).toBeGreaterThanOrEqual(1);

    // The Claude (Anthropic) key uses the per-provider form (label localized).
    expect(within(settingsRegion).getByRole("heading", { name: "Claude (Anthropic)" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("Claude (Anthropic) Klucz API"), "test-claude-key");
    await user.click(screen.getByRole("button", { name: "Zapisz Claude (Anthropic) Klucz API" }));

    expect(invoke).toHaveBeenCalledWith("set_provider_api_key", {
      input: {
        providerId: "provider_anthropic",
        apiKey: "test-claude-key",
      },
    });

    await user.click(screen.getByRole("button", { name: "Pobierz klucz API Gemini" }));

    expect(openUrl).toHaveBeenCalledWith("https://aistudio.google.com/app/apikey");

    const primaryNavigation = screen.getByLabelText("Nawigacja główna");

    await user.click(within(primaryNavigation).getByRole("button", { name: "Źródła" }));
    expect(await screen.findByRole("heading", { name: "Źródła" })).toBeInTheDocument();
    expect(screen.getByText("Odśwież źródła")).toBeInTheDocument();

    await user.click(within(primaryNavigation).getByRole("button", { name: "Transkrypcje" }));
    expect(await screen.findByRole("heading", { name: "Transkrypcje" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Odśwież zadania" })).toBeInTheDocument();
  }, 20_000);

  it("routes an AI capability to an ordered provider pool and configures the OpenAI-compatible provider (ADR 0060 amended)", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "AI" }));

    expect(within(settingsRegion).getByRole("heading", { name: "AI capability routing" })).toBeInTheDocument();

    const kpiRow = screen
      .getByRole("heading", { name: "KPI extraction", level: 3 })
      .closest(".capability-routing-row") as HTMLElement;

    expect(within(kpiRow).getByText("Uses the general AI provider.")).toBeInTheDocument();

    // (a) adding a member defaults to the catalog's first provider + its default model.
    await user.click(within(kpiRow).getByRole("button", { name: "Add provider" }));

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        capabilityProviders: expect.objectContaining({
          kpi_extraction: [expect.objectContaining({ provider: "provider_gemini", model: "gemini-3.5-flash" })],
        }),
      },
    });
    expect(within(kpiRow).queryByText("Uses the general AI provider.")).not.toBeInTheDocument();

    // (b) a second member appended preserves order.
    await user.click(within(kpiRow).getByRole("button", { name: "Add provider" }));

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        capabilityProviders: expect.objectContaining({
          kpi_extraction: [
            expect.objectContaining({ provider: "provider_gemini", model: "gemini-3.5-flash" }),
            expect.objectContaining({ provider: "provider_gemini", model: "gemini-3.5-flash" }),
          ],
        }),
      },
    });

    // (c) switching a member to the OpenAI-compatible provider swaps its model
    // select for a free-text field. Free-text settings fields use a local
    // draft, committed on blur (docs/ui-authoring.md) — a field bound directly
    // to the settings value cannot be typed into, because the async
    // round-trip reverts each keystroke before the next one lands.
    await user.selectOptions(screen.getByLabelText("Provider KPI extraction 2"), "provider_openai_compatible");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        capabilityProviders: expect.objectContaining({
          kpi_extraction: [
            expect.objectContaining({ provider: "provider_gemini" }),
            expect.objectContaining({ provider: "provider_openai_compatible", model: "" }),
          ],
        }),
      },
    });

    const modelField = screen.getByLabelText("Model KPI extraction 2");
    expect(modelField.tagName).toBe("INPUT");

    const callsBeforeModelTyping = updateSettingsCallCount();

    fireEvent.change(modelField, { target: { value: "c" } });
    fireEvent.change(modelField, { target: { value: "cu" } });
    fireEvent.change(modelField, { target: { value: "custom-model-x" } });

    // Typing keeps the typed value in the input locally and fires no commit.
    expect(modelField).toHaveValue("custom-model-x");
    expect(updateSettingsCallCount()).toBe(callsBeforeModelTyping);

    fireEvent.blur(modelField);

    // Blur commits exactly once, with the full typed value.
    await waitFor(() => {
      expect(updateSettingsCallCount()).toBe(callsBeforeModelTyping + 1);
    });
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        capabilityProviders: expect.objectContaining({
          kpi_extraction: [
            expect.objectContaining({ provider: "provider_gemini" }),
            expect.objectContaining({ provider: "provider_openai_compatible", model: "custom-model-x" }),
          ],
        }),
      },
    });
    expect(modelField).toHaveValue("custom-model-x");

    // (d) the OpenAI-compatible base URL follows the same draft-then-blur
    // contract.
    const baseUrlField = screen.getByLabelText("OpenAI-compatible base URL");
    const callsBeforeUrlTyping = updateSettingsCallCount();

    fireEvent.change(baseUrlField, { target: { value: "h" } });
    fireEvent.change(baseUrlField, { target: { value: "ht" } });
    fireEvent.change(baseUrlField, { target: { value: "https://compat.example.com/v1" } });

    // Typing keeps the typed value in the input locally and fires no commit,
    // even though a partial URL like "h"/"ht" fails backend validation.
    expect(baseUrlField).toHaveValue("https://compat.example.com/v1");
    expect(updateSettingsCallCount()).toBe(callsBeforeUrlTyping);

    fireEvent.blur(baseUrlField);

    // Blur commits exactly once, with the full typed value.
    await waitFor(() => {
      expect(updateSettingsCallCount()).toBe(callsBeforeUrlTyping + 1);
    });
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { openaiCompatibleBaseUrl: "https://compat.example.com/v1" },
    });
    expect(screen.getByLabelText("OpenAI-compatible base URL")).toHaveValue("https://compat.example.com/v1");

    // (e) the OpenAI-compatible provider gets its own credential form, same as
    // the other additional providers (ADR 0028's per-provider credential form).
    await user.click(within(settingsRegion).getByRole("button", { name: "Credentials" }));

    expect(
      within(settingsRegion).getByRole("heading", { name: "OpenAI-compatible (custom)" }),
    ).toBeInTheDocument();

    await user.type(screen.getByLabelText("OpenAI-compatible (custom) API key"), "test-compat-key");
    await user.click(screen.getByRole("button", { name: "Save OpenAI-compatible (custom) API key" }));

    expect(invoke).toHaveBeenCalledWith("set_provider_api_key", {
      input: {
        providerId: "provider_openai_compatible",
        apiKey: "test-compat-key",
      },
    });
  });

  it("switches the general AI model to a free-text draft field for the OpenAI-compatible provider and commits it on blur", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "AI" }));

    await user.selectOptions(screen.getByLabelText("General AI provider"), "provider_openai_compatible");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { generalAnalysisProvider: "provider_openai_compatible" },
    });

    // The catalog has no models for the compatible provider, so the general
    // model field must become a free-text draft input rather than a disabled,
    // empty select.
    const generalModelField = screen.getByLabelText("General AI model");
    expect(generalModelField.tagName).toBe("INPUT");
    expect(generalModelField).not.toBeDisabled();

    const callsBeforeTyping = updateSettingsCallCount();

    fireEvent.change(generalModelField, { target: { value: "c" } });
    fireEvent.change(generalModelField, { target: { value: "custom-general-model" } });

    // Typing keeps the typed value locally and fires no commit yet.
    expect(generalModelField).toHaveValue("custom-general-model");
    expect(updateSettingsCallCount()).toBe(callsBeforeTyping);

    fireEvent.blur(generalModelField);

    // Blur commits exactly once, with the full typed value.
    await waitFor(() => {
      expect(updateSettingsCallCount()).toBe(callsBeforeTyping + 1);
    });
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { generalAnalysisModel: "custom-general-model" },
    });
    expect(generalModelField).toHaveValue("custom-general-model");
  });

  it("seeds an empty model when addMember's first catalog provider is the OpenAI-compatible provider", async () => {
    const user = userEvent.setup();
    // The default sample catalog always lists Gemini first, so exercise the
    // compatible-provider-is-first case directly by overriding just this one
    // command's response, falling back to the shared mock runtime otherwise.
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "list_ai_provider_catalog") {
        return Promise.resolve([
          {
            providerId: "provider_openai_compatible",
            label: "OpenAI-compatible (custom)",
            models: [],
            defaultModel: "",
            requiresCredential: true,
          },
          {
            providerId: "provider_gemini",
            label: "Gemini",
            models: ["gemini-3.5-flash"],
            defaultModel: "gemini-3.5-flash",
            requiresCredential: true,
          },
        ]);
      }
      return handleAppCommand(command, args as Record<string, unknown> | undefined);
    });

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "AI" }));

    const kpiRow = screen
      .getByRole("heading", { name: "KPI extraction", level: 3 })
      .closest(".capability-routing-row") as HTMLElement;

    await user.click(within(kpiRow).getByRole("button", { name: "Add provider" }));

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        capabilityProviders: expect.objectContaining({
          kpi_extraction: [expect.objectContaining({ provider: "provider_openai_compatible", model: "" })],
        }),
      },
    });
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

  it("edits and resets database connection-pool settings", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");

    await user.click(within(settingsRegion).getByRole("button", { name: "Data storage" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Data storage" })).toBeInTheDocument();
    expect(
      within(settingsRegion).getByText(
        "Advanced connection-pool tuning. Changes apply on the next app launch.",
      ),
    ).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Max connections"), "8");
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { dbMaxConnections: 8 },
    });

    await user.selectOptions(screen.getByLabelText("Busy timeout (ms)"), "30000");
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { dbBusyTimeoutMs: 30000 },
    });

    const databaseSection = within(settingsRegion).getByRole("region", { name: "Data storage" });
    await user.click(within(databaseSection).getByRole("button", { name: "Reset to defaults" }));
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { dbMaxConnections: 4, dbBusyTimeoutMs: 5000, dbAcquireTimeoutMs: 10000 },
    });
  });

  it("edits and resets background-work worker settings (ADR 0059)", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");

    await user.click(within(settingsRegion).getByRole("button", { name: "Data storage" }));

    const queueSection = within(settingsRegion).getByRole("region", { name: "Background work" });
    expect(
      within(queueSection).getByRole("heading", { name: "Background work" }),
    ).toBeInTheDocument();

    await user.selectOptions(within(queueSection).getByLabelText("Autopilot workers"), "6");
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { autopilotWorkers: 6 },
    });

    await user.selectOptions(
      within(queueSection).getByLabelText("Max concurrent calls per AI provider"),
      "4",
    );
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { aiProviderConcurrency: 4 },
    });

    await user.click(within(queueSection).getByRole("button", { name: "Reset to defaults" }));
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { sourcesWorkers: 2, autopilotWorkers: 3, aiWorkers: 2, aiProviderConcurrency: 2 },
    });
  });

  // U7-E2 density contract (ADR 0076 D6): at the S tier the section tab list
  // collapses to a SelectField (same options + selection state as the Subnav).
  // Both controls always render and share `activeSettingsTab`; the tier switch
  // that hides one or the other is CSS-only, so here we assert the select's
  // semantics (jsdom-visible) — the visual collapse is asserted in the browser
  // spec (tests/browser/density-utility.spec.ts).
  it("mirrors the settings sections in an S-tier section select", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Settings" });

    const sectionSelect = await screen.findByLabelText("Settings section");
    expect(sectionSelect).toHaveValue("appearance");
    expect(within(sectionSelect).getByRole("option", { name: "Sources" })).toBeInTheDocument();
    expect(within(sectionSelect).getByRole("option", { name: "AI" })).toBeInTheDocument();

    // Selecting a section through the collapsed control switches the panel, the
    // same effect as clicking the Subnav tab.
    await user.selectOptions(sectionSelect, "ai");
    const settingsRegion = screen.getByLabelText("Application settings");
    expect(within(settingsRegion).getByRole("heading", { name: "AI" })).toBeInTheDocument();
  });
});
