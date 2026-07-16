//! Deterministic holder-type classification (v0.56 T5, ADR 0072 §3).
//!
//! Classification order for a holder's `holder_type`:
//! 1. **Dictionary** — the seeded `ownership_holder_dictionary` (TFI/OFE/state
//!    registries), matched on a *canonical key* two ways: an exact alias hit, or
//!    a containment hit (a canonical alias appearing as a whole-token run inside
//!    the holder's canonical key, **longest-alias-wins**).
//! 2. **Heuristic name markers** — an unambiguous type signal carried by the name
//!    itself when the dictionary misses (`OFE`/"otwarty fundusz emerytalny" →
//!    `ofe_pension`; `TFI` → `tfi`; a name beginning `FUNDACJA` → `family_foundation`;
//!    "akcje własne" → `treasury_shares`; "skarb państwa" → `state_treasury`).
//! 3. Everything else stays unclassified (NULL) for AI classify-with-confirm.
//!
//! This module is **pure and storage-agnostic**: the dictionary is passed in as
//! (alias, holder_type) pairs, so `storage::ownership` owns the DB read and this
//! module owns only the matching logic. [`canonical_holder_key`] is deterministic
//! and idempotent (`f(f(x)) == f(x)`), so it is safe to fold into a stable id.

/// Fold a Polish diacritic to its ASCII base **for matching only** (the raw and
/// normalized names are stored unchanged). Non-diacritic characters pass through.
fn fold_diacritic(c: char) -> char {
    match c {
        'ą' | 'Ą' => 'A',
        'ć' | 'Ć' => 'C',
        'ę' | 'Ę' => 'E',
        'ł' | 'Ł' => 'L',
        'ń' | 'Ń' => 'N',
        'ó' | 'Ó' => 'O',
        'ś' | 'Ś' => 'S',
        'ż' | 'Ż' | 'ź' | 'Ź' => 'Z',
        other => other,
    }
}

/// Legal-form suffixes/prefixes stripped from a canonical key. Type-signalling
/// forms (TFI/OFE/PTE/DFE/OFE/FIZ/SFIO) are deliberately NOT here — they are the
/// classification signal. Each is matched as a whole-token run (space-padded), so
/// `SA` never bites `VISA` and `AG` never bites a longer token.
const LEGAL_FORMS: &[&str] = &[
    "SPOLKA AKCYJNA",
    "SPOLKA KOMANDYTOWO AKCYJNA",
    "SPOLKA KOMANDYTOWA",
    "SPOLKA Z OGRANICZONA ODPOWIEDZIALNOSCIA",
    "SP Z O O",
    "SP ZOO",
    "SPZOO",
    "SP K",
    "SPK",
    "S A R L",
    "SARL",
    "S A",
    "SA",
    "S E",
    "SE",
    "GMBH",
    "N V",
    "NV",
    "PLC",
    "LTD",
    "INC",
    "AG",
];

/// Strip legal-form tokens from a space-collapsed uppercase string, iterating to a
/// fixed point so a second pass over the result is a no-op (idempotence).
fn strip_legal_forms(collapsed: &str) -> String {
    let mut padded = format!(" {collapsed} ");
    loop {
        let mut changed = false;
        for form in LEGAL_FORMS {
            let needle = format!(" {form} ");
            while let Some(pos) = padded.find(&needle) {
                // Replace the run but keep one boundary space so adjacent tokens
                // stay separated for the next match.
                padded.replace_range(pos..pos + needle.len(), " ");
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    padded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Build the canonical matching key for a holder name: fold Polish diacritics,
/// map every non-alphanumeric character to a separator, uppercase, collapse
/// whitespace, and strip legal-form suffixes/prefixes. Pure, deterministic, and
/// idempotent. Operates purely on `char`s, so it never panics on any input.
pub fn canonical_holder_key(name: &str) -> String {
    let mut cleaned = String::with_capacity(name.len());
    for c in name.chars() {
        let folded = fold_diacritic(c);
        if folded.is_alphanumeric() {
            for upper in folded.to_uppercase() {
                cleaned.push(upper);
            }
        } else {
            cleaned.push(' ');
        }
    }
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    strip_legal_forms(&collapsed)
}

/// Whether `alias` appears as a whole-token run inside `key` (both canonical).
fn contains_token_run(key: &str, alias: &str) -> bool {
    format!(" {key} ").contains(&format!(" {alias} "))
}

/// Remove parenthesized qualifiers — "(akcje własne)", "(dawniej R22 S.A.)",
/// "(zarządzane fundusze łącznie)" — including an unbalanced trailing "(...".
/// Qualifiers describe a holder; they are not part of its identity.
fn strip_parentheticals(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0usize;
    for c in name.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// The IDENTITY key for a holder row: the canonical key of the name with
/// parenthesized qualifiers removed. Used by the current-state read model to
/// merge cosmetic variants of one holder ("cyber_Folks S.A." vs "cyber_Folks
/// S.A. (akcje własne)") while `canonical_holder_key` — which keeps qualifier
/// tokens — stays the CLASSIFICATION key (the treasury heuristic needs "akcje
/// własne" to survive). Owner dogfooding 2026-07-16.
pub fn canonical_holder_identity(name: &str) -> String {
    canonical_holder_key(&strip_parentheticals(name))
}

/// Alias → shared-identity resolver built from the seeded dictionary rows: two
/// disclosure spellings of one entity ("NN PTE" and "Nationale-Nederlanden PTE
/// S.A.") resolve to the same identity when their dictionary entries share a
/// `display_name`. Same canonical key space and longest-alias-wins containment
/// as [`HolderDictionary`].
#[derive(Debug, Clone)]
pub struct HolderIdentityMap {
    /// `(canonical_alias, identity)`, sorted by alias token-length desc then text.
    entries: Vec<(String, String)>,
}

impl HolderIdentityMap {
    pub fn from_pairs<I, S>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (S, S)>,
        S: AsRef<str>,
    {
        let mut entries: Vec<(String, String)> = pairs
            .into_iter()
            .map(|(alias, identity)| {
                (
                    canonical_holder_key(alias.as_ref()),
                    identity.as_ref().to_owned(),
                )
            })
            .filter(|(alias, _)| !alias.is_empty())
            .collect();
        entries.sort_by(|a, b| {
            let tokens = |s: &str| s.split_whitespace().count();
            tokens(&b.0)
                .cmp(&tokens(&a.0))
                .then_with(|| a.0.len().cmp(&b.0.len()).reverse())
                .then_with(|| a.0.cmp(&b.0))
        });
        Self { entries }
    }

    /// Resolve a holder name to its shared identity, if any dictionary alias
    /// matches exactly or as a whole-token run (longest alias wins).
    pub fn resolve(&self, name: &str) -> Option<&str> {
        let key = canonical_holder_key(name);
        if key.is_empty() {
            return None;
        }
        self.entries
            .iter()
            .find(|(alias, _)| alias == &key || contains_token_run(&key, alias))
            .map(|(_, identity)| identity.as_str())
    }
}

/// A loaded holder dictionary: canonical aliases sorted longest-first so a
/// containment match resolves to the most specific alias (deterministic tie-break
/// by alias text). Built once per classification pass from the seeded rows.
#[derive(Debug, Clone)]
pub struct HolderDictionary {
    /// `(canonical_alias, holder_type)`, sorted by alias token-length desc then
    /// alias text asc.
    entries: Vec<(String, String)>,
}

impl HolderDictionary {
    /// Build from `(alias, holder_type)` pairs (the seeded dictionary rows). Aliases
    /// are canonicalized to the same key space as the holder names they match.
    pub fn from_aliases<I, S>(aliases: I) -> Self
    where
        I: IntoIterator<Item = (S, S)>,
        S: AsRef<str>,
    {
        let mut entries: Vec<(String, String)> = aliases
            .into_iter()
            .map(|(alias, holder_type)| {
                (
                    canonical_holder_key(alias.as_ref()),
                    holder_type.as_ref().to_owned(),
                )
            })
            .filter(|(alias, _)| !alias.is_empty())
            .collect();
        // Longest alias first (by whole-token length), then alphabetical, so
        // containment matches are deterministic and prefer the most specific.
        entries.sort_by(|a, b| {
            b.0.split_whitespace()
                .count()
                .cmp(&a.0.split_whitespace().count())
                .then_with(|| b.0.chars().count().cmp(&a.0.chars().count()))
                .then_with(|| a.0.cmp(&b.0))
        });
        Self { entries }
    }

    /// Classify a holder name via the dictionary: exact alias hit first, then the
    /// longest containment hit. `None` when no alias matches.
    pub fn classify(&self, name: &str) -> Option<String> {
        let key = canonical_holder_key(name);
        if key.is_empty() {
            return None;
        }
        for (alias, holder_type) in &self.entries {
            if *alias == key {
                return Some(holder_type.clone());
            }
        }
        for (alias, holder_type) in &self.entries {
            if contains_token_run(&key, alias) {
                return Some(holder_type.clone());
            }
        }
        None
    }
}

/// Heuristic type hint from the holder name itself, for a dictionary miss that
/// still carries an **unambiguous** marker. Deterministic and documented (ADR
/// 0072 §3); anything without a clear marker returns `None` and stays NULL for AI.
pub fn heuristic_holder_type(name: &str) -> Option<&'static str> {
    let key = canonical_holder_key(name);
    if key.is_empty() {
        return None;
    }
    let padded = format!(" {key} ");
    // Open pension fund: the `OFE` token or the fully spelled-out Polish form.
    if padded.contains(" OFE ") || key.contains("OTWARTY FUNDUSZ EMERYTALNY") {
        return Some("ofe_pension");
    }
    // Pension-fund manager (PTE) holding on behalf of its funds — disclosure
    // tables routinely name the PTE for the aggregated OFE/DFE stakes (owner
    // dogfooding 2026-07-16: "NN PTE" sat unclassified).
    if padded.contains(" PTE ") || key.contains("POWSZECHNE TOWARZYSTWO EMERYTALNE") {
        return Some("ofe_pension");
    }
    // Mutual-fund manager: the `TFI` token or the spelled-out form.
    if padded.contains(" TFI ") || key.contains("TOWARZYSTWO FUNDUSZY INWESTYCYJNYCH") {
        return Some("tfi");
    }
    // A private foundation vehicle holding shares of a listed issuer is, in the
    // register context, a founder/family holding vehicle (covers "Fundacja
    // Rodzinna X" and named foundations like "Fundacja Semper Simul").
    if key == "FUNDACJA" || key.starts_with("FUNDACJA ") {
        return Some("family_foundation");
    }
    // Company's own (treasury) shares — only when explicitly marked.
    if key.contains("AKCJE WLASNE") {
        return Some("treasury_shares");
    }
    // The State Treasury.
    if key.contains("SKARB PANSTWA") {
        return Some("state_treasury");
    }
    None
}

/// Full deterministic classification: dictionary first, then heuristic markers.
/// `None` means the holder is left unclassified for AI classify-with-confirm.
pub fn classify_holder(dictionary: &HolderDictionary, name: &str) -> Option<String> {
    dictionary
        .classify(name)
        .or_else(|| heuristic_holder_type(name).map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The seeded starter dictionary, mirrored from migration 0082 (subset that
    /// the real-name assertions exercise). Storage builds the real matcher from
    /// `load_holder_dictionary`; here we exercise the pure matcher directly.
    fn seeded_dictionary() -> HolderDictionary {
        HolderDictionary::from_aliases([
            ("PKO TFI", "tfi"),
            ("QUERCUS TFI", "tfi"),
            ("NATIONALE-NEDERLANDEN OFE", "ofe_pension"),
            ("ALLIANZ POLSKA OFE", "ofe_pension"),
            ("OFE PZU ZŁOTA JESIEŃ", "ofe_pension"),
            ("PKO BP BANKOWY OFE", "ofe_pension"),
            ("SKARB PAŃSTWA", "state_treasury"),
            ("AKCJE WŁASNE", "treasury_shares"),
        ])
    }

    fn classify(name: &str) -> Option<String> {
        classify_holder(&seeded_dictionary(), name)
    }

    #[test]
    fn dictionary_exact_and_containment_hits_on_real_names() {
        assert_eq!(
            classify("Nationale-Nederlanden OFE*").as_deref(),
            Some("ofe_pension"),
            "containment: alias inside decorated real name"
        );
        assert_eq!(
            classify("OFE PZU „Złota Jesień”").as_deref(),
            Some("ofe_pension"),
            "diacritics + quotes fold to the seeded alias"
        );
        assert_eq!(
            classify("Skarb Państwa Rzeczypospolitej Polskiej").as_deref(),
            Some("state_treasury"),
            "containment: SKARB PAŃSTWA inside the full legal name"
        );
        assert_eq!(
            classify("PKO BP Bankowy OFE").as_deref(),
            Some("ofe_pension")
        );
    }

    #[test]
    fn spelled_out_ofe_classifies_via_heuristic() {
        // The seeded alias is the abbreviated "ALLIANZ POLSKA OFE"; the disclosure
        // spells it out. The heuristic OFE marker still resolves it.
        assert_eq!(
            classify("Allianz Polska Otwarty Fundusz Emerytalny").as_deref(),
            Some("ofe_pension")
        );
    }

    #[test]
    fn family_foundation_via_heuristic() {
        assert_eq!(
            classify("Fundacja Semper Simul").as_deref(),
            Some("family_foundation")
        );
        assert_eq!(
            classify("Fundacja Rodzinna Kowalskiego").as_deref(),
            Some("family_foundation")
        );
    }

    #[test]
    fn plain_issuer_name_never_auto_classifies() {
        // cyber_Folks S.A. is the issuer, not a treasury/holder-type marker.
        assert_eq!(
            classify("cyber_Folks S.A."),
            None,
            "a plain issuer name must stay NULL for AI"
        );
    }

    #[test]
    fn treasury_only_when_marked_akcje_wlasne() {
        assert_eq!(classify("Akcje własne").as_deref(), Some("treasury_shares"));
        assert_eq!(
            classify("cyber_Folks S.A."),
            None,
            "issuer name must NOT classify as treasury"
        );
    }

    #[test]
    fn foreign_entity_stays_null_for_ai() {
        assert_eq!(
            classify("ULTRO S.a.r.l."),
            None,
            "unknown foreign entity is left for AI classify-with-confirm"
        );
    }

    #[test]
    fn canonical_key_strips_legal_forms_and_folds_diacritics() {
        assert_eq!(canonical_holder_key("cyber_Folks S.A."), "CYBER FOLKS");
        assert_eq!(canonical_holder_key("ULTRO S.a.r.l."), "ULTRO");
        assert_eq!(
            canonical_holder_key("OFE PZU „Złota Jesień”"),
            "OFE PZU ZLOTA JESIEN"
        );
        assert_eq!(
            canonical_holder_key("Spółka Akcyjna Grupa Azoty"),
            "GRUPA AZOTY"
        );
    }

    #[test]
    fn longest_alias_wins_on_containment() {
        let dict = HolderDictionary::from_aliases([
            ("PZU", "other_institutional"),
            ("OFE PZU ZŁOTA JESIEŃ", "ofe_pension"),
        ]);
        assert_eq!(
            dict.classify("OFE PZU „Złota Jesień”").as_deref(),
            Some("ofe_pension"),
            "the longer, more specific alias must win"
        );
    }

    proptest! {
        #[test]
        fn canonical_key_is_idempotent(input in ".*") {
            let once = canonical_holder_key(&input);
            let twice = canonical_holder_key(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn canonical_key_never_panics(input in ".*") {
            let _ = canonical_holder_key(&input);
        }

        #[test]
        fn classify_never_panics(input in ".*") {
            let dict = HolderDictionary::from_aliases([("PKO TFI", "tfi")]);
            let _ = classify_holder(&dict, &input);
        }
    }
}
