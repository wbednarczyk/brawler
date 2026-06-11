use std::collections::{HashMap, HashSet};

use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::*;

const RESEARCH_SCHEMA_VERSION: i64 = 1;
const SETTINGS_SCHEMA_VERSION: i64 = 1;

const SETTINGS_KEYS: &[&str] = &[
    "theme",
    "accent_palette",
    "locale",
    "poll_interval_seconds",
    "youtube_transcription_provider",
    "youtube_transcription_model",
    "youtube_transcription_timeout_seconds",
    "general_analysis_provider",
    "general_analysis_model",
    "general_analysis_timeout_seconds",
    "ai_analysis_mode",
    "log_level",
    "log_max_files",
    "log_max_file_bytes",
    "shortcut_bindings",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPayload {
    pub file_name: String,
    pub media_type: String,
    pub contents: String,
    pub summary: ImportExportSummary,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExportSummary {
    pub companies: usize,
    pub watchlists: usize,
    pub memberships: usize,
    pub notebook_entries: usize,
    pub research_questions: usize,
    pub evidence_links: usize,
    pub settings: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub valid: bool,
    pub summary: ImportApplySummary,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportApplySummary {
    pub companies_created: usize,
    pub companies_merged: usize,
    pub watchlists_created: usize,
    pub watchlists_merged: usize,
    pub memberships_created: usize,
    pub notebook_entries_created: usize,
    pub notebook_entries_skipped: usize,
    pub research_questions_created: usize,
    pub research_questions_merged: usize,
    pub evidence_links_created: usize,
    pub evidence_links_skipped: usize,
    pub settings_updated: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportApplyResult {
    pub summary: ImportApplySummary,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchExportDocument {
    schema_version: i64,
    exported_at: String,
    app_version: String,
    sections: Vec<String>,
    companies: Vec<ExportCompany>,
    watchlists: Vec<ExportWatchlist>,
    memberships: Vec<ExportMembership>,
    notebook_entries: Vec<ExportNotebookEntry>,
    #[serde(default)]
    research_questions: Vec<ExportResearchQuestion>,
    #[serde(default)]
    evidence_links: Vec<ExportEvidenceLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportCompany {
    id: String,
    exchange: String,
    ticker: String,
    qualified_ticker: String,
    display_name: String,
    isin: Option<String>,
    cik: Option<String>,
    lei: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportWatchlist {
    id: String,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMembership {
    watchlist_id: String,
    company_qualified_ticker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportNotebookEntry {
    id: String,
    company_qualified_ticker: String,
    title: String,
    body: String,
    body_format: String,
    tags: Vec<String>,
    kind: String,
    claim_status: Option<String>,
    event_date: Option<String>,
    follow_up_after: Option<String>,
    follow_up_date: Option<String>,
    created_at: String,
    updated_at: String,
    origins: Vec<ExportNotebookOrigin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportNotebookOrigin {
    source_type: String,
    source_id: Option<String>,
    source_url: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportResearchQuestion {
    id: String,
    scope_type: String,
    scope_id: String,
    scope_company_qualified_ticker: Option<String>,
    title: String,
    body: String,
    status: String,
    closed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportEvidenceLink {
    id: String,
    from_type: String,
    from_id: String,
    to_type: String,
    to_id: String,
    relation_type: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsExportDocument {
    schema_version: i64,
    exported_at: String,
    app_version: String,
    settings: ExportSettings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportSettings {
    theme: Option<String>,
    accent_palette: Option<String>,
    locale: Option<String>,
    poll_interval_seconds: Option<i64>,
    youtube_transcription_provider: Option<String>,
    youtube_transcription_model: Option<String>,
    youtube_transcription_timeout_seconds: Option<i64>,
    general_analysis_provider: Option<String>,
    general_analysis_model: Option<String>,
    general_analysis_timeout_seconds: Option<i64>,
    ai_analysis_mode: Option<String>,
    log_level: Option<String>,
    log_max_files: Option<i64>,
    log_max_file_bytes: Option<i64>,
    shortcut_bindings: Option<HashMap<String, ShortcutBindingSetting>>,
}

pub(super) fn export_research_data(connection: &Connection) -> StorageResult<ExportPayload> {
    let companies = export_companies(connection)?;
    let watchlists = export_watchlists(connection)?;
    let memberships = export_memberships(connection)?;
    let notebook_entries = export_notebook_entries(connection)?;
    let research_questions = export_research_questions(connection)?;
    let evidence_links = export_evidence_links(connection)?;
    let exported_at = now_rfc3339()?;

    let document = ResearchExportDocument {
        schema_version: RESEARCH_SCHEMA_VERSION,
        exported_at: exported_at.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        sections: vec![
            "companies".to_owned(),
            "watchlists".to_owned(),
            "notebooks".to_owned(),
            "research_questions".to_owned(),
            "evidence_links".to_owned(),
        ],
        companies,
        watchlists,
        memberships,
        notebook_entries,
        research_questions,
        evidence_links,
    };
    let summary = ImportExportSummary {
        companies: document.companies.len(),
        watchlists: document.watchlists.len(),
        memberships: document.memberships.len(),
        notebook_entries: document.notebook_entries.len(),
        research_questions: document.research_questions.len(),
        evidence_links: document.evidence_links.len(),
        settings: 0,
    };

    Ok(ExportPayload {
        file_name: format!(
            "brawler-research-data-{}.json",
            exported_at[..10].to_owned()
        ),
        media_type: "application/json".to_owned(),
        contents: serde_json::to_string_pretty(&document)?,
        summary,
    })
}

pub(super) fn preview_research_import(
    connection: &Connection,
    contents: &str,
) -> StorageResult<ImportPreview> {
    let document = parse_research_document(contents)?;
    Ok(plan_research_import(connection, &document))
}

pub(super) fn apply_research_import(
    connection: &mut Connection,
    contents: &str,
) -> StorageResult<ImportApplyResult> {
    let document = parse_research_document(contents)?;
    let preview = plan_research_import(connection, &document);
    if !preview.valid {
        return Err(StorageError::InvalidSettingValue {
            key: "import_export",
            value: preview.errors.join("; "),
        });
    }
    let planned_summary = preview.summary.clone();

    let transaction = connection.transaction()?;
    let company_id_by_ticker = apply_companies(&transaction, &document.companies)?;
    let summary = apply_watchlists_notebooks_questions(
        &transaction,
        &document,
        &company_id_by_ticker,
        planned_summary,
    )?;
    transaction.commit()?;

    Ok(ImportApplyResult {
        summary,
        warnings: preview.warnings,
    })
}

pub(super) fn export_settings_data(connection: &Connection) -> StorageResult<ExportPayload> {
    let settings = settings::get_settings(connection)?;
    let exported_at = now_rfc3339()?;
    let document = SettingsExportDocument {
        schema_version: SETTINGS_SCHEMA_VERSION,
        exported_at: exported_at.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        settings: ExportSettings {
            theme: Some(settings.theme),
            accent_palette: Some(settings.accent_palette),
            locale: Some(settings.locale),
            poll_interval_seconds: Some(settings.poll_interval_seconds),
            youtube_transcription_provider: Some(
                settings.ai_providers.youtube_transcription_provider,
            ),
            youtube_transcription_model: Some(settings.ai_providers.youtube_transcription_model),
            youtube_transcription_timeout_seconds: Some(
                settings.ai_providers.youtube_transcription_timeout_seconds,
            ),
            general_analysis_provider: settings.ai_providers.general_analysis_provider,
            general_analysis_model: Some(settings.ai_providers.general_analysis_model),
            general_analysis_timeout_seconds: Some(
                settings.ai_providers.general_analysis_timeout_seconds,
            ),
            ai_analysis_mode: Some(settings.ai_analysis_mode),
            log_level: Some(settings.logs.level),
            log_max_files: Some(settings.logs.max_files),
            log_max_file_bytes: Some(settings.logs.max_file_bytes),
            shortcut_bindings: Some(settings.shortcut_bindings),
        },
    };

    Ok(ExportPayload {
        file_name: format!("brawler-settings-{}.yaml", exported_at[..10].to_owned()),
        media_type: "application/x-yaml".to_owned(),
        contents: serde_yaml::to_string(&document)?,
        summary: ImportExportSummary {
            settings: SETTINGS_KEYS.len(),
            ..ImportExportSummary::default()
        },
    })
}

pub(super) fn preview_settings_import(contents: &str) -> StorageResult<ImportPreview> {
    let document = parse_settings_document(contents)?;
    Ok(plan_settings_import(&document))
}

pub(super) fn apply_settings_import(
    connection: &Connection,
    contents: &str,
) -> StorageResult<ImportApplyResult> {
    let document = parse_settings_document(contents)?;
    let preview = plan_settings_import(&document);
    if !preview.valid {
        return Err(StorageError::InvalidSettingValue {
            key: "import_export",
            value: preview.errors.join("; "),
        });
    }

    let summary = settings_to_update_summary(&document.settings);
    settings::update_settings(connection, settings_to_update(document.settings)?)?;

    Ok(ImportApplyResult {
        summary,
        warnings: preview.warnings,
    })
}

fn parse_research_document(contents: &str) -> StorageResult<ResearchExportDocument> {
    serde_json::from_str::<ResearchExportDocument>(contents).map_err(StorageError::from)
}

fn parse_settings_document(contents: &str) -> StorageResult<SettingsExportDocument> {
    serde_yaml::from_str::<SettingsExportDocument>(contents).map_err(StorageError::from)
}

fn plan_research_import(
    connection: &Connection,
    document: &ResearchExportDocument,
) -> ImportPreview {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut summary = ImportApplySummary::default();

    if document.schema_version != RESEARCH_SCHEMA_VERSION {
        errors.push(format!(
            "Unsupported research data schema version: {}",
            document.schema_version
        ));
    }

    let existing_companies = existing_companies_by_ticker(connection).unwrap_or_default();
    let imported_company_tickers = document
        .companies
        .iter()
        .map(|company| normalize_qualified_ticker(&company.exchange, &company.ticker))
        .collect::<HashSet<_>>();

    for company in &document.companies {
        let qualified_ticker = normalize_qualified_ticker(&company.exchange, &company.ticker);
        if company.qualified_ticker.trim().to_uppercase() != qualified_ticker {
            errors.push(format!(
                "Company {} has mismatched qualified ticker",
                company.qualified_ticker
            ));
            continue;
        }
        if existing_companies.contains_key(&qualified_ticker) {
            summary.companies_merged += 1;
        } else {
            summary.companies_created += 1;
        }
    }

    let existing_watchlist_ids = existing_ids(connection, "watchlists").unwrap_or_default();
    let existing_watchlist_ids_by_name =
        existing_watchlist_ids_by_name(connection).unwrap_or_default();
    let imported_watchlist_ids = document
        .watchlists
        .iter()
        .map(|watchlist| watchlist.id.clone())
        .collect::<HashSet<_>>();
    for watchlist in &document.watchlists {
        if watchlist.id.trim().is_empty() || watchlist.name.trim().is_empty() {
            errors.push("Watchlist id and name are required".to_owned());
        } else if existing_watchlist_ids.contains(&watchlist.id) {
            summary.watchlists_merged += 1;
        } else if existing_watchlist_ids_by_name.contains_key(&watchlist.name.trim().to_lowercase())
        {
            summary.watchlists_merged += 1;
            warnings.push(format!(
                "Watchlist {} already exists by name and will be merged",
                watchlist.name
            ));
        } else {
            summary.watchlists_created += 1;
        }
    }

    let existing_memberships = existing_memberships(connection).unwrap_or_default();
    for membership in &document.memberships {
        if !imported_watchlist_ids.contains(&membership.watchlist_id)
            && !existing_watchlist_ids.contains(&membership.watchlist_id)
        {
            errors.push(format!(
                "Membership references missing watchlist {}",
                membership.watchlist_id
            ));
        }
        let company_ticker = membership.company_qualified_ticker.trim().to_uppercase();
        if !imported_company_tickers.contains(&company_ticker)
            && !existing_companies.contains_key(&company_ticker)
        {
            errors.push(format!(
                "Membership references missing company {}",
                membership.company_qualified_ticker
            ));
        }
        if !existing_memberships.contains(&(membership.watchlist_id.clone(), company_ticker)) {
            summary.memberships_created += 1;
        }
    }

    let existing_note_ids = existing_ids(connection, "notebook_entries").unwrap_or_default();
    let imported_note_ids = document
        .notebook_entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<HashSet<_>>();
    for entry in &document.notebook_entries {
        if existing_note_ids.contains(&entry.id) {
            summary.notebook_entries_skipped += 1;
            warnings.push(format!(
                "Notebook entry {} already exists and will be skipped",
                entry.id
            ));
            continue;
        }

        let company_ticker = entry.company_qualified_ticker.trim().to_uppercase();
        if !imported_company_tickers.contains(&company_ticker)
            && !existing_companies.contains_key(&company_ticker)
        {
            errors.push(format!(
                "Notebook entry {} references missing company {}",
                entry.id, entry.company_qualified_ticker
            ));
            continue;
        }

        if entry.body_format != "markdown" {
            errors.push(format!(
                "Notebook entry {} has unsupported body format {}",
                entry.id, entry.body_format
            ));
        }
        if !["manual", "observation", "claim", "question", "follow_up"]
            .contains(&entry.kind.as_str())
        {
            errors.push(format!(
                "Notebook entry {} has unsupported kind {}",
                entry.id, entry.kind
            ));
        }
        summary.notebook_entries_created += 1;
    }

    let existing_question_ids = existing_ids(connection, "research_questions").unwrap_or_default();
    let imported_question_ids = document
        .research_questions
        .iter()
        .map(|question| question.id.clone())
        .collect::<HashSet<_>>();
    for question in &document.research_questions {
        if question.id.trim().is_empty() || question.title.trim().is_empty() {
            errors.push("Research question id and title are required".to_owned());
            continue;
        }
        if question.scope_type != "company" {
            errors.push(format!(
                "Research question {} has unsupported scope {}",
                question.id, question.scope_type
            ));
            continue;
        }
        let Some(company_ticker) = question.scope_company_qualified_ticker.as_deref() else {
            errors.push(format!(
                "Research question {} is missing company scope ticker",
                question.id
            ));
            continue;
        };
        let company_ticker = company_ticker.trim().to_uppercase();
        if !imported_company_tickers.contains(&company_ticker)
            && !existing_companies.contains_key(&company_ticker)
        {
            errors.push(format!(
                "Research question {} references missing company {}",
                question.id, company_ticker
            ));
            continue;
        }
        if !["open", "answered", "closed"].contains(&question.status.as_str()) {
            errors.push(format!(
                "Research question {} has unsupported status {}",
                question.id, question.status
            ));
            continue;
        }
        if existing_question_ids.contains(&question.id) {
            summary.research_questions_merged += 1;
        } else {
            summary.research_questions_created += 1;
        }
    }

    let existing_link_ids = existing_ids(connection, "evidence_links").unwrap_or_default();
    for link in &document.evidence_links {
        if existing_link_ids.contains(&link.id) {
            summary.evidence_links_skipped += 1;
            warnings.push(format!(
                "Evidence link {} already exists and will be skipped",
                link.id
            ));
            continue;
        }
        if !imported_or_existing_evidence_reference(
            connection,
            &link.from_type,
            &link.from_id,
            &imported_note_ids,
            &imported_question_ids,
        ) || !imported_or_existing_evidence_reference(
            connection,
            &link.to_type,
            &link.to_id,
            &imported_note_ids,
            &imported_question_ids,
        ) {
            summary.evidence_links_skipped += 1;
            warnings.push(format!(
                "Evidence link {} references unavailable evidence and will be skipped",
                link.id
            ));
            continue;
        }
        summary.evidence_links_created += 1;
    }

    ImportPreview {
        valid: errors.is_empty(),
        summary,
        warnings,
        errors,
    }
}

fn plan_settings_import(document: &SettingsExportDocument) -> ImportPreview {
    let mut errors = Vec::new();
    let warnings = Vec::new();

    if document.schema_version != SETTINGS_SCHEMA_VERSION {
        errors.push(format!(
            "Unsupported settings schema version: {}",
            document.schema_version
        ));
    }

    if let Err(error) = settings_to_update(document.settings.clone()) {
        errors.push(error.to_string());
    }

    ImportPreview {
        valid: errors.is_empty(),
        summary: settings_to_update_summary(&document.settings),
        warnings,
        errors,
    }
}

fn apply_companies(
    connection: &Connection,
    companies: &[ExportCompany],
) -> StorageResult<HashMap<String, String>> {
    let mut company_id_by_ticker = existing_companies_by_ticker(connection)?;

    for company in companies {
        let exchange = company.exchange.trim().to_uppercase();
        let ticker = company.ticker.trim().to_uppercase();
        let qualified_ticker = normalize_qualified_ticker(&exchange, &ticker);
        let id = company_id(&exchange, &ticker);

        connection.execute(
            "
            INSERT INTO companies (
                id,
                exchange,
                ticker,
                qualified_ticker,
                display_name,
                isin,
                cik,
                lei
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(qualified_ticker) DO UPDATE SET
                isin = COALESCE(companies.isin, excluded.isin),
                cik = COALESCE(companies.cik, excluded.cik),
                lei = COALESCE(companies.lei, excluded.lei),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                id,
                exchange,
                ticker,
                qualified_ticker,
                company.display_name.trim(),
                empty_string_to_none(company.isin.clone()),
                empty_string_to_none(company.cik.clone()),
                empty_string_to_none(company.lei.clone()),
            ],
        )?;
    }

    company_id_by_ticker.extend(existing_companies_by_ticker(connection)?);
    Ok(company_id_by_ticker)
}

fn apply_watchlists_notebooks_questions(
    connection: &Connection,
    document: &ResearchExportDocument,
    company_id_by_ticker: &HashMap<String, String>,
    mut summary: ImportApplySummary,
) -> StorageResult<ImportApplySummary> {
    summary.memberships_created = 0;
    summary.notebook_entries_created = 0;
    summary.notebook_entries_skipped = 0;
    summary.research_questions_created = 0;
    summary.research_questions_merged = 0;
    summary.evidence_links_created = 0;
    summary.evidence_links_skipped = 0;
    let existing_watchlist_ids_by_name = existing_watchlist_ids_by_name(connection)?;
    let mut watchlist_id_map = HashMap::<String, String>::new();

    for watchlist in &document.watchlists {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM watchlists WHERE id = ?1)",
            [&watchlist.id],
            |row| row.get(0),
        )?;
        if exists {
            watchlist_id_map.insert(watchlist.id.clone(), watchlist.id.clone());
            summary.watchlists_merged += 1;
            continue;
        }

        if let Some(existing_id) =
            existing_watchlist_ids_by_name.get(&watchlist.name.trim().to_lowercase())
        {
            watchlist_id_map.insert(watchlist.id.clone(), existing_id.clone());
            summary.watchlists_merged += 1;
            continue;
        }

        connection.execute(
            "
            INSERT INTO watchlists (id, name, description)
            VALUES (?1, ?2, ?3)
            ",
            params![
                watchlist.id.trim(),
                watchlist.name.trim(),
                empty_string_to_none(watchlist.description.clone()),
            ],
        )?;
        watchlist_id_map.insert(watchlist.id.clone(), watchlist.id.clone());
        summary.watchlists_created += 1;
    }

    for membership in &document.memberships {
        let watchlist_id = watchlist_id_map
            .get(&membership.watchlist_id)
            .map(String::as_str)
            .unwrap_or(membership.watchlist_id.trim());
        let company_id = company_id_by_ticker
            .get(&membership.company_qualified_ticker.trim().to_uppercase())
            .ok_or_else(|| StorageError::InvalidSettingValue {
                key: "import_export",
                value: format!(
                    "missing company {}",
                    membership.company_qualified_ticker.trim()
                ),
            })?;
        let inserted = connection.execute(
            "
            INSERT OR IGNORE INTO watchlist_companies (watchlist_id, company_id)
            VALUES (?1, ?2)
            ",
            params![watchlist_id, company_id],
        )?;
        summary.memberships_created += inserted;
    }

    for entry in &document.notebook_entries {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM notebook_entries WHERE id = ?1)",
            [&entry.id],
            |row| row.get(0),
        )?;
        if exists {
            summary.notebook_entries_skipped += 1;
            continue;
        }

        let company_id = company_id_by_ticker
            .get(&entry.company_qualified_ticker.trim().to_uppercase())
            .ok_or_else(|| StorageError::InvalidSettingValue {
                key: "import_export",
                value: format!("missing note company {}", entry.company_qualified_ticker),
            })?;

        connection.execute(
            "
            INSERT INTO notebook_entries (
                id,
                company_id,
                title,
                body,
                body_format,
                kind,
                claim_status,
                event_date,
                follow_up_after,
                follow_up_date,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                entry.id.trim(),
                company_id,
                entry.title.trim(),
                entry.body.trim(),
                entry.body_format.trim(),
                entry.kind.trim(),
                empty_string_to_none(entry.claim_status.clone()),
                empty_string_to_none(entry.event_date.clone()),
                empty_string_to_none(entry.follow_up_after.clone()),
                empty_string_to_none(entry.follow_up_date.clone()),
                entry.created_at.trim(),
                entry.updated_at.trim(),
            ],
        )?;

        for tag in notebooks::normalize_tags(entry.tags.clone()) {
            connection.execute(
                "
                INSERT OR IGNORE INTO notebook_entry_tags (notebook_entry_id, tag)
                VALUES (?1, ?2)
                ",
                params![entry.id.trim(), tag],
            )?;
        }

        for (index, origin) in entry.origins.iter().enumerate() {
            let source_type = origin.source_type.trim().to_owned();
            connection.execute(
                "
                INSERT INTO notebook_entry_origins (
                    id,
                    notebook_entry_id,
                    source_type,
                    source_id,
                    source_url,
                    label
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    notebooks::notebook_origin_id(entry.id.trim(), &source_type, index),
                    entry.id.trim(),
                    source_type,
                    empty_string_to_none(origin.source_id.clone()),
                    empty_string_to_none(origin.source_url.clone()),
                    empty_string_to_none(origin.label.clone()),
                ],
            )?;
        }

        summary.notebook_entries_created += 1;
    }

    for question in &document.research_questions {
        let scope_id = if question.scope_type == "company" {
            let company_ticker = question
                .scope_company_qualified_ticker
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_uppercase();
            company_id_by_ticker
                .get(&company_ticker)
                .ok_or_else(|| StorageError::InvalidSettingValue {
                    key: "import_export",
                    value: format!("missing question company {company_ticker}"),
                })?
                .to_owned()
        } else {
            question.scope_id.trim().to_owned()
        };
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM research_questions WHERE id = ?1)",
            [&question.id],
            |row| row.get(0),
        )?;
        connection.execute(
            "
            INSERT INTO research_questions (
                id,
                scope_type,
                scope_id,
                title,
                body,
                status,
                closed_at,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                status = excluded.status,
                closed_at = excluded.closed_at,
                updated_at = excluded.updated_at
            ",
            params![
                question.id.trim(),
                question.scope_type.trim(),
                scope_id,
                question.title.trim(),
                question.body.trim(),
                question.status.trim(),
                empty_string_to_none(question.closed_at.clone()),
                question.created_at.trim(),
                question.updated_at.trim(),
            ],
        )?;
        if exists {
            summary.research_questions_merged += 1;
        } else {
            summary.research_questions_created += 1;
        }
    }

    for link in &document.evidence_links {
        if !evidence_reference_exists_for_import(connection, &link.from_type, &link.from_id)?
            || !evidence_reference_exists_for_import(connection, &link.to_type, &link.to_id)?
        {
            summary.evidence_links_skipped += 1;
            continue;
        }
        let inserted = connection.execute(
            "
            INSERT OR IGNORE INTO evidence_links (
                id,
                from_type,
                from_id,
                to_type,
                to_id,
                relation_type,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                link.id.trim(),
                link.from_type.trim(),
                link.from_id.trim(),
                link.to_type.trim(),
                link.to_id.trim(),
                link.relation_type.trim(),
                link.created_at.trim(),
            ],
        )?;
        if inserted == 0 {
            summary.evidence_links_skipped += 1;
        } else {
            summary.evidence_links_created += 1;
        }
    }

    Ok(summary)
}

fn export_companies(connection: &Connection) -> StorageResult<Vec<ExportCompany>> {
    Ok(companies::list_companies(connection)?
        .into_iter()
        .map(|company| ExportCompany {
            id: company.id,
            exchange: company.exchange,
            ticker: company.ticker,
            qualified_ticker: company.qualified_ticker,
            display_name: company.display_name,
            isin: company.isin,
            cik: company.cik,
            lei: company.lei,
        })
        .collect())
}

fn export_watchlists(connection: &Connection) -> StorageResult<Vec<ExportWatchlist>> {
    Ok(watchlists::list_watchlists(connection)?
        .into_iter()
        .map(|watchlist| ExportWatchlist {
            id: watchlist.id,
            name: watchlist.name,
            description: watchlist.description,
        })
        .collect())
}

fn export_memberships(connection: &Connection) -> StorageResult<Vec<ExportMembership>> {
    let mut statement = connection.prepare(
        "
        SELECT watchlist_companies.watchlist_id, companies.qualified_ticker
        FROM watchlist_companies
        INNER JOIN companies ON companies.id = watchlist_companies.company_id
        ORDER BY watchlist_companies.watchlist_id, companies.qualified_ticker
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportMembership {
            watchlist_id: row.get(0)?,
            company_qualified_ticker: row.get(1)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn export_notebook_entries(connection: &Connection) -> StorageResult<Vec<ExportNotebookEntry>> {
    let mut statement = connection.prepare(
        "
        SELECT
            notebook_entries.id,
            companies.qualified_ticker,
            notebook_entries.title,
            notebook_entries.body,
            notebook_entries.body_format,
            notebook_entries.kind,
            notebook_entries.claim_status,
            notebook_entries.event_date,
            notebook_entries.follow_up_after,
            notebook_entries.follow_up_date,
            notebook_entries.created_at,
            notebook_entries.updated_at
        FROM notebook_entries
        INNER JOIN companies ON companies.id = notebook_entries.company_id
        ORDER BY companies.qualified_ticker, notebook_entries.updated_at DESC, notebook_entries.id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        let id: String = row.get(0)?;
        let origins = export_notebook_origins(connection, &id)?;
        Ok(ExportNotebookEntry {
            id,
            company_qualified_ticker: row.get(1)?,
            title: row.get(2)?,
            body: row.get(3)?,
            body_format: row.get(4)?,
            kind: row.get(5)?,
            claim_status: row.get(6)?,
            event_date: row.get(7)?,
            follow_up_after: row.get(8)?,
            follow_up_date: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
            tags: Vec::new(),
            origins,
        })
    })?;
    let mut entries = rows.collect::<Result<Vec<_>, _>>()?;
    for entry in &mut entries {
        entry.tags = notebooks::notebook_entry_tags(connection, &entry.id)?;
    }
    Ok(entries)
}

fn export_notebook_origins(
    connection: &Connection,
    notebook_entry_id: &str,
) -> rusqlite::Result<Vec<ExportNotebookOrigin>> {
    let mut statement = connection.prepare(
        "
        SELECT source_type, source_id, source_url, label
        FROM notebook_entry_origins
        WHERE notebook_entry_id = ?1
        ORDER BY created_at, id
        ",
    )?;
    let rows = statement.query_map([notebook_entry_id], |row| {
        Ok(ExportNotebookOrigin {
            source_type: row.get(0)?,
            source_id: row.get(1)?,
            source_url: row.get(2)?,
            label: row.get(3)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

fn export_research_questions(
    connection: &Connection,
) -> StorageResult<Vec<ExportResearchQuestion>> {
    let mut statement = connection.prepare(
        "
        SELECT
            research_questions.id,
            research_questions.scope_type,
            research_questions.scope_id,
            companies.qualified_ticker,
            research_questions.title,
            research_questions.body,
            research_questions.status,
            research_questions.closed_at,
            research_questions.created_at,
            research_questions.updated_at
        FROM research_questions
        LEFT JOIN companies
            ON research_questions.scope_type = 'company'
            AND companies.id = research_questions.scope_id
        ORDER BY research_questions.scope_type,
            COALESCE(companies.qualified_ticker, research_questions.scope_id),
            research_questions.updated_at DESC,
            research_questions.id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportResearchQuestion {
            id: row.get(0)?,
            scope_type: row.get(1)?,
            scope_id: row.get(2)?,
            scope_company_qualified_ticker: row.get(3)?,
            title: row.get(4)?,
            body: row.get(5)?,
            status: row.get(6)?,
            closed_at: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn export_evidence_links(connection: &Connection) -> StorageResult<Vec<ExportEvidenceLink>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            from_type,
            from_id,
            to_type,
            to_id,
            relation_type,
            created_at
        FROM evidence_links
        ORDER BY datetime(created_at) DESC, id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportEvidenceLink {
            id: row.get(0)?,
            from_type: row.get(1)?,
            from_id: row.get(2)?,
            to_type: row.get(3)?,
            to_id: row.get(4)?,
            relation_type: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn existing_companies_by_ticker(connection: &Connection) -> StorageResult<HashMap<String, String>> {
    let mut statement = connection.prepare("SELECT qualified_ticker, id FROM companies")?;
    let rows = statement.query_map([], |row| {
        let qualified_ticker: String = row.get(0)?;
        let id: String = row.get(1)?;
        Ok((qualified_ticker.to_uppercase(), id))
    })?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(StorageError::from)
}

fn existing_ids(connection: &Connection, table_name: &str) -> StorageResult<HashSet<String>> {
    let sql = format!("SELECT id FROM {table_name}");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(StorageError::from)
}

fn existing_watchlist_ids_by_name(
    connection: &Connection,
) -> StorageResult<HashMap<String, String>> {
    let mut statement = connection.prepare("SELECT LOWER(name), id FROM watchlists")?;
    let rows = statement.query_map([], |row| {
        let name: String = row.get(0)?;
        let id: String = row.get(1)?;
        Ok((name, id))
    })?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(StorageError::from)
}

fn existing_memberships(connection: &Connection) -> StorageResult<HashSet<(String, String)>> {
    let mut statement = connection.prepare(
        "
        SELECT watchlist_companies.watchlist_id, companies.qualified_ticker
        FROM watchlist_companies
        INNER JOIN companies ON companies.id = watchlist_companies.company_id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        let watchlist_id: String = row.get(0)?;
        let qualified_ticker: String = row.get(1)?;
        Ok((watchlist_id, qualified_ticker.to_uppercase()))
    })?;

    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(StorageError::from)
}

fn imported_or_existing_evidence_reference(
    connection: &Connection,
    evidence_type: &str,
    evidence_id: &str,
    imported_note_ids: &HashSet<String>,
    imported_question_ids: &HashSet<String>,
) -> bool {
    match evidence_type {
        "notebook_entry" | "claim" if imported_note_ids.contains(evidence_id) => true,
        "research_question" if imported_question_ids.contains(evidence_id) => true,
        _ => evidence_reference_exists_for_import(connection, evidence_type, evidence_id)
            .unwrap_or(false),
    }
}

fn evidence_reference_exists_for_import(
    connection: &Connection,
    evidence_type: &str,
    evidence_id: &str,
) -> StorageResult<bool> {
    match evidence_type {
        "feed_item" => table_reference_exists(connection, "feed_items", evidence_id),
        "notebook_entry" | "claim" => {
            table_reference_exists(connection, "notebook_entries", evidence_id)
        }
        "transcript_segment" => {
            table_reference_exists(connection, "transcript_segments", evidence_id)
        }
        "company_event" => table_reference_exists(connection, "company_events", evidence_id),
        "ai_analysis" => table_reference_exists(connection, "ai_analysis_results", evidence_id),
        "research_question" => {
            table_reference_exists(connection, "research_questions", evidence_id)
        }
        _ => Ok(false),
    }
}

fn table_reference_exists(
    connection: &Connection,
    table_name: &str,
    id: &str,
) -> StorageResult<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)");
    connection
        .query_row(&sql, [id], |row| row.get(0))
        .map_err(StorageError::from)
}

fn settings_to_update(settings: ExportSettings) -> StorageResult<SettingsUpdate> {
    if let Some(theme) = settings.theme.as_deref() {
        validate_allowed_import_setting("theme", theme, &["dark", "light", "system"])?;
    }
    if let Some(accent_palette) = settings.accent_palette.as_deref() {
        validate_allowed_import_setting(
            "accent_palette",
            accent_palette,
            &["night-neon", "midnight-horizon"],
        )?;
    }
    if let Some(locale) = settings.locale.as_deref() {
        validate_allowed_import_setting("locale", locale, &["en", "pl"])?;
    }
    if let Some(value) = settings.poll_interval_seconds {
        validate_allowed_import_setting_i64(
            "poll_interval_seconds",
            value,
            &[300, 900, 1800, 3600],
        )?;
    }
    if let Some(provider) = settings.youtube_transcription_provider.as_deref() {
        validate_allowed_import_setting(
            "youtube_transcription_provider",
            provider,
            &["provider_gemini"],
        )?;
    }
    if let Some(model) = settings.youtube_transcription_model.as_deref() {
        validate_allowed_import_setting(
            "youtube_transcription_model",
            model,
            &[
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
                "gemini-3.1-flash-lite",
                "gemini-3.5-flash",
            ],
        )?;
    }
    if let Some(value) = settings.youtube_transcription_timeout_seconds {
        validate_allowed_import_setting_i64(
            "youtube_transcription_timeout_seconds",
            value,
            &[45, 90, 180, 300, 600],
        )?;
    }
    if let Some(provider) = settings.general_analysis_provider.as_deref() {
        validate_allowed_import_setting(
            "general_analysis_provider",
            provider,
            &["", "provider_gemini"],
        )?;
    }
    if let Some(model) = settings.general_analysis_model.as_deref() {
        validate_allowed_import_setting(
            "general_analysis_model",
            model,
            &[
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
                "gemini-3.1-flash-lite",
                "gemini-3.5-flash",
            ],
        )?;
    }
    if let Some(value) = settings.general_analysis_timeout_seconds {
        validate_allowed_import_setting_i64(
            "general_analysis_timeout_seconds",
            value,
            &[45, 90, 180, 300, 600],
        )?;
    }
    if let Some(mode) = settings.ai_analysis_mode.as_deref() {
        validate_allowed_import_setting("ai_analysis_mode", mode, &["source_grounded"])?;
    }
    if let Some(level) = settings.log_level.as_deref() {
        validate_allowed_import_setting(
            "log_level",
            level,
            &["off", "error", "warn", "info", "debug", "trace"],
        )?;
    }
    if let Some(value) = settings.log_max_files {
        validate_import_i64_range("log_max_files", value, 1, 20)?;
    }
    if let Some(value) = settings.log_max_file_bytes {
        validate_import_i64_range("log_max_file_bytes", value, 1_048_576, 104_857_600)?;
    }

    Ok(SettingsUpdate {
        theme: settings.theme,
        accent_palette: settings.accent_palette,
        locale: settings.locale,
        poll_interval_seconds: settings.poll_interval_seconds,
        youtube_transcription_provider: settings.youtube_transcription_provider,
        youtube_transcription_model: settings.youtube_transcription_model,
        youtube_transcription_timeout_seconds: settings.youtube_transcription_timeout_seconds,
        general_analysis_provider: settings.general_analysis_provider,
        general_analysis_model: settings.general_analysis_model,
        general_analysis_timeout_seconds: settings.general_analysis_timeout_seconds,
        ai_analysis_mode: settings.ai_analysis_mode,
        log_level: settings.log_level,
        log_max_files: settings.log_max_files,
        log_max_file_bytes: settings.log_max_file_bytes,
        shortcut_bindings: settings.shortcut_bindings,
    })
}

fn settings_to_update_summary(settings: &ExportSettings) -> ImportApplySummary {
    let mut updated = 0usize;
    if settings.theme.is_some() {
        updated += 1;
    }
    if settings.accent_palette.is_some() {
        updated += 1;
    }
    if settings.locale.is_some() {
        updated += 1;
    }
    if settings.poll_interval_seconds.is_some() {
        updated += 1;
    }
    if settings.youtube_transcription_provider.is_some() {
        updated += 1;
    }
    if settings.youtube_transcription_model.is_some() {
        updated += 1;
    }
    if settings.youtube_transcription_timeout_seconds.is_some() {
        updated += 1;
    }
    if settings.general_analysis_provider.is_some() {
        updated += 1;
    }
    if settings.general_analysis_model.is_some() {
        updated += 1;
    }
    if settings.general_analysis_timeout_seconds.is_some() {
        updated += 1;
    }
    if settings.ai_analysis_mode.is_some() {
        updated += 1;
    }
    if settings.log_level.is_some() {
        updated += 1;
    }
    if settings.log_max_files.is_some() {
        updated += 1;
    }
    if settings.log_max_file_bytes.is_some() {
        updated += 1;
    }
    if settings.shortcut_bindings.is_some() {
        updated += 1;
    }
    ImportApplySummary {
        settings_updated: updated,
        ..ImportApplySummary::default()
    }
}

fn validate_allowed_import_setting(
    key: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn validate_allowed_import_setting_i64(
    key: &'static str,
    value: i64,
    allowed: &[i64],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_string(),
        })
    }
}

fn validate_import_i64_range(
    key: &'static str,
    value: i64,
    min: i64,
    max: i64,
) -> StorageResult<()> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_string(),
        })
    }
}

fn normalize_qualified_ticker(exchange: &str, ticker: &str) -> String {
    format!(
        "{}:{}",
        exchange.trim().to_uppercase(),
        ticker.trim().to_uppercase()
    )
}

fn now_rfc3339() -> StorageResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| StorageError::InvalidSettingValue {
            key: "import_export",
            value: error.to_string(),
        })
}
