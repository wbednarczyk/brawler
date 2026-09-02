import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsProvider } from "../../app/state/SettingsContext";
import { McpSettings } from "./McpSettings";

// sol fix1 item 5: the environment-fallback note (`McpSettings.tsx:432`) is
// developer-gated, like the equivalent credential note
// (`CredentialSettings.tsx:94`, `developerMode && …devFallbackAvailable`).
// The stateful mock runtime (`src/test/scenarios/runtime.ts`) always reports
// `devFallbackAvailable: false` for MCP — no env-var seam to drive it true —
// so this mocks `api/mcp` directly, isolated in its own file (a module-level
// `vi.mock` here would otherwise break every other `McpSettings.test.tsx`
// case that relies on the real command wiring through the full-app harness).
vi.mock("../../api/mcp", () => ({
  mcpTokenStatus: vi.fn(),
  kpiAcquisitionTokenStatus: vi.fn(),
  mcpStatus: vi.fn(),
  regenerateMcpToken: vi.fn(),
  revokeMcpToken: vi.fn(),
  regenerateKpiAcquisitionToken: vi.fn(),
  revokeKpiAcquisitionToken: vi.fn(),
  setMcpEnabled: vi.fn(),
}));

import { mcpTokenStatus, kpiAcquisitionTokenStatus, mcpStatus } from "../../api/mcp";

const mcpTokenStatusMock = vi.mocked(mcpTokenStatus);
const kpiAcquisitionTokenStatusMock = vi.mocked(kpiAcquisitionTokenStatus);
const mcpStatusMock = vi.mocked(mcpStatus);

const fallbackTokenStatus = {
  providerId: "mcp",
  secretKind: "auth_token",
  configured: true,
  storage: "os_keychain",
  label: "MCP server auth token",
  devFallbackAvailable: true,
  error: null,
};

const kpiTokenStatus = {
  providerId: "mcp",
  secretKind: "kpi_acquisition_token",
  configured: false,
  storage: "not_configured",
  label: "MCP acquisition token",
  devFallbackAvailable: false,
  error: null,
};

const serverStatus = { running: false, port: 8317, error: null, kpiAcquisitionConfigured: false };

const props = {
  settings: null,
  onMcpPortChange: vi.fn(),
  onMcpWritesEnabledChange: vi.fn(),
  onKpiAcquisitionEnabledChange: vi.fn(),
};

describe("McpSettings development-fallback note (sol fix1 item 5)", () => {
  beforeEach(() => {
    mcpTokenStatusMock.mockResolvedValue(fallbackTokenStatus);
    kpiAcquisitionTokenStatusMock.mockResolvedValue(kpiTokenStatus);
    mcpStatusMock.mockResolvedValue(serverStatus);
  });

  it("stays hidden outside developer mode even when the fallback is available", async () => {
    render(<McpSettings {...props} />);

    await screen.findByText("A token is configured.");
    expect(screen.queryByText(/Development fallback is active/i)).not.toBeInTheDocument();
  });

  it("shows once developer mode is on", async () => {
    render(
      <SettingsProvider value={{ developerMode: true } as never}>
        <McpSettings {...props} />
      </SettingsProvider>,
    );

    expect(await screen.findByText(/Development fallback is active/i)).toBeInTheDocument();
  });
});
