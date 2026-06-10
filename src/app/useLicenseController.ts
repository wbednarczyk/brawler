import type { Dispatch, FormEvent, SetStateAction } from "react";
import * as licensingApi from "../api/licensing";
import type { LicenseStatus } from "../api/types";

type LicenseControllerInput = {
  licenseKeyDraft: string;
  setLicenseError: Dispatch<SetStateAction<string | null>>;
  setLicenseInFlight: Dispatch<SetStateAction<boolean>>;
  setLicenseKeyDraft: Dispatch<SetStateAction<string>>;
  setLicenseStatus: Dispatch<SetStateAction<LicenseStatus | null>>;
};

export function useLicenseController({
  licenseKeyDraft,
  setLicenseError,
  setLicenseInFlight,
  setLicenseKeyDraft,
  setLicenseStatus,
}: LicenseControllerInput) {
  function refreshLicenseStatus() {
    return licensingApi.getLicenseStatus()
      .then((status) => {
        setLicenseStatus(status);
        setLicenseError(null);
      })
      .catch((error) => {
        setLicenseStatus({
          status: "storage_error",
          canUseApp: true,
          reason: String(error),
          license: null,
          checkedAt: new Date().toISOString(),
        });
        setLicenseError(String(error));
      });
  }

  function submitLicenseKey(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();
    const licenseKey = licenseKeyDraft.trim();
    if (!licenseKey) {
      setLicenseError("License key is required.");
      return;
    }

    setLicenseInFlight(true);
    setLicenseError(null);

    licensingApi.submitLicenseKey(licenseKey)
      .then((status) => {
        setLicenseStatus(status);
        if (status.status === "valid") {
          setLicenseKeyDraft("");
        } else {
          setLicenseError(status.reason ?? "License key was rejected.");
        }
      })
      .catch((error) => {
        setLicenseError(String(error));
      })
      .finally(() => {
        setLicenseInFlight(false);
      });
  }

  function clearLicenseKey() {
    setLicenseInFlight(true);
    setLicenseError(null);

    licensingApi.clearLicenseKey()
      .then((status) => {
        setLicenseStatus(status);
        setLicenseKeyDraft("");
      })
      .catch((error) => {
        setLicenseError(String(error));
      })
      .finally(() => {
        setLicenseInFlight(false);
      });
  }

  return {
    clearLicenseKey,
    refreshLicenseStatus,
    submitLicenseKey,
  };
}
