//! Per-capability evaluation harness (ADR 0035, section 7).
//!
//! The interpretative layer adopts a model-backed implementation for a capability
//! only where it measurably beats the static baseline. This harness scores a
//! [`Classifier`] against labeled samples so that comparison is data-driven and
//! deterministic (sample-backed, no live dependencies). The same pattern extends
//! to the other capabilities as they gain implementations.

use super::{ClassificationRequest, Classifier, InterpretationError};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::rule_classifier::{CategoryRule, RuleClassifier};
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
}
