import { useEffect, useState, type FormEvent } from "react";
import { Save, Trash2 } from "lucide-react";

import {
  clearProviderApiKey,
  getProviderCredentialStatus,
  setProviderApiKey,
} from "../../api/credentials";
import type { CredentialStatus } from "../../api/types";
import { ActionRow, Button, InfoGrid, TextField } from "../../ui";
import { useLocale } from "../../shared/locale";

type ProviderApiKeyFormProps = {
  providerId: string;
  label: string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
};

/// Self-contained API-key form for one AI provider. Manages its own credential
/// status/draft via the generic provider credential commands (ADR 0028), so
/// adding providers needs no controller wiring.
export function ProviderApiKeyForm({
  providerId,
  label,
  formatCredentialConfigured,
}: ProviderApiKeyFormProps) {
  const { text } = useLocale();
  const [status, setStatus] = useState<CredentialStatus | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [inFlight, setInFlight] = useState(false);

  useEffect(() => {
    let active = true;
    getProviderCredentialStatus(providerId)
      .then((value) => {
        if (active) {
          setStatus(value);
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [providerId]);

  function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const apiKey = draft.trim();
    if (!apiKey) {
      setError(text("API key is required."));
      return;
    }
    setInFlight(true);
    setProviderApiKey(providerId, apiKey)
      .then((value) => {
        setStatus(value);
        setError(null);
        setDraft("");
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setInFlight(false));
  }

  function clear() {
    setInFlight(true);
    clearProviderApiKey(providerId)
      .then((value) => {
        setStatus(value);
        setError(null);
        setDraft("");
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setInFlight(false));
  }

  const keyLabel = `${label} ${text("API key")}`;

  return (
    <div className="provider-credential">
      <h3>{label}</h3>
      <InfoGrid
        className="settings-grid"
        items={[
          {
            label: text("Credential status"),
            value: text(formatCredentialConfigured(status)),
          },
        ]}
      />
      <form className="credential-form" onSubmit={save}>
        <TextField
          label={keyLabel}
          aria-label={keyLabel}
          autoComplete="off"
          placeholder={status?.configured ? text("Replace configured key") : text("Paste API key")}
          type="password"
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
        />
        <ActionRow className="credential-actions">
          <Button
            aria-label={`${text("Save")} ${keyLabel}`}
            disabled={inFlight || !draft.trim()}
            type="submit"
            variant="action"
          >
            <Save size={14} />
            {text("Save")}
          </Button>
          <Button
            aria-label={`${text("Clear")} ${keyLabel}`}
            disabled={inFlight}
            onClick={clear}
            variant="ghost"
          >
            <Trash2 size={14} />
            {text("Clear")}
          </Button>
        </ActionRow>
      </form>
      {status?.devFallbackAvailable ? (
        <p className="settings-note">
          {text("Development fallback is active through environment configuration.")}
        </p>
      ) : null}
      {error ? (
        <p className="error-text">
          {text("Credential command failed")}: {error}
        </p>
      ) : null}
    </div>
  );
}
