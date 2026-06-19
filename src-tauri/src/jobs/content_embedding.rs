//! Embed + re-embed background job for the interpretative AI layer's vector
//! index (ADR 0035, v0.45.0).
//!
//! Runs when the `embedding` similarity strategy is active and the model is
//! loaded. It embeds feed items into `content_embeddings`, skipping content whose
//! text is unchanged (by `content_hash`), and rebuilds the index when the active
//! `model_id` changed (pruning stale-model vectors first so models are never
//! mixed). The index is disposable — a lost or partial run is never harmful and
//! reads tolerate partial population.

use sha2::{Digest, Sha256};
use tauri::async_runtime::block_on;

use crate::interpretation::{self, Embedder, EMBEDDING_STRATEGY};
use crate::storage::{AppState, EmbeddableContent, NewContentEmbedding, FEED_ITEM_CONTENT_TYPE};

/// Feed items embedded per model forward. Batching keeps a full index build to a
/// handful of forwards; capped so peak memory (batch × max-seq × hidden) stays modest.
const EMBED_BATCH_SIZE: usize = 32;

/// Why a run did the work it did — surfaced in diagnostics/status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedJobStatus {
    /// Embedded/refreshed vectors for the active model.
    Embedded,
    /// The active strategy is not `embedding`; nothing to do.
    SkippedStrategyStatic,
    /// The strategy is `embedding` but no model is loaded (feature off or weights
    /// absent); the static baseline serves similarity meanwhile.
    SkippedModelUnavailable,
}

/// Outcome of one embed run.
#[derive(Debug, Clone)]
pub struct EmbedJobOutcome {
    pub status: EmbedJobStatus,
    pub model_id: Option<String>,
    pub embedded: usize,
    pub skipped: usize,
    pub pruned: usize,
}

impl EmbedJobOutcome {
    fn skipped(status: EmbedJobStatus) -> Self {
        Self {
            status,
            model_id: None,
            embedded: 0,
            skipped: 0,
            pruned: 0,
        }
    }
}

fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Run the embed/re-embed job. Returns a structured outcome; storage and model
/// errors are surfaced as a recoverable string for the caller to log.
pub fn run_content_embedding_job(state: &AppState) -> Result<EmbedJobOutcome, String> {
    let strategy = state
        .get_similarity_strategy()
        .map_err(|error| error.to_string())?;
    if strategy != EMBEDDING_STRATEGY {
        return Ok(EmbedJobOutcome::skipped(
            EmbedJobStatus::SkippedStrategyStatic,
        ));
    }

    let embedder = match interpretation::try_load_default_embedder(state.data_dir()) {
        Some(embedder) => embedder,
        None => {
            return Ok(EmbedJobOutcome::skipped(
                EmbedJobStatus::SkippedModelUnavailable,
            ))
        }
    };

    let contents = state
        .feed_item_embeddable_contents()
        .map_err(|error| error.to_string())?;
    embed_corpus(state, embedder.as_ref(), contents)
}

/// Embed `contents` with `embedder` into the vector store: rebuild when the model
/// changed (prune other-model vectors), skip content whose hash is unchanged, and
/// upsert the rest in batches. This is the deterministic core of the job, free of
/// strategy/model resolution, so it is unit-testable with any [`Embedder`].
pub(crate) fn embed_corpus(
    state: &AppState,
    embedder: &dyn Embedder,
    contents: Vec<EmbeddableContent>,
) -> Result<EmbedJobOutcome, String> {
    let model_id = embedder.model_id().to_string();

    // Rebuild when the active model changed: drop vectors from any other model so
    // a scan never mixes model_ids.
    let index_model = state
        .embedding_index_model_id()
        .map_err(|error| error.to_string())?;
    let pruned = if index_model.as_deref() != Some(model_id.as_str()) {
        state
            .prune_embeddings_other_models(&model_id)
            .map_err(|error| error.to_string())?
    } else {
        0
    };

    let existing = state
        .embedded_content_hashes(FEED_ITEM_CONTENT_TYPE, &model_id)
        .map_err(|error| error.to_string())?;

    // Select the content that actually needs (re-)embedding, skipping unchanged.
    let mut pending: Vec<(String, String, String)> = Vec::new(); // (id, text, hash)
    let mut skipped = 0;
    for content in contents {
        if content.text.trim().is_empty() {
            continue;
        }
        let hash = content_hash(&content.text);
        if existing.get(&content.content_id) == Some(&hash) {
            skipped += 1;
            continue;
        }
        pending.push((content.content_id, content.text, hash));
    }

    // Embed in batches (one model forward per batch) rather than one-per-item, so
    // a full index build is a handful of forwards instead of hundreds.
    let mut embedded = 0;
    for chunk in pending.chunks(EMBED_BATCH_SIZE) {
        let texts: Vec<String> = chunk.iter().map(|(_, text, _)| text.clone()).collect();
        let vectors = block_on(embedder.embed(&texts)).map_err(|error| error.to_string())?;
        if vectors.len() != chunk.len() {
            return Err(format!(
                "embedder returned {} vectors for {} inputs",
                vectors.len(),
                chunk.len()
            ));
        }
        for ((content_id, _text, hash), vector) in chunk.iter().zip(vectors) {
            state
                .upsert_content_embedding(&NewContentEmbedding {
                    content_type: FEED_ITEM_CONTENT_TYPE.to_string(),
                    content_id: content_id.clone(),
                    model_id: model_id.clone(),
                    dim: vector.len(),
                    vector,
                    content_hash: hash.clone(),
                })
                .map_err(|error| error.to_string())?;
            embedded += 1;
        }
    }

    Ok(EmbedJobOutcome {
        status: EmbedJobStatus::Embedded,
        model_id: Some(model_id),
        embedded,
        skipped,
        pruned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::HashingEmbedder;
    use crate::storage::AppState;

    fn content(id: &str, text: &str) -> EmbeddableContent {
        EmbeddableContent {
            content_id: id.to_string(),
            text: text.to_string(),
        }
    }

    fn embedder(model_id: &str) -> HashingEmbedder {
        HashingEmbedder::new(model_id, 16)
    }

    #[test]
    fn embed_corpus_populates_the_index() {
        let state = AppState::new(crate::storage::open_in_memory_database().expect("db"));
        let contents = vec![
            content("feed_01", "zarząd rekomenduje dywidendę"),
            content("feed_02", "raport okresowy za trzeci kwartał"),
        ];
        let outcome = embed_corpus(&state, &embedder("m1"), contents).expect("embed");

        assert_eq!(outcome.status, EmbedJobStatus::Embedded);
        assert_eq!(outcome.embedded, 2);
        assert_eq!(outcome.skipped, 0);
        assert_eq!(
            state.embedding_index_model_id().expect("idx"),
            Some("m1".to_owned())
        );
        let vectors = state
            .scan_content_vectors(FEED_ITEM_CONTENT_TYPE, "m1")
            .expect("scan");
        assert_eq!(vectors.len(), 2);
    }

    #[test]
    fn embed_corpus_skips_unchanged_and_reembeds_changed() {
        let state = AppState::new(crate::storage::open_in_memory_database().expect("db"));
        let first = vec![content("feed_01", "alfa"), content("feed_02", "beta")];
        embed_corpus(&state, &embedder("m1"), first).expect("first");

        // Re-run with one item changed: the unchanged one is skipped by hash.
        let second = vec![
            content("feed_01", "alfa"),
            content("feed_02", "beta zmienione"),
        ];
        let outcome = embed_corpus(&state, &embedder("m1"), second).expect("second");
        assert_eq!(outcome.embedded, 1);
        assert_eq!(outcome.skipped, 1);
    }

    #[test]
    fn embed_corpus_prunes_and_rebuilds_on_model_change() {
        let state = AppState::new(crate::storage::open_in_memory_database().expect("db"));
        embed_corpus(&state, &embedder("old"), vec![content("feed_01", "alfa")]).expect("old");

        let outcome =
            embed_corpus(&state, &embedder("new"), vec![content("feed_01", "alfa")]).expect("new");
        assert_eq!(outcome.pruned, 1);
        assert_eq!(
            state.embedding_index_model_id().expect("idx"),
            Some("new".to_owned())
        );
        // No stale-model vectors remain.
        assert!(state
            .scan_content_vectors(FEED_ITEM_CONTENT_TYPE, "old")
            .expect("scan old")
            .is_empty());
    }

    #[test]
    fn embed_corpus_handles_batches_and_skips_blank() {
        let state = AppState::new(crate::storage::open_in_memory_database().expect("db"));
        let mut contents: Vec<EmbeddableContent> = (0..(EMBED_BATCH_SIZE * 2 + 5))
            .map(|i| content(&format!("feed_{i:03}"), &format!("treść numer {i}")))
            .collect();
        contents.push(content("blank", "   "));

        let outcome = embed_corpus(&state, &embedder("m1"), contents).expect("embed");
        assert_eq!(outcome.embedded, EMBED_BATCH_SIZE * 2 + 5);
        let vectors = state
            .scan_content_vectors(FEED_ITEM_CONTENT_TYPE, "m1")
            .expect("scan");
        assert_eq!(vectors.len(), EMBED_BATCH_SIZE * 2 + 5);
    }

    #[test]
    fn skips_when_strategy_is_static() {
        let state = AppState::new(crate::storage::open_in_memory_database().expect("db"));
        let outcome = run_content_embedding_job(&state).expect("run");
        assert_eq!(outcome.status, EmbedJobStatus::SkippedStrategyStatic);
        assert_eq!(outcome.embedded, 0);
    }

    #[test]
    fn skips_when_embedding_selected_but_no_model() {
        let state = AppState::new(crate::storage::open_in_memory_database().expect("db"));
        state
            .set_similarity_strategy(EMBEDDING_STRATEGY)
            .expect("select embedding");
        let outcome = run_content_embedding_job(&state).expect("run");
        // No weights present in the test data dir, so the model never loads.
        assert_eq!(outcome.status, EmbedJobStatus::SkippedModelUnavailable);
    }

    #[test]
    fn content_hash_is_stable_and_text_sensitive() {
        assert_eq!(content_hash("alfa"), content_hash("alfa"));
        assert_ne!(content_hash("alfa"), content_hash("beta"));
    }
}
