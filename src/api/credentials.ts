import { callCommand } from "./tauri";
import type { CredentialStatus } from "./types";

/** Stable provider ids that authenticate with a single OS-keychain API key. */
export const GEMINI_PROVIDER_ID = "provider_gemini";
export const ANTHROPIC_PROVIDER_ID = "provider_anthropic";
export const OPENAI_PROVIDER_ID = "provider_openai";
export const OPENAI_COMPATIBLE_PROVIDER_ID = "provider_openai_compatible";
export const MISTRAL_PROVIDER_ID = "provider_mistral";

/**
 * Credentialed analysis providers shown in settings besides Gemini (which keeps
 * its own form). The set of providers is stable; model lists come from the
 * server catalog (ADR 0028).
 */
export const ADDITIONAL_CREDENTIAL_PROVIDERS: ReadonlyArray<{
  providerId: string;
  label: string;
}> = [
  { providerId: ANTHROPIC_PROVIDER_ID, label: "Claude (Anthropic)" },
  { providerId: OPENAI_PROVIDER_ID, label: "OpenAI (ChatGPT)" },
  { providerId: OPENAI_COMPATIBLE_PROVIDER_ID, label: "OpenAI-compatible (custom)" },
  { providerId: MISTRAL_PROVIDER_ID, label: "Mistral" },
];

export function getProviderCredentialStatus(providerId: string) {
  return callCommand<CredentialStatus>("get_provider_credential_status", {
    input: { providerId },
  });
}

export function setProviderApiKey(providerId: string, apiKey: string) {
  return callCommand<CredentialStatus>("set_provider_api_key", {
    input: { providerId, apiKey },
  });
}

export function clearProviderApiKey(providerId: string) {
  return callCommand<CredentialStatus>("clear_provider_api_key", {
    input: { providerId },
  });
}

// Convenience wrappers for the Gemini provider key (one key per provider serves
// both transcription and analysis usages).
export function getGeminiTranscriptionCredentialStatus() {
  return getProviderCredentialStatus(GEMINI_PROVIDER_ID);
}

export function setGeminiTranscriptionApiKey(apiKey: string) {
  return setProviderApiKey(GEMINI_PROVIDER_ID, apiKey);
}

export function clearGeminiTranscriptionApiKey() {
  return clearProviderApiKey(GEMINI_PROVIDER_ID);
}
