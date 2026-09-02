import type { FormEvent } from "react";
import { KeyRound, Save, Trash2 } from "lucide-react";
import type { LicenseStatus } from "../../api/types";
import { ActionButton, ActionRow, ErrorText, InfoGrid, TextareaField } from "../../ui";
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

      <InfoGrid
        className="settings-grid"
        items={[
          { label: text("Status"), value: text(formatLicenseStatus(licenseStatus)) },
          { label: text("Holder"), value: license?.holder ?? text("Not available") },
          { label: text("Channel"), value: license?.channel ?? text("Not available") },
          { label: text("Expires"), value: license?.expiresAt ?? text("Not available") },
        ]}
      />

      <form className="credential-form" onSubmit={onSubmitLicenseKey}>
        <TextareaField
          label={text("Replace license key")}
          aria-label={text("Replace license key")}
          autoComplete="off"
          placeholder="BRAWLER-LIC-1..."
          rows={4}
          value={licenseKeyDraft}
          onChange={(event) => onLicenseKeyDraftChange(event.target.value)}
        />
        <ActionRow className="credential-actions">
          <ActionButton
            verb="save"
            disabled={licenseInFlight || !licenseKeyDraft.trim()}
            type="submit"
            variant="primary"
            data-ux-primary-action="true"
          >
            {licenseInFlight ? <KeyRound size={14} /> : <Save size={14} />}
            {text("Save license")}
          </ActionButton>
          <ActionButton
            verb="remove"
            disabled={licenseInFlight}
            onClick={onClearLicenseKey}
            variant="ghost"
          >
            <Trash2 size={14} />
            {text("Clear license")}
          </ActionButton>
        </ActionRow>
      </form>

      {/* The backend's reason is one of a few fixed sentences (licensing/mod.rs) — translated by literal lookup, never rendered raw. */}
      {licenseStatus?.reason ? <p className="settings-note">{text(licenseStatus.reason)}</p> : null}
      {licenseError ? <ErrorText>{licenseError}</ErrorText> : null}
    </section>
  );
}

function formatLicenseStatus(status: LicenseStatus | null) {
  if (!status) {
    return "checking";
  }

  return status.status.replace(/_/g, " ");
}
