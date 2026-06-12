import type { FormEvent } from "react";
import { KeyRound, Save, Trash2 } from "lucide-react";
import type { LicenseStatus } from "../../api/types";
import { Button } from "../../ui";
import { useLocale } from "../../shared/locale";

type LicenseSettingsProps = {
  licenseError: string | null;
  licenseInFlight: boolean;
  licenseKeyDraft: string;
  licenseStatus: LicenseStatus | null;
  onClearLicenseKey: () => void;
  onLicenseKeyDraftChange: (value: string) => void;
  onSubmitLicenseKey: (event: FormEvent<HTMLFormElement>) => void;
};

export function LicenseSettings({
  licenseError,
  licenseInFlight,
  licenseKeyDraft,
  licenseStatus,
  onClearLicenseKey,
  onLicenseKeyDraftChange,
  onSubmitLicenseKey,
}: LicenseSettingsProps) {
  const { text } = useLocale();
  const license = licenseStatus?.license;

  return (
    <section className="settings-group" aria-labelledby="settings-license-title">
      <h2 id="settings-license-title">{text("License")}</h2>

      <dl className="settings-grid">
        <div>
          <dt>{text("Status")}</dt>
          <dd>{formatLicenseStatus(licenseStatus)}</dd>
        </div>
        <div>
          <dt>{text("Holder")}</dt>
          <dd>{license?.holder ?? text("Not available")}</dd>
        </div>
        <div>
          <dt>{text("Channel")}</dt>
          <dd>{license?.channel ?? text("Not available")}</dd>
        </div>
        <div>
          <dt>{text("Expires")}</dt>
          <dd>{license?.expiresAt ?? text("Not available")}</dd>
        </div>
      </dl>

      <form className="credential-form" onSubmit={onSubmitLicenseKey}>
        <label>
          {text("Replace license key")}
          <textarea
            aria-label={text("Replace license key")}
            autoComplete="off"
            placeholder="BRAWLER-LIC-1..."
            rows={4}
            value={licenseKeyDraft}
            onChange={(event) => onLicenseKeyDraftChange(event.target.value)}
          />
        </label>
        <div className="credential-actions">
          <Button
            disabled={licenseInFlight || !licenseKeyDraft.trim()}
            type="submit"
            variant="action"
          >
            {licenseInFlight ? <KeyRound size={14} /> : <Save size={14} />}
            {text("Save license")}
          </Button>
          <Button
            disabled={licenseInFlight}
            onClick={onClearLicenseKey}
            variant="ghost"
          >
            <Trash2 size={14} />
            {text("Clear license")}
          </Button>
        </div>
      </form>

      {licenseStatus?.reason ? <p className="settings-note">{licenseStatus.reason}</p> : null}
      {licenseError ? <p className="error-text">{licenseError}</p> : null}
    </section>
  );
}

function formatLicenseStatus(status: LicenseStatus | null) {
  if (!status) {
    return "checking";
  }

  return status.status.replace(/_/g, " ");
}
