import { callCommand } from "./tauri";
import type { LicenseStatus } from "./types";

export function getLicenseStatus() {
  return callCommand<LicenseStatus>("get_license_status");
}

export function submitLicenseKey(licenseKey: string) {
  return callCommand<LicenseStatus>("submit_license_key", { input: { licenseKey } });
}

export function clearLicenseKey() {
  return callCommand<LicenseStatus>("clear_license_key");
}

