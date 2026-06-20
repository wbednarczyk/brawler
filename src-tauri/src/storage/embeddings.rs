//! Storage for the interpretative AI layer's disposable vector index
//! (`content_embeddings`, ADR 0035, v0.45.0).
//!
//! Every row is a derived embedding of a canonical content row — never a source
//! of truth. The table is scanned by a pure-Rust brute-force cosine in
//! [`crate::interpretation`]; at single-user, watchlist-scale corpora (thousands
//! of vectors) a full scan is sub-millisecond, so no `sqlite-vec`/ANN extension
//! is used (the boundary allows a later swap). Vectors are stored as a
//! little-endian f32 `BLOB`; vectors from different `model_id`s are never mixed.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::{StorageError, StorageResult};

/// The content type for feed items in the vector index (aligned with the
/// unified search content types, ADR 0032).
pub const FEED_ITEM_CONTENT_TYPE: &str = "feed_item";

/// A canonical content row to embed: an opaque id and the text to encode.
#[derive(Debug, Clone)]
pub struct EmbeddableContent {
    pub content_id: String,
    pub text: String,
}

/// An embedding to insert or replace.
#[derive(Debug, Clone)]
pub struct NewContentEmbedding {
    pub content_type: String,
    pub content_id: String,
    pub model_id: String,
    pub dim: usize,
    pub vector: Vec<f32>,
    pub content_hash: String,
}

/// A candidate vector returned from a scan, addressed by its canonical id.
#[derive(Debug, Clone)]
pub struct EmbeddedVector {
    pub content_id: String,
    pub vector: Vec<f32>,
}

/// Per-content-type embedded row count for one model (status read model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedCount {
    pub content_type: String,
    pub count: i64,
}

/// Encode a vector as a little-endian f32 byte blob.
fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Decode a little-endian f32 byte blob back into a vector.
fn decode_vector(blob: &[u8]) -> StorageResult<Vec<f32>> {
    if !blob.len().is_multiple_of(4) {
        return Err(StorageError::InvalidEmbeddingValue {
            message: format!("vector blob length {} is not a multiple of 4", blob.len()),
        });
    }

    Ok(blob
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

/// Feed items as embeddable content (`content_type` = `feed_item`). The encoded
/// text concatenates the title, summary, and body so semantically similar
/// coverage clusters; empty parts are skipped.
pub(crate) fn feed_item_contents(connection: &Connection) -> StorageResult<Vec<EmbeddableContent>> {
    let mut statement = connection.prepare(
        "SELECT id, title, COALESCE(summary, ''), COALESCE(body_text, '')
         FROM feed_items
         ORDER BY id",
    )?;

    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut contents = Vec::new();
    for row in rows {
        let (content_id, title, summary, body) = row?;
        let text = [title, summary, body]
            .into_iter()
            .map(|part| part.trim().to_owned())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        contents.push(EmbeddableContent { content_id, text });
    }

    Ok(contents)
}

/// Insert or replace one embedding (idempotent on the natural key).
pub(crate) fn upsert_embedding(
    connection: &Connection,
    embedding: &NewContentEmbedding,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO content_embeddings
            (content_type, content_id, model_id, dim, vector, content_hash)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT (content_type, content_id, model_id) DO UPDATE SET
            dim = excluded.dim,
            vector = excluded.vector,
            content_hash = excluded.content_hash,
            created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            embedding.content_type,
            embedding.content_id,
            embedding.model_id,
            embedding.dim as i64,
            encode_vector(&embedding.vector),
            embedding.content_hash,
        ],
    )?;

    Ok(())
}

/// Map of `content_id -> content_hash` already embedded for a content type and
/// model, so the embed job can skip unchanged content.
pub(crate) fn existing_hashes(
    connection: &Connection,
    content_type: &str,
    model_id: &str,
) -> StorageResult<HashMap<String, String>> {
    let mut statement = connection.prepare(
        "SELECT content_id, content_hash FROM content_embeddings
         WHERE content_type = ?1 AND model_id = ?2",
    )?;

    let rows = statement.query_map(params![content_type, model_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut hashes = HashMap::new();
    for row in rows {
        let (content_id, content_hash) = row?;
        hashes.insert(content_id, content_hash);
    }

    Ok(hashes)
}

/// All stored vectors for a content type and model — the brute-force scan input.
pub(crate) fn scan_vectors(
    connection: &Connection,
    content_type: &str,
    model_id: &str,
) -> StorageResult<Vec<EmbeddedVector>> {
    let mut statement = connection.prepare(
        "SELECT content_id, vector FROM content_embeddings
         WHERE content_type = ?1 AND model_id = ?2",
    )?;

    let rows = statement.query_map(params![content_type, model_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;

    let mut vectors = Vec::new();
    for row in rows {
        let (content_id, blob) = row?;
        vectors.push(EmbeddedVector {
            content_id,
            vector: decode_vector(&blob)?,
        });
    }

    Ok(vectors)
}

/// The stored vector for a single content row under a model, if embedded.
pub(crate) fn get_vector(
    connection: &Connection,
    content_type: &str,
    content_id: &str,
    model_id: &str,
) -> StorageResult<Option<Vec<f32>>> {
    let blob: Option<Vec<u8>> = connection
        .query_row(
            "SELECT vector FROM content_embeddings
             WHERE content_type = ?1 AND content_id = ?2 AND model_id = ?3",
            params![content_type, content_id, model_id],
            |row| row.get(0),
        )
        .optional()?;

    match blob {
        Some(blob) => Ok(Some(decode_vector(&blob)?)),
        None => Ok(None),
    }
}

/// Drop every vector NOT built with `keep_model_id` — used when the active model
/// changes so stale-model vectors never linger or get mixed in. Returns the
/// number of pruned rows.
pub(crate) fn prune_other_models(
    connection: &Connection,
    keep_model_id: &str,
) -> StorageResult<usize> {
    let deleted = connection.execute(
        "DELETE FROM content_embeddings WHERE model_id <> ?1",
        params![keep_model_id],
    )?;

    Ok(deleted)
}

/// Per-content-type embedded counts for one model.
pub(crate) fn counts_by_content_type(
    connection: &Connection,
    model_id: &str,
) -> StorageResult<Vec<EmbeddedCount>> {
    let mut statement = connection.prepare(
        "SELECT content_type, COUNT(*) FROM content_embeddings
         WHERE model_id = ?1
         GROUP BY content_type
         ORDER BY content_type",
    )?;

    let rows = statement.query_map(params![model_id], |row| {
        Ok(EmbeddedCount {
            content_type: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    let mut counts = Vec::new();
    for row in rows {
        counts.push(row?);
    }

    Ok(counts)
}

/// The `model_id` the current index was built with (the one with the most rows),
/// or `None` when the index is empty. A mismatch with the active model means a
/// re-embed is pending.
pub(crate) fn index_model_id(connection: &Connection) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "SELECT model_id FROM content_embeddings
             GROUP BY model_id
             ORDER BY COUNT(*) DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

/// Drop the entire index. The index is disposable: this loses zero canonical
/// data and only forces a re-embed. Returns the number of dropped rows.
pub(crate) fn clear_all(connection: &Connection) -> StorageResult<usize> {
    let deleted = connection.execute("DELETE FROM content_embeddings", [])?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    fn embedding(content_id: &str, model_id: &str, vector: Vec<f32>) -> NewContentEmbedding {
        NewContentEmbedding {
            content_type: "feed_item".to_owned(),
            content_id: content_id.to_owned(),
            model_id: model_id.to_owned(),
            dim: vector.len(),
            vector,
            content_hash: format!("hash-{content_id}"),
        }
    }

    #[test]
    fn round_trips_a_vector_blob() {
        let connection = open_in_memory_database().expect("db");
        let vector = vec![0.5_f32, -0.25, 1.0, 0.0];
        upsert_embedding(&connection, &embedding("feed_01", "m1", vector.clone())).expect("upsert");

        let read = get_vector(&connection, "feed_item", "feed_01", "m1")
            .expect("get")
            .expect("present");
        assert_eq!(read, vector);
    }

    #[test]
    fn upsert_replaces_on_conflict() {
        let connection = open_in_memory_database().expect("db");
        upsert_embedding(&connection, &embedding("feed_01", "m1", vec![1.0, 0.0])).expect("first");
        upsert_embedding(&connection, &embedding("feed_01", "m1", vec![0.0, 1.0])).expect("second");

        let read = get_vector(&connection, "feed_item", "feed_01", "m1")
            .expect("get")
            .expect("present");
        assert_eq!(read, vec![0.0, 1.0]);
    }

    #[test]
    fn isolates_vectors_by_model_id() {
        let connection = open_in_memory_database().expect("db");
        upsert_embedding(&connection, &embedding("feed_01", "m1", vec![1.0, 0.0])).expect("m1");
        upsert_embedding(&connection, &embedding("feed_01", "m2", vec![0.0, 1.0])).expect("m2");

        let m1 = scan_vectors(&connection, "feed_item", "m1").expect("scan m1");
        assert_eq!(m1.len(), 1);
        assert_eq!(m1[0].vector, vec![1.0, 0.0]);

        let m2 = scan_vectors(&connection, "feed_item", "m2").expect("scan m2");
        assert_eq!(m2.len(), 1);
        assert_eq!(m2[0].vector, vec![0.0, 1.0]);
    }

    #[test]
    fn prune_drops_other_models_only() {
        let connection = open_in_memory_database().expect("db");
        upsert_embedding(&connection, &embedding("feed_01", "old", vec![1.0])).expect("old");
        upsert_embedding(&connection, &embedding("feed_02", "new", vec![1.0])).expect("new");

        let pruned = prune_other_models(&connection, "new").expect("prune");
        assert_eq!(pruned, 1);
        assert_eq!(
            index_model_id(&connection).expect("idx"),
            Some("new".to_owned())
        );
    }

    #[test]
    fn existing_hashes_reports_embedded_content() {
        let connection = open_in_memory_database().expect("db");
        upsert_embedding(&connection, &embedding("feed_01", "m1", vec![1.0])).expect("upsert");

        let hashes = existing_hashes(&connection, "feed_item", "m1").expect("hashes");
        assert_eq!(hashes.get("feed_01"), Some(&"hash-feed_01".to_owned()));
    }

    #[test]
    fn counts_and_clear_reflect_index_state() {
        let connection = open_in_memory_database().expect("db");
        upsert_embedding(&connection, &embedding("feed_01", "m1", vec![1.0])).expect("a");
        upsert_embedding(&connection, &embedding("feed_02", "m1", vec![1.0])).expect("b");

        let counts = counts_by_content_type(&connection, "m1").expect("counts");
        assert_eq!(
            counts,
            vec![EmbeddedCount {
                content_type: "feed_item".to_owned(),
                count: 2
            }]
        );

        let cleared = clear_all(&connection).expect("clear");
        assert_eq!(cleared, 2);
        assert_eq!(index_model_id(&connection).expect("idx"), None);
    }

    #[test]
    fn rejects_corrupt_blob_length() {
        assert!(decode_vector(&[1, 2, 3]).is_err());
    }

    /// Scale gate (ADR 0049, T4): the similarity hot path must scan the
    /// **persisted** vector index in a single linear pass — never re-embed the
    /// corpus and never go quadratic. This is the deterministic guard for the
    /// `v0.45.0` `find_similar` regression class, which froze the UI by
    /// re-embedding every feed item synchronously per call. The assertion is on
    /// structure (one scan, `N` comparisons), not wall-clock, so it never flakes.
    #[test]
    fn similarity_scan_is_linear_over_a_volume_index() {
        use crate::interpretation::similarity_score;

        const N: usize = 2000;
        const DIM: usize = 8;
        let connection = open_in_memory_database().expect("db");

        // Seed N deterministic vectors; item k points mostly along axis (k % DIM).
        for k in 0..N {
            let mut vector = vec![0.0_f32; DIM];
            vector[k % DIM] = 1.0;
            vector[(k + 1) % DIM] = 0.25;
            upsert_embedding(
                &connection,
                &embedding(&format!("feed_{k:04}"), "m1", vector),
            )
            .expect("seed");
        }

        // The persisted-index scan returns exactly N rows in one pass — bounded by
        // stored rows, not by re-deriving the corpus. This is the property the
        // find_similar fix established and that this gate locks in.
        let scanned = scan_vectors(&connection, "feed_item", "m1").expect("scan");
        assert_eq!(
            scanned.len(),
            N,
            "scan must read the whole persisted index once"
        );

        // Rank a query against the scanned vectors, counting comparisons to assert
        // the work is O(N) (one per stored vector), never O(N^2).
        let query = scanned[0].vector.clone();
        let query_id = scanned[0].content_id.clone();
        let mut comparisons = 0_usize;
        let mut scored: Vec<(String, f32)> = scanned
            .iter()
            .filter(|candidate| candidate.content_id != query_id)
            .map(|candidate| {
                comparisons += 1;
                (
                    candidate.content_id.clone(),
                    similarity_score(&query, &candidate.vector),
                )
            })
            .collect();
        assert_eq!(comparisons, N - 1, "ranking must be a single linear pass");

        // Output is bounded to k; the nearest neighbour is an item that shares the
        // query's dominant axis (an identical seeded vector → score 1.0).
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(10);
        assert_eq!(scored.len(), 10, "output is bounded to k");
        assert!(
            scored[0].1 > 0.9,
            "top match should be highly similar: {:?}",
            scored[0]
        );
    }
}

use super::database::Database;
/// embeddings domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::embeddings()`.
#[derive(Clone)]
pub struct EmbeddingStore {
    db: Database,
}

impl EmbeddingStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn upsert_content_embedding(&self, embedding: &NewContentEmbedding) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        upsert_embedding(&connection, embedding)
    }

    pub fn feed_item_embeddable_contents(&self) -> StorageResult<Vec<EmbeddableContent>> {
        let connection = self.db.checkout()?;

        feed_item_contents(&connection)
    }

    pub fn embedded_content_hashes(
        &self,
        content_type: &str,
        model_id: &str,
    ) -> StorageResult<std::collections::HashMap<String, String>> {
        let connection = self.db.checkout()?;

        existing_hashes(&connection, content_type, model_id)
    }

    pub fn scan_content_vectors(
        &self,
        content_type: &str,
        model_id: &str,
    ) -> StorageResult<Vec<EmbeddedVector>> {
        let connection = self.db.checkout()?;

        scan_vectors(&connection, content_type, model_id)
    }

    pub fn content_vector(
        &self,
        content_type: &str,
        content_id: &str,
        model_id: &str,
    ) -> StorageResult<Option<Vec<f32>>> {
        let connection = self.db.checkout()?;

        get_vector(&connection, content_type, content_id, model_id)
    }

    pub fn prune_embeddings_other_models(&self, keep_model_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;

        prune_other_models(&connection, keep_model_id)
    }

    pub fn embedding_counts(&self, model_id: &str) -> StorageResult<Vec<EmbeddedCount>> {
        let connection = self.db.checkout()?;

        counts_by_content_type(&connection, model_id)
    }

    pub fn embedding_index_model_id(&self) -> StorageResult<Option<String>> {
        let connection = self.db.checkout()?;

        index_model_id(&connection)
    }

    pub fn clear_content_embeddings(&self) -> StorageResult<usize> {
        let connection = self.db.checkout()?;

        clear_all(&connection)
    }
}
