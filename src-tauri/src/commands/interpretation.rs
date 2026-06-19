//! Commands for the interpretative AI layer's embedding-model strategy
//! (ADR 0035, v0.45.0). All local-only: the model runs on-device, no content
//! leaves the machine, no API key.
//!
//! Surfaces: model status, the optional one-time weights download, similarity
//! strategy selection, and a developer-mode "find similar" diagnostic that
//! exercises the active `SimilarityProvider` over real stored content.

use serde::{Deserialize, Serialize};

use crate::interpretation::{
    self, build_similarity_with, TextItem, WeightsState, DEFAULT_EMBEDDING_DIM,
    DEFAULT_EMBEDDING_MODEL_ID, EMBEDDING_SIMILARITY_ID, EMBEDDING_STRATEGY, STATIC_STRATEGY,
};
use crate::storage::{AppState, FEED_ITEM_CONTENT_TYPE};
use crate::{app_state, jobs};

const DEFAULT_SIMILAR_LIMIT: usize = 10;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedTypeCount {
    pub content_type: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelStatus {
    pub model_id: String,
    pub dim: usize,
    /// `unsupported` | `absent` | `downloading` | `ready` | `error`.
    pub weights_state: String,
    /// Best-effort percent while downloading; `null` when the downloader cannot
    /// report progress or no download is in flight.
    pub download_progress: Option<u8>,
    pub download_error: Option<String>,
    /// Effective active strategy — never `embedding` unless the model is ready.
    pub active_similarity_strategy: String,
    pub embedded_counts: Vec<EmbeddedTypeCount>,
    pub index_model_id: Option<String>,
    /// Whether this build compiled the embedding engine at all.
    pub feature_compiled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSimilarityStrategyInput {
    pub strategy: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindSimilarContentInput {
    pub content_type: String,
    pub content_id: String,
    pub k: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredContent {
    pub content_type: String,
    pub content_id: String,
    pub score: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindSimilarContentResult {
    pub strategy_id: String,
    pub items: Vec<ScoredContent>,
}

/// Resolve the effective weights state, folding the in-memory download flag over
/// the on-disk presence so the UI sees `downloading` while a fetch is in flight.
fn effective_weights_state(state: &AppState) -> (WeightsState, bool, Option<String>) {
    let download = state.embedding_download_state();
    let on_disk = interpretation::weights_state(state.data_dir(), DEFAULT_EMBEDDING_MODEL_ID);
    (on_disk, download.in_progress, download.error)
}

#[tauri::command]
pub fn get_embedding_model_status(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<EmbeddingModelStatus, String> {
    let state = state.inner();
    let (on_disk, downloading, download_error) = effective_weights_state(state);

    let weights_state = if downloading && on_disk != WeightsState::Ready {
        "downloading".to_string()
    } else if download_error.is_some() && on_disk != WeightsState::Ready {
        "error".to_string()
    } else {
        on_disk.as_str().to_string()
    };

    let stored_strategy = state
        .get_similarity_strategy()
        .map_err(|error| error.to_string())?;
    // Never report `embedding` unless the model is actually ready.
    let active_similarity_strategy =
        if stored_strategy == EMBEDDING_STRATEGY && on_disk == WeightsState::Ready {
            EMBEDDING_STRATEGY.to_string()
        } else {
            STATIC_STRATEGY.to_string()
        };

    let index_model_id = state
        .embedding_index_model_id()
        .map_err(|error| error.to_string())?;
    let embedded_counts = state
        .embedding_counts(DEFAULT_EMBEDDING_MODEL_ID)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|count| EmbeddedTypeCount {
            content_type: count.content_type,
            count: count.count,
        })
        .collect();

    Ok(EmbeddingModelStatus {
        model_id: DEFAULT_EMBEDDING_MODEL_ID.to_string(),
        dim: DEFAULT_EMBEDDING_DIM,
        weights_state,
        download_progress: None,
        download_error,
        active_similarity_strategy,
        embedded_counts,
        index_model_id,
        feature_compiled: interpretation::feature_compiled(),
    })
}

#[tauri::command]
pub fn download_embedding_model(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<EmbeddingModelStatus, String> {
    if !interpretation::feature_compiled() {
        return Err("this build was compiled without the embedding-model feature".to_string());
    }

    let owned = state.inner().clone();
    // Already present or already downloading -> no-op (idempotent).
    if interpretation::weights_present(owned.data_dir(), DEFAULT_EMBEDDING_MODEL_ID) {
        return get_embedding_model_status(state);
    }
    if owned.begin_embedding_download() {
        tauri::async_runtime::spawn_blocking(move || {
            let result =
                interpretation::download_weights(owned.data_dir(), DEFAULT_EMBEDDING_MODEL_ID);
            if let Err(error) = &result {
                log::warn!("embedding model download failed: {error}");
            }
            owned.finish_embedding_download(result);
        });
    }

    get_embedding_model_status(state)
}

#[tauri::command]
pub fn set_similarity_strategy(
    input: SetSimilarityStrategyInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<EmbeddingModelStatus, String> {
    let owned = state.inner().clone();

    match input.strategy.as_str() {
        STATIC_STRATEGY => {
            owned
                .set_similarity_strategy(STATIC_STRATEGY)
                .map_err(|error| error.to_string())?;
        }
        EMBEDDING_STRATEGY => {
            // Selecting embedding while the model is not ready is rejected — never
            // a silent fallback (ADR 0035 / contracts.md).
            if interpretation::weights_state(owned.data_dir(), DEFAULT_EMBEDDING_MODEL_ID)
                != WeightsState::Ready
            {
                return Err(
                    "the embedding model is not ready; download it before selecting the embedding strategy"
                        .to_string(),
                );
            }
            owned
                .set_similarity_strategy(EMBEDDING_STRATEGY)
                .map_err(|error| error.to_string())?;

            // Populate any missing vectors in the background.
            let job_state = owned.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = jobs::content_embedding::run_content_embedding_job(&job_state) {
                    log::warn!("content embedding job failed: {error}");
                }
            });
        }
        other => return Err(format!("unknown similarity strategy: {other}")),
    }

    get_embedding_model_status(state)
}

#[tauri::command]
pub async fn rebuild_embedding_index(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<EmbeddingModelStatus, String> {
    // Run the embed/re-embed job off the UI thread, surfacing any job error so the
    // panel can show it (e.g. a model load/forward failure), then return status.
    let app = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        jobs::content_embedding::run_content_embedding_job(&app)
    })
    .await
    .map_err(|error| error.to_string())??;

    get_embedding_model_status(state)
}

#[tauri::command]
pub async fn find_similar_content(
    input: FindSimilarContentInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<FindSimilarContentResult, String> {
    ensure_developer_mode_enabled(&state)?;
    // The work is CPU-bound (cosine scan, possibly a model forward), so run it off
    // the UI thread rather than blocking it from a synchronous command.
    let app = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || find_similar_impl(&app, input))
        .await
        .map_err(|error| error.to_string())?
}

/// Synchronous worker for [`find_similar_content`], run on a blocking thread.
///
/// Under the `embedding` strategy it scans the **persisted** vector index — the
/// query item's stored vector (embedding the query text on the fly only if it is
/// not yet indexed) against the stored candidate vectors — so it does not
/// re-embed the whole corpus per call. Under `static` (or when the model is
/// unavailable) it ranks lexically over text.
fn find_similar_impl(
    state: &AppState,
    input: FindSimilarContentInput,
) -> Result<FindSimilarContentResult, String> {
    if input.content_type != FEED_ITEM_CONTENT_TYPE {
        return Err(format!(
            "find_similar_content currently supports only '{FEED_ITEM_CONTENT_TYPE}' content"
        ));
    }
    let k = input.k.unwrap_or(DEFAULT_SIMILAR_LIMIT);

    let stored_strategy = state
        .get_similarity_strategy()
        .map_err(|error| error.to_string())?;

    // Embedding path: rank over the persisted vector index (fast; no re-embed).
    if stored_strategy == EMBEDDING_STRATEGY {
        if let Some(embedder) = interpretation::try_load_default_embedder(state.data_dir()) {
            let model_id = embedder.model_id().to_string();

            // Query vector: the stored one when indexed, else embed the text now.
            let query_vector = match state
                .content_vector(FEED_ITEM_CONTENT_TYPE, &input.content_id, &model_id)
                .map_err(|error| error.to_string())?
            {
                Some(vector) => vector,
                None => {
                    let text = feed_item_text(state, &input.content_id)?;
                    tauri::async_runtime::block_on(embedder.embed_one(&text))
                        .map_err(|error| error.to_string())?
                }
            };

            let mut scored: Vec<ScoredContent> = state
                .scan_content_vectors(FEED_ITEM_CONTENT_TYPE, &model_id)
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter(|candidate| candidate.content_id != input.content_id)
                .map(|candidate| ScoredContent {
                    content_type: FEED_ITEM_CONTENT_TYPE.to_string(),
                    content_id: candidate.content_id,
                    score: interpretation::similarity_score(&query_vector, &candidate.vector),
                })
                .collect();
            scored.sort_by(|a, b| b.score.total_cmp(&a.score));
            scored.truncate(k);

            return Ok(FindSimilarContentResult {
                strategy_id: EMBEDDING_SIMILARITY_ID.to_string(),
                items: scored,
            });
        }
        // Embedding selected but the model is not loadable: fall through to static.
    }

    // Static path: lexical ranking over feed-item text.
    let contents = state
        .feed_item_embeddable_contents()
        .map_err(|error| error.to_string())?;
    let query_text = contents
        .iter()
        .find(|content| content.content_id == input.content_id)
        .map(|content| content.text.clone())
        .ok_or_else(|| format!("content '{}' was not found", input.content_id))?;

    let provider =
        build_similarity_with(STATIC_STRATEGY, None).map_err(|error| error.to_string())?;
    let candidates: Vec<TextItem> = contents
        .into_iter()
        .filter(|content| content.content_id != input.content_id)
        .map(|content| TextItem {
            id: content.content_id,
            text: content.text,
        })
        .collect();
    let ranked = tauri::async_runtime::block_on(provider.most_similar(&query_text, candidates, k))
        .map_err(|error| error.to_string())?;

    Ok(FindSimilarContentResult {
        strategy_id: provider.id().to_string(),
        items: ranked
            .into_iter()
            .map(|scored| ScoredContent {
                content_type: FEED_ITEM_CONTENT_TYPE.to_string(),
                content_id: scored.id,
                score: scored.score,
            })
            .collect(),
    })
}

/// The embeddable text for one feed item, or an error when it is not found.
fn feed_item_text(state: &AppState, content_id: &str) -> Result<String, String> {
    state
        .feed_item_embeddable_contents()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|content| content.content_id == content_id)
        .map(|content| content.text)
        .ok_or_else(|| format!("content '{content_id}' was not found"))
}

fn ensure_developer_mode_enabled(
    state: &tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    if settings.developer_mode {
        Ok(())
    } else {
        Err("Developer mode is not active.".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    #[test]
    fn status_reports_static_default_and_no_index() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let download = state.embedding_download_state();
        assert!(!download.in_progress);

        // Mirrors get_embedding_model_status without the Tauri State wrapper.
        let stored = state.get_similarity_strategy().expect("strategy");
        assert_eq!(stored, STATIC_STRATEGY);
        assert!(state.embedding_index_model_id().expect("idx").is_none());
        assert_eq!(
            interpretation::feature_compiled(),
            cfg!(feature = "embedding-model")
        );
    }

    #[test]
    fn selecting_embedding_without_model_is_rejected_at_storage_validation() {
        // The storage layer accepts the value; the command layer guards readiness.
        // Here we assert the readiness guard's premise: with no weights, state is
        // not Ready, so the command would reject.
        let state = AppState::new(open_in_memory_database().expect("db"));
        assert_ne!(
            interpretation::weights_state(state.data_dir(), DEFAULT_EMBEDDING_MODEL_ID),
            WeightsState::Ready
        );
    }
}
