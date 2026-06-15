//! Deterministic rule-based [`Classifier`] — the static baseline for the
//! classification capability (ADR 0035). It is taxonomy-agnostic: callers
//! construct it with a set of [`CategoryRule`]s (e.g. the ESPI
//! `signal_categories` rules, supplied by the classification milestone). A
//! confident match names a category; no match returns the unknown outcome
//! rather than guessing.

use async_trait::async_trait;

use super::{Classification, ClassificationRequest, Classifier, InterpretationError};

/// Stable id of the rule-based classifier implementation.
pub const RULE_CLASSIFIER_ID: &str = "classifier_rules";

/// One category plus the case-insensitive substring patterns that select it.
#[derive(Debug, Clone)]
pub struct CategoryRule {
    /// Category key this rule assigns (e.g. `"insider_transaction"`).
    pub category: String,
    /// Case-insensitive substrings; if any is present in the text, the rule matches.
    pub patterns: Vec<String>,
    /// Confidence reported when this rule matches (`0.0..=1.0`).
    pub confidence: f32,
}

impl CategoryRule {
    /// Lowercased text matches this rule if it contains any (non-empty) pattern.
    fn matches(&self, lowercased_text: &str) -> bool {
        self.patterns.iter().any(|pattern| {
            let needle = pattern.trim().to_lowercase();
            !needle.is_empty() && lowercased_text.contains(&needle)
        })
    }
}

/// A deterministic classifier that assigns the highest-confidence matching rule,
/// or unknown when nothing matches.
pub struct RuleClassifier {
    id: String,
    rules: Vec<CategoryRule>,
}

impl RuleClassifier {
    /// Build a rule classifier with the default id and the given rules.
    pub fn new(rules: Vec<CategoryRule>) -> Self {
        Self::with_id(RULE_CLASSIFIER_ID, rules)
    }

    /// Build a rule classifier with an explicit id (e.g. a taxonomy-specific id).
    pub fn with_id(id: impl Into<String>, rules: Vec<CategoryRule>) -> Self {
        Self {
            id: id.into(),
            rules,
        }
    }
}

#[async_trait]
impl Classifier for RuleClassifier {
    fn id(&self) -> &str {
        &self.id
    }

    async fn classify(
        &self,
        request: ClassificationRequest,
    ) -> Result<Classification, InterpretationError> {
        let trimmed = request.text.trim();
        if trimmed.is_empty() {
            return Err(InterpretationError::InvalidRequest(
                "text to classify is empty".to_string(),
            ));
        }
        let lowercased = trimmed.to_lowercase();
        let constrained = !request.candidate_categories.is_empty();

        let best = self
            .rules
            .iter()
            .filter(|rule| !constrained || request.candidate_categories.contains(&rule.category))
            .filter(|rule| rule.matches(&lowercased))
            // Highest confidence wins; ties resolve to the earlier rule.
            .max_by(|a, b| a.confidence.total_cmp(&b.confidence));

        Ok(match best {
            Some(rule) => Classification {
                category: Some(rule.category.clone()),
                confidence: rule.confidence,
                rationale: Some(format!("matched rule for {}", rule.category)),
            },
            None => Classification {
                category: None,
                confidence: 0.0,
                rationale: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::async_runtime::block_on;

    fn sample_rules() -> Vec<CategoryRule> {
        vec![
            CategoryRule {
                category: "insider_transaction".to_string(),
                patterns: vec!["transakcje".to_string(), "art. 19 MAR".to_string()],
                confidence: 0.95,
            },
            CategoryRule {
                category: "dividend".to_string(),
                patterns: vec!["dywidend".to_string()],
                confidence: 0.9,
            },
        ]
    }

    fn classify_request(text: &str, candidates: Vec<String>) -> ClassificationRequest {
        ClassificationRequest {
            text: text.to_string(),
            candidate_categories: candidates,
        }
    }

    fn classify(text: &str) -> Classification {
        block_on(RuleClassifier::new(sample_rules()).classify(classify_request(text, Vec::new())))
            .expect("classify")
    }

    #[test]
    fn matches_category_case_insensitively() {
        let result = classify("Zawiadomienie o TRANSAKCJE na akcjach");
        assert_eq!(result.category.as_deref(), Some("insider_transaction"));
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn returns_unknown_when_no_rule_matches() {
        let result = classify("Raport okresowy za III kwartał");
        assert_eq!(result.category, None);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn empty_text_is_invalid() {
        let error = block_on(
            RuleClassifier::new(sample_rules()).classify(classify_request("   ", Vec::new())),
        )
        .unwrap_err();
        assert!(matches!(error, InterpretationError::InvalidRequest(_)));
    }

    #[test]
    fn honours_candidate_category_constraint() {
        // Text matches the dividend rule, but it is excluded from the candidates.
        let result = block_on(
            RuleClassifier::new(sample_rules()).classify(classify_request(
                "Rekomendacja wypłaty dywidendy",
                vec!["insider_transaction".to_string()],
            )),
        )
        .expect("classify");
        assert_eq!(result.category, None);
    }
}
