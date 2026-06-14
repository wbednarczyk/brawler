use super::*;

/// A single ranked match from the unified `search_index` FTS5 table (ADR 0032).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub content_type: String,
    pub source_id: String,
    pub company_id: Option<String>,
    /// Navigational container when the item's own id is not the navigation target
    /// (e.g. the transcript job for a transcript segment). `None` otherwise.
    pub parent_id: Option<String>,
    pub title: String,
    pub snippet: String,
    /// Relevance score; higher is more relevant (derived from bm25).
    pub score: f64,
}

/// Turn raw user input into a safe FTS5 MATCH expression.
///
/// Each whitespace-separated token becomes a quoted prefix term, so user input
/// is never interpreted as FTS5 operator syntax. Returns an empty string when
/// there is nothing to search for.
fn sanitize_fts_query(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| {
            let escaped = token.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run a ranked full-text search over the unified index, grouped/scoped by the
/// caller. `content_types` and `company_id` are optional filters; an empty or
/// blank `query` returns no matches.
pub(super) fn run_search(
    connection: &Connection,
    query: &str,
    content_types: &[String],
    company_id: Option<&str>,
    limit: i64,
) -> StorageResult<Vec<SearchMatch>> {
    let match_query = sanitize_fts_query(query);
    if match_query.is_empty() {
        return Ok(Vec::new());
    }

    // Highlight markers are control characters (STX/ETX), not HTML, so callers can
    // render snippets as plain text without an HTML-injection risk from source content.
    let mut sql = String::from(
        "SELECT content_type, source_id, company_id, parent_id, title, \
         snippet(search_index, 1, char(2), char(3), '…', 12) AS snippet, \
         bm25(search_index) AS score \
         FROM search_index WHERE search_index MATCH ?1",
    );

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(match_query)];

    if !content_types.is_empty() {
        let placeholders = content_types
            .iter()
            .enumerate()
            .map(|(index, _)| format!("?{}", index + 2))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND content_type IN ({placeholders})"));
        for content_type in content_types {
            params.push(Box::new(content_type.clone()));
        }
    }

    if let Some(company_id) = company_id {
        sql.push_str(&format!(" AND company_id = ?{}", params.len() + 1));
        params.push(Box::new(company_id.to_owned()));
    }

    // bm25() returns more-negative values for better matches, so ascending order
    // is best-first; the returned score is negated so higher means more relevant.
    sql.push_str(&format!(" ORDER BY score LIMIT ?{}", params.len() + 1));
    params.push(Box::new(limit));

    let mut statement = connection.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|param| param.as_ref()).collect();
    let matches = statement
        .query_map(param_refs.as_slice(), |row| {
            let score: f64 = row.get(6)?;
            Ok(SearchMatch {
                content_type: row.get(0)?,
                source_id: row.get(1)?,
                company_id: row.get(2)?,
                parent_id: row.get(3)?,
                title: row.get(4)?,
                snippet: row.get(5)?,
                score: -score,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(matches)
}
