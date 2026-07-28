import { callCommand } from "./tauri";
import type { CredentialStatus } from "./types";

/**
 * The transcript provider's OS-keychain API key (ADR 0084 decision 7): since the
 * in-app analysis layer was retired, Gemini transcription is the only
 * credentialed provider left.
 */
const GEMINI_PROVIDER_ID = "provider_gemini";

function getProviderCredentialStatus(providerId: string) {
  return callCommand<CredentialStatus>("get_provider_credential_status", {
    input: { providerId },
  });
}

function setProviderApiKey(providerId: string, apiKey: string) {
  return callCommand<CredentialStatus>("set_provider_api_key", {
    input: { providerId, apiKey },
  });
}

function clearProviderApiKey(providerId: string) {
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
