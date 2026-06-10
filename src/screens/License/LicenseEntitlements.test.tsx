import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  invalidLicenseStatus,
  missingLicenseStatus,
  invoke,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

describe("License entitlement workflows", () => {
  it("keeps normal navigation available when no license is present", async () => {
    appTestState.licenseStatusResponse = missingLicenseStatus;

    renderApp();

    expect(await screen.findByTitle("Inbox")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "License" })).not.toBeInTheDocument();
  });

  it("shows invalid license feedback in Settings without blocking navigation", async () => {
    const user = userEvent.setup();
    appTestState.licenseStatusResponse = missingLicenseStatus;

    renderApp();

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "License" }));

    await user.type(within(settingsRegion).getByLabelText("Replace license key"), "tampered-license");
    await user.click(within(settingsRegion).getByRole("button", { name: "Save license" }));

    expect(await screen.findAllByText("This license key could not be verified.")).toHaveLength(2);
    expect(screen.getByTitle("Inbox")).toBeInTheDocument();
  });

  it("lets a user save, inspect, and clear license status in Settings", async () => {
    const user = userEvent.setup();
    appTestState.licenseStatusResponse = missingLicenseStatus;

    renderApp();

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "License" }));

    await user.type(within(settingsRegion).getByLabelText("Replace license key"), "valid-friend-license");
    await user.click(within(settingsRegion).getByRole("button", { name: "Save license" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("submit_license_key", {
        input: { licenseKey: "valid-friend-license" },
      });
    });

    expect(await within(settingsRegion).findByText("Friend Tester")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("friend_test")).toBeInTheDocument();

    await user.click(within(settingsRegion).getByRole("button", { name: "Clear license" }));

    expect(await within(settingsRegion).findByText("missing")).toBeInTheDocument();
    expect(screen.getByTitle("Inbox")).toBeInTheDocument();
  });

  it("keeps expired statuses recoverable in Settings", async () => {
    const user = userEvent.setup();
    appTestState.licenseStatusResponse = {
      ...invalidLicenseStatus,
      status: "expired",
      canUseApp: true,
      reason: "This license has expired.",
    };

    renderApp();

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    await user.click(within(settingsRegion).getByRole("button", { name: "License" }));

    expect(await within(settingsRegion).findByText("This license has expired.")).toBeInTheDocument();
    expect(within(settingsRegion).getByLabelText("Replace license key")).toBeInTheDocument();
    expect(screen.getByTitle("Inbox")).toBeInTheDocument();
  });
});
