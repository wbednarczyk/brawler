//! Seeded app framework templates (ADR 0046 Decision 6; ADR 0076 Decision 8).
//!
//! A template definition lives here as a Rust constant — the single source for
//! the startup seed, `reset_framework_to_template`, and the non-destructive
//! top-up, so they cannot drift. Templates are seeded as `app_template`-origin
//! frameworks; they are editable in place like any framework and resettable to
//! these defaults.
//!
//! Every human-facing string is **bilingual** ([`LocalizedText`] `{pl, en}`) so
//! the seed/reset/top-up resolve the language from the persisted app-locale
//! setting with no runtime translation lookups (ADR 0076 Decision 8). The
//! criterion *keys* — `expression`, `partial_band`, `kind` — stay
//! locale-independent; only `label`, template `name`/`description`, and
//! `assessment_guidance` are localized.
//!
//! Note: this Kroeze-*style* template is a generic quality checklist built from
//! well-known quality-investing concepts over the seeded canonical metrics. It
//! deliberately does not reproduce any private-document criteria text.

/// A bilingual string: the Polish and English renderings of one piece of
/// template text (ADR 0076 Decision 8). Both are authored here — there is no
/// runtime translation lookup.
pub struct LocalizedText {
    pub pl: &'static str,
    pub en: &'static str,
}

impl LocalizedText {
    /// Resolve the text for a supported locale code (`pl` | `en`). Any other
    /// value falls back to English; callers normalize the persisted setting to
    /// one of the two supported codes before calling (see `seed_locale`).
    pub fn get(&self, locale: &str) -> &'static str {
        match locale {
            "pl" => self.pl,
            _ => self.en,
        }
    }
}

/// A criterion within a template. A quantitative criterion carries a DSL
/// `expression` and an optional partial-band threshold (relaxed threshold that
/// yields a `partial` verdict). A qualitative criterion (ADR 0075) carries an
/// empty `expression`, `kind = "qualitative"`, and an owner-authored bilingual
/// `assessment_guidance` seed for the agent.
pub struct TemplateCriterion {
    /// Bilingual human-facing label (localized at seed/reset/top-up).
    pub label: LocalizedText,
    /// Locale-independent DSL predicate (the criterion key); empty for qualitative.
    pub expression: &'static str,
    /// Locale-independent relaxed threshold for a `partial` verdict.
    pub partial_band: Option<&'static str>,
    /// Locale-independent `"quantitative"` (default) or `"qualitative"` (ADR 0075).
    pub kind: &'static str,
    /// Bilingual owner-authored prompt seed for a qualitative criterion; `None`
    /// for quantitative criteria.
    pub assessment_guidance: Option<LocalizedText>,
}

/// A seeded framework template.
pub struct FrameworkTemplate {
    pub template_key: &'static str,
    pub name: LocalizedText,
    pub description: LocalizedText,
    pub criteria: &'static [TemplateCriterion],
}

/// All app templates seeded on startup.
pub const TEMPLATES: &[FrameworkTemplate] = &[KROEZE_QUALITY];

/// A Kroeze-style quality checklist: durable returns, healthy margins,
/// conservative leverage, cash generation, and steady growth.
pub const KROEZE_QUALITY: FrameworkTemplate = FrameworkTemplate {
    template_key: "kroeze_quality",
    name: LocalizedText {
        pl: "Jakość (styl Kroeze)",
        en: "Quality (Kroeze-style)",
    },
    description: LocalizedText {
        pl: "Lista kontrolna jakości łącząca miary ilościowe (trwała rentowność kapitału, zdrowe marże, konserwatywne zadłużenie, realna generacja gotówki, stabilny wzrost) z ocenami jakościowymi wykonywanymi przez agenta (fosa, siła cenowa, powtarzalne przychody, alokacja kapitału, zrozumiałość biznesu, własność insiderów). Wyłącznie wsparcie decyzji.",
        en: "A quality checklist blending quantitative measures (durable returns on capital, healthy margins, conservative leverage, real cash generation, steady growth) with agent-assessed qualitative judgments (moat, pricing power, recurring revenue, capital allocation, business clarity, insider ownership). Decision support only.",
    },
    criteria: &[
        TemplateCriterion {
            label: LocalizedText {
                pl: "Wysoka rentowność kapitału własnego",
                en: "Strong return on equity",
            },
            expression: "roe >= 15%",
            partial_band: Some("10%"),
            kind: "quantitative",
            assessment_guidance: None,
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Zdrowa marża operacyjna",
                en: "Healthy operating margin",
            },
            expression: "operating_margin >= 15%",
            partial_band: Some("10%"),
            kind: "quantitative",
            assessment_guidance: None,
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Dodatnie wolne przepływy pieniężne",
                en: "Positive free cash flow",
            },
            expression: "free_cash_flow > 0",
            partial_band: None,
            kind: "quantitative",
            assessment_guidance: None,
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Wysoka konwersja FCF",
                en: "Strong FCF conversion",
            },
            expression: "fcf_conversion >= 80%",
            partial_band: Some("60%"),
            kind: "quantitative",
            assessment_guidance: None,
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Konserwatywne zadłużenie",
                en: "Conservative leverage",
            },
            expression: "net_debt_to_ebitda < 2.5",
            partial_band: Some("3.5"),
            kind: "quantitative",
            assessment_guidance: None,
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Solidna płynność",
                en: "Solid liquidity",
            },
            expression: "current_ratio >= 1.5",
            partial_band: Some("1"),
            kind: "quantitative",
            assessment_guidance: None,
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Komfortowe pokrycie odsetek",
                en: "Comfortable interest coverage",
            },
            expression: "interest_coverage >= 4",
            partial_band: Some("2"),
            kind: "quantitative",
            assessment_guidance: None,
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Stabilny wzrost przychodów",
                en: "Steady revenue growth",
            },
            expression: "cagr(revenue, 3) >= 8%",
            partial_band: Some("4%"),
            kind: "quantitative",
            assessment_guidance: None,
        },
        // Qualitative criteria (ADR 0075): no DSL expression; agent-assessed
        // against app-held evidence. Guidance is decision-support only — never
        // buy/sell/hold or allocation language.
        TemplateCriterion {
            label: LocalizedText {
                pl: "Trwała przewaga konkurencyjna (fosa)",
                en: "Durable moat",
            },
            expression: "",
            partial_band: None,
            kind: "qualitative",
            assessment_guidance: Some(LocalizedText {
                pl: "Oceń, czy spółka ma trwałą przewagę konkurencyjną — markę, efekty sieciowe, koszty zmiany dostawcy, skalę lub bariery regulacyjne — która chroni rentowność kapitału w czasie. Oprzyj ocenę na dowodach dotyczących cen, udziału w rynku lub trwałości marż z dostarczonych źródeł.",
                en: "Assess whether the company has a durable competitive advantage — brand, network effects, switching costs, scale, or regulatory barriers — that protects returns on capital over time. Ground the judgment in evidence of pricing, market share, or margin durability from the supplied sources.",
            }),
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Siła cenowa",
                en: "Pricing power",
            },
            expression: "",
            partial_band: None,
            kind: "qualitative",
            assessment_guidance: Some(LocalizedText {
                pl: "Oceń zdolność spółki do podnoszenia cen bez utraty klientów — dowody na stabilne lub rosnące marże mimo inflacji kosztów albo wprost podwyżki cen zaakceptowane przez klientów, z dostarczonych źródeł.",
                en: "Assess the company's ability to raise prices without losing customers — evidence of stable or expanding margins through cost inflation, or explicit price increases absorbed by customers, from the supplied sources.",
            }),
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Powtarzalne przychody",
                en: "Recurring revenue",
            },
            expression: "",
            partial_band: None,
            kind: "qualitative",
            assessment_guidance: Some(LocalizedText {
                pl: "Oceń, jaka część przychodów jest powtarzalna, kontraktowa lub subskrypcyjna, a jaka jednorazowa. Oprzyj ocenę na ujawnionej strukturze przychodów, portfelu zamówień lub wskaźnikach utrzymania klientów z dostarczonych źródeł.",
                en: "Assess how much of revenue is recurring, contracted, or subscription-based versus one-off. Ground the judgment in disclosed revenue mix, backlog, or retention figures from the supplied sources.",
            }),
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Jakość alokacji kapitału",
                en: "Capital-allocation quality",
            },
            expression: "",
            partial_band: None,
            kind: "qualitative",
            assessment_guidance: Some(LocalizedText {
                pl: "Oceń historię zarządu w alokacji kapitału — zwroty z reinwestycji, zdyscyplinowane przejęcia, skup akcji przy rozsądnych wycenach oraz politykę dywidendy spójną z generacją gotówki — z dostarczonych źródeł.",
                en: "Assess management's track record of allocating capital — reinvestment returns, disciplined acquisitions, buybacks at sensible valuations, and a dividend policy consistent with cash generation — from the supplied sources.",
            }),
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Zrozumiały model biznesowy",
                en: "Understandable business",
            },
            expression: "",
            partial_band: None,
            kind: "qualitative",
            assessment_guidance: Some(LocalizedText {
                pl: "Oceń, czy model biznesowy jest prosty i zrozumiały — jak spółka zarabia, jakie są jej kluczowe czynniki wzrostu i czy ujawnienia są jasne, a nie nieprzejrzyste — z dostarczonych źródeł.",
                en: "Assess whether the business model is simple and understandable — how it earns money, its key drivers, and whether disclosures are clear rather than opaque — from the supplied sources.",
            }),
        },
        TemplateCriterion {
            label: LocalizedText {
                pl: "Własność założycieli/insiderów",
                en: "Founder/insider ownership",
            },
            expression: "",
            partial_band: None,
            kind: "qualitative",
            assessment_guidance: Some(LocalizedText {
                pl: "Oceń stopień znaczącej własności insiderów lub założycieli, która wiąże interesy zarządu z długoterminowymi akcjonariuszami. Oprzyj ocenę na ujawnionych udziałach lub sygnałach z transakcji insiderów z dostarczonych źródeł.",
                en: "Assess the degree of meaningful insider or founder ownership that aligns management with long-term shareholders. Ground the judgment in disclosed ownership stakes or insider transaction signals from the supplied sources.",
            }),
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
            // Only quantitative criteria carry a DSL expression (ADR 0075);
            // qualitative criteria are agent-assessed with no expression.
            for criterion in template
                .criteria
                .iter()
                .filter(|c| c.kind == "quantitative")
            {
                let expr = parse(criterion.expression).unwrap_or_else(|e| {
                    panic!(
                        "template '{}' criterion '{}' does not parse: {e}",
                        template.name.en, criterion.label.en
                    )
                });
                assert!(
                    is_predicate(&expr),
                    "template '{}' criterion '{}' is not a predicate",
                    template.name.en,
                    criterion.label.en
                );
                if let Some(band) = criterion.partial_band {
                    parse(band).unwrap_or_else(|e| {
                        panic!("template '{}' criterion '{}' partial band '{band}' does not parse: {e}", template.name.en, criterion.label.en)
                    });
                }
            }
        }
    }

    #[test]
    fn every_qualitative_criterion_has_guidance_and_no_expression() {
        // ADR 0075 invariant: a qualitative template criterion carries an
        // owner-authored guidance seed and no DSL expression.
        for template in TEMPLATES {
            for criterion in template.criteria.iter().filter(|c| c.kind == "qualitative") {
                assert!(
                    criterion.expression.is_empty(),
                    "template '{}' qualitative criterion '{}' must have no expression",
                    template.name.en,
                    criterion.label.en
                );
                let guidance = criterion.assessment_guidance.as_ref().unwrap_or_else(|| {
                    panic!(
                        "template '{}' qualitative criterion '{}' must carry guidance",
                        template.name.en, criterion.label.en
                    )
                });
                assert!(
                    !guidance.pl.trim().is_empty() && !guidance.en.trim().is_empty(),
                    "template '{}' qualitative criterion '{}' must carry guidance in both languages",
                    template.name.en,
                    criterion.label.en
                );
            }
        }
    }

    #[test]
    fn every_localized_string_is_populated_in_both_languages() {
        // ADR 0076 Decision 8 guardrail: a bilingual template carries non-empty
        // Polish AND English text for every human-facing string, so seed/reset/
        // top-up never emit a blank in either locale.
        let non_empty = |t: &LocalizedText| !t.pl.trim().is_empty() && !t.en.trim().is_empty();
        for template in TEMPLATES {
            assert!(
                non_empty(&template.name),
                "template '{}' name must be bilingual",
                template.template_key
            );
            assert!(
                non_empty(&template.description),
                "template '{}' description must be bilingual",
                template.template_key
            );
            for criterion in template.criteria {
                assert!(
                    non_empty(&criterion.label),
                    "template '{}' criterion label must be bilingual",
                    template.template_key
                );
                if let Some(guidance) = &criterion.assessment_guidance {
                    assert!(
                        non_empty(guidance),
                        "template '{}' criterion '{}' guidance must be bilingual",
                        template.template_key,
                        criterion.label.en
                    );
                }
            }
        }
    }
}
