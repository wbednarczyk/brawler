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
    pub ai_research_briefs: usize,
    pub ai_research_brief_citations: usize,
    pub research_reminders: usize,
    pub ai_research_digests: usize,
    pub ai_research_digest_citations: usize,
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
    pub ai_research_briefs_created: usize,
    pub ai_research_briefs_skipped: usize,
    pub ai_research_brief_citations_created: usize,
    pub ai_research_brief_citations_skipped: usize,
    pub research_reminders_created: usize,
    pub research_reminders_skipped: usize,
    pub ai_research_digests_created: usize,
    pub ai_research_digests_skipped: usize,
    pub ai_research_digest_citations_created: usize,
    pub ai_research_digest_citations_skipped: usize,
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
    #[serde(default)]
    ai_research_briefs: Vec<ExportAiResearchBrief>,
    #[serde(default)]
    ai_research_brief_citations: Vec<ExportAiResearchBriefCitation>,
    #[serde(default)]
    research_reminders: Vec<ExportResearchReminder>,
    #[serde(default)]
    ai_research_digests: Vec<ExportAiResearchDigest>,
    #[serde(default)]
    ai_research_digest_citations: Vec<ExportAiResearchDigestCitation>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAiResearchBrief {
    id: String,
    job_id: String,
    scope_type: String,
    scope_id: String,
    scope_company_qualified_ticker: Option<String>,
    provider_id: String,
    model: String,
    prompt_version: String,
    evidence_collector_version: String,
    renderer_version: String,
    title: String,
    summary: String,
    content_markdown: String,
    language: Option<String>,
    generated_at: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAiResearchBriefCitation {
    id: String,
    brief_id: String,
    citation_key: String,
    evidence_type: String,
    evidence_id: String,
    label: String,
    snippet: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportResearchReminder {
    id: String,
    scope_type: String,
    scope_id: String,
    scope_company_qualified_ticker: Option<String>,
    company_qualified_ticker: Option<String>,
    reminder_kind: String,
    source_type: Option<String>,
    source_id: Option<String>,
    title: String,
    body: String,
    due_at: Option<String>,
    status: String,
    snoozed_until: Option<String>,
    completed_at: Option<String>,
    dismissed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAiResearchDigest {
    id: String,
    job_id: String,
    scope_type: String,
    scope_id: String,
    scope_company_qualified_ticker: Option<String>,
    provider_id: String,
    model: String,
    prompt_version: String,
    evidence_collector_version: String,
    renderer_version: String,
    title: String,
    summary: String,
    content_markdown: String,
    language: Option<String>,
    generated_at: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAiResearchDigestCitation {
    id: String,
    digest_id: String,
    citation_key: String,
    evidence_type: String,
    evidence_id: String,
    label: String,
    snippet: Option<String>,
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
    let ai_research_briefs = export_ai_research_briefs(connection)?;
    let ai_research_brief_citations = export_ai_research_brief_citations(connection)?;
    let research_reminders = export_research_reminders(connection)?;
    let ai_research_digests = export_ai_research_digests(connection)?;
    let ai_research_digest_citations = export_ai_research_digest_citations(connection)?;
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
            "ai_research_briefs".to_owned(),
            "research_reminders".to_owned(),
            "ai_research_digests".to_owned(),
        ],
        companies,
        watchlists,
        memberships,
        notebook_entries,
        research_questions,
        evidence_links,
        ai_research_briefs,
        ai_research_brief_citations,
        research_reminders,
        ai_research_digests,
        ai_research_digest_citations,
    };
    let summary = ImportExportSummary {
        companies: document.companies.len(),
        watchlists: document.watchlists.len(),
        memberships: document.memberships.len(),
        notebook_entries: document.notebook_entries.len(),
        research_questions: document.research_questions.len(),
        evidence_links: document.evidence_links.len(),
        ai_research_briefs: document.ai_research_briefs.len(),
        ai_research_brief_citations: document.ai_research_brief_citations.len(),
        research_reminders: document.research_reminders.len(),
        ai_research_digests: document.ai_research_digests.len(),
        ai_research_digest_citations: document.ai_research_digest_citations.len(),
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

    let existing_brief_ids = existing_ids(connection, "ai_research_briefs").unwrap_or_default();
    let imported_brief_ids = document
        .ai_research_briefs
        .iter()
        .map(|brief| brief.id.clone())
        .collect::<HashSet<_>>();
    for brief in &document.ai_research_briefs {
        if existing_brief_ids.contains(&brief.id) {
            summary.ai_research_briefs_skipped += 1;
            warnings.push(format!(
                "Research brief {} already exists and will be skipped",
                brief.id
            ));
            continue;
        }
        if brief.scope_type == "company" {
            let Some(company_ticker) = brief.scope_company_qualified_ticker.as_deref() else {
                errors.push(format!(
                    "Research brief {} is missing company scope ticker",
                    brief.id
                ));
                continue;
            };
            let company_ticker = company_ticker.trim().to_uppercase();
            if !imported_company_tickers.contains(&company_ticker)
                && !existing_companies.contains_key(&company_ticker)
            {
                errors.push(format!(
                    "Research brief {} references missing company {}",
                    brief.id, company_ticker
                ));
                continue;
            }
        } else if brief.scope_type != "watchlist" {
            errors.push(format!(
                "Research brief {} has unsupported scope {}",
                brief.id, brief.scope_type
            ));
            continue;
        } else if !imported_watchlist_ids.contains(&brief.scope_id)
            && !existing_watchlist_ids.contains(&brief.scope_id)
        {
            errors.push(format!(
                "Research brief {} references missing watchlist {}",
                brief.id, brief.scope_id
            ));
            continue;
        }

        if brief.title.trim().is_empty()
            || brief.summary.trim().is_empty()
            || brief.content_markdown.trim().is_empty()
        {
            errors.push(format!("Research brief {} is missing content", brief.id));
            continue;
        }
        summary.ai_research_briefs_created += 1;
    }

    let existing_citation_ids =
        existing_ids(connection, "ai_research_brief_citations").unwrap_or_default();
    for citation in &document.ai_research_brief_citations {
        if existing_citation_ids.contains(&citation.id) {
            summary.ai_research_brief_citations_skipped += 1;
            warnings.push(format!(
                "Research brief citation {} already exists and will be skipped",
                citation.id
            ));
            continue;
        }
        if !imported_brief_ids.contains(&citation.brief_id)
            && !existing_brief_ids.contains(&citation.brief_id)
        {
            summary.ai_research_brief_citations_skipped += 1;
            warnings.push(format!(
                "Research brief citation {} references unavailable brief and will be skipped",
                citation.id
            ));
            continue;
        }
        if !imported_or_existing_evidence_reference(
            connection,
            &citation.evidence_type,
            &citation.evidence_id,
            &imported_note_ids,
            &imported_question_ids,
        ) {
            summary.ai_research_brief_citations_skipped += 1;
            warnings.push(format!(
                "Research brief citation {} references unavailable evidence and will be skipped",
                citation.id
            ));
            continue;
        }
        summary.ai_research_brief_citations_created += 1;
    }

    let existing_reminder_ids = existing_ids(connection, "research_reminders").unwrap_or_default();
    let imported_reminder_ids = document
        .research_reminders
        .iter()
        .map(|reminder| reminder.id.clone())
        .collect::<HashSet<_>>();
    for reminder in &document.research_reminders {
        if existing_reminder_ids.contains(&reminder.id) {
            summary.research_reminders_skipped += 1;
            warnings.push(format!(
                "Research reminder {} already exists and will be skipped",
                reminder.id
            ));
            continue;
        }
        if reminder.id.trim().is_empty() || reminder.title.trim().is_empty() {
            errors.push("Research reminder id and title are required".to_owned());
            continue;
        }
        if ![
            "claim_follow_up",
            "event_review",
            "question_review",
            "manual_research",
            "digest_review",
        ]
        .contains(&reminder.reminder_kind.as_str())
        {
            errors.push(format!(
                "Research reminder {} has unsupported kind {}",
                reminder.id, reminder.reminder_kind
            ));
            continue;
        }
        if !["open", "completed", "dismissed"].contains(&reminder.status.as_str()) {
            errors.push(format!(
                "Research reminder {} has unsupported status {}",
                reminder.id, reminder.status
            ));
            continue;
        }
        if reminder.scope_type == "company" {
            let Some(company_ticker) = reminder.scope_company_qualified_ticker.as_deref() else {
                errors.push(format!(
                    "Research reminder {} is missing company scope ticker",
                    reminder.id
                ));
                continue;
            };
            let company_ticker = company_ticker.trim().to_uppercase();
            if !imported_company_tickers.contains(&company_ticker)
                && !existing_companies.contains_key(&company_ticker)
            {
                errors.push(format!(
                    "Research reminder {} references missing company {}",
                    reminder.id, company_ticker
                ));
                continue;
            }
        } else if reminder.scope_type != "watchlist" {
            errors.push(format!(
                "Research reminder {} has unsupported scope {}",
                reminder.id, reminder.scope_type
            ));
            continue;
        } else if !imported_watchlist_ids.contains(&reminder.scope_id)
            && !existing_watchlist_ids.contains(&reminder.scope_id)
        {
            errors.push(format!(
                "Research reminder {} references missing watchlist {}",
                reminder.id, reminder.scope_id
            ));
            continue;
        }
        if let Some(company_ticker) = reminder.company_qualified_ticker.as_deref() {
            let company_ticker = company_ticker.trim().to_uppercase();
            if !imported_company_tickers.contains(&company_ticker)
                && !existing_companies.contains_key(&company_ticker)
            {
                errors.push(format!(
                    "Research reminder {} references missing company {}",
                    reminder.id, company_ticker
                ));
                continue;
            }
        }
        summary.research_reminders_created += 1;
    }

    let existing_digest_ids = existing_ids(connection, "ai_research_digests").unwrap_or_default();
    let imported_digest_ids = document
        .ai_research_digests
        .iter()
        .map(|digest| digest.id.clone())
        .collect::<HashSet<_>>();
    for digest in &document.ai_research_digests {
        if existing_digest_ids.contains(&digest.id) {
            summary.ai_research_digests_skipped += 1;
            warnings.push(format!(
                "Research digest {} already exists and will be skipped",
                digest.id
            ));
            continue;
        }
        if digest.scope_type == "company" {
            let Some(company_ticker) = digest.scope_company_qualified_ticker.as_deref() else {
                errors.push(format!(
                    "Research digest {} is missing company scope ticker",
                    digest.id
                ));
                continue;
            };
            let company_ticker = company_ticker.trim().to_uppercase();
            if !imported_company_tickers.contains(&company_ticker)
                && !existing_companies.contains_key(&company_ticker)
            {
                errors.push(format!(
                    "Research digest {} references missing company {}",
                    digest.id, company_ticker
                ));
                continue;
            }
        } else if digest.scope_type != "watchlist" {
            errors.push(format!(
                "Research digest {} has unsupported scope {}",
                digest.id, digest.scope_type
            ));
            continue;
        } else if !imported_watchlist_ids.contains(&digest.scope_id)
            && !existing_watchlist_ids.contains(&digest.scope_id)
        {
            errors.push(format!(
                "Research digest {} references missing watchlist {}",
                digest.id, digest.scope_id
            ));
            continue;
        }
        if digest.title.trim().is_empty()
            || digest.summary.trim().is_empty()
            || digest.content_markdown.trim().is_empty()
        {
            errors.push(format!("Research digest {} is missing content", digest.id));
            continue;
        }
        summary.ai_research_digests_created += 1;
    }

    let existing_digest_citation_ids =
        existing_ids(connection, "ai_research_digest_citations").unwrap_or_default();
    for citation in &document.ai_research_digest_citations {
        if existing_digest_citation_ids.contains(&citation.id) {
            summary.ai_research_digest_citations_skipped += 1;
            warnings.push(format!(
                "Research digest citation {} already exists and will be skipped",
                citation.id
            ));
            continue;
        }
        if !imported_digest_ids.contains(&citation.digest_id)
            && !existing_digest_ids.contains(&citation.digest_id)
        {
            summary.ai_research_digest_citations_skipped += 1;
            warnings.push(format!(
                "Research digest citation {} references unavailable digest and will be skipped",
                citation.id
            ));
            continue;
        }
        if !imported_or_existing_evidence_reference_with_reminders(
            connection,
            &citation.evidence_type,
            &citation.evidence_id,
            &imported_note_ids,
            &imported_question_ids,
            &imported_reminder_ids,
        ) {
            summary.ai_research_digest_citations_skipped += 1;
            warnings.push(format!(
                "Research digest citation {} references unavailable evidence and will be skipped",
                citation.id
            ));
            continue;
        }
        summary.ai_research_digest_citations_created += 1;
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
    summary.ai_research_briefs_created = 0;
    summary.ai_research_briefs_skipped = 0;
    summary.ai_research_brief_citations_created = 0;
    summary.ai_research_brief_citations_skipped = 0;
    summary.research_reminders_created = 0;
    summary.research_reminders_skipped = 0;
    summary.ai_research_digests_created = 0;
    summary.ai_research_digests_skipped = 0;
    summary.ai_research_digest_citations_created = 0;
    summary.ai_research_digest_citations_skipped = 0;
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

    for brief in &document.ai_research_briefs {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM ai_research_briefs WHERE id = ?1)",
            [&brief.id],
            |row| row.get(0),
        )?;
        if exists {
            summary.ai_research_briefs_skipped += 1;
            continue;
        }

        let scope_id = if brief.scope_type == "company" {
            let company_ticker = brief
                .scope_company_qualified_ticker
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_uppercase();
            company_id_by_ticker
                .get(&company_ticker)
                .ok_or_else(|| StorageError::InvalidSettingValue {
                    key: "import_export",
                    value: format!("missing brief company {company_ticker}"),
                })?
                .to_owned()
        } else if brief.scope_type == "watchlist" {
            watchlist_id_map
                .get(&brief.scope_id)
                .map(String::as_str)
                .unwrap_or(brief.scope_id.trim())
                .to_owned()
        } else {
            summary.ai_research_briefs_skipped += 1;
            continue;
        };

        connection.execute(
            "
            INSERT INTO ai_research_brief_jobs (
                id,
                scope_type,
                scope_id,
                provider_id,
                model,
                prompt_version,
                evidence_collector_version,
                renderer_version,
                status,
                created_at,
                started_at,
                finished_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'succeeded', ?9, ?9, ?9)
            ON CONFLICT(id) DO NOTHING
            ",
            params![
                brief.job_id.trim(),
                brief.scope_type.trim(),
                scope_id,
                brief.provider_id.trim(),
                brief.model.trim(),
                brief.prompt_version.trim(),
                brief.evidence_collector_version.trim(),
                brief.renderer_version.trim(),
                brief.created_at.trim(),
            ],
        )?;

        connection.execute(
            "
            INSERT INTO ai_research_briefs (
                id,
                job_id,
                scope_type,
                scope_id,
                provider_id,
                model,
                prompt_version,
                evidence_collector_version,
                renderer_version,
                title,
                summary,
                content_markdown,
                language,
                generated_at,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ",
            params![
                brief.id.trim(),
                brief.job_id.trim(),
                brief.scope_type.trim(),
                scope_id,
                brief.provider_id.trim(),
                brief.model.trim(),
                brief.prompt_version.trim(),
                brief.evidence_collector_version.trim(),
                brief.renderer_version.trim(),
                brief.title.trim(),
                brief.summary.trim(),
                brief.content_markdown.trim(),
                empty_string_to_none(brief.language.clone()),
                brief.generated_at.trim(),
                brief.created_at.trim(),
            ],
        )?;
        summary.ai_research_briefs_created += 1;
    }

    for citation in &document.ai_research_brief_citations {
        if !evidence_reference_exists_for_import(
            connection,
            &citation.evidence_type,
            &citation.evidence_id,
        )? {
            summary.ai_research_brief_citations_skipped += 1;
            continue;
        }
        let inserted = connection.execute(
            "
            INSERT OR IGNORE INTO ai_research_brief_citations (
                id,
                brief_id,
                citation_key,
                evidence_type,
                evidence_id,
                label,
                snippet,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                citation.id.trim(),
                citation.brief_id.trim(),
                citation.citation_key.trim(),
                citation.evidence_type.trim(),
                citation.evidence_id.trim(),
                citation.label.trim(),
                citation.snippet.clone(),
                citation.created_at.trim(),
            ],
        )?;
        if inserted == 0 {
            summary.ai_research_brief_citations_skipped += 1;
        } else {
            summary.ai_research_brief_citations_created += 1;
        }
    }

    for reminder in &document.research_reminders {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM research_reminders WHERE id = ?1)",
            [&reminder.id],
            |row| row.get(0),
        )?;
        if exists {
            summary.research_reminders_skipped += 1;
            continue;
        }

        let scope_id = if reminder.scope_type == "company" {
            let company_ticker = reminder
                .scope_company_qualified_ticker
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_uppercase();
            company_id_by_ticker
                .get(&company_ticker)
                .ok_or_else(|| StorageError::InvalidSettingValue {
                    key: "import_export",
                    value: format!("missing reminder company scope {company_ticker}"),
                })?
                .to_owned()
        } else if reminder.scope_type == "watchlist" {
            watchlist_id_map
                .get(&reminder.scope_id)
                .map(String::as_str)
                .unwrap_or(reminder.scope_id.trim())
                .to_owned()
        } else {
            summary.research_reminders_skipped += 1;
            continue;
        };
        let company_id = match reminder.company_qualified_ticker.as_deref() {
            Some(ticker) if !ticker.trim().is_empty() => company_id_by_ticker
                .get(&ticker.trim().to_uppercase())
                .cloned(),
            _ => None,
        };

        connection.execute(
            "
            INSERT INTO research_reminders (
                id,
                scope_type,
                scope_id,
                company_id,
                reminder_kind,
                source_type,
                source_id,
                title,
                body,
                due_at,
                status,
                snoozed_until,
                completed_at,
                dismissed_at,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ",
            params![
                reminder.id.trim(),
                reminder.scope_type.trim(),
                scope_id,
                company_id,
                reminder.reminder_kind.trim(),
                empty_string_to_none(reminder.source_type.clone()),
                empty_string_to_none(reminder.source_id.clone()),
                reminder.title.trim(),
                reminder.body.trim(),
                empty_string_to_none(reminder.due_at.clone()),
                reminder.status.trim(),
                empty_string_to_none(reminder.snoozed_until.clone()),
                empty_string_to_none(reminder.completed_at.clone()),
                empty_string_to_none(reminder.dismissed_at.clone()),
                reminder.created_at.trim(),
                reminder.updated_at.trim(),
            ],
        )?;
        summary.research_reminders_created += 1;
    }

    for digest in &document.ai_research_digests {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM ai_research_digests WHERE id = ?1)",
            [&digest.id],
            |row| row.get(0),
        )?;
        if exists {
            summary.ai_research_digests_skipped += 1;
            continue;
        }

        let scope_id = if digest.scope_type == "company" {
            let company_ticker = digest
                .scope_company_qualified_ticker
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_uppercase();
            company_id_by_ticker
                .get(&company_ticker)
                .ok_or_else(|| StorageError::InvalidSettingValue {
                    key: "import_export",
                    value: format!("missing digest company {company_ticker}"),
                })?
                .to_owned()
        } else if digest.scope_type == "watchlist" {
            watchlist_id_map
                .get(&digest.scope_id)
                .map(String::as_str)
                .unwrap_or(digest.scope_id.trim())
                .to_owned()
        } else {
            summary.ai_research_digests_skipped += 1;
            continue;
        };

        connection.execute(
            "
            INSERT INTO ai_research_digest_jobs (
                id,
                scope_type,
                scope_id,
                provider_id,
                model,
                prompt_version,
                evidence_collector_version,
                renderer_version,
                status,
                created_at,
                started_at,
                finished_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'succeeded', ?9, ?9, ?9)
            ON CONFLICT(id) DO NOTHING
            ",
            params![
                digest.job_id.trim(),
                digest.scope_type.trim(),
                scope_id,
                digest.provider_id.trim(),
                digest.model.trim(),
                digest.prompt_version.trim(),
                digest.evidence_collector_version.trim(),
                digest.renderer_version.trim(),
                digest.created_at.trim(),
            ],
        )?;

        connection.execute(
            "
            INSERT INTO ai_research_digests (
                id,
                job_id,
                scope_type,
                scope_id,
                provider_id,
                model,
                prompt_version,
                evidence_collector_version,
                renderer_version,
                title,
                summary,
                content_markdown,
                language,
                generated_at,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ",
            params![
                digest.id.trim(),
                digest.job_id.trim(),
                digest.scope_type.trim(),
                scope_id,
                digest.provider_id.trim(),
                digest.model.trim(),
                digest.prompt_version.trim(),
                digest.evidence_collector_version.trim(),
                digest.renderer_version.trim(),
                digest.title.trim(),
                digest.summary.trim(),
                digest.content_markdown.trim(),
                empty_string_to_none(digest.language.clone()),
                digest.generated_at.trim(),
                digest.created_at.trim(),
            ],
        )?;
        summary.ai_research_digests_created += 1;
    }

    for citation in &document.ai_research_digest_citations {
        if !evidence_reference_exists_for_import(
            connection,
            &citation.evidence_type,
            &citation.evidence_id,
        )? {
            summary.ai_research_digest_citations_skipped += 1;
            continue;
        }
        let inserted = connection.execute(
            "
            INSERT OR IGNORE INTO ai_research_digest_citations (
                id,
                digest_id,
                citation_key,
                evidence_type,
                evidence_id,
                label,
                snippet,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                citation.id.trim(),
                citation.digest_id.trim(),
                citation.citation_key.trim(),
                citation.evidence_type.trim(),
                citation.evidence_id.trim(),
                citation.label.trim(),
                citation.snippet.clone(),
                citation.created_at.trim(),
            ],
        )?;
        if inserted == 0 {
            summary.ai_research_digest_citations_skipped += 1;
        } else {
            summary.ai_research_digest_citations_created += 1;
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

fn export_ai_research_briefs(connection: &Connection) -> StorageResult<Vec<ExportAiResearchBrief>> {
    let mut statement = connection.prepare(
        "
        SELECT
            ai_research_briefs.id,
            ai_research_briefs.job_id,
            ai_research_briefs.scope_type,
            ai_research_briefs.scope_id,
            companies.qualified_ticker,
            ai_research_briefs.provider_id,
            ai_research_briefs.model,
            ai_research_briefs.prompt_version,
            ai_research_briefs.evidence_collector_version,
            ai_research_briefs.renderer_version,
            ai_research_briefs.title,
            ai_research_briefs.summary,
            ai_research_briefs.content_markdown,
            ai_research_briefs.language,
            ai_research_briefs.generated_at,
            ai_research_briefs.created_at
        FROM ai_research_briefs
        LEFT JOIN companies
            ON ai_research_briefs.scope_type = 'company'
            AND companies.id = ai_research_briefs.scope_id
        ORDER BY datetime(ai_research_briefs.generated_at) DESC, ai_research_briefs.id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportAiResearchBrief {
            id: row.get(0)?,
            job_id: row.get(1)?,
            scope_type: row.get(2)?,
            scope_id: row.get(3)?,
            scope_company_qualified_ticker: row.get(4)?,
            provider_id: row.get(5)?,
            model: row.get(6)?,
            prompt_version: row.get(7)?,
            evidence_collector_version: row.get(8)?,
            renderer_version: row.get(9)?,
            title: row.get(10)?,
            summary: row.get(11)?,
            content_markdown: row.get(12)?,
            language: row.get(13)?,
            generated_at: row.get(14)?,
            created_at: row.get(15)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn export_ai_research_brief_citations(
    connection: &Connection,
) -> StorageResult<Vec<ExportAiResearchBriefCitation>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            brief_id,
            citation_key,
            evidence_type,
            evidence_id,
            label,
            snippet,
            created_at
        FROM ai_research_brief_citations
        ORDER BY brief_id, citation_key, id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportAiResearchBriefCitation {
            id: row.get(0)?,
            brief_id: row.get(1)?,
            citation_key: row.get(2)?,
            evidence_type: row.get(3)?,
            evidence_id: row.get(4)?,
            label: row.get(5)?,
            snippet: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn export_research_reminders(
    connection: &Connection,
) -> StorageResult<Vec<ExportResearchReminder>> {
    let mut statement = connection.prepare(
        "
        SELECT
            research_reminders.id,
            research_reminders.scope_type,
            research_reminders.scope_id,
            scope_companies.qualified_ticker,
            reminder_companies.qualified_ticker,
            research_reminders.reminder_kind,
            research_reminders.source_type,
            research_reminders.source_id,
            research_reminders.title,
            research_reminders.body,
            research_reminders.due_at,
            research_reminders.status,
            research_reminders.snoozed_until,
            research_reminders.completed_at,
            research_reminders.dismissed_at,
            research_reminders.created_at,
            research_reminders.updated_at
        FROM research_reminders
        LEFT JOIN companies AS scope_companies
            ON research_reminders.scope_type = 'company'
            AND scope_companies.id = research_reminders.scope_id
        LEFT JOIN companies AS reminder_companies
            ON reminder_companies.id = research_reminders.company_id
        ORDER BY datetime(COALESCE(research_reminders.due_at, research_reminders.updated_at)) DESC,
            research_reminders.id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportResearchReminder {
            id: row.get(0)?,
            scope_type: row.get(1)?,
            scope_id: row.get(2)?,
            scope_company_qualified_ticker: row.get(3)?,
            company_qualified_ticker: row.get(4)?,
            reminder_kind: row.get(5)?,
            source_type: row.get(6)?,
            source_id: row.get(7)?,
            title: row.get(8)?,
            body: row.get(9)?,
            due_at: row.get(10)?,
            status: row.get(11)?,
            snoozed_until: row.get(12)?,
            completed_at: row.get(13)?,
            dismissed_at: row.get(14)?,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn export_ai_research_digests(
    connection: &Connection,
) -> StorageResult<Vec<ExportAiResearchDigest>> {
    let mut statement = connection.prepare(
        "
        SELECT
            ai_research_digests.id,
            ai_research_digests.job_id,
            ai_research_digests.scope_type,
            ai_research_digests.scope_id,
            companies.qualified_ticker,
            ai_research_digests.provider_id,
            ai_research_digests.model,
            ai_research_digests.prompt_version,
            ai_research_digests.evidence_collector_version,
            ai_research_digests.renderer_version,
            ai_research_digests.title,
            ai_research_digests.summary,
            ai_research_digests.content_markdown,
            ai_research_digests.language,
            ai_research_digests.generated_at,
            ai_research_digests.created_at
        FROM ai_research_digests
        LEFT JOIN companies
            ON ai_research_digests.scope_type = 'company'
            AND companies.id = ai_research_digests.scope_id
        ORDER BY datetime(ai_research_digests.generated_at) DESC, ai_research_digests.id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportAiResearchDigest {
            id: row.get(0)?,
            job_id: row.get(1)?,
            scope_type: row.get(2)?,
            scope_id: row.get(3)?,
            scope_company_qualified_ticker: row.get(4)?,
            provider_id: row.get(5)?,
            model: row.get(6)?,
            prompt_version: row.get(7)?,
            evidence_collector_version: row.get(8)?,
            renderer_version: row.get(9)?,
            title: row.get(10)?,
            summary: row.get(11)?,
            content_markdown: row.get(12)?,
            language: row.get(13)?,
            generated_at: row.get(14)?,
            created_at: row.get(15)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn export_ai_research_digest_citations(
    connection: &Connection,
) -> StorageResult<Vec<ExportAiResearchDigestCitation>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            digest_id,
            citation_key,
            evidence_type,
            evidence_id,
            label,
            snippet,
            created_at
        FROM ai_research_digest_citations
        ORDER BY digest_id, citation_key, id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportAiResearchDigestCitation {
            id: row.get(0)?,
            digest_id: row.get(1)?,
            citation_key: row.get(2)?,
            evidence_type: row.get(3)?,
            evidence_id: row.get(4)?,
            label: row.get(5)?,
            snippet: row.get(6)?,
            created_at: row.get(7)?,
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

fn imported_or_existing_evidence_reference_with_reminders(
    connection: &Connection,
    evidence_type: &str,
    evidence_id: &str,
    imported_note_ids: &HashSet<String>,
    imported_question_ids: &HashSet<String>,
    imported_reminder_ids: &HashSet<String>,
) -> bool {
    match evidence_type {
        "reminder" if imported_reminder_ids.contains(evidence_id) => true,
        _ => imported_or_existing_evidence_reference(
            connection,
            evidence_type,
            evidence_id,
            imported_note_ids,
            imported_question_ids,
        ),
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
        "reminder" => table_reference_exists(connection, "research_reminders", evidence_id),
        "digest" => table_reference_exists(connection, "ai_research_digests", evidence_id),
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
