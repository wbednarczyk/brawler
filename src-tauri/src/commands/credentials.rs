use serde::Deserialize;

use crate::providers::credentials;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetGeminiTranscriptionApiKeyInput {
    api_key: String,
}

#[tauri::command]
pub fn get_gemini_transcription_credential_status() -> Result<credentials::CredentialStatus, String>
{
    Ok(credentials::get_gemini_transcription_credential_status())
}

#[tauri::command]
pub fn set_gemini_transcription_api_key(
    input: SetGeminiTranscriptionApiKeyInput,
) -> Result<credentials::CredentialStatus, String> {
    credentials::set_gemini_transcription_api_key(&input.api_key).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_gemini_transcription_api_key() -> Result<credentials::CredentialStatus, String> {
    credentials::clear_gemini_transcription_api_key().map_err(|error| error.to_string())
}
