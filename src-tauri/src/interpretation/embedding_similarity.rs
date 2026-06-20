//! Embedding-backed [`SimilarityProvider`] — the model implementation of the
//! similarity capability (ADR 0035, section 3). It embeds the query and the
//! candidates with an [`Embedder`] and ranks them by cosine. It is layered
//! behind the *same* `SimilarityProvider` trait as the lexical baseline, so the
//! registry can switch between them with no consumer change; its first model
//! consumer is story clustering (v0.46.0).
//!
//! This implementation embeds text on demand. Consumers that already maintain
//! the persisted vector index (the `content_embeddings` store) can scan it
//! directly via [`crate::interpretation::vector`] to avoid re-embedding — both
//! paths share the same cosine.

use std::sync::Arc;

use async_trait::async_trait;

use super::vector::similarity_score;
use super::vector_index::{BruteForceVectorIndex, VectorIndex};
use super::{Embedder, InterpretationError, ScoredItem, SimilarityProvider, TextItem};

/// Stable id of the embedding similarity implementation.
pub const EMBEDDING_SIMILARITY_ID: &str = "similarity_embedding";

/// A similarity provider that ranks by cosine over on-device embeddings.
pub struct EmbeddingSimilarity {
    id: String,
    embedder: Arc<dyn Embedder>,
    /// The nearest-neighbour ranking strategy (ADR 0050 swap boundary). Defaults
    /// to the exhaustive cosine scan; an ANN index swaps in here.
    index: Arc<dyn VectorIndex>,
}

impl EmbeddingSimilarity {
    /// Build the embedding similarity provider over the given embedder, ranking
    /// with the default brute-force cosine index.
    pub fn new(embedder: Arc<dyn Embedder>) -> Self {
        Self::with_index(embedder, Arc::new(BruteForceVectorIndex))
    }

    /// Build the provider with an explicit [`VectorIndex`] — the seam an ANN
    /// index plugs into without changing any `SimilarityProvider` consumer.
    pub fn with_index(embedder: Arc<dyn Embedder>, index: Arc<dyn VectorIndex>) -> Self {
        Self {
            id: EMBEDDING_SIMILARITY_ID.to_string(),
            embedder,
            index,
        }
    }
}

#[async_trait]
impl SimilarityProvider for EmbeddingSimilarity {
    fn id(&self) -> &str {
        &self.id
    }

    async fn score(&self, a: &str, b: &str) -> Result<f32, InterpretationError> {
        let vectors = self.embedder.embed(&[a.to_string(), b.to_string()]).await?;
        if vectors.len() != 2 {
            return Err(InterpretationError::Backend(
                "embedder returned the wrong number of vectors".to_string(),
            ));
        }
        Ok(similarity_score(&vectors[0], &vectors[1]))
    }

    async fn most_similar(
        &self,
        query: &str,
        candidates: Vec<TextItem>,
        k: usize,
    ) -> Result<Vec<ScoredItem>, InterpretationError> {
        if query.trim().is_empty() {
            return Err(InterpretationError::InvalidRequest(
                "similarity query is empty".to_string(),
            ));
        }
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // One batch: the query first, then every candidate, preserving order.
        let mut texts = Vec::with_capacity(candidates.len() + 1);
        texts.push(query.to_string());
        for candidate in &candidates {
            texts.push(candidate.text.clone());
        }

        let vectors = self.embedder.embed(&texts).await?;
        if vectors.len() != candidates.len() + 1 {
            return Err(InterpretationError::Backend(
                "embedder returned the wrong number of vectors".to_string(),
            ));
        }

        let query_vector = vectors[0].clone();
        // Pair each candidate id with its embedding, then rank through the
        // swappable VectorIndex (ADR 0050): the brute-force cosine scan today, an
        // ANN index later — with no change here.
        let pairs: Vec<(String, Vec<f32>)> = candidates
            .into_iter()
            .zip(vectors.into_iter().skip(1))
            .map(|(candidate, candidate_vector)| (candidate.id, candidate_vector))
            .collect();

        let ranked = self.index.rank_top_k(&query_vector, &pairs, k);
        Ok(ranked
            .into_iter()
            .map(|candidate| ScoredItem {
                id: candidate.id,
                score: candidate.score,
            })
            .collect())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    //! A deterministic, dependency-free embedder for exercising the embedding
    //! capability/registry/job plumbing without the candle model. It is NOT a
    //! shipped model (it has no semantic meaning) — tests only.

    use async_trait::async_trait;

    use super::super::{Embedder, InterpretationError};

    /// Embeds text into a small fixed-dim vector by hashing tokens into buckets.
    /// Deterministic and order-stable, so identical text always yields identical
    /// vectors and overlapping text yields higher cosine.
    pub struct HashingEmbedder {
        model_id: String,
        dim: usize,
    }

    impl HashingEmbedder {
        pub fn new(model_id: &str, dim: usize) -> Self {
            Self {
                model_id: model_id.to_string(),
                dim,
            }
        }

        fn embed_text(&self, text: &str) -> Vec<f32> {
            let mut vector = vec![0.0_f32; self.dim];
            for token in text
                .split(|c: char| !c.is_alphanumeric())
                .filter(|t| !t.is_empty())
            {
                let mut hash: u64 = 1469598103934665603;
                for byte in token.to_lowercase().bytes() {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(1099511628211);
                }
                let bucket = (hash as usize) % self.dim;
                vector[bucket] += 1.0;
            }
            vector
        }
    }

    #[async_trait]
    impl Embedder for HashingEmbedder {
        fn model_id(&self) -> &str {
            &self.model_id
        }

        fn dim(&self) -> usize {
            self.dim
        }

        async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InterpretationError> {
            Ok(texts.iter().map(|text| self.embed_text(text)).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::test_support::HashingEmbedder;
    use super::*;
    use tauri::async_runtime::block_on;

    fn provider() -> EmbeddingSimilarity {
        EmbeddingSimilarity::new(Arc::new(HashingEmbedder::new("test-embedder", 64)))
    }

    fn item(id: &str, text: &str) -> TextItem {
        TextItem {
            id: id.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn identical_text_scores_near_one() {
        let score = block_on(provider().score("zarząd dywidenda", "zarząd dywidenda")).unwrap();
        assert!(score > 0.99, "identical text should score ~1, got {score}");
    }

    #[test]
    fn most_similar_ranks_overlapping_text_first() {
        let candidates = vec![
            item("a", "raport okresowy za trzeci kwartał"),
            item("b", "rekomendacja wypłaty dywidendy przez zarząd"),
        ];
        let ranked = block_on(provider().most_similar(
            "zarząd rekomenduje wypłatę dywidendy",
            candidates,
            2,
        ))
        .unwrap();
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].id, "b");
        assert!(ranked[0].score >= ranked[1].score);
    }

    #[test]
    fn empty_query_is_invalid() {
        let error = block_on(provider().most_similar("   ", Vec::new(), 3)).unwrap_err();
        assert!(matches!(error, InterpretationError::InvalidRequest(_)));
    }

    #[test]
    fn no_candidates_returns_empty() {
        let ranked = block_on(provider().most_similar("query", Vec::new(), 3)).unwrap();
        assert!(ranked.is_empty());
    }
}
