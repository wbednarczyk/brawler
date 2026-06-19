//! Embedding-model lifecycle: where weights live on disk, their readiness, the
//! optional one-time download, and loading the [`Embedder`] (ADR 0035).
//!
//! Weights are an optional download (not bundled): the capability serves the
//! static baseline until they are present. Everything that needs `candle` is
//! behind the `embedding-model` feature; with the feature off this module still
//! answers status queries (reporting the engine as not compiled in) so the
//! command/UI surface works in every build.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::embedder::{model_dir_name, Embedder, DEFAULT_EMBEDDING_MODEL_ID};

/// Files an embedding model needs on disk to be loadable.
const REQUIRED_FILES: &[&str] = &["config.json", "tokenizer.json", "model.safetensors"];

/// Readiness of the on-device model weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightsState {
    /// This build was compiled without the `embedding-model` feature; the engine
    /// cannot run regardless of weights.
    Unsupported,
    /// The feature is compiled in but the weights are not present on disk.
    Absent,
    /// Weights are present and loadable.
    Ready,
}

impl WeightsState {
    /// Stable string for the contract (`get_embedding_model_status`).
    pub fn as_str(self) -> &'static str {
        match self {
            WeightsState::Unsupported => "unsupported",
            WeightsState::Absent => "absent",
            WeightsState::Ready => "ready",
        }
    }
}

/// Whether this build can run the embedding model at all.
pub const fn feature_compiled() -> bool {
    cfg!(feature = "embedding-model")
}

/// Directory holding the weights for `model_id` under the app data dir.
pub fn model_dir(data_dir: &Path, model_id: &str) -> PathBuf {
    data_dir.join("models").join(model_dir_name(model_id))
}

/// Whether every required weight file is present for `model_id`.
pub fn weights_present(data_dir: &Path, model_id: &str) -> bool {
    let dir = model_dir(data_dir, model_id);
    REQUIRED_FILES.iter().all(|file| dir.join(file).exists())
}

/// The current weights state for `model_id`, accounting for feature compilation.
pub fn weights_state(data_dir: &Path, model_id: &str) -> WeightsState {
    if !feature_compiled() {
        WeightsState::Unsupported
    } else if weights_present(data_dir, model_id) {
        WeightsState::Ready
    } else {
        WeightsState::Absent
    }
}

/// Load the default embedder if the feature is compiled in and weights are
/// present; otherwise `None` (caller falls back to the static baseline).
pub fn try_load_default_embedder(data_dir: &Path) -> Option<Arc<dyn Embedder>> {
    try_load_embedder(data_dir, DEFAULT_EMBEDDING_MODEL_ID)
}

/// Load a specific embedder, or `None` when unsupported/absent/unloadable.
pub fn try_load_embedder(data_dir: &Path, model_id: &str) -> Option<Arc<dyn Embedder>> {
    #[cfg(feature = "embedding-model")]
    {
        if !weights_present(data_dir, model_id) {
            return None;
        }
        match super::candle_embedder::CandleEmbedder::load(
            &model_dir(data_dir, model_id),
            model_id,
            super::embedder::DEFAULT_EMBEDDING_DIM,
        ) {
            Ok(embedder) => Some(Arc::new(embedder) as Arc<dyn Embedder>),
            Err(error) => {
                log::warn!("failed to load embedding model {model_id}: {error}");
                None
            }
        }
    }
    #[cfg(not(feature = "embedding-model"))]
    {
        let _ = (data_dir, model_id);
        None
    }
}

/// Download the weights for `model_id` into the app data dir (one-time, opt-in).
/// Blocking; callers run it off the UI thread. Errors are recoverable strings.
///
/// Fetches each required file from the Hugging Face `resolve/main` endpoint with
/// the project's existing `reqwest` (native-tls), avoiding `hf-hub`'s
/// ureq+rustls+ring stack which does not cross-compile to windows-msvc. Each file
/// is written to a `.part` path and renamed on success so an interrupted download
/// never leaves a truncated file that looks "ready".
#[cfg(feature = "embedding-model")]
pub fn download_weights(data_dir: &Path, model_id: &str) -> Result<(), String> {
    let target_dir = model_dir(data_dir, model_id);
    std::fs::create_dir_all(&target_dir)
        .map_err(|error| format!("creating model directory: {error}"))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .build()
        .map_err(|error| format!("building http client: {error}"))?;

    for file in REQUIRED_FILES {
        let url = format!("https://huggingface.co/{model_id}/resolve/main/{file}");
        let response = client
            .get(&url)
            .send()
            .map_err(|error| format!("downloading {file}: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("downloading {file}: HTTP {}", response.status()));
        }
        let bytes = response
            .bytes()
            .map_err(|error| format!("reading {file}: {error}"))?;

        let final_path = target_dir.join(file);
        let part_path = target_dir.join(format!("{file}.part"));
        std::fs::write(&part_path, &bytes).map_err(|error| format!("saving {file}: {error}"))?;
        std::fs::rename(&part_path, &final_path)
            .map_err(|error| format!("finalizing {file}: {error}"))?;
    }

    Ok(())
}

/// Without the feature, downloading is unavailable; report it as a recoverable
/// error rather than silently doing nothing.
#[cfg(not(feature = "embedding-model"))]
pub fn download_weights(_data_dir: &Path, _model_id: &str) -> Result<(), String> {
    Err("this build was compiled without the embedding-model feature".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_dir_is_filesystem_safe() {
        let dir = model_dir(Path::new("/data"), "intfloat/multilingual-e5-small");
        assert_eq!(
            dir,
            Path::new("/data/models/intfloat__multilingual-e5-small")
        );
    }

    #[test]
    fn weights_absent_on_empty_dir() {
        let temp = std::env::temp_dir().join("brawler-embed-test-absent");
        let _ = std::fs::remove_dir_all(&temp);
        assert!(!weights_present(&temp, DEFAULT_EMBEDDING_MODEL_ID));
    }

    #[test]
    fn state_reflects_feature_compilation() {
        let temp = std::env::temp_dir().join("brawler-embed-test-state");
        let _ = std::fs::remove_dir_all(&temp);
        let state = weights_state(&temp, DEFAULT_EMBEDDING_MODEL_ID);
        if feature_compiled() {
            assert_eq!(state, WeightsState::Absent);
        } else {
            assert_eq!(state, WeightsState::Unsupported);
        }
    }
}
