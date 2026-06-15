//! Lexical [`SimilarityProvider`] — the static baseline for the similarity
//! capability (ADR 0035). It scores two texts by Jaccard overlap of their token
//! sets: deterministic, dependency-free, and good enough as the baseline that a
//! future embedding-backed implementation must beat (per the eval policy) before
//! it is adopted. Its first model consumer is story clustering.

use async_trait::async_trait;
use std::collections::BTreeSet;

use super::{InterpretationError, ScoredItem, SimilarityProvider, TextItem};

/// Stable id of the lexical similarity implementation.
pub const LEXICAL_SIMILARITY_ID: &str = "similarity_lexical";

/// Split text into a set of lowercased alphanumeric tokens.
fn token_set(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

/// Jaccard similarity of two token sets: `|A ∩ B| / |A ∪ B|`, in `0.0..=1.0`.
/// Two texts with no tokens (after tokenization) score `0.0`.
fn jaccard(a: &str, b: &str) -> f32 {
    let set_a = token_set(a);
    let set_b = token_set(b);
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
    }
    let intersection = set_a.intersection(&set_b).count();
    intersection as f32 / union as f32
}

/// A deterministic similarity provider backed by token-set Jaccard overlap.
pub struct LexicalSimilarity {
    id: String,
}

impl LexicalSimilarity {
    /// Build the lexical similarity provider with the default id.
    pub fn new() -> Self {
        Self {
            id: LEXICAL_SIMILARITY_ID.to_string(),
        }
    }
}

impl Default for LexicalSimilarity {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SimilarityProvider for LexicalSimilarity {
    fn id(&self) -> &str {
        &self.id
    }

    async fn score(&self, a: &str, b: &str) -> Result<f32, InterpretationError> {
        Ok(jaccard(a, b))
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
        let mut scored: Vec<ScoredItem> = candidates
            .into_iter()
            .map(|candidate| ScoredItem {
                id: candidate.id,
                score: jaccard(query, &candidate.text),
            })
            .collect();
        // Highest score first; ties keep input order (stable sort on reversed key).
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored.truncate(k);
        Ok(scored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::async_runtime::block_on;

    fn item(id: &str, text: &str) -> TextItem {
        TextItem {
            id: id.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn identical_text_scores_one_and_disjoint_scores_zero() {
        assert_eq!(
            block_on(LexicalSimilarity::new().score("alfa beta", "beta alfa")).unwrap(),
            1.0
        );
        assert_eq!(
            block_on(LexicalSimilarity::new().score("alfa", "gamma")).unwrap(),
            0.0
        );
    }

    #[test]
    fn most_similar_ranks_and_truncates() {
        let candidates = vec![
            item("a", "zarząd rekomenduje wypłatę dywidendy"),
            item("b", "raport okresowy za trzeci kwartał"),
            item("c", "rekomendacja wypłaty dywidendy przez zarząd"),
        ];
        let ranked = block_on(LexicalSimilarity::new().most_similar(
            "rekomendacja wypłaty dywidendy",
            candidates,
            2,
        ))
        .unwrap();
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].id, "c");
        assert!(ranked[0].score >= ranked[1].score);
    }

    #[test]
    fn empty_query_is_invalid() {
        let error =
            block_on(LexicalSimilarity::new().most_similar("  ", Vec::new(), 5)).unwrap_err();
        assert!(matches!(error, InterpretationError::InvalidRequest(_)));
    }
}
