use crate::{app_state, storage};

#[tauri::command]
pub fn list_notebook_entries(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::NotebookEntry>, String> {
    state
        .list_notebook_entries(&company_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_notebook_entry(
    input: storage::NewNotebookEntry,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::NotebookEntry, String> {
    state
        .create_notebook_entry(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_note_from_transcript_selection(
    input: storage::CreateNoteFromTranscriptSelectionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::NotebookEntry, String> {
    state
        .create_note_from_transcript_selection(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_notebook_entry(
    input: storage::NotebookEntryUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::NotebookEntry, String> {
    state
        .update_notebook_entry(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_notebook_entry(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_notebook_entry(&id)
        .map_err(|error| error.to_string())
}
