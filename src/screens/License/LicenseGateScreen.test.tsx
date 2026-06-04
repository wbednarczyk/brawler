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

describe("License gate workflows", () => {
  it("blocks normal navigation until a valid friend-test license is submitted", async () => {
    const user = userEvent.setup();
    appTestState.licenseStatusResponse = missingLicenseStatus;

    renderApp();

    expect(await screen.findByRole("heading", { name: "License required" })).toBeInTheDocument();
    expect(screen.queryByTitle("Inbox")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("License key"), "valid-friend-license");
    await user.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("submit_license_key", {
        input: { licenseKey: "valid-friend-license" },
      });
    });

    expect(await screen.findByTitle("Inbox")).toBeInTheDocument();
  });

  it("shows invalid license feedback without opening normal navigation", async () => {
    const user = userEvent.setup();
    appTestState.licenseStatusResponse = missingLicenseStatus;

    renderApp();

    await user.type(await screen.findByLabelText("License key"), "tampered-license");
    await user.click(screen.getByRole("button", { name: "Activate" }));

    expect(await screen.findByText("This license key could not be verified.")).toBeInTheDocument();
    expect(screen.queryByTitle("Inbox")).not.toBeInTheDocument();
  });

  it("lets a valid user inspect and clear license status in Settings", async () => {
    const user = userEvent.setup();
    appTestState.licenseStatusResponse = {
      ...appTestState.licenseStatusResponse,
      status: "valid",
      canUseApp: true,
    };

    renderApp();

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    const licenseSection = within(settingsRegion).getByRole("heading", { name: "License" })
      .closest("section");

    expect(licenseSection).not.toBeNull();
    expect(within(licenseSection as HTMLElement).getByText("Friend Tester")).toBeInTheDocument();
    expect(within(licenseSection as HTMLElement).getByText("friend_test")).toBeInTheDocument();

    await user.click(within(licenseSection as HTMLElement).getByRole("button", { name: "Clear license" }));

    expect(await screen.findByRole("heading", { name: "License required" })).toBeInTheDocument();
  });

  it("keeps expired or invalid statuses recoverable at the gate", async () => {
    appTestState.licenseStatusResponse = {
      ...invalidLicenseStatus,
      status: "expired",
      reason: "This license has expired.",
    };

    renderApp();

    expect(await screen.findByText("This license has expired.")).toBeInTheDocument();
    expect(screen.getByLabelText("License key")).toBeInTheDocument();
  });
});
