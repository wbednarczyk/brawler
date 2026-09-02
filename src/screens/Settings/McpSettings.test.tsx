import { describe, it } from "vitest";
import { axe } from "jest-axe";
import {
  appTestState,
  expect,
  invoke,
  renderApp,
  screen,
  userEvent,
  vi,
  within,
} from "../../test/appWorkflowHarness";

// M4 (ADR 0078): the "MCP server" Settings section is the capability's UI entry
// point — enable/disable the local server live, manage the one-time bearer
// token, and copy connection snippets. These are full-app workflow tests
// against the stateful mock runtime (ADR 0048), so they exercise the real
// commands (set_mcp_enabled / mcp_status / regenerate_mcp_token / revoke /
// update_settings mcpPort) the same way the browser harness does.

async function openMcpSection(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Settings" }));
  const region = await screen.findByLabelText("Application settings");
  await user.click(within(region).getByRole("button", { name: "MCP server" }));
  return region;
}

describe("MCP server settings (M4, ADR 0078)", () => {
  it("generates a token, enables the server, and renders the live running status", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    // No token yet: enabling refuses with a surfaced error (never a crash).
    await user.click(
      within(region).getByRole("switch", { name: "Let assistants connect" }),
    );
    expect(invoke).toHaveBeenCalledWith("set_mcp_enabled", { enabled: true });
    expect(
      await within(region).findByText(/auth token is not configured/i),
    ).toBeInTheDocument();
    expect(within(region).getByText("Stopped")).toBeInTheDocument();

    // Generate a token: rotation restarts the listener from keychain truth
    // (ADR 0099 dec. 2) and the toggle's setting persisted from the refused
    // attempt — the server comes up without a second click, and the fresh
    // status fetch shows it.
    await user.click(
      within(region).getByRole("button", { name: "Generate token" }),
    );
    await within(region).findByLabelText("Access token");
    expect(await within(region).findByText("Active")).toBeInTheDocument();

    // The enabled state persists: navigate away and back, the section re-reads
    // mcp_status on mount and the toggle + pill still show the server running.
    await user.click(within(region).getByRole("button", { name: "Logs" }));
    await user.click(
      within(region).getByRole("button", { name: "MCP server" }),
    );
    expect(await within(region).findByText("Active")).toBeInTheDocument();
    expect(
      within(region).getByRole("switch", { name: "Let assistants connect" }),
    ).toBeChecked();
  });

  // v0.52 dogfooding gap: the app runs on Windows and its server is loopback-only,
  // so a Claude in WSL can't reach it under default networking. The connect section
  // must surface where to add the server (same machine / WSL mirrored caveat).
  // F4c S4: this instruction is developer-gated (docs/plans/f4c-contracts/
  // s4-settings-pass-banner.md item 1) — a copy-paste snippet caveat, not
  // product copy every user needs.
  it("shows the same-machine / WSL hint near the connection snippets", async () => {
    const user = userEvent.setup();
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };
    renderApp();
    const region = await openMcpSection(user);

    expect(
      within(region).getByText(
        /same machine as this app \(Windows\).*mirrored networking/i,
      ),
    ).toBeInTheDocument();
  });

  it("reveals the plaintext token exactly once — it is gone after navigating away", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    await user.click(
      within(region).getByRole("button", { name: "Generate token" }),
    );
    const tokenField =
      await within(region).findByLabelText<HTMLInputElement>("Access token");
    expect(tokenField.value.length).toBeGreaterThan(0);
    expect(within(region).getByText(/shown once/i)).toBeInTheDocument();

    // Leave the section and come back: the reveal is gone; only status remains.
    await user.click(within(region).getByRole("button", { name: "Logs" }));
    await user.click(
      within(region).getByRole("button", { name: "MCP server" }),
    );

    expect(
      within(region).queryByLabelText("Access token"),
    ).not.toBeInTheDocument();
    expect(
      within(region).getByText("A token is configured."),
    ).toBeInTheDocument();
  });

  it("copies the revealed token to the clipboard", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    });
    renderApp();
    const region = await openMcpSection(user);

    await user.click(
      within(region).getByRole("button", { name: "Generate token" }),
    );
    const tokenField =
      await within(region).findByLabelText<HTMLInputElement>("Access token");
    await user.click(
      within(region).getByRole("button", { name: "Copy token" }),
    );

    expect(writeText).toHaveBeenCalledWith(tokenField.value);
  });

  it("revokes the token behind an inline confirm", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    await user.click(
      within(region).getByRole("button", { name: "Generate token" }),
    );
    await within(region).findByText("A token is configured.");

    // The trigger opens the InlineConfirm; its confirm affordance carries the
    // same "Remove token" label and commits the revoke (the trigger is hidden
    // while confirming, so only one such button exists at a time).
    await user.click(
      within(region).getByRole("button", { name: "Remove token" }),
    );
    await user.click(
      within(region).getByRole("button", { name: "Remove token" }),
    );

    expect(invoke).toHaveBeenCalledWith("revoke_mcp_token");
    expect(
      await within(region).findByText(/No token yet/i),
    ).toBeInTheDocument();
  });

  it("commits the listen port on blur through update_settings, clamped", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    const portField =
      within(region).getByLabelText<HTMLInputElement>("Port");
    await user.clear(portField);
    await user.type(portField, "9000");
    await user.tab();

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { mcpPort: 9000 },
    });

    // Out-of-range input clamps and surfaces the range hint rather than persisting the raw value.
    await user.clear(portField);
    await user.type(portField, "70000");
    await user.tab();

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { mcpPort: 65535 },
    });
    expect(
      within(region).getByText(/between 1024 and 65535/i),
    ).toBeInTheDocument();
  });

  // ADR 0088 M3: the write tier is a second toggle, off by default, committed
  // through update_settings. Its helper copy states the citation requirement and
  // that an assistant can never enable it itself (update_settings is MCP-excluded).
  it("toggles the write tier through update_settings and shows the citation caveat", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    const writesToggle = within(region).getByRole("switch", {
      name: "Allow the assistant to write",
    });
    expect(writesToggle).not.toBeChecked();
    expect(within(region).getByText("Read only")).toBeInTheDocument();
    expect(
      within(region).getByText(
        /Every write needs a citation.*never turn this on itself/i,
      ),
    ).toBeInTheDocument();

    await user.click(writesToggle);
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { mcpWritesEnabled: true },
    });
  });

  // ADR 0099 dec. 2: the acquisition scope is a second, limited credential
  // with its own gate toggle (kill switch at auth) and its own token section.
  it("toggles the acquisition gate through update_settings and explains the scope", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    const gateToggle = within(region).getByRole("switch", {
      name: "Allow report-data processing",
    });
    expect(gateToggle).not.toBeChecked();
    expect(
      within(region).getByText(/sees only the report-data workflow/i),
    ).toBeInTheDocument();
    // Honest transitional copy: the credential exists before its tools do.
    expect(
      within(region).getByText(/ingest tools themselves arrive in a later update/i),
    ).toBeInTheDocument();

    await user.click(gateToggle);
    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: { kpiAcquisitionEnabled: true },
    });
  });

  it("manages the acquisition token and refreshes the live server status", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    expect(
      within(region).getByText(/No acquisition token yet/i),
    ).toBeInTheDocument();

    await user.click(
      within(region).getByRole("button", { name: "Generate acquisition token" }),
    );
    expect(invoke).toHaveBeenCalledWith("regenerate_kpi_acquisition_token");
    // Every rotate/revoke restarts the listener, so the section re-fetches
    // the live status (ADR 0099 dec. 2 — the restart outcome never goes stale).
    expect(invoke).toHaveBeenCalledWith("mcp_status");
    const tokenField = await within(region).findByLabelText<HTMLInputElement>(
      "Acquisition token",
    );
    expect(tokenField.value.length).toBeGreaterThan(0);
    // Gate off ⇒ the composed state says configured-but-disabled.
    expect(
      within(region).getByText(/Configured, disabled/i),
    ).toBeInTheDocument();

    // Revoke behind an inline confirm, mirroring the primary token.
    await user.click(
      within(region).getByRole("button", { name: "Remove acquisition token" }),
    );
    await user.click(
      within(region).getByRole("button", { name: "Remove acquisition token" }),
    );
    expect(invoke).toHaveBeenCalledWith("revoke_kpi_acquisition_token");
    expect(
      await within(region).findByText(/No acquisition token yet/i),
    ).toBeInTheDocument();
  });

  it("revoking the primary token refreshes the pill to Stopped (the restart refused)", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    // Token + enabled server first.
    await user.click(
      within(region).getByRole("button", { name: "Generate token" }),
    );
    await within(region).findByLabelText("Access token");
    await user.click(
      within(region).getByRole("switch", { name: "Let assistants connect" }),
    );
    expect(await within(region).findByText("Active")).toBeInTheDocument();

    // Revoke: the restart refuses (no token) and the SECTION shows it without
    // a remount — the fresh mcp_status fetch is the fix under test.
    await user.click(
      within(region).getByRole("button", { name: "Remove token" }),
    );
    await user.click(
      within(region).getByRole("button", { name: "Remove token" }),
    );
    expect(await within(region).findByText("Stopped")).toBeInTheDocument();
    expect(
      await within(region).findByText(/auth token is not configured/i),
    ).toBeInTheDocument();
  });

  it("has no accessibility violations", async () => {
    const user = userEvent.setup();
    const { container } = renderApp();
    await openMcpSection(user);
    // Reveal the widest content (token field + snippets) so axe covers it too.
    await user.click(screen.getByRole("button", { name: "Generate token" }));
    await screen.findByLabelText("Access token");

    const results = await axe(container, {
      rules: { region: { enabled: false } },
    });
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
