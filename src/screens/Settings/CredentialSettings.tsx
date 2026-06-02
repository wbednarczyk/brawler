import type { FormEvent } from "react";
import { ExternalLink, Save, Trash2 } from "lucide-react";
import type { CredentialStatus } from "../../api/types";
import { Button } from "../../shared/components/Button";

type CredentialSettingsProps = {
  geminiApiKeyDraft: string;
  geminiCredentialError: string | null;
  geminiCredentialInFlight: boolean;
  geminiCredentialStatus: CredentialStatus | null;
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
  onClearGeminiApiKey,
  onGeminiApiKeyDraftChange,
  onOpenGeminiApiKeyPage,
  onSaveGeminiApiKey,
}: CredentialSettingsProps) {
  return (
    <section className="settings-group" aria-labelledby="settings-credentials-title">
      <h2 id="settings-credentials-title">Credentials</h2>
      <form className="credential-form" onSubmit={onSaveGeminiApiKey}>
        <label>
          Gemini API key
          <input
            aria-label="Gemini API key"
            autoComplete="off"
            placeholder={geminiCredentialStatus?.configured ? "Replace configured key" : "Paste API key"}
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
            Save
          </Button>
          <Button
            disabled={geminiCredentialInFlight}
            onClick={onClearGeminiApiKey}
            variant="ghost"
          >
            <Trash2 size={14} />
            Clear
          </Button>
        </div>
      </form>
      <Button
        className="settings-link-button"
        onClick={onOpenGeminiApiKeyPage}
        title="Open Google AI Studio API keys page"
        variant="ghost"
      >
        <ExternalLink size={14} />
        Get Gemini API key
      </Button>
      {geminiCredentialStatus?.devFallbackAvailable ? (
        <p className="settings-note">
          Development fallback is active through environment configuration.
        </p>
      ) : null}
      {geminiCredentialStatus?.error ? (
        <p className="error-text">Credential backend: {geminiCredentialStatus.error}</p>
      ) : null}
      {geminiCredentialError ? (
        <p className="error-text">Credential command failed: {geminiCredentialError}</p>
      ) : null}
    </section>
  );
}
