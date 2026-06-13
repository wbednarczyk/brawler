import { useEffect, useState, type FormEvent } from "react";
import { ExternalLink, Save, Trash2 } from "lucide-react";
import { listAiProviderCatalog } from "../../api/aiProviders";
import { GEMINI_PROVIDER_ID } from "../../api/credentials";
import type { AiProviderCatalogEntry, CredentialStatus } from "../../api/types";
import { ActionRow, Button, InfoGrid } from "../../ui";
import { useLocale } from "../../shared/locale";
import { ProviderApiKeyForm } from "./ProviderApiKeyForm";

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
  const [catalog, setCatalog] = useState<AiProviderCatalogEntry[]>([]);

  useEffect(() => {
    let active = true;
    listAiProviderCatalog()
      .then((entries) => {
        if (active) {
          setCatalog(entries);
        }
      })
      .catch(() => {
        if (active) {
          setCatalog([]);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  // The Gemini key keeps its dedicated form (above); other credentialed
  // providers get a self-contained per-provider form (ADR 0028).
  const additionalProviders = catalog.filter(
    (entry) => entry.requiresCredential && entry.providerId !== GEMINI_PROVIDER_ID,
  );

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
        <p className="error-text">{text("Credential check failed")}: {geminiCredentialStatus.error}</p>
      ) : null}
      {geminiCredentialError ? (
        <p className="error-text">{text("Credential command failed")}: {geminiCredentialError}</p>
      ) : null}
      {additionalProviders.map((entry) => (
        <ProviderApiKeyForm
          key={entry.providerId}
          providerId={entry.providerId}
          label={entry.label}
          formatCredentialConfigured={formatCredentialConfigured}
        />
      ))}
    </section>
  );
}
