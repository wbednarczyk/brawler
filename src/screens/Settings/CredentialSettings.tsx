import type { FormEvent } from "react";
import { ExternalLink, Save, Trash2 } from "lucide-react";
import type { CredentialStatus } from "../../api/types";
import { ActionRow, Button, ErrorText, InfoGrid, TextField } from "../../ui";
import { useLocale } from "../../shared/locale";

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
  formatCredentialKind,
  onClearGeminiApiKey,
  onGeminiApiKeyDraftChange,
  onOpenGeminiApiKeyPage,
  onSaveGeminiApiKey,
}: CredentialSettingsProps) {
  const { t, text } = useLocale();

  return (
    <section className="settings-group" aria-labelledby="settings-credentials-title">
      <h2 id="settings-credentials-title">{t("settings.credentials.title")}</h2>
      <InfoGrid
        className="settings-grid"
        items={[
          {
            label: text("Credential status"),
            value: text(formatCredentialConfigured(geminiCredentialStatus)),
          },
          {
            label: text("Credential kind"),
            value: text(formatCredentialKind(geminiCredentialStatus?.secretKind)),
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
          <Button
            disabled={geminiCredentialInFlight || !geminiApiKeyDraft.trim()}
            type="submit"
            variant="action"
          >
            <Save size={14} />
            {t("settings.credentials.save")}
          </Button>
          <Button
            disabled={geminiCredentialInFlight}
            onClick={onClearGeminiApiKey}
            variant="ghost"
          >
            <Trash2 size={14} />
            {t("settings.credentials.clear")}
          </Button>
        </ActionRow>
      </form>
      <Button
        className="settings-link-button"
        onClick={onOpenGeminiApiKeyPage}
        title={text("Open Google AI Studio API keys page")}
        variant="ghost"
      >
        <ExternalLink size={14} />
        {t("settings.credentials.getGeminiKey")}
      </Button>
      {geminiCredentialStatus?.devFallbackAvailable ? (
        <p className="settings-note">
          {text("Development fallback is active through environment configuration.")}
        </p>
      ) : null}
      {geminiCredentialStatus?.error ? (
        <ErrorText>{text("Credential check failed")}: {geminiCredentialStatus.error}</ErrorText>
      ) : null}
      {geminiCredentialError ? (
        <ErrorText>{text("Credential command failed")}: {geminiCredentialError}</ErrorText>
      ) : null}
    </section>
  );
}
