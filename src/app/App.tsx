import { AppStateRoot } from "./AppStateRoot";
import type { Section } from "./navigation";
import type { LicenseStatus } from "../api/types";
import { ToastProvider } from "../ui";

type AppProps = {
  initialLicenseStatus?: LicenseStatus | null;
  /** Override the starting section. Defaults to the Cockpit (ADR 0053 phase 6).
   *  Used for deep links, and by tests to land directly on a screen that the
   *  slimmed top-nav no longer exposes (it is now a Cockpit panel). */
  initialSection?: Section;
};

export function App({ initialLicenseStatus, initialSection }: AppProps = {}) {
  // ToastProvider mounts above AppStateRoot so the state controllers (called in
  // AppStateRoot's body) can consume useToast for the undo-vs-confirm contract
  // (ADR 0076 Decision 5). Its viewport renders locale-agnostic, pre-translated
  // strings, so sitting above LocaleContext is fine.
  return (
    <ToastProvider>
      <AppStateRoot initialLicenseStatus={initialLicenseStatus} initialSection={initialSection} />
    </ToastProvider>
  );
}
