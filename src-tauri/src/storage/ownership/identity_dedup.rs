//! Holder-identity dedup for a scoped stake set (one row per identity), the
//! read-model collapse behind `current_state` / `disclosed_reference_state`
//! (data-model.md § Ownership; #496: the equal-length tie is `as_of`, never
//! `created_at`).

use rusqlite::Connection;

use super::{load_holder_dictionary, OwnershipStakeRow};
use crate::storage::StorageResult;

/// Merge a scoped stake set by holder IDENTITY, keeping one row per identity.
///
/// Same-basis documents (e.g. the quarterly report and its attachment) print the
/// same holder with cosmetic name variants ("PTE Allianz Polska" vs "PTE Allianz
/// Polska S.A."), which are distinct `holder_name_normalized` identities by design
/// (append-only ids stay stable). Merge by IDENTITY:
/// a shared dictionary `display_name` when the name resolves to a seeded alias
/// ("NN PTE" and "Nationale-Nederlanden PTE S.A." are one entity), else the
/// parenthetical-stripped canonical key ("cyber_Folks S.A." vs "cyber_Folks S.A.
/// (akcje własne)"). The most specific (longest) raw name represents the merged
/// holder — it carries the richer type signal; on equal length the newer
/// disclosure (`as_of`) wins, then the larger id — never `created_at`
/// (data-model.md § Model principles, #496).
pub(in crate::storage) fn dedup_stakes_by_identity(
    connection: &Connection,
    rows: Vec<OwnershipStakeRow>,
) -> StorageResult<Vec<OwnershipStakeRow>> {
    let identity_map = crate::fundamentals::ownership::classify::HolderIdentityMap::from_pairs(
        load_holder_dictionary(connection)?
            .into_iter()
            // Generic MARKER aliases ("akcje własne") classify a row but are
            // not an entity identity — the issuer's own name is; the
            // parenthetical-strip path merges those. Entity aliases (funds,
            // state bodies) do act as identities.
            .filter(|entry| entry.holder_type != "treasury_shares")
            .map(|entry| {
                let identity = entry
                    .display_name
                    .unwrap_or_else(|| entry.alias_normalized.clone());
                (entry.alias_normalized, identity)
            }),
    );
    let mut by_identity: std::collections::BTreeMap<String, OwnershipStakeRow> =
        std::collections::BTreeMap::new();
    for row in rows {
        let key = identity_map
            .resolve(&row.holder_name_raw)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                crate::fundamentals::ownership::classify::canonical_holder_identity(
                    &row.holder_name_raw,
                )
            });
        match by_identity.get(&key) {
            Some(existing)
                if (
                    existing.holder_name_raw.len(),
                    &existing.as_of,
                    &existing.id,
                ) >= (row.holder_name_raw.len(), &row.as_of, &row.id) => {}
            _ => {
                by_identity.insert(key, row);
            }
        }
    }
    Ok(by_identity.into_values().collect())
}
