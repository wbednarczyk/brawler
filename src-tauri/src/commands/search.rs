use crate::{app_state, storage};
use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;

/// Request for the unified global search (ADR 0032). `contentTypes` and
/// `companyId` are optional scoping filters.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub content_types: Vec<String>,
    #[serde(default)]
    pub company_id: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroup {
    pub content_type: String,
    pub matches: Vec<storage::SearchMatch>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub groups: Vec<SearchGroup>,
}

#[tauri::command]
pub fn search(
    input: SearchInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<SearchResults, String> {
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let matches = state
        .search(
            &input.query,
            &input.content_types,
            input.company_id.as_deref(),
            limit,
        )
        .map_err(|error| error.to_string())?;

    Ok(group_matches(matches))
}

/// Group ranked matches by content type while preserving rank order: groups
/// appear in order of their best match, and matches stay ranked within a group.
fn group_matches(matches: Vec<storage::SearchMatch>) -> SearchResults {
    let mut groups: Vec<SearchGroup> = Vec::new();
    for search_match in matches {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.content_type == search_match.content_type)
        {
            group.matches.push(search_match);
        } else {
            groups.push(SearchGroup {
                content_type: search_match.content_type.clone(),
                matches: vec![search_match],
            });
        }
    }

    SearchResults { groups }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(content_type: &str, source_id: &str, score: f64) -> storage::SearchMatch {
        storage::SearchMatch {
            content_type: content_type.to_owned(),
            source_id: source_id.to_owned(),
            company_id: None,
            parent_id: None,
            title: source_id.to_owned(),
            snippet: String::new(),
            score,
        }
    }

    #[test]
    fn groups_by_type_and_preserves_rank_order() {
        let matches = vec![
            sample("notebook_entry", "n1", 3.0),
            sample("company", "c1", 2.5),
            sample("notebook_entry", "n2", 2.0),
            sample("company", "c2", 1.0),
        ];

        let results = group_matches(matches);

        // Groups appear in order of their best (first-seen) match.
        let group_types: Vec<&str> = results
            .groups
            .iter()
            .map(|group| group.content_type.as_str())
            .collect();
        assert_eq!(group_types, vec!["notebook_entry", "company"]);

        // Matches stay rank-ordered within their group.
        let notebook = &results.groups[0];
        assert_eq!(
            notebook
                .matches
                .iter()
                .map(|m| m.source_id.as_str())
                .collect::<Vec<_>>(),
            vec!["n1", "n2"]
        );
    }
}
