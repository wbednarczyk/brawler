use super::*;

pub(super) fn export_research_data(connection: &Connection) -> StorageResult<ExportPayload> {
    let companies = export_companies(connection)?;
    let watchlists = export_watchlists(connection)?;
    let memberships = export_memberships(connection)?;
    let notebook_entries = export_notebook_entries(connection)?;
    let management_claims = export_management_claims(connection)?;
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
            "management_claims".to_owned(),
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
        management_claims,
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
        management_claims: document.management_claims.len(),
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

fn export_management_claims(connection: &Connection) -> StorageResult<Vec<ExportManagementClaim>> {
    let mut statement = connection.prepare(
        "
        SELECT
            management_claims.id,
            companies.qualified_ticker,
            management_claims.statement,
            management_claims.body,
            management_claims.body_format,
            management_claims.made_at,
            management_claims.due_fiscal_year,
            management_claims.due_period_type,
            management_claims.status,
            management_claims.source_evidence_type,
            management_claims.source_evidence_id,
            management_claims.target_metric_key,
            management_claims.target_comparator,
            management_claims.target_value_numeric,
            management_claims.target_unit,
            management_claims.created_at,
            management_claims.updated_at
        FROM management_claims
        INNER JOIN companies ON companies.id = management_claims.company_id
        ORDER BY companies.qualified_ticker, management_claims.updated_at DESC, management_claims.id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ExportManagementClaim {
            id: row.get(0)?,
            company_qualified_ticker: row.get(1)?,
            statement: row.get(2)?,
            body: row.get(3)?,
            body_format: row.get(4)?,
            made_at: row.get(5)?,
            due_fiscal_year: row.get(6)?,
            due_period_type: row.get(7)?,
            status: row.get(8)?,
            source_evidence_type: row.get(9)?,
            source_evidence_id: row.get(10)?,
            target_metric_key: row.get(11)?,
            target_comparator: row.get(12)?,
            target_value_numeric: row.get(13)?,
            target_unit: row.get(14)?,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
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
