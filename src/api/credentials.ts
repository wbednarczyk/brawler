import { callCommand } from "./tauri";
import type { CredentialStatus } from "./types";

/** Stable provider ids that authenticate with a single OS-keychain API key. */
export const GEMINI_PROVIDER_ID = "provider_gemini";
export const ANTHROPIC_PROVIDER_ID = "provider_anthropic";
export const OPENAI_PROVIDER_ID = "provider_openai";

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
