//! The const extraction-profile registry (ADR 0099 dec. 6): `profile_version`
//! = `{profile_id}@v{N}`, validated at run creation, with a per-`statement_type`
//! expected-KPI pack that joins the company's live relevance union in the
//! creation-time `expected_kpis_json` stamp.

use super::{StorageError, StorageResult};

/// Every registered profile version. Position IS the whole cutover (no `@vN`
/// parser — ADR 0102 dec. 13): `@v2` literals are listed BEFORE their `@v1`
/// sibling so `resolve_profile_version` (first-match) hands a fresh run the
/// newer version while the `@v1` string stays registered and resolvable for
/// runs that already carry it. `gpw_interim`, `nc_uor`, and
/// `company_characteristic` have no `@v2` yet and stay at `@v1`.
pub const PROFILE_VERSIONS: [&str; 7] = [
    "gpw_ifrs_annual@v2",
    "gpw_preliminary@v2",
    "gpw_ifrs_annual@v1",
    "gpw_interim@v1",
    "gpw_preliminary@v1",
    "nc_uor@v1",
    "company_characteristic@v1",
];

pub fn is_registered_profile_version(profile_version: &str) -> bool {
    PROFILE_VERSIONS.contains(&profile_version)
}

/// `profile_id` → its CURRENT `{id}@v{N}` string (the contract's `profileId`
/// input maps through this). `None` for an unknown id.
pub fn resolve_profile_version(profile_id: &str) -> Option<&'static str> {
    PROFILE_VERSIONS.iter().copied().find(|v| {
        v.strip_prefix(profile_id)
            .is_some_and(|rest| rest.starts_with("@v"))
    })
}

// Per-statement_type full floors: the type's core keys (banking loses
// `revenue`/`operating_profit` per the measured #284 evidence) plus the type's
// layer-2 statement-pack keys — identical content to what relevance seeding
// gives a fresh company, so for a well-seeded company the union stamp is a
// no-op; the pack's job is to hold the floor when relevance rows are absent.
// Lockstep with the financials consts is test-enforced below.
const FLOOR_INDUSTRIAL: &[&str] = &[
    "net_profit",
    "operating_profit",
    "revenue",
    "total_assets",
    "total_equity",
];
const FLOOR_BANKING: &[&str] = &[
    "net_fee_commission_income",
    "net_interest_income",
    "net_profit",
    "total_assets",
    "total_deposits",
    "total_equity",
    "total_loans",
];
const FLOOR_INSURANCE: &[&str] = &[
    "gross_insurance_revenue",
    "net_profit",
    "operating_profit",
    "revenue",
    "total_assets",
    "total_equity",
];
const FLOOR_SPECIALTY_FINANCE: &[&str] = &[
    "cash_ebitda",
    "erc",
    "net_profit",
    "operating_profit",
    "portfolio_purchases",
    "recoveries",
    "revenue",
    "total_assets",
    "total_equity",
];
const FLOOR_REIT: &[&str] = &[
    "ffo",
    "net_profit",
    "operating_profit",
    "revenue",
    "total_assets",
    "total_equity",
];

// A GPW preliminary/estimate report carries headline figures only; a bank's
// preliminary has no revenue line at all (#284).
const PRELIMINARY: &[&str] = &["net_profit", "revenue"];
const PRELIMINARY_BANKING: &[&str] = &["net_profit"];

/// The profile's expected-KPI pack for one `statement_type`. An unknown
/// statement type falls back to the industrial floor (mirroring
/// `get_statement_type`'s `'industrial'` default and the seeding join).
///
/// `profile_version` here is the user-input validation route (ADR 0102 dec.
/// 13): the only production caller, `create_run_if_absent`, checks
/// `is_registered_profile_version` immediately before calling this, so an
/// unregistered version is caller input, never a silent full-floor fallback
/// (no `_` catch-all — a future `@v2` must be given its own arm, not inherit
/// this one's).
pub fn expected_pack(
    profile_version: &str,
    statement_type: &str,
) -> StorageResult<&'static [&'static str]> {
    let pack = match profile_version {
        "company_characteristic@v1" => &[],
        // Pack stays minimal at v2 (ADR 0102 dec. 13): the denominator is not
        // the capture-doctrine axis — reuse v1's pack unchanged.
        "gpw_preliminary@v2" | "gpw_preliminary@v1" => {
            if statement_type == "banking" {
                PRELIMINARY_BANKING
            } else {
                PRELIMINARY
            }
        }
        "gpw_ifrs_annual@v2" | "gpw_ifrs_annual@v1" | "gpw_interim@v1" | "nc_uor@v1" => {
            match statement_type {
                "banking" => FLOOR_BANKING,
                "insurance" => FLOOR_INSURANCE,
                "specialty_finance" => FLOOR_SPECIALTY_FINANCE,
                "reit" => FLOOR_REIT,
                _ => FLOOR_INDUSTRIAL, // industrial | brokerage | unknown
            }
        }
        other => {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "profile_version",
                value: other.to_owned(),
            })
        }
    };
    Ok(pack)
}

/// The profile's extraction doctrine (#385): short imperative rules the MCP
/// context read model serves verbatim as `profileRules`. Content is VERSIONED
/// BEHAVIOR — a semantic change to any rule requires a new `@vN` profile
/// version (the golden test freezes full equality per version); each rule is
/// ≤512 UTF-8 bytes (contracts.md § Budgets).
///
/// `profile_version` here is the stored-value route (ADR 0102 dec. 13): the
/// only production caller reads it back off an existing run's row without
/// re-validation, so an unregistered version reaching this function is
/// invariant corruption, not caller input — a typed
/// [`StorageError::UnknownKpiIngestProfileVersion`], never a silent empty
/// doctrine (no `_` catch-all).
pub fn profile_rules(profile_version: &str) -> StorageResult<&'static [&'static str]> {
    let rules: &'static [&'static str] = match profile_version {
        "gpw_ifrs_annual@v2" => &[
            "Prefer the consolidated statements; use standalone only when the report has no consolidated section, and set scope accordingly.",
            "Map every disclosed line to a catalog metric key, propose one with propose_kpi_definition, or stage it excluded with a non-blank exclusionReason — never invent a metric key and never leave a disclosed number silently absent.",
            "net_profit and per-share figures are owners-of-parent when the report splits non-controlling interests; otherwise total.",
            "Annual income and cash-flow lines are full-year flows; balance-sheet lines are point_in_time at period end.",
            "Preserve rawValue exactly as printed and set unitScale from the statement header; normalizedValue is the value in base units (rawValue scaled by unitScale).",
            "Cite every value: page plus table or row, with a short quote.",
        ],
        "gpw_preliminary@v2" => &[
            "Capture every disclosed number: map it to a catalog metric key, propose one with propose_kpi_definition, or stage it excluded with a non-blank exclusionReason — never invent a metric key and never leave a disclosed number silently absent.",
            "Start or resume the run with dataQuality=preliminary before staging; the final report supersedes these values.",
            "Confirm scope from the report wording and cite the exact sentence.",
        ],
        "gpw_ifrs_annual@v1" => &[
            "Prefer the consolidated statements; use standalone only when the report has no consolidated section, and set scope accordingly.",
            "Map statement lines to catalog metric keys; stage rows you cannot map with mappingStatus=unmapped — never invent a metric key.",
            "net_profit and per-share figures are owners-of-parent when the report splits non-controlling interests; otherwise total.",
            "Annual income and cash-flow lines are full-year flows; balance-sheet lines are point_in_time at period end.",
            "Preserve rawValue exactly as printed and set unitScale from the statement header; normalizedValue is the value in base units (rawValue scaled by unitScale).",
            "Cite every value: page plus table or row, with a short quote.",
        ],
        "gpw_interim@v1" => &[
            "Stage the period the run declares; pick the report column matching that window — never derive quarters by subtraction.",
            "Interim income and cash-flow figures are year-to-date: stage them with measureWindow=cumulative against the declared period; balance-sheet lines are point_in_time at the interim date.",
            "Prefer consolidated; owners-of-parent attribution when NCI is split.",
            "Set unitScale from the statement header and cite page plus table for every value.",
        ],
        "gpw_preliminary@v1" => &[
            "Preliminary reports carry only headline figures: stage revenue and net_profit only (banking: net_profit only); do not mine full statements.",
            "Start or resume the run with dataQuality=preliminary before staging; the final report supersedes these values.",
            "Confirm scope from the report wording and cite the exact sentence.",
        ],
        "nc_uor@v1" => &[
            "NewConnect UoR reports use Polish accounting-act vocabulary; map lines to the catalog by meaning, not literal translation.",
            "UoR statements are usually standalone; set scope=standalone unless a consolidated section exists.",
            "Amounts are commonly thousands of PLN — read the header and set unitScale accordingly.",
            "Use missingReasons for expected keys the report genuinely lacks.",
        ],
        "company_characteristic@v1" => &[
            "This profile records durable company characteristics, not periodic KPIs; stage point_in_time values only.",
            "There is no expected pack: stage exactly what the document states, with precise citations.",
        ],
        other => {
            return Err(StorageError::UnknownKpiIngestProfileVersion {
                value: other.to_owned(),
            })
        }
    };
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::storage::financials::{
        BANKING_EXCLUDED_CORE_KEYS, CORE_KPI_METRIC_KEYS, STATEMENT_PACK_METRIC_KEYS,
    };
    use crate::storage::migrations::open_in_memory_database;

    const STATEMENT_TYPES: [&str; 6] = [
        "industrial",
        "banking",
        "insurance",
        "specialty_finance",
        "brokerage",
        "reit",
    ];

    fn set(keys: &[&str]) -> BTreeSet<&'static str> {
        keys.iter()
            .map(|k| {
                // Round-trip through the static tables so the sets borrow 'static.
                *CORE_KPI_METRIC_KEYS
                    .iter()
                    .chain(STATEMENT_PACK_METRIC_KEYS.iter())
                    .find(|s| **s == *k)
                    .unwrap_or_else(|| panic!("pack key {k} missing from the financials consts"))
            })
            .collect()
    }

    /// The full floor is derivable from the financials consts — the registry
    /// and the relevance-seeding layer can never drift apart silently.
    #[test]
    fn full_floors_are_in_lockstep_with_the_financials_constants() {
        let pack_slice = |statement_type: &str| -> BTreeSet<&str> {
            STATEMENT_PACK_METRIC_KEYS
                .iter()
                .copied()
                .filter(|k| match statement_type {
                    "banking" => {
                        matches!(
                            *k,
                            "net_interest_income"
                                | "net_fee_commission_income"
                                | "total_loans"
                                | "total_deposits"
                        )
                    }
                    "insurance" => *k == "gross_insurance_revenue",
                    "reit" => *k == "ffo",
                    "specialty_finance" => {
                        matches!(
                            *k,
                            "recoveries" | "erc" | "cash_ebitda" | "portfolio_purchases"
                        )
                    }
                    _ => false,
                })
                .collect()
        };
        for statement_type in STATEMENT_TYPES {
            let mut expected: BTreeSet<&str> = CORE_KPI_METRIC_KEYS.iter().copied().collect();
            if statement_type == "banking" {
                for k in BANKING_EXCLUDED_CORE_KEYS {
                    expected.remove(k);
                }
            }
            expected.extend(pack_slice(statement_type));
            let actual: BTreeSet<&str> = expected_pack("gpw_ifrs_annual@v1", statement_type)
                .expect("registered profile")
                .iter()
                .copied()
                .collect();
            assert_eq!(actual, expected, "full floor for {statement_type}");
        }
    }

    /// The exact 5×6 matrix, frozen table-driven — an implementation
    /// special-casing any pair fails here (plus the unknown-type fallback).
    #[test]
    fn the_whole_registry_matrix_is_frozen() {
        let full = |t: &str| match t {
            "banking" => set(FLOOR_BANKING),
            "insurance" => set(FLOOR_INSURANCE),
            "specialty_finance" => set(FLOOR_SPECIALTY_FINANCE),
            "reit" => set(FLOOR_REIT),
            _ => set(FLOOR_INDUSTRIAL),
        };
        for statement_type in STATEMENT_TYPES {
            for profile in ["gpw_ifrs_annual@v1", "gpw_interim@v1", "nc_uor@v1"] {
                assert_eq!(
                    set(expected_pack(profile, statement_type).expect("registered profile")),
                    full(statement_type),
                    "{profile} × {statement_type}"
                );
            }
            let preliminary =
                expected_pack("gpw_preliminary@v1", statement_type).expect("registered profile");
            if statement_type == "banking" {
                assert_eq!(preliminary, PRELIMINARY_BANKING);
            } else {
                assert_eq!(preliminary, PRELIMINARY);
            }
            assert!(expected_pack("company_characteristic@v1", statement_type)
                .expect("registered profile")
                .is_empty());
        }
        for profile in PROFILE_VERSIONS {
            assert_eq!(
                expected_pack(profile, "future_unknown").expect("registered profile"),
                expected_pack(profile, "industrial").expect("registered profile"),
                "unknown statement_type falls back to the industrial floor"
            );
        }
    }

    /// Every pack key names a definition the migrations actually seed — a pack
    /// can never demand a metric key that does not exist.
    #[test]
    fn every_pack_key_exists_in_seeded_kpi_definitions() {
        let connection = open_in_memory_database().expect("db");
        let mut statement = connection
            .prepare("SELECT metric_key FROM kpi_definitions")
            .expect("prepare");
        let seeded: BTreeSet<String> = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        for profile in PROFILE_VERSIONS {
            for statement_type in STATEMENT_TYPES {
                for key in expected_pack(profile, statement_type).expect("registered profile") {
                    assert!(
                        seeded.contains(*key),
                        "{profile} × {statement_type}: pack key {key} not in seeded kpi_definitions"
                    );
                }
            }
        }
    }

    /// `resolve_profile_version("gpw_ifrs_annual")` now yields `@v2` — the
    /// ADR 0102 dec. 13 positional cutover, not a leftover `@v1` id (that
    /// exact fact is `fresh_run_resolves_v2_v1_still_resolvable` above).
    #[test]
    fn resolve_profile_version_maps_ids_and_rejects_unknown_and_partial() {
        assert_eq!(
            resolve_profile_version("gpw_ifrs_annual"),
            Some("gpw_ifrs_annual@v2")
        );
        assert_eq!(
            resolve_profile_version("company_characteristic"),
            Some("company_characteristic@v1")
        );
        assert_eq!(resolve_profile_version("gpw_ifrs"), None);
        assert_eq!(resolve_profile_version("unknown_profile"), None);
        assert_eq!(resolve_profile_version(""), None);
    }

    /// `@v2` is registered membership now (ADR 0102 dec. 13 cutover); the
    /// stale/future/bare checks move to `@v0`/`@v3` so they keep exercising
    /// "not every version string is a member" without contradicting cutover.
    #[test]
    fn membership_rejects_stale_future_and_bare_versions() {
        assert!(is_registered_profile_version("gpw_ifrs_annual@v1"));
        assert!(is_registered_profile_version("gpw_ifrs_annual@v2"));
        assert!(!is_registered_profile_version("gpw_ifrs_annual@v0"));
        assert!(!is_registered_profile_version("gpw_ifrs_annual@v3"));
        assert!(!is_registered_profile_version("gpw_ifrs_annual"));
        assert!(!is_registered_profile_version("p1"));
    }

    /// An unregistered profile version must never silently resolve to the
    /// FULL floor pack or an empty doctrine (ADR 0102 dec. 13 — `@v2` must not
    /// inherit either by falling through a catch-all). Two typed error
    /// classes, one per call route (sol finding 8): `expected_pack` serves
    /// the user-input route (validated at run creation) and returns the
    /// existing `InvalidKpiIngestRunValue`; `profile_rules` serves the
    /// stored-value route (read back from an existing run's row without
    /// re-validation) and returns `UnknownKpiIngestProfileVersion` — an
    /// unknown value there is invariant corruption, not bad input.
    #[test]
    fn unregistered_version_is_typed_error_never_floors() {
        match expected_pack("gpw_ifrs_annual@v99", "industrial") {
            Err(StorageError::InvalidKpiIngestRunValue { key, value }) => {
                assert_eq!(key, "profile_version");
                assert_eq!(value, "gpw_ifrs_annual@v99");
            }
            other => panic!("expected InvalidKpiIngestRunValue, got {other:?}"),
        }

        match profile_rules("gpw_ifrs_annual@v99") {
            Err(StorageError::UnknownKpiIngestProfileVersion { value }) => {
                assert_eq!(value, "gpw_ifrs_annual@v99");
            }
            other => panic!("expected UnknownKpiIngestProfileVersion, got {other:?}"),
        }
    }

    /// Doctrine repair is versioned (ADR 0102 dec. 13): `gpw_preliminary@v2`
    /// and `gpw_ifrs_annual@v2` carry the map/propose/exclude full-capture
    /// rule; full-equality goldens per version, same discipline as the `@v1`
    /// goldens below.
    #[test]
    fn profile_rules_v2_are_frozen_verbatim() {
        assert_eq!(
            profile_rules("gpw_preliminary@v2").expect("registered profile"),
            &[
                "Capture every disclosed number: map it to a catalog metric key, propose one with propose_kpi_definition, or stage it excluded with a non-blank exclusionReason — never invent a metric key and never leave a disclosed number silently absent.",
                "Start or resume the run with dataQuality=preliminary before staging; the final report supersedes these values.",
                "Confirm scope from the report wording and cite the exact sentence.",
            ]
        );
        assert_eq!(
            profile_rules("gpw_ifrs_annual@v2").expect("registered profile"),
            &[
                "Prefer the consolidated statements; use standalone only when the report has no consolidated section, and set scope accordingly.",
                "Map every disclosed line to a catalog metric key, propose one with propose_kpi_definition, or stage it excluded with a non-blank exclusionReason — never invent a metric key and never leave a disclosed number silently absent.",
                "net_profit and per-share figures are owners-of-parent when the report splits non-controlling interests; otherwise total.",
                "Annual income and cash-flow lines are full-year flows; balance-sheet lines are point_in_time at period end.",
                "Preserve rawValue exactly as printed and set unitScale from the statement header; normalizedValue is the value in base units (rawValue scaled by unitScale).",
                "Cite every value: page plus table or row, with a short quote.",
            ]
        );
    }

    /// Positional cutover (ADR 0102 dec. 13): a fresh run resolving the base
    /// `profile_id` gets `@v2` for the two profiles this slice versions; an
    /// explicit `@v1` id stays resolvable/registered exactly. Profiles this
    /// slice does not touch (`gpw_interim`, `nc_uor`, `company_characteristic`)
    /// still resolve to `@v1` — no accidental cutover beyond the two named.
    #[test]
    fn fresh_run_resolves_v2_v1_still_resolvable() {
        assert_eq!(
            resolve_profile_version("gpw_preliminary"),
            Some("gpw_preliminary@v2")
        );
        assert_eq!(
            resolve_profile_version("gpw_ifrs_annual"),
            Some("gpw_ifrs_annual@v2")
        );
        assert!(is_registered_profile_version("gpw_preliminary@v1"));
        assert!(is_registered_profile_version("gpw_ifrs_annual@v1"));
        assert!(is_registered_profile_version("gpw_preliminary@v2"));
        assert!(is_registered_profile_version("gpw_ifrs_annual@v2"));
        assert!(profile_rules("gpw_preliminary@v1").is_ok());
        assert!(profile_rules("gpw_ifrs_annual@v1").is_ok());

        assert_eq!(
            resolve_profile_version("gpw_interim"),
            Some("gpw_interim@v1")
        );
        assert_eq!(resolve_profile_version("nc_uor"), Some("nc_uor@v1"));
        assert_eq!(
            resolve_profile_version("company_characteristic"),
            Some("company_characteristic@v1")
        );
    }

    /// The doctrine is versioned behavior (#385): full-equality goldens per
    /// `@v1` profile — a silent wording change that shifts extraction meaning
    /// fails here; a deliberate one requires a new profile version.
    #[test]
    fn profile_rules_v1_are_frozen_verbatim() {
        assert_eq!(
            profile_rules("gpw_ifrs_annual@v1").expect("registered profile"),
            &[
                "Prefer the consolidated statements; use standalone only when the report has no consolidated section, and set scope accordingly.",
                "Map statement lines to catalog metric keys; stage rows you cannot map with mappingStatus=unmapped — never invent a metric key.",
                "net_profit and per-share figures are owners-of-parent when the report splits non-controlling interests; otherwise total.",
                "Annual income and cash-flow lines are full-year flows; balance-sheet lines are point_in_time at period end.",
                "Preserve rawValue exactly as printed and set unitScale from the statement header; normalizedValue is the value in base units (rawValue scaled by unitScale).",
                "Cite every value: page plus table or row, with a short quote.",
            ]
        );
        assert_eq!(
            profile_rules("gpw_interim@v1").expect("registered profile"),
            &[
                "Stage the period the run declares; pick the report column matching that window — never derive quarters by subtraction.",
                "Interim income and cash-flow figures are year-to-date: stage them with measureWindow=cumulative against the declared period; balance-sheet lines are point_in_time at the interim date.",
                "Prefer consolidated; owners-of-parent attribution when NCI is split.",
                "Set unitScale from the statement header and cite page plus table for every value.",
            ]
        );
        assert_eq!(
            profile_rules("gpw_preliminary@v1").expect("registered profile"),
            &[
                "Preliminary reports carry only headline figures: stage revenue and net_profit only (banking: net_profit only); do not mine full statements.",
                "Start or resume the run with dataQuality=preliminary before staging; the final report supersedes these values.",
                "Confirm scope from the report wording and cite the exact sentence.",
            ]
        );
        assert_eq!(
            profile_rules("nc_uor@v1").expect("registered profile"),
            &[
                "NewConnect UoR reports use Polish accounting-act vocabulary; map lines to the catalog by meaning, not literal translation.",
                "UoR statements are usually standalone; set scope=standalone unless a consolidated section exists.",
                "Amounts are commonly thousands of PLN — read the header and set unitScale accordingly.",
                "Use missingReasons for expected keys the report genuinely lacks.",
            ]
        );
        assert_eq!(
            profile_rules("company_characteristic@v1").expect("registered profile"),
            &[
                "This profile records durable company characteristics, not periodic KPIs; stage point_in_time values only.",
                "There is no expected pack: stage exactly what the document states, with precise citations.",
            ]
        );
    }

    /// Every registered profile has non-empty doctrine, every rule fits the
    /// frozen ≤512-byte budget (contracts.md § Budgets), and an unregistered
    /// version is a typed error, never a silently empty doctrine.
    #[test]
    fn profile_rules_are_present_and_byte_bounded() {
        for version in PROFILE_VERSIONS {
            let rules = profile_rules(version).expect("registered profile");
            assert!(!rules.is_empty(), "{version} must carry doctrine");
            for rule in rules {
                assert!(
                    rule.len() <= 512,
                    "{version} rule exceeds 512 bytes: {rule}"
                );
            }
        }
        assert!(matches!(
            profile_rules("gpw_ifrs_annual@v99"),
            Err(StorageError::UnknownKpiIngestProfileVersion { .. })
        ));
    }
}
