use super::*;

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
    let summary = apply_quality_frameworks(&transaction, &document, summary)?;
    transaction.commit()?;

    Ok(ImportApplyResult {
        summary,
        warnings: preview.warnings,
    })
}

fn parse_research_document(contents: &str) -> StorageResult<ResearchExportDocument> {
    serde_json::from_str::<ResearchExportDocument>(contents).map_err(StorageError::from)
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

    let existing_claim_ids = existing_ids(connection, "management_claims").unwrap_or_default();
    let imported_claim_ids = document
        .management_claims
        .iter()
        .map(|claim| claim.id.clone())
        .collect::<HashSet<_>>();
    for claim in &document.management_claims {
        if claim.id.trim().is_empty() || claim.statement.trim().is_empty() {
            errors.push("Management claim id and statement are required".to_owned());
            continue;
        }
        if existing_claim_ids.contains(&claim.id) {
            summary.management_claims_skipped += 1;
            warnings.push(format!(
                "Management claim {} already exists and will be skipped",
                claim.id
            ));
            continue;
        }
        let company_ticker = claim.company_qualified_ticker.trim().to_uppercase();
        if !imported_company_tickers.contains(&company_ticker)
            && !existing_companies.contains_key(&company_ticker)
        {
            errors.push(format!(
                "Management claim {} references missing company {}",
                claim.id, claim.company_qualified_ticker
            ));
            continue;
        }
        if ![
            "pending",
            "delivered",
            "partially_delivered",
            "missed",
            "revised",
        ]
        .contains(&claim.status.as_str())
        {
            errors.push(format!(
                "Management claim {} has unsupported status {}",
                claim.id, claim.status
            ));
            continue;
        }
        summary.management_claims_created += 1;
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
            &imported_claim_ids,
            &imported_question_ids,
        ) || !imported_or_existing_evidence_reference(
            connection,
            &link.to_type,
            &link.to_id,
            &imported_note_ids,
            &imported_claim_ids,
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
            &imported_claim_ids,
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
            &imported_claim_ids,
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

    // Quality frameworks + user metrics (ADR 0046): global, keyed by id; created
    // when absent, skipped when already present. Counts are advisory (the apply
    // step is the source of truth and re-counts).
    let existing_framework_ids = existing_ids(connection, "quality_frameworks").unwrap_or_default();
    for framework in &document.quality_frameworks {
        if framework.id.trim().is_empty() || framework.name.trim().is_empty() {
            errors.push("Quality framework id and name are required".to_owned());
            continue;
        }
        if existing_framework_ids.contains(&framework.id) {
            summary.quality_frameworks_skipped += 1;
        } else {
            summary.quality_frameworks_created += 1;
        }
    }
    let existing_metric_ids = existing_ids(connection, "kpi_definitions").unwrap_or_default();
    for metric in &document.user_metrics {
        if existing_metric_ids.contains(&metric.id) {
            summary.user_metrics_skipped += 1;
        } else {
            summary.user_metrics_created += 1;
        }
    }

    ImportPreview {
        valid: errors.is_empty(),
        summary,
        warnings,
        errors,
    }
}

/// Apply imported quality frameworks + user metrics (ADR 0046). Global state,
/// keyed by id: created when absent, skipped when present. User metrics import
/// first so a framework referencing one resolves. An `app_template` framework
/// whose `template_key` the receiving app already has is skipped (the template
/// is already shipped), and a dangling `cloned_from` is dropped to null.
fn apply_quality_frameworks(
    connection: &Connection,
    document: &ResearchExportDocument,
    mut summary: ImportApplySummary,
) -> StorageResult<ImportApplySummary> {
    summary.quality_frameworks_created = 0;
    summary.quality_frameworks_skipped = 0;
    summary.user_metrics_created = 0;
    summary.user_metrics_skipped = 0;

    for metric in &document.user_metrics {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM kpi_definitions WHERE id = ?1)",
            [&metric.id],
            |row| row.get(0),
        )?;
        if exists {
            summary.user_metrics_skipped += 1;
            continue;
        }
        connection.execute(
            "INSERT INTO kpi_definitions
                (id, scope, metric_key, label, value_kind, unit, computation, formula, display_format)
             VALUES (?1, 'user', ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                metric.id.trim(),
                metric.metric_key.trim(),
                metric.label.trim(),
                metric.value_kind.trim(),
                empty_string_to_none(metric.unit.clone()),
                metric.computation.trim(),
                empty_string_to_none(metric.formula.clone()),
                empty_string_to_none(metric.display_format.clone()),
            ],
        )?;
        summary.user_metrics_created += 1;
    }

    for framework in &document.quality_frameworks {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM quality_frameworks WHERE id = ?1)",
            [&framework.id],
            |row| row.get(0),
        )?;
        // Skip if the template is already shipped here (template_key is unique).
        let template_taken: bool = match framework.template_key.as_deref() {
            Some(key) if !key.is_empty() => connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM quality_frameworks WHERE template_key = ?1)",
                [key],
                |row| row.get(0),
            )?,
            _ => false,
        };
        if exists || template_taken {
            summary.quality_frameworks_skipped += 1;
            continue;
        }

        // Drop a dangling cloned_from rather than violate the FK.
        let cloned_from = match framework.cloned_from.as_deref() {
            Some(id) if !id.is_empty() => {
                let present: bool = connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM quality_frameworks WHERE id = ?1)",
                    [id],
                    |row| row.get(0),
                )?;
                if present {
                    Some(id.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        };

        connection.execute(
            "INSERT INTO quality_frameworks
                (id, name, description, origin, template_key, cloned_from, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                framework.id.trim(),
                framework.name.trim(),
                empty_string_to_none(framework.description.clone()),
                framework.origin.trim(),
                empty_string_to_none(framework.template_key.clone()),
                cloned_from,
                framework.version,
            ],
        )?;
        for criterion in &framework.criteria {
            // ADR 0075: carry kind + guidance so a qualitative criterion round-trips
            // (a pre-v0.50 bundle has no kind ⇒ quantitative, guidance ⇒ None).
            let kind = criterion
                .kind
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("quantitative");
            connection.execute(
                "INSERT INTO framework_criteria
                    (id, framework_id, ordinal, label, expression, weight, partial_band,
                     kind, assessment_guidance)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    criterion.id.trim(),
                    framework.id.trim(),
                    criterion.ordinal,
                    criterion.label.trim(),
                    criterion.expression.trim(),
                    empty_string_to_none(criterion.weight.clone()),
                    empty_string_to_none(criterion.partial_band.clone()),
                    kind,
                    empty_string_to_none(criterion.assessment_guidance.clone()),
                ],
            )?;
        }
        summary.quality_frameworks_created += 1;
    }

    Ok(summary)
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

    for claim in &document.management_claims {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM management_claims WHERE id = ?1)",
            [&claim.id],
            |row| row.get(0),
        )?;
        if exists {
            summary.management_claims_skipped += 1;
            continue;
        }

        let company_id = company_id_by_ticker
            .get(&claim.company_qualified_ticker.trim().to_uppercase())
            .ok_or_else(|| StorageError::InvalidSettingValue {
                key: "import_export",
                value: format!("missing claim company {}", claim.company_qualified_ticker),
            })?;

        connection.execute(
            "
            INSERT INTO management_claims (
                id, company_id, statement, body, body_format, made_at,
                due_fiscal_year, due_period_type, status, source_evidence_type,
                source_evidence_id, target_metric_key, target_comparator,
                target_value_numeric, target_unit, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ",
            params![
                claim.id.trim(),
                company_id,
                claim.statement.trim(),
                claim.body.trim(),
                claim.body_format.trim(),
                empty_string_to_none(claim.made_at.clone()),
                claim.due_fiscal_year,
                empty_string_to_none(claim.due_period_type.clone()),
                claim.status.trim(),
                claim.source_evidence_type.trim(),
                empty_string_to_none(claim.source_evidence_id.clone()),
                empty_string_to_none(claim.target_metric_key.clone()),
                empty_string_to_none(claim.target_comparator.clone()),
                empty_string_to_none(claim.target_value_numeric.clone()),
                empty_string_to_none(claim.target_unit.clone()),
                claim.created_at.trim(),
                claim.updated_at.trim(),
            ],
        )?;

        summary.management_claims_created += 1;
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
