import { callCommand } from "./tauri";
import type { CredentialStatus } from "./types";

export function getGeminiTranscriptionCredentialStatus() {
  return callCommand<CredentialStatus>("get_gemini_transcription_credential_status");
}

export function setGeminiTranscriptionApiKey(apiKey: string) {
  return callCommand<CredentialStatus>("set_gemini_transcription_api_key", {
    input: {
      apiKey,
    },
  });
}

export function clearGeminiTranscriptionApiKey() {
  return callCommand<CredentialStatus>("clear_gemini_transcription_api_key");
}
