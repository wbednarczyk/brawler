import { describe, it } from "vitest";
import { axe } from "jest-axe";
import {
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
    await user.click(within(region).getByRole("switch", { name: "Enable the server" }));
    expect(invoke).toHaveBeenCalledWith("set_mcp_enabled", { enabled: true });
    expect(
      await within(region).findByText(/auth token is not configured/i),
    ).toBeInTheDocument();
    expect(within(region).getByText("Stopped")).toBeInTheDocument();

    // Generate a token, then enabling succeeds and the pill flips to Running.
    await user.click(within(region).getByRole("button", { name: "Generate token" }));
    await within(region).findByLabelText("Access token");

    await user.click(within(region).getByRole("switch", { name: "Enable the server" }));
    expect(await within(region).findByText("Active")).toBeInTheDocument();

    // The enabled state persists: navigate away and back, the section re-reads
    // mcp_status on mount and the toggle + pill still show the server running.
    await user.click(within(region).getByRole("button", { name: "Logs" }));
    await user.click(within(region).getByRole("button", { name: "MCP server" }));
    expect(await within(region).findByText("Active")).toBeInTheDocument();
    expect(within(region).getByRole("switch", { name: "Enable the server" })).toBeChecked();
  });

  // v0.52 dogfooding gap: the app runs on Windows and its server is loopback-only,
  // so a Claude in WSL can't reach it under default networking. The connect section
  // must surface where to add the server (same machine / WSL mirrored caveat).
  it("shows the same-machine / WSL hint near the connection snippets", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    expect(
      within(region).getByText(/same machine as this app \(Windows\).*mirrored networking/i),
    ).toBeInTheDocument();
  });

  it("reveals the plaintext token exactly once — it is gone after navigating away", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    await user.click(within(region).getByRole("button", { name: "Generate token" }));
    const tokenField = await within(region).findByLabelText<HTMLInputElement>("Access token");
    expect(tokenField.value.length).toBeGreaterThan(0);
    expect(within(region).getByText(/shown once/i)).toBeInTheDocument();

    // Leave the section and come back: the reveal is gone; only status remains.
    await user.click(within(region).getByRole("button", { name: "Logs" }));
    await user.click(within(region).getByRole("button", { name: "MCP server" }));

    expect(within(region).queryByLabelText("Access token")).not.toBeInTheDocument();
    expect(within(region).getByText("A token is configured.")).toBeInTheDocument();
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

    await user.click(within(region).getByRole("button", { name: "Generate token" }));
    const tokenField = await within(region).findByLabelText<HTMLInputElement>("Access token");
    await user.click(within(region).getByRole("button", { name: "Copy token" }));

    expect(writeText).toHaveBeenCalledWith(tokenField.value);
  });

  it("revokes the token behind an inline confirm", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    await user.click(within(region).getByRole("button", { name: "Generate token" }));
    await within(region).findByText("A token is configured.");

    // The trigger opens the InlineConfirm; its confirm affordance carries the
    // same "Revoke token" label and commits the revoke (the trigger is hidden
    // while confirming, so only one such button exists at a time).
    await user.click(within(region).getByRole("button", { name: "Revoke token" }));
    await user.click(within(region).getByRole("button", { name: "Revoke token" }));

    expect(invoke).toHaveBeenCalledWith("revoke_mcp_token");
    expect(await within(region).findByText(/No token yet/i)).toBeInTheDocument();
  });

  it("commits the listen port on blur through update_settings, clamped", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openMcpSection(user);

    const portField = within(region).getByLabelText<HTMLInputElement>("Listen port");
    await user.clear(portField);
    await user.type(portField, "9000");
    await user.tab();

    expect(invoke).toHaveBeenCalledWith("update_settings", { input: { mcpPort: 9000 } });

    // Out-of-range input clamps and surfaces the range hint rather than persisting the raw value.
    await user.clear(portField);
    await user.type(portField, "70000");
    await user.tab();

    expect(invoke).toHaveBeenCalledWith("update_settings", { input: { mcpPort: 65535 } });
    expect(within(region).getByText(/between 1024 and 65535/i)).toBeInTheDocument();
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
