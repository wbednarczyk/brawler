//! Seeded app framework templates (ADR 0046, Decision 6).
//!
//! A template definition lives here as a Rust constant — the single source for
//! both the startup seed and `reset_framework_to_template`, so the two cannot
//! drift. Templates are seeded as `app_template`-origin frameworks; they are
//! editable in place like any framework and resettable to these defaults.
//!
//! Note: this Kroeze-*style* template is a generic quality checklist built from
//! well-known quality-investing concepts over the seeded canonical metrics. It
//! deliberately does not reproduce any private-document criteria text.

/// A criterion within a template: a label, a DSL expression, and an optional
/// partial-band threshold (relaxed threshold that yields a `partial` verdict).
pub struct TemplateCriterion {
    pub label: &'static str,
    pub expression: &'static str,
    pub partial_band: Option<&'static str>,
}

/// A seeded framework template.
pub struct FrameworkTemplate {
    pub template_key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub criteria: &'static [TemplateCriterion],
}

/// All app templates seeded on startup.
pub const TEMPLATES: &[FrameworkTemplate] = &[KROEZE_QUALITY];

/// A Kroeze-style quality checklist: durable returns, healthy margins,
/// conservative leverage, cash generation, and steady growth.
pub const KROEZE_QUALITY: FrameworkTemplate = FrameworkTemplate {
    template_key: "kroeze_quality",
    name: "Quality (Kroeze-style)",
    description: "A quantitative quality checklist: durable returns on capital, healthy margins, conservative leverage, real cash generation, and steady growth. Decision support only.",
    criteria: &[
        TemplateCriterion {
            label: "Strong return on equity",
            expression: "roe >= 15%",
            partial_band: Some("10%"),
        },
        TemplateCriterion {
            label: "Healthy operating margin",
            expression: "operating_margin >= 15%",
            partial_band: Some("10%"),
        },
        TemplateCriterion {
            label: "Positive free cash flow",
            expression: "free_cash_flow > 0",
            partial_band: None,
        },
        TemplateCriterion {
            label: "Strong FCF conversion",
            expression: "fcf_conversion >= 80%",
            partial_band: Some("60%"),
        },
        TemplateCriterion {
            label: "Conservative leverage",
            expression: "net_debt_to_ebitda < 2.5",
            partial_band: Some("3.5"),
        },
        TemplateCriterion {
            label: "Solid liquidity",
            expression: "current_ratio >= 1.5",
            partial_band: Some("1"),
        },
        TemplateCriterion {
            label: "Comfortable interest coverage",
            expression: "interest_coverage >= 4",
            partial_band: Some("2"),
        },
        TemplateCriterion {
            label: "Steady revenue growth",
            expression: "cagr(revenue, 3) >= 8%",
            partial_band: Some("4%"),
        },
    ],
};

/// Look up a template by its stable key (for reset).
pub fn template_by_key(key: &str) -> Option<&'static FrameworkTemplate> {
    TEMPLATES.iter().find(|t| t.template_key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fundamentals::expr::{is_predicate, parse};

    #[test]
    fn every_template_criterion_parses_and_is_a_predicate() {
        for template in TEMPLATES {
            for criterion in template.criteria {
                let expr = parse(criterion.expression).unwrap_or_else(|e| {
                    panic!(
                        "template '{}' criterion '{}' does not parse: {e}",
                        template.name, criterion.label
                    )
                });
                assert!(
                    is_predicate(&expr),
                    "template '{}' criterion '{}' is not a predicate",
                    template.name,
                    criterion.label
                );
                if let Some(band) = criterion.partial_band {
                    parse(band).unwrap_or_else(|e| {
                        panic!("template '{}' criterion '{}' partial band '{band}' does not parse: {e}", template.name, criterion.label)
                    });
                }
            }
        }
    }
}
