//! The nearest-neighbour ranking boundary — the explicit ANN swap point
//! (Architecture v2 / ADR 0050 decision 6).
//!
//! Similarity ranking is "given a query vector and candidate `(id, vector)`
//! pairs, return the top `k` by similarity." Today that is an exhaustive cosine
//! scan ([`BruteForceVectorIndex`]) — O(N), documented as adequate at the current
//! watchlist corpus scale. This trait isolates that step so that when corpus
//! scale justifies it (near the `v0.53` cross-company compare / large
//! watchlists), an **ANN-backed index drops in behind the same trait** without
//! touching any [`crate::interpretation::SimilarityProvider`] consumer.
//!
//! Candidate evaluation (ADR 0050): `sqlite-vec` is **rejected** as the engine
//! because it is a C/native SQLite extension and the shipped engine must stay
//! pure-Rust to keep the `cargo-xwin` Linux→Windows cross-build working (see
//! engineering-workflow.md). The pure-Rust candidate (e.g. `hnsw_rs` /
//! `instant-distance`) is chosen at swap time against that posture and the
//! conservative-dependency rule; the brute-force scan stays the default until
//! the swap is justified. The **T4 behavioral scale gate** guards the linear-scan
//! contract, and any ANN impl must preserve top-k correctness against it.

use instant_distance::{Builder, Point as AnnPoint, Search};

use super::vector::{cosine, similarity_score};

/// A candidate id with its similarity score (`(cos + 1) / 2`, in `[0, 1]`).
#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub id: String,
    pub score: f32,
}

/// The swappable top-`k` nearest-neighbour ranking step. The vector index stays
/// a disposable, derived artifact (rebuildable from canonical data), so swapping
/// the implementation is reversible with no data loss.
pub trait VectorIndex: Send + Sync {
    /// Rank `candidates` against `query`, returning the top `k` highest-similarity
    /// candidates, highest first.
    fn rank_top_k(
        &self,
        query: &[f32],
        candidates: &[(String, Vec<f32>)],
        k: usize,
    ) -> Vec<RankedCandidate>;
}

/// Exhaustive cosine scan — the default index and the correctness contract any
/// ANN replacement must match. O(N) in the candidate count.
#[derive(Debug, Default, Clone, Copy)]
pub struct BruteForceVectorIndex;

impl VectorIndex for BruteForceVectorIndex {
    fn rank_top_k(
        &self,
        query: &[f32],
        candidates: &[(String, Vec<f32>)],
        k: usize,
    ) -> Vec<RankedCandidate> {
        let mut scored: Vec<RankedCandidate> = candidates
            .iter()
            .map(|(id, vector)| RankedCandidate {
                id: id.clone(),
                score: similarity_score(query, vector),
            })
            .collect();
        // Highest score first; ties keep input order (stable sort).
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored.truncate(k);
        scored
    }
}

/// A vector wrapped as an HNSW point whose distance is `1 - cosine`, so a smaller
/// distance means more similar — the metric `instant-distance` minimizes.
#[derive(Clone, Debug)]
struct CosinePoint(Vec<f32>);

impl AnnPoint for CosinePoint {
    fn distance(&self, other: &Self) -> f32 {
        1.0 - cosine(&self.0, &other.0)
    }
}

/// An **approximate** nearest-neighbour index (pure-Rust HNSW via
/// `instant-distance`) implementing the [`VectorIndex`] swap boundary (ADR 0050
/// decision 6). It is the scale-time replacement for [`BruteForceVectorIndex`]:
/// where a large, persisted candidate set is queried repeatedly, HNSW makes
/// ranking sublinear instead of O(N) per query.
///
/// Note on the per-call API: building an HNSW over candidates supplied fresh each
/// call costs more than a single linear scan, so this index pays off only when
/// the candidate set is large and/or the built index is reused. Its production
/// activation is the persisted `content_embeddings` path; here it is provided,
/// correct, and tested behind the same trait so the swap is a one-line change.
/// A fixed seed makes index construction deterministic.
pub struct AnnVectorIndex {
    seed: u64,
}

impl Default for AnnVectorIndex {
    fn default() -> Self {
        Self { seed: 0x5EED }
    }
}

impl VectorIndex for AnnVectorIndex {
    fn rank_top_k(
        &self,
        query: &[f32],
        candidates: &[(String, Vec<f32>)],
        k: usize,
    ) -> Vec<RankedCandidate> {
        if candidates.is_empty() || k == 0 {
            return Vec::new();
        }

        let points = candidates
            .iter()
            .map(|(_, vector)| CosinePoint(vector.clone()))
            .collect::<Vec<_>>();
        let values = (0..candidates.len()).collect::<Vec<usize>>();
        let map = Builder::default().seed(self.seed).build(points, values);

        let query_point = CosinePoint(query.to_vec());
        let mut search = Search::default();
        map.search(&query_point, &mut search)
            .take(k)
            .map(|item| {
                let (id, vector) = &candidates[*item.value];
                RankedCandidate {
                    id: id.clone(),
                    // Report the same [0,1] similarity score as the brute-force
                    // index, recomputed exactly for the returned candidates.
                    score: similarity_score(query, vector),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<(String, Vec<f32>)> {
        vec![
            ("a".to_string(), vec![1.0, 0.0]),
            ("b".to_string(), vec![0.0, 1.0]),
            ("c".to_string(), vec![1.0, 1.0]),
        ]
    }

    #[test]
    fn ranks_nearest_first_and_truncates_to_k() {
        let index = BruteForceVectorIndex;
        let ranked = index.rank_top_k(&[1.0, 0.0], &candidates(), 2);

        assert_eq!(ranked.len(), 2);
        // "a" is identical to the query (highest), "c" (45°) outranks "b" (90°).
        assert_eq!(ranked[0].id, "a");
        assert_eq!(ranked[1].id, "c");
        assert!(ranked[0].score >= ranked[1].score);
    }

    #[test]
    fn empty_candidates_yield_empty_ranking() {
        let index = BruteForceVectorIndex;
        assert!(index.rank_top_k(&[1.0, 0.0], &[], 5).is_empty());
    }

    #[test]
    fn ties_keep_input_order() {
        let index = BruteForceVectorIndex;
        let tied = vec![
            ("first".to_string(), vec![1.0, 0.0]),
            ("second".to_string(), vec![1.0, 0.0]),
        ];
        let ranked = index.rank_top_k(&[1.0, 0.0], &tied, 2);
        assert_eq!(ranked[0].id, "first");
        assert_eq!(ranked[1].id, "second");
    }

    /// Well-separated candidates so the approximate index returns the true
    /// nearest neighbour — its top result must match the brute-force index.
    #[test]
    fn ann_index_agrees_with_brute_force_on_separated_vectors() {
        let candidates: Vec<(String, Vec<f32>)> = (0..16)
            .map(|i| {
                let mut v = vec![0.0_f32; 16];
                v[i] = 1.0;
                (format!("c{i}"), v)
            })
            .collect();

        let mut query = vec![0.0_f32; 16];
        query[7] = 1.0;

        let ann = AnnVectorIndex::default().rank_top_k(&query, &candidates, 1);
        let brute = BruteForceVectorIndex.rank_top_k(&query, &candidates, 1);

        assert_eq!(ann.len(), 1);
        assert_eq!(ann[0].id, "c7");
        assert_eq!(ann[0].id, brute[0].id);
        assert!((ann[0].score - brute[0].score).abs() < 1e-6);
    }

    #[test]
    fn ann_index_handles_empty_and_zero_k() {
        let ann = AnnVectorIndex::default();
        assert!(ann.rank_top_k(&[1.0, 0.0], &[], 3).is_empty());
        assert!(ann
            .rank_top_k(&[1.0, 0.0], &[("a".to_string(), vec![1.0, 0.0])], 0)
            .is_empty());
    }
}
