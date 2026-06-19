//! Vector math for the embedding-backed interpretative capabilities (ADR 0035).
//!
//! The vector store keeps raw embeddings; similarity is a pure-Rust cosine scan
//! over them (no `sqlite-vec`/ANN at this corpus scale). Scores are mapped into
//! the capability contract's `0.0..=1.0` range while preserving ordering.

/// Cosine similarity of two equal-length vectors, in `-1.0..=1.0`.
///
/// Returns `0.0` for length mismatch or a zero-norm vector — degenerate inputs
/// are treated as "no similarity" rather than an error, so a partially-populated
/// index never breaks a scan.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Cosine similarity mapped to the capability contract's `0.0..=1.0` range as
/// `(cos + 1) / 2`. Order-preserving (a higher cosine always maps to a higher
/// score), so ranking is unaffected; only the absolute scale changes.
pub fn similarity_score(a: &[f32], b: &[f32]) -> f32 {
    ((cosine(a, b) + 1.0) / 2.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors_score_one() {
        let v = vec![0.2_f32, 0.4, -0.1];
        assert!((similarity_score(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn opposite_vectors_score_zero() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![-1.0_f32, 0.0];
        assert!(similarity_score(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors_score_half() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        assert!((similarity_score(&a, &b) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn degenerate_inputs_score_zero() {
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }

    #[test]
    fn ranking_is_order_preserving() {
        let query = vec![1.0_f32, 1.0];
        let near = vec![1.0_f32, 0.9];
        let far = vec![1.0_f32, -0.5];
        assert!(similarity_score(&query, &near) > similarity_score(&query, &far));
    }
}
