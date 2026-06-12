import type { FormEvent } from "react";
import { ExternalLink, Save, Trash2 } from "lucide-react";
import type { CredentialStatus } from "../../api/types";
import { Button } from "../../ui";
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
      <dl className="settings-grid">
        <div>
          <dt>{text("Credential status")}</dt>
          <dd>{text(formatCredentialConfigured(geminiCredentialStatus))}</dd>
        </div>
        <div>
          <dt>{text("Credential kind")}</dt>
          <dd>{text(formatCredentialKind(geminiCredentialStatus?.secretKind))}</dd>
        </div>
      </dl>
      <form className="credential-form" onSubmit={onSaveGeminiApiKey}>
        <label>
          {t("settings.credentials.geminiApiKey")}
          <input
            aria-label={t("settings.credentials.geminiApiKey")}
            autoComplete="off"
            placeholder={geminiCredentialStatus?.configured ? text("Replace configured key") : text("Paste API key")}
            type="password"
            value={geminiApiKeyDraft}
            onChange={(event) => onGeminiApiKeyDraftChange(event.target.value)}
          />
        </label>
        <div className="credential-actions">
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
        </div>
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
        <p className="error-text">{text("Credential check failed")}: {geminiCredentialStatus.error}</p>
      ) : null}
      {geminiCredentialError ? (
        <p className="error-text">{text("Credential command failed")}: {geminiCredentialError}</p>
      ) : null}
    </section>
  );
}
