//! The `Embedder` engine boundary (ADR 0035, section 4).
//!
//! This is the lower-level swap point *beneath* the model-backed capabilities:
//! a capability binds to [`Embedder`], never to a concrete model. Changing the
//! encoder (or the engine behind it) is constructing a different `Embedder`; no
//! capability or consumer changes. Every embedding carries its [`Embedder::model_id`]
//! and [`Embedder::dim`] so vectors from different models are never mixed
//! (enforced at the vector store).
//!
//! The shipped implementation is a pure-Rust on-device encoder
//! ([`crate::interpretation::candle_embedder`], behind the `embedding-model`
//! feature). The trait stays engine-neutral so a remote embedding API or an
//! alternate local engine could be added later behind it.

use async_trait::async_trait;

use super::InterpretationError;

/// The default on-device encoder (ADR 0035 amendment): a pure-Rust `candle`
/// model, multilingual with strong Polish coverage, desktop-friendly size. The
/// final encoder is confirmed by the per-capability eval (ADR 0035 section 7);
/// this constant is the single place to change it.
pub const DEFAULT_EMBEDDING_MODEL_ID: &str = "intfloat/multilingual-e5-small";

/// Output dimensionality of [`DEFAULT_EMBEDDING_MODEL_ID`].
pub const DEFAULT_EMBEDDING_DIM: usize = 384;

/// Turn a model id into a filesystem-safe directory name (no path separators).
pub fn model_dir_name(model_id: &str) -> String {
    model_id.replace(['/', '\\'], "__")
}

/// Maps text to vectors. Implementations are interchangeable behind this trait;
/// each reports its `model_id` and `dim` so callers can detect a model change
/// and rebuild the disposable index.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Stable identifier of the model that produces these vectors
    /// (e.g. `intfloat/multilingual-e5-small`).
    fn model_id(&self) -> &str;

    /// Output dimensionality of every produced vector.
    fn dim(&self) -> usize;

    /// Embed a batch of texts, returning one `dim`-length vector per input in
    /// order. An empty input slice yields an empty result.
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InterpretationError>;

    /// Embed a single text (convenience over [`Embedder::embed`]).
    async fn embed_one(&self, text: &str) -> Result<Vec<f32>, InterpretationError> {
        let mut vectors = self.embed(std::slice::from_ref(&text.to_string())).await?;
        vectors.pop().ok_or_else(|| {
            InterpretationError::Backend("embedder returned no vector for one input".to_string())
        })
    }
}
