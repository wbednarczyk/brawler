import { AppStateRoot } from "./AppStateRoot";
import type { LicenseStatus } from "../api/types";

type AppProps = {
  initialLicenseStatus?: LicenseStatus | null;
};

export function App({ initialLicenseStatus }: AppProps = {}) {
  return <AppStateRoot initialLicenseStatus={initialLicenseStatus} />;
}
