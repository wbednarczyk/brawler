import type { FormEvent } from "react";
import { KeyRound, Save } from "lucide-react";
import type { LicenseStatus } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { useLocale } from "../../shared/locale";

type LicenseGateScreenProps = {
  licenseError: string | null;
  licenseInFlight: boolean;
  licenseKeyDraft: string;
  licenseStatus: LicenseStatus | null;
  onLicenseKeyDraftChange: (value: string) => void;
  onSubmitLicenseKey: (event: FormEvent<HTMLFormElement>) => void;
};

export function LicenseGateScreen({
  licenseError,
  licenseInFlight,
  licenseKeyDraft,
  licenseStatus,
  onLicenseKeyDraftChange,
  onSubmitLicenseKey,
}: LicenseGateScreenProps) {
  const { text } = useLocale();
  const reason = licenseError ?? licenseStatus?.reason;

  return (
    <main className="license-gate" aria-labelledby="license-gate-title">
      <section className="license-gate-panel">
        <div className="license-gate-brand">
          <div className="brand-mark">B</div>
          <div>
            <p className="eyebrow">{text("License")}</p>
            <h1 id="license-gate-title">{text("License required")}</h1>
          </div>
        </div>

        <p className="license-gate-copy">
          {text("Enter license key")}
        </p>

        <form className="license-gate-form" onSubmit={onSubmitLicenseKey}>
          <label>
            {text("License key")}
            <textarea
              aria-label={text("License key")}
              autoComplete="off"
              placeholder="BRAWLER-LIC-1..."
              rows={5}
              value={licenseKeyDraft}
              onChange={(event) => onLicenseKeyDraftChange(event.target.value)}
            />
          </label>
          <Button
            disabled={licenseInFlight || !licenseKeyDraft.trim()}
            type="submit"
            variant="action"
          >
            {licenseInFlight ? <KeyRound size={14} /> : <Save size={14} />}
            {licenseInFlight ? text("Activating") : text("Activate")}
          </Button>
        </form>

        {reason ? <p className="error-text">{reason}</p> : null}
      </section>
    </main>
  );
}
