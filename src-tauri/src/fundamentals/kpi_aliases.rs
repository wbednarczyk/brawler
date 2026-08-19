//! Curated catalog aliases — one metric key that means exactly what another
//! already-populated key means (ADR 0100 decision 12, epic #398).
//!
//! Catalog fragmentation predates any agent: `inventory` was seeded by
//! migration `0048` and holds **0 facts**, while `inventories` (seeded by
//! `0084`) holds 771 across 44 companies. Both are canonical rows, so a writer
//! resolving `inventory` lands on a definition nothing else ever reads, and a
//! reader asking for `inventory` finds nothing — silently, because a missing
//! fact is indistinguishable from a metric the issuer never reported.
//!
//! An alias is **one-sided and evidence-proven**: the source key must hold no
//! facts, so redirecting it can never merge two real series. A pair where both
//! sides hold facts is a MERGE, which needs its own migration and its own
//! evidence — never an entry here. That asymmetry is what keeps this table
//! from becoming a quiet repaint (ADR 0077 decision 8).
//!
//! Resolution happens at the write boundary
//! (`storage::kpi_extraction::resolve_kpi_definition`) and redirects ONLY
//! while the source key's definitions hold zero facts on the database at
//! hand — the one-sidedness rule is enforced per write, not merely asserted
//! here. A database where the source key already carries a series (import,
//! older schema, manual entry) never redirects. Never applied in reverse.

/// A dead catalog key and the live key it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KpiAlias {
    /// The key to redirect. Holds no facts — asserted by the migration-safety
    /// gate, which also refuses to let a derived formula reference it.
    pub from: &'static str,
    /// The key that already carries the series.
    pub to: &'static str,
}

/// Every curated alias. Sorted by `from`; one entry per dead key.
const ALIASES: &[KpiAlias] = &[
    // `inventory` (0 facts, migration 0048) → `inventories` (771 facts / 44
    // companies, migration 0084). The ESEF crosswalk already refuses to name
    // `Inventories` as `inventory`; this closes the other direction — the
    // Polish-label and aggregator paths — and unbreaks `quick_ratio`, whose
    // seeded formula referenced the dead key and therefore never computed for
    // any company (repaired forward by migration 0147).
    // The six non-`inventory` entries are harvested from the epic #399 §G
    // live runs (ADR 0101 dec. 5): each is a key the driving agent actually
    // guessed for a mapped observation and lost a `mapping.unresolved` flag
    // to, or the propose guard should redirect. None is a seeded definition —
    // a never-seeded guessable synonym is one-sided by construction (zero
    // facts, no row).
    KpiAlias {
        from: "financial_costs",
        to: "finance_costs",
    },
    KpiAlias {
        from: "financial_income",
        to: "finance_income",
    },
    KpiAlias {
        from: "income_tax",
        to: "income_tax_expense",
    },
    // `inventory` (0 facts, migration 0048) → `inventories` (771 facts / 44
    // companies, migration 0084). The ESEF crosswalk already refuses to name
    // `Inventories` as `inventory`; this closes the other direction — the
    // Polish-label and aggregator paths — and unbreaks `quick_ratio`, whose
    // seeded formula referenced the dead key and therefore never computed for
    // any company (repaired forward by migration 0147).
    KpiAlias {
        from: "inventory",
        to: "inventories",
    },
    KpiAlias {
        from: "liabilities_total",
        to: "total_liabilities",
    },
    KpiAlias {
        from: "operating_costs",
        to: "operating_expense",
    },
    KpiAlias {
        from: "pretax_profit",
        to: "wdf_pretax_profit",
    },
];

pub fn aliases() -> &'static [KpiAlias] {
    ALIASES
}

/// The live key `metric_key` means, or `None` when it is not an alias source.
pub fn resolve(metric_key: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|alias| alias.from == metric_key)
        .map(|alias| alias.to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_are_sorted_and_unique_by_source() {
        let sources: Vec<&str> = ALIASES.iter().map(|alias| alias.from).collect();
        let mut sorted = sources.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sources, sorted,
            "aliases must be sorted and unique by `from`"
        );
    }

    /// One-sided by construction: an alias target must never itself be an
    /// alias source, or resolution would need a chain (and a cycle check).
    #[test]
    fn no_alias_target_is_itself_an_alias_source() {
        for alias in ALIASES {
            assert_eq!(
                resolve(alias.to),
                None,
                "{} is both an alias target and an alias source",
                alias.to
            );
        }
    }

    #[test]
    fn resolve_is_never_applied_in_reverse() {
        assert_eq!(resolve("inventory"), Some("inventories"));
        assert_eq!(resolve("inventories"), None);
    }
}
