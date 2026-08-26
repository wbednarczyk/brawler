import { describe, it } from "vitest";
import { fireEvent } from "@testing-library/react";
import {
  expect,
  invoke,
  join,
  openUrl,
  renderApp,
  save,
  screen,
  userEvent,
  vi,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

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

    // Feed mass-delete removed (#329, owner decision 2026-08-05): the Sources tab
    // no longer exposes a cleanup/retention subsection, only poll and backfill.
    expect(within(settingsRegion).getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(
      within(settingsRegion).queryByRole("heading", { name: "Feed Cleanup" }),
    ).not.toBeInTheDocument();
    expect(
      within(settingsRegion).queryByRole("button", { name: "Clean up feed now" }),
    ).not.toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Transcripts" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
    expect(screen.getByLabelText("Gemini transcription model")).toHaveValue("gemini-2.5-flash");
    expect(screen.getByLabelText("Gemini transcription timeout")).toHaveValue("300");
    expect(within(settingsRegion).queryByText("provider_gemini")).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("Cheapest supported")).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("YouTube transcription provider ID")).not.toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Credentials" }));

    expect(within(settingsRegion).getByText("Credential kind")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("API Key")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Keyboard shortcuts" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Shortcuts are ignored while typing in fields and editors.")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Open Inbox")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Open Inbox search")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Open command palette")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ctrl+1")).toBeInTheDocument();
    // The command palette now owns Ctrl+K; Focus Inbox search moved to Ctrl+F.
    expect(within(settingsRegion).getByText("Ctrl+K")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Ctrl+F")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("F9")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Shift+F9")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Open next inbox item")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Open notebook entry editor")).toBeInTheDocument();

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

    await user.click(within(settingsRegion).getByRole("button", { name: "Import i eksport" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Import i eksport" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Dane badawcze")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Skróty klawiaturowe" }));

    expect(within(settingsRegion).getByRole("heading", { name: "Skróty klawiaturowe" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Otwórz Inbox")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Otwórz wyszukiwanie inboxu")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Źródła" }));

    await user.selectOptions(screen.getByLabelText("Interwał odpytywania źródeł w ustawieniach"), "1800");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        pollIntervalSeconds: 1800,
      },
    });
    expect(screen.getByLabelText("Interwał odpytywania źródeł w ustawieniach")).toHaveValue("1800");

    await user.click(within(settingsRegion).getByRole("button", { name: "Transkrypcje" }));

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
    // Issue #106: the write is a typed backend command (extension enforcement
    // happens Rust-side) — the webview holds no filesystem permission.
    expect(invoke).toHaveBeenCalledWith("write_export_file", {
      input: {
        path: "/tmp/research-export",
        contents: "{\"schemaVersion\":1}",
        allowedExtensions: ["json"],
        defaultExtension: "json",
      },
    });

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
    // v0.54 T6: applying an import raises a transient success toast in addition
    // to the (persistent) result summary grid.
    expect(await screen.findByRole("status")).toHaveTextContent("Import applied");

    await user.click(within(settingsPanel).getByRole("button", { name: "Export" }));

    expect(invoke).toHaveBeenCalledWith("export_settings_data");
    expect(join).toHaveBeenCalledWith("/home/test/Downloads", "brawler-settings-2026-06-05.yaml");
    expect(save).toHaveBeenCalledWith({
      title: "Export file",
      defaultPath: "/home/test/Downloads/brawler-settings-2026-06-05.yaml",
      filters: [{ name: "Settings", extensions: ["yaml", "yml"] }],
      canCreateDirectories: true,
    });
    expect(invoke).toHaveBeenCalledWith("write_export_file", {
      input: {
        path: "/tmp/settings-export",
        contents: "schemaVersion: 1\nsettings:\n  theme: dark\n",
        allowedExtensions: ["yaml", "yml"],
        defaultExtension: "yaml",
      },
    });

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

    // ADR 0084: the AI lane is retired with the analysis layer — its worker and
    // per-provider-concurrency controls must not come back.
    expect(within(queueSection).queryByLabelText("AI workers")).not.toBeInTheDocument();
    expect(
      within(queueSection).queryByLabelText("Max concurrent calls per AI provider"),
    ).not.toBeInTheDocument();

    await user.click(within(queueSection).getByRole("button", { name: "Reset to defaults" }));
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { sourcesWorkers: 2, autopilotWorkers: 3 },
    });
  });

  // T3.3 (ADR 0077 §3): backfill depth is configurable (default 3, 1–10). The
  // Sources section offers clickable presets bound to a numeric input; a preset
  // click persists the new depth and updates the bound value.
  it("persists a backfill-depth preset selection and reflects it in the bound input", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Settings" });

    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "Sources" }));

    // Defaults to 3 (the sample settings value), shown in the bound numeric input.
    const yearsInput = screen.getByLabelText("Backfill history depth in years");
    expect(yearsInput).toHaveValue(3);

    await user.click(within(settingsRegion).getByRole("button", { name: "5" }));

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { backfillYears: 5 },
    });
    await waitFor(() => expect(yearsInput).toHaveValue(5));
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
    expect(within(sectionSelect).getByRole("option", { name: "Transcripts" })).toBeInTheDocument();

    // Selecting a section through the collapsed control switches the panel, the
    // same effect as clicking the Subnav tab.
    await user.selectOptions(sectionSelect, "transcripts");
    const settingsRegion = screen.getByLabelText("Application settings");
    expect(within(settingsRegion).getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
  });
  // ADR 0084 (retire the in-app AI analysis layer): Settings must expose the
  // transcript provider — the only surviving model-backed capability, because
  // transcription is data acquisition, not interpretation — and must expose NO
  // AI-analysis routing surface. The routed layer (general analysis provider /
  // model / timeout, the OpenAI-compatible base URL, the per-capability routing
  // pools, the ESPI AI fallback toggle, the tier-4 sweep budget, and the
  // Claude/OpenAI/Mistral key forms) is gone; intelligence arrives over MCP.
  it("exposes the transcript provider and no AI-analysis routing surface (ADR 0084)", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Settings" });

    const settingsRegion = await screen.findByLabelText("Application settings");

    // No AI tab at all — the section is Transcripts now.
    expect(within(settingsRegion).queryByRole("button", { name: "AI" })).not.toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Transcripts" }));

    // The transcript-provider section survives, with both of its controls.
    expect(within(settingsRegion).getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
    expect(screen.getByLabelText("Gemini transcription model")).toBeInTheDocument();
    expect(screen.getByLabelText("Gemini transcription timeout")).toBeInTheDocument();

    // Every analysis-routing control is gone.
    for (const label of [
      "General AI provider",
      "General AI model",
      "General AI timeout",
      "OpenAI-compatible base URL",
      "ESPI AI classification fallback",
      "History sweep AI budget in calls",
    ]) {
      expect(screen.queryByLabelText(label)).not.toBeInTheDocument();
    }
    expect(
      within(settingsRegion).queryByRole("heading", { name: "AI capability routing" }),
    ).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("Claim extraction")).not.toBeInTheDocument();
    expect(within(settingsRegion).queryByText("Vision extraction")).not.toBeInTheDocument();

    // No analysis-provider catalog is fetched any more.
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).not.toContain(
      "list_ai_provider_catalog",
    );

    // Credentials keeps the Gemini transcript key and drops the analysis keys.
    await user.click(within(settingsRegion).getByRole("button", { name: "Credentials" }));
    expect(screen.getByLabelText("Gemini API key")).toBeInTheDocument();
    for (const label of [
      "Claude (Anthropic) API key",
      "OpenAI (ChatGPT) API key",
      "OpenAI-compatible (custom) API key",
      "Mistral API key",
    ]) {
      expect(screen.queryByLabelText(label)).not.toBeInTheDocument();
    }
  });
});
