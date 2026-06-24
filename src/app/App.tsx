import { AppStateRoot } from "./AppStateRoot";
import type { Section } from "./navigation";
import type { LicenseStatus } from "../api/types";

type AppProps = {
  initialLicenseStatus?: LicenseStatus | null;
  /** Override the starting section. Defaults to the Cockpit (ADR 0053 phase 6).
   *  Used for deep links, and by tests to land directly on a screen that the
   *  slimmed top-nav no longer exposes (it is now a Cockpit panel). */
  initialSection?: Section;
};

export function App({ initialLicenseStatus, initialSection }: AppProps = {}) {
  return (
    <AppStateRoot initialLicenseStatus={initialLicenseStatus} initialSection={initialSection} />
  );
}
