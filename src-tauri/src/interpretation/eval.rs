//! Per-capability evaluation harness (ADR 0035, section 7).
//!
//! The interpretative layer adopts a model-backed implementation for a capability
//! only where it measurably beats the static baseline. This harness scores a
//! [`Classifier`] against labeled samples so that comparison is data-driven and
//! deterministic (sample-backed, no live dependencies). The same pattern extends
//! to the other capabilities as they gain implementations.

use super::{ClassificationRequest, Classifier, InterpretationError, SimilarityProvider, TextItem};

/// One labeled classification sample. `expected = None` means the sample should
/// be classified as unknown (no category should be assigned).
#[derive(Debug, Clone)]
pub struct LabeledSample {
    pub text: String,
    pub expected: Option<String>,
}

/// The outcome of evaluating a [`Classifier`] over a set of [`LabeledSample`]s.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifierEvalReport {
    /// Total samples evaluated.
    pub total: usize,
    /// Predicted category equals the expected label (including both being unknown).
    pub correct: usize,
    /// Predicted a category that differs from the expected label — a real
    /// misclassification (the outcome the unknown path exists to avoid).
    pub wrong: usize,
    /// Samples the classifier returned as unknown.
    pub unknown_predicted: usize,
}

impl ClassifierEvalReport {
    /// Fraction of all samples classified correctly (`0.0..=1.0`).
    pub fn accuracy(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.correct as f32 / self.total as f32
    }

    /// Fraction of samples for which a category was assigned (not unknown).
    pub fn coverage(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.total - self.unknown_predicted) as f32 / self.total as f32
    }

    /// Of the samples where a category was assigned, the fraction assigned
    /// correctly. `1.0` when nothing was assigned (no wrong assignments made).
    pub fn precision(&self) -> f32 {
        let assigned = self.total - self.unknown_predicted;
        if assigned == 0 {
            return 1.0;
        }
        (assigned - self.wrong) as f32 / assigned as f32
    }
}

/// Run `classifier` over `samples` and report accuracy/coverage/precision.
///
/// A sample errors (e.g. empty text) are treated as misclassifications against a
/// non-unknown expectation, and as correct unknowns otherwise — the harness
/// never panics on a bad sample, so an eval run always produces a report.
pub async fn evaluate_classifier(
    classifier: &dyn Classifier,
    samples: &[LabeledSample],
) -> ClassifierEvalReport {
    let mut report = ClassifierEvalReport {
        total: samples.len(),
        correct: 0,
        wrong: 0,
        unknown_predicted: 0,
    };

    for sample in samples {
        let predicted = match classifier
            .classify(ClassificationRequest {
                text: sample.text.clone(),
                candidate_categories: Vec::new(),
            })
            .await
        {
            Ok(classification) => classification.category,
            // A request error yields no category (treated as an unknown prediction).
            Err(InterpretationError::InvalidRequest(_))
            | Err(InterpretationError::Unavailable(_))
            | Err(InterpretationError::Backend(_)) => None,
        };

        if predicted.is_none() {
            report.unknown_predicted += 1;
        }
        if predicted == sample.expected {
            report.correct += 1;
        } else if predicted.is_some() {
            report.wrong += 1;
        }
    }

    report
}

/// One ranking sample for the similarity capability: a query, its candidate set,
/// and the id of the candidate that *should* rank first (the relevant one).
#[derive(Debug, Clone)]
pub struct SimilarityRankingSample {
    pub query: String,
    pub candidates: Vec<TextItem>,
    pub expected_id: String,
}

/// The outcome of evaluating a [`SimilarityProvider`] over ranking samples.
/// Used to compare a model-backed provider against the lexical baseline so the
/// keep/drop decision is data-driven (ADR 0035 section 7). Run it once per
/// provider and compare the reports.
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarityEvalReport {
    /// The implementation id that produced this report (`similarity_lexical` /
    /// `similarity_embedding`), so a comparison names the winner.
    pub provider_id: String,
    /// Total samples evaluated.
    pub total: usize,
    /// Samples where the expected candidate ranked first.
    pub top1_hits: usize,
    /// Sum of reciprocal ranks of the expected candidate (0 when absent).
    pub reciprocal_rank_sum: f32,
}

impl SimilarityEvalReport {
    /// Fraction of samples where the expected candidate ranked first.
    pub fn top1_accuracy(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.top1_hits as f32 / self.total as f32
    }

    /// Mean reciprocal rank of the expected candidate across samples.
    pub fn mean_reciprocal_rank(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.reciprocal_rank_sum / self.total as f32
    }
}

/// Run `provider` over ranking `samples` and report top-1 accuracy and MRR.
/// Provider errors on a sample are treated as a miss (no hit, zero reciprocal
/// rank), so an eval run always produces a report.
pub async fn evaluate_similarity(
    provider: &dyn SimilarityProvider,
    samples: &[SimilarityRankingSample],
) -> SimilarityEvalReport {
    let mut report = SimilarityEvalReport {
        provider_id: provider.id().to_string(),
        total: samples.len(),
        top1_hits: 0,
        reciprocal_rank_sum: 0.0,
    };

    for sample in samples {
        let k = sample.candidates.len();
        let ranked = provider
            .most_similar(&sample.query, sample.candidates.clone(), k)
            .await
            .unwrap_or_default();

        if let Some(position) = ranked.iter().position(|item| item.id == sample.expected_id) {
            if position == 0 {
                report.top1_hits += 1;
            }
            report.reciprocal_rank_sum += 1.0 / (position as f32 + 1.0);
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::rule_classifier::{CategoryRule, RuleClassifier};
    use crate::interpretation::LexicalSimilarity;
    use tauri::async_runtime::block_on;

    fn classifier() -> RuleClassifier {
        RuleClassifier::new(vec![CategoryRule {
            category: "dividend".to_string(),
            patterns: vec!["dywidend".to_string()],
            confidence: 0.9,
        }])
    }

    fn samples() -> Vec<LabeledSample> {
        vec![
            LabeledSample {
                text: "Rekomendacja wypłaty dywidendy".to_string(),
                expected: Some("dividend".to_string()),
            },
            LabeledSample {
                text: "Raport okresowy".to_string(),
                expected: None,
            },
        ]
    }

    #[test]
    fn scores_correct_unknown_and_coverage() {
        let report = block_on(evaluate_classifier(&classifier(), &samples()));
        assert_eq!(report.total, 2);
        assert_eq!(report.correct, 2);
        assert_eq!(report.wrong, 0);
        assert_eq!(report.unknown_predicted, 1);
        assert_eq!(report.accuracy(), 1.0);
        assert_eq!(report.coverage(), 0.5);
        assert_eq!(report.precision(), 1.0);
    }

    #[test]
    fn counts_misclassification_as_wrong() {
        // Expected unknown, but the rule assigns "dividend" -> a wrong assignment.
        let mislabeled = vec![LabeledSample {
            text: "Polityka dywidendowa spółki".to_string(),
            expected: None,
        }];
        let report = block_on(evaluate_classifier(&classifier(), &mislabeled));
        assert_eq!(report.wrong, 1);
        assert_eq!(report.correct, 0);
        assert_eq!(report.precision(), 0.0);
    }

    fn item(id: &str, text: &str) -> TextItem {
        TextItem {
            id: id.to_string(),
            text: text.to_string(),
        }
    }

    fn similarity_samples() -> Vec<SimilarityRankingSample> {
        vec![SimilarityRankingSample {
            query: "zarząd rekomenduje wypłatę dywidendy".to_string(),
            candidates: vec![
                item("relevant", "rekomendacja wypłaty dywidendy przez zarząd"),
                item("other", "raport okresowy za trzeci kwartał"),
            ],
            expected_id: "relevant".to_string(),
        }]
    }

    #[test]
    fn similarity_eval_scores_top1_and_mrr() {
        let report = block_on(evaluate_similarity(
            &LexicalSimilarity::new(),
            &similarity_samples(),
        ));
        assert_eq!(report.provider_id, "similarity_lexical");
        assert_eq!(report.total, 1);
        assert_eq!(report.top1_hits, 1);
        assert_eq!(report.top1_accuracy(), 1.0);
        assert_eq!(report.mean_reciprocal_rank(), 1.0);
    }

    #[test]
    fn similarity_eval_compares_model_against_lexical_baseline() {
        // The keep/drop decision (ADR 0035 section 7) is "does the model beat the
        // baseline on the same samples". This exercises that comparison path with
        // a deterministic test embedder (not a shipped model); the real model eval
        // runs against cached weights in the periodic tier.
        use crate::interpretation::embedding_similarity::test_support::HashingEmbedder;
        use crate::interpretation::EmbeddingSimilarity;
        use std::sync::Arc;

        let samples = similarity_samples();
        let lexical = block_on(evaluate_similarity(&LexicalSimilarity::new(), &samples));
        let model = block_on(evaluate_similarity(
            &EmbeddingSimilarity::new(Arc::new(HashingEmbedder::new("test-embedder", 64))),
            &samples,
        ));

        assert_eq!(model.provider_id, "similarity_embedding");
        assert_eq!(lexical.provider_id, "similarity_lexical");
        // Both find the relevant candidate on this sample; the harness yields
        // comparable metrics so a winner can be chosen data-driven.
        assert_eq!(model.top1_accuracy(), lexical.top1_accuracy());
        assert!(model.mean_reciprocal_rank() >= 0.0);
    }
}
