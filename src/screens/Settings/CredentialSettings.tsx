import type { FormEvent } from "react";
import { ExternalLink, Save, Trash2 } from "lucide-react";
import type { CredentialStatus } from "../../api/types";
import { ActionButton, ActionRow, ErrorText, InfoGrid, TextField } from "../../ui";
import { useLocale } from "../../shared/locale";
import { useDeveloperMode } from "../../app/state/SettingsContext";
import { formatCredentialStorage } from "../../shared/formatting/labels";

type CredentialSettingsProps = {
  geminiApiKeyDraft: string;
  geminiCredentialError: string | null;
  geminiCredentialInFlight: boolean;
  geminiCredentialStatus: CredentialStatus | null;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
  formatCredentialKind: (value: string | null | undefined) => string;
  onClearGeminiApiKey: () => void;
  onGeminiApiKeyDraftChange: (apiKey: string) => void;
  onOpenGeminiApiKeyPage: () => void;
  onSaveGeminiApiKey: (event: FormEvent<HTMLFormElement>) => void;
};

export function CredentialSettings({
  geminiApiKeyDraft,
  geminiCredentialError,
  geminiCredentialInFlight,
  geminiCredentialStatus,
  formatCredentialConfigured,
  onClearGeminiApiKey,
  onGeminiApiKeyDraftChange,
  onOpenGeminiApiKeyPage,
  onSaveGeminiApiKey,
}: CredentialSettingsProps) {
  const { t, text } = useLocale();
  const developerMode = useDeveloperMode();

  return (
    <section className="settings-group" aria-labelledby="settings-credentials-title">
      <h2 id="settings-credentials-title">{t("settings.credentials.title")}</h2>
      <InfoGrid
        className="settings-grid"
        items={[
          {
            label: text("API key"),
            value: text(formatCredentialConfigured(geminiCredentialStatus)),
          },
          {
            label: text("Stored in"),
            value: text(formatCredentialStorage(geminiCredentialStatus?.storage)),
          },
        ]}
      />
      <form className="credential-form" onSubmit={onSaveGeminiApiKey}>
        <TextField
          label={t("settings.credentials.geminiApiKey")}
          aria-label={t("settings.credentials.geminiApiKey")}
          autoComplete="off"
          placeholder={geminiCredentialStatus?.configured ? text("Replace configured key") : text("Paste API key")}
          type="password"
          value={geminiApiKeyDraft}
          onChange={(event) => onGeminiApiKeyDraftChange(event.target.value)}
        />
        <ActionRow className="credential-actions">
          <ActionButton
            verb="save"
            disabled={geminiCredentialInFlight || !geminiApiKeyDraft.trim()}
            type="submit"
            variant="primary"
            data-ux-primary-action="true"
          >
            <Save size={14} />
            {t("settings.credentials.save")}
          </ActionButton>
          <ActionButton
            verb="remove"
            disabled={geminiCredentialInFlight}
            onClick={onClearGeminiApiKey}
            variant="ghost"
          >
            <Trash2 size={14} />
            {t("settings.credentials.clear")}
          </ActionButton>
        </ActionRow>
      </form>
      <ActionButton
        verb="open"
        className="settings-link-button"
        onClick={onOpenGeminiApiKeyPage}
        title={text("Open Google AI Studio API keys page")}
        variant="ghost"
      >
        <ExternalLink size={14} />
        {t("settings.credentials.getGeminiKey")}
      </ActionButton>
      {developerMode && geminiCredentialStatus?.devFallbackAvailable ? (
        <p className="settings-note">
          {text("Development fallback is active through environment configuration.")}
        </p>
      ) : null}
      {geminiCredentialStatus?.error ? (
        <ErrorText>{text("Couldn't read the saved key")}: {geminiCredentialStatus.error}</ErrorText>
      ) : null}
      {geminiCredentialError ? (
        <ErrorText>{text("Couldn't save that setting")}: {geminiCredentialError}</ErrorText>
      ) : null}
    </section>
  );
}
