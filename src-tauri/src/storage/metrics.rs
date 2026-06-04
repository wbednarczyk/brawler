use std::{
    collections::BTreeMap,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;
use serde::Serialize;

use super::{settings, StorageError, StorageResult};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Counter,
    Gauge,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricUnit {
    Count,
    Seconds,
    Bytes,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetricLabel {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetricSample {
    pub name: String,
    pub description: String,
    pub kind: MetricKind,
    pub unit: MetricUnit,
    pub value: f64,
    pub labels: Vec<MetricLabel>,
    pub collected_at: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalMetricsSnapshot {
    pub collected_at: String,
    pub samples: Vec<MetricSample>,
}

#[derive(Default)]
pub struct RuntimeMetricCounters {
    counters: Mutex<BTreeMap<RuntimeMetricKey, f64>>,
}

impl RuntimeMetricCounters {
    pub fn increment(&self, name: &'static str, labels: &[(&'static str, &str)]) {
        self.add(name, labels, 1.0);
    }

    pub fn add_counter_value(
        &self,
        name: &'static str,
        labels: &[(&'static str, &str)],
        value: f64,
    ) {
        if value.is_finite() && value >= 0.0 {
            self.add(name, labels, value);
        }
    }

    pub fn observe_duration_seconds(
        &self,
        name: &'static str,
        labels: &[(&'static str, &str)],
        duration_seconds: f64,
    ) {
        if duration_seconds.is_finite() && duration_seconds >= 0.0 {
            self.add(name, labels, duration_seconds);
        }
    }

    fn add(&self, name: &'static str, labels: &[(&'static str, &str)], value: f64) {
        if !is_allowed_metric_name(name)
            || labels
                .iter()
                .any(|(key, value)| !is_allowed_label(key, value))
        {
            return;
        }

        let key = RuntimeMetricKey::new(name, labels);
        let mut counters = self
            .counters
            .lock()
            .expect("runtime metrics mutex poisoned");
        *counters.entry(key).or_insert(0.0) += value;
    }

    fn snapshot(&self) -> Vec<(RuntimeMetricKey, f64)> {
        self.counters
            .lock()
            .expect("runtime metrics mutex poisoned")
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct RuntimeMetricKey {
    name: String,
    labels: Vec<(String, String)>,
}

impl RuntimeMetricKey {
    fn new(name: &str, labels: &[(&str, &str)]) -> Self {
        let mut labels = labels
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>();
        labels.sort();

        Self {
            name: name.to_owned(),
            labels,
        }
    }
}

trait MetricCollector {
    fn collect(
        &self,
        connection: &Connection,
        runtime_metrics: &RuntimeMetricCounters,
        app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>>;
}

pub(crate) fn collect_local_metrics_snapshot(
    connection: &Connection,
    runtime_metrics: &RuntimeMetricCounters,
    app_data_dir: &Path,
) -> StorageResult<LocalMetricsSnapshot> {
    let collected_at = current_timestamp(connection)?;
    let mut samples = Vec::new();

    for collector in collectors() {
        samples.extend(collector.collect(
            connection,
            runtime_metrics,
            app_data_dir,
            &collected_at,
        )?);
    }

    samples.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| label_sort_key(&left.labels).cmp(&label_sort_key(&right.labels)))
    });

    Ok(LocalMetricsSnapshot {
        collected_at,
        samples,
    })
}

fn collectors() -> [&'static dyn MetricCollector; 8] {
    [
        &SourceMetricsCollector,
        &AiAnalysisMetricsCollector,
        &TranscriptMetricsCollector,
        &CredentialMetricsCollector,
        &DiagnosticMetricsCollector,
        &LogMetricsCollector,
        &SqliteMetricsCollector,
        &RuntimeMetricsCollector,
    ]
}

struct SourceMetricsCollector;

impl MetricCollector for SourceMetricsCollector {
    fn collect(
        &self,
        connection: &Connection,
        _runtime_metrics: &RuntimeMetricCounters,
        _app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        let mut samples = Vec::new();
        let mut statement = connection.prepare(
            "
            SELECT
                source_adapters.id,
                source_adapters.enabled,
                source_adapters.last_error_at IS NOT NULL,
                COALESCE(fetched.state_value, '0'),
                COALESCE(created.state_value, '0'),
                COALESCE(matched.state_value, '0'),
                COALESCE(unmatched.state_value, '0'),
                COALESCE(detail_failed.state_value, '0')
            FROM source_adapters
            LEFT JOIN source_adapter_state fetched
                ON fetched.source_adapter_id = source_adapters.id
                AND fetched.state_key = 'last_items_fetched'
            LEFT JOIN source_adapter_state created
                ON created.source_adapter_id = source_adapters.id
                AND created.state_key = 'last_items_created'
            LEFT JOIN source_adapter_state matched
                ON matched.source_adapter_id = source_adapters.id
                AND matched.state_key = 'last_items_matched'
            LEFT JOIN source_adapter_state unmatched
                ON unmatched.source_adapter_id = source_adapters.id
                AND unmatched.state_key = 'last_items_unmatched'
            LEFT JOIN source_adapter_state detail_failed
                ON detail_failed.source_adapter_id = source_adapters.id
                AND detail_failed.state_key = 'last_detail_items_failed'
            ORDER BY source_adapters.id
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;

        for row in rows {
            let (
                adapter_id,
                enabled,
                has_error,
                fetched,
                created,
                matched,
                unmatched,
                detail_failed,
            ) = row?;
            let base_labels = [
                ("adapter_id", adapter_id.as_str()),
                ("status", if enabled == 1 { "enabled" } else { "disabled" }),
            ];

            push_sample(
                &mut samples,
                "brawler_source_last_items_fetched",
                "Items fetched by the adapter during the latest successful refresh.",
                MetricKind::Gauge,
                MetricUnit::Count,
                parse_metric_value(&fetched),
                &base_labels,
                collected_at,
            )?;
            push_sample(
                &mut samples,
                "brawler_source_last_items_created",
                "Items created by the adapter during the latest successful refresh.",
                MetricKind::Gauge,
                MetricUnit::Count,
                parse_metric_value(&created),
                &base_labels,
                collected_at,
            )?;
            push_sample(
                &mut samples,
                "brawler_source_last_items_matched",
                "Items matched by the adapter during the latest successful refresh.",
                MetricKind::Gauge,
                MetricUnit::Count,
                parse_metric_value(&matched),
                &base_labels,
                collected_at,
            )?;
            push_sample(
                &mut samples,
                "brawler_source_last_items_unmatched",
                "Items not matched by the adapter during the latest successful refresh.",
                MetricKind::Gauge,
                MetricUnit::Count,
                parse_metric_value(&unmatched),
                &base_labels,
                collected_at,
            )?;
            push_sample(
                &mut samples,
                "brawler_source_last_detail_items_failed",
                "Detail items that failed during the latest successful refresh.",
                MetricKind::Gauge,
                MetricUnit::Count,
                parse_metric_value(&detail_failed),
                &base_labels,
                collected_at,
            )?;
            push_sample(
                &mut samples,
                "brawler_source_adapter_error_state",
                "Whether the adapter currently has a recorded last error.",
                MetricKind::Gauge,
                MetricUnit::Count,
                if has_error == 1 { 1.0 } else { 0.0 },
                &base_labels,
                collected_at,
            )?;
        }

        let unmatched_total = count_unmatched_source_items(connection)?;
        push_sample(
            &mut samples,
            "brawler_source_unmatched_items_total",
            "Current unmatched source item records.",
            MetricKind::Gauge,
            MetricUnit::Count,
            unmatched_total,
            &[("collector", "sources")],
            collected_at,
        )?;

        Ok(samples)
    }
}

struct AiAnalysisMetricsCollector;

impl MetricCollector for AiAnalysisMetricsCollector {
    fn collect(
        &self,
        connection: &Connection,
        _runtime_metrics: &RuntimeMetricCounters,
        _app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        collect_job_status_metrics(
            connection,
            "ai_analysis_jobs",
            "brawler_ai_analysis_jobs_total",
            "AI analysis jobs by provider, model, and status.",
            &[
                ("provider_id", "provider_id"),
                ("model", "model"),
                ("status", "status"),
            ],
            collected_at,
        )
    }
}

struct TranscriptMetricsCollector;

impl MetricCollector for TranscriptMetricsCollector {
    fn collect(
        &self,
        connection: &Connection,
        _runtime_metrics: &RuntimeMetricCounters,
        _app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        let mut samples = collect_job_status_metrics(
            connection,
            "transcript_jobs",
            "brawler_transcript_jobs_total",
            "Transcript jobs by provider and status.",
            &[("provider_id", "provider_id"), ("status", "status")],
            collected_at,
        )?;
        push_sample(
            &mut samples,
            "brawler_transcript_segments_total",
            "Current transcript segment records.",
            MetricKind::Gauge,
            MetricUnit::Count,
            count_rows(connection, "transcript_segments")?,
            &[("collector", "transcripts")],
            collected_at,
        )?;

        Ok(samples)
    }
}

struct CredentialMetricsCollector;

impl MetricCollector for CredentialMetricsCollector {
    fn collect(
        &self,
        connection: &Connection,
        _runtime_metrics: &RuntimeMetricCounters,
        _app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        let settings = settings::get_settings(connection)?;
        let providers = [
            (
                "provider_gemini",
                "youtube_transcription",
                settings
                    .ai_providers
                    .youtube_transcription_provider
                    .as_str(),
            ),
            (
                "provider_gemini",
                "general_analysis",
                settings
                    .ai_providers
                    .general_analysis_provider
                    .as_deref()
                    .unwrap_or("not_configured"),
            ),
        ];
        let mut samples = Vec::new();

        for (provider_id, purpose, configured_provider) in providers {
            let status = if configured_provider == provider_id {
                "configured"
            } else {
                "not_configured"
            };
            push_sample(
                &mut samples,
                "brawler_credential_configuration_state",
                "Whether a credential-backed provider is selected in settings.",
                MetricKind::Gauge,
                MetricUnit::Count,
                if status == "configured" { 1.0 } else { 0.0 },
                &[
                    ("provider_id", provider_id),
                    ("module", purpose),
                    ("status", status),
                ],
                collected_at,
            )?;
        }

        Ok(samples)
    }
}

struct DiagnosticMetricsCollector;

impl MetricCollector for DiagnosticMetricsCollector {
    fn collect(
        &self,
        connection: &Connection,
        _runtime_metrics: &RuntimeMetricCounters,
        _app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        let mut samples = Vec::new();
        let mut statement = connection.prepare(
            "
            SELECT module, severity, COUNT(*)
            FROM diagnostic_events
            GROUP BY module, severity
            ORDER BY module, severity
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;

        for row in rows {
            let (module, severity, count) = row?;
            push_sample(
                &mut samples,
                "brawler_diagnostic_events_total",
                "Stored diagnostic events by module and severity.",
                MetricKind::Gauge,
                MetricUnit::Count,
                count as f64,
                &[("module", module.as_str()), ("severity", severity.as_str())],
                collected_at,
            )?;
        }

        Ok(samples)
    }
}

struct LogMetricsCollector;

impl MetricCollector for LogMetricsCollector {
    fn collect(
        &self,
        connection: &Connection,
        _runtime_metrics: &RuntimeMetricCounters,
        app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        let logs_dir = app_data_dir.join("logs");
        let mut total_bytes = 0u64;
        let mut file_count = 0u64;

        if let Ok(entries) = std::fs::read_dir(logs_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|value| value.to_str()) == Some("log") {
                    if let Ok(metadata) = entry.metadata() {
                        file_count += 1;
                        total_bytes += metadata.len();
                    }
                }
            }
        }

        let settings = settings::get_settings(connection)?;
        let mut samples = Vec::new();
        push_sample(
            &mut samples,
            "brawler_logs_files_total",
            "Current local runtime log files.",
            MetricKind::Gauge,
            MetricUnit::Count,
            file_count as f64,
            &[("collector", "logs")],
            collected_at,
        )?;
        push_sample(
            &mut samples,
            "brawler_logs_bytes",
            "Current total local runtime log size.",
            MetricKind::Gauge,
            MetricUnit::Bytes,
            total_bytes as f64,
            &[("collector", "logs")],
            collected_at,
        )?;
        push_sample(
            &mut samples,
            "brawler_logs_max_file_bytes",
            "Configured maximum size for each runtime log file.",
            MetricKind::Gauge,
            MetricUnit::Bytes,
            settings.logs.max_file_bytes as f64,
            &[("collector", "logs")],
            collected_at,
        )?;

        Ok(samples)
    }
}

struct SqliteMetricsCollector;

impl MetricCollector for SqliteMetricsCollector {
    fn collect(
        &self,
        connection: &Connection,
        _runtime_metrics: &RuntimeMetricCounters,
        app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        let database_bytes = database_size_bytes(connection, app_data_dir)?;
        let mut samples = Vec::new();
        push_sample(
            &mut samples,
            "brawler_sqlite_database_bytes",
            "Current SQLite database size.",
            MetricKind::Gauge,
            MetricUnit::Bytes,
            database_bytes as f64,
            &[("collector", "sqlite")],
            collected_at,
        )?;

        for table in [
            "feed_items",
            "diagnostic_events",
            "transcript_jobs",
            "transcript_segments",
            "ai_analysis_jobs",
            "notebook_entries",
            "company_events",
        ] {
            push_sample(
                &mut samples,
                "brawler_sqlite_table_rows",
                "Current row count for high-growth local tables.",
                MetricKind::Gauge,
                MetricUnit::Count,
                count_rows(connection, table)?,
                &[("table", table)],
                collected_at,
            )?;
        }

        Ok(samples)
    }
}

struct RuntimeMetricsCollector;

impl MetricCollector for RuntimeMetricsCollector {
    fn collect(
        &self,
        _connection: &Connection,
        runtime_metrics: &RuntimeMetricCounters,
        _app_data_dir: &Path,
        collected_at: &str,
    ) -> StorageResult<Vec<MetricSample>> {
        let mut samples = Vec::new();

        for (key, value) in runtime_metrics.snapshot() {
            let (description, kind, unit) = runtime_metric_contract(&key.name);
            let label_refs = key
                .labels
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect::<Vec<_>>();
            push_sample(
                &mut samples,
                &key.name,
                description,
                kind,
                unit,
                value,
                &label_refs,
                collected_at,
            )?;
        }

        Ok(samples)
    }
}

fn collect_job_status_metrics(
    connection: &Connection,
    table: &str,
    metric_name: &'static str,
    description: &'static str,
    label_columns: &[(&'static str, &'static str)],
    collected_at: &str,
) -> StorageResult<Vec<MetricSample>> {
    let select_columns = label_columns
        .iter()
        .map(|(_, column)| *column)
        .collect::<Vec<_>>()
        .join(", ");
    let group_columns = select_columns.clone();
    let sql = format!(
        "SELECT {select_columns}, COUNT(*) FROM {table} GROUP BY {group_columns} ORDER BY {group_columns}"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        let mut values = Vec::new();
        for index in 0..label_columns.len() {
            values.push(row.get::<_, String>(index)?);
        }
        Ok((values, row.get::<_, i64>(label_columns.len())?))
    })?;
    let mut samples = Vec::new();

    for row in rows {
        let (values, count) = row?;
        let label_refs = label_columns
            .iter()
            .zip(values.iter())
            .map(|((label, _), value)| (*label, value.as_str()))
            .collect::<Vec<_>>();
        push_sample(
            &mut samples,
            metric_name,
            description,
            MetricKind::Gauge,
            MetricUnit::Count,
            count as f64,
            &label_refs,
            collected_at,
        )?;
    }

    Ok(samples)
}

fn runtime_metric_contract(name: &str) -> (&'static str, MetricKind, MetricUnit) {
    match name {
        "brawler_source_refresh_total" => (
            "Process-lifetime source refresh attempts by adapter and status.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        "brawler_source_refresh_duration_seconds" => (
            "Process-lifetime cumulative source refresh duration by adapter and status.",
            MetricKind::Counter,
            MetricUnit::Seconds,
        ),
        "brawler_scheduler_skips_total" => (
            "Process-lifetime scheduled task skips.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        "brawler_ai_analysis_runs_total" => (
            "Process-lifetime AI analysis executions by provider, model, and status.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        "brawler_ai_analysis_duration_seconds" => (
            "Process-lifetime cumulative AI analysis execution duration by provider, model, and status.",
            MetricKind::Counter,
            MetricUnit::Seconds,
        ),
        "brawler_transcript_runs_total" => (
            "Process-lifetime transcript executions by provider and status.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        "brawler_transcript_duration_seconds" => (
            "Process-lifetime cumulative transcript execution duration by provider and status.",
            MetricKind::Counter,
            MetricUnit::Seconds,
        ),
        "brawler_feed_cleanup_runs_total" => (
            "Process-lifetime feed cleanup executions by mode and status.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        "brawler_feed_cleanup_deleted_total" => (
            "Process-lifetime feed items deleted by cleanup mode.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        "brawler_feed_cleanup_duration_seconds" => (
            "Process-lifetime cumulative feed cleanup duration by mode and status.",
            MetricKind::Counter,
            MetricUnit::Seconds,
        ),
        "brawler_credential_checks_total" => (
            "Process-lifetime credential check outcomes by provider and purpose.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        "brawler_credential_operations_total" => (
            "Process-lifetime credential operations by provider and purpose.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
        _ => (
            "Process-lifetime runtime metric.",
            MetricKind::Counter,
            MetricUnit::Count,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_sample(
    samples: &mut Vec<MetricSample>,
    name: &str,
    description: &str,
    kind: MetricKind,
    unit: MetricUnit,
    value: f64,
    labels: &[(&str, &str)],
    collected_at: &str,
) -> StorageResult<()> {
    if !is_allowed_metric_name(name) {
        return Err(StorageError::InvalidDiagnosticValue {
            key: "metric_name",
            value: name.to_owned(),
        });
    }

    let labels = labels
        .iter()
        .map(|(key, value)| {
            if !is_allowed_label(key, value) {
                return Err(StorageError::InvalidDiagnosticValue {
                    key: "metric_label",
                    value: format!("{key}={value}"),
                });
            }

            Ok(MetricLabel {
                key: (*key).to_owned(),
                value: (*value).to_owned(),
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;

    samples.push(MetricSample {
        name: name.to_owned(),
        description: description.to_owned(),
        kind,
        unit,
        value,
        labels,
        collected_at: collected_at.to_owned(),
    });

    Ok(())
}

fn is_allowed_metric_name(value: &str) -> bool {
    !value.is_empty()
        && value.starts_with("brawler_")
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_allowed_label(key: &str, value: &str) -> bool {
    let allowed_key = matches!(
        key,
        "module"
            | "collector"
            | "adapter_id"
            | "provider_id"
            | "model"
            | "status"
            | "severity"
            | "table"
            | "unit"
    );
    let safe_value = !value.trim().is_empty()
        && value.len() <= 80
        && !value.contains("://")
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains(' ')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
        });

    allowed_key && safe_value
}

fn count_rows(connection: &Connection, table: &str) -> StorageResult<f64> {
    if !is_allowed_table_name(table) {
        return Err(StorageError::InvalidDiagnosticValue {
            key: "table",
            value: table.to_owned(),
        });
    }

    let sql = format!("SELECT COUNT(*) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map(|value| value as f64)
        .map_err(StorageError::from)
}

fn is_allowed_table_name(table: &str) -> bool {
    matches!(
        table,
        "feed_items"
            | "diagnostic_events"
            | "transcript_jobs"
            | "transcript_segments"
            | "ai_analysis_jobs"
            | "notebook_entries"
            | "company_events"
    )
}

fn count_unmatched_source_items(connection: &Connection) -> StorageResult<f64> {
    connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM feed_items
            LEFT JOIN feed_item_companies
                ON feed_item_companies.feed_item_id = feed_items.id
            WHERE feed_item_companies.feed_item_id IS NULL
                AND COALESCE(feed_items.display_company, '') NOT IN (
                    SELECT qualified_ticker FROM companies
                )
            ",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value as f64)
        .map_err(StorageError::from)
}

fn database_size_bytes(connection: &Connection, app_data_dir: &Path) -> StorageResult<u64> {
    if let Some(path) = connection.path() {
        if !path.is_empty() {
            if let Ok(metadata) = std::fs::metadata(path) {
                return Ok(metadata.len());
            }
        }
    }

    let database_path = app_data_dir.join("brawler.sqlite3");
    if let Ok(metadata) = std::fs::metadata(database_path) {
        return Ok(metadata.len());
    }

    let page_count: i64 = connection.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let page_size: i64 = connection.query_row("PRAGMA page_size", [], |row| row.get(0))?;

    Ok((page_count * page_size).max(0) as u64)
}

fn parse_metric_value(value: &str) -> f64 {
    value.parse::<f64>().unwrap_or(0.0)
}

fn current_timestamp(connection: &Connection) -> StorageResult<String> {
    connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(StorageError::from)
}

fn label_sort_key(labels: &[MetricLabel]) -> String {
    labels
        .iter()
        .map(|label| format!("{}={}", label.key, label.value))
        .collect::<Vec<_>>()
        .join(",")
}

#[allow(dead_code)]
fn process_started_at_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        source_adapters::bankier_company::BankierCompanyItem,
        storage::{open_in_memory_database, AppState, NewAiAnalysisJob, NewCompany},
    };

    #[test]
    fn rejects_private_or_high_cardinality_label_values() {
        assert!(is_allowed_label("adapter_id", "bankier-company"));
        assert!(!is_allowed_label(
            "adapter_id",
            "https://example.test/source"
        ));
        assert!(!is_allowed_label("title", "bankier-company"));
        assert!(!is_allowed_label("adapter_id", "Company name with spaces"));
    }

    #[test]
    fn runtime_counters_use_prometheus_friendly_samples() {
        let counters = RuntimeMetricCounters::default();
        counters.increment(
            "brawler_source_refresh_total",
            &[("adapter_id", "bankier-company"), ("status", "succeeded")],
        );
        let connection = open_in_memory_database().expect("database should open");
        let snapshot = collect_local_metrics_snapshot(&connection, &counters, Path::new("/tmp"))
            .expect("metrics should collect");

        let sample = snapshot
            .samples
            .iter()
            .find(|sample| sample.name == "brawler_source_refresh_total")
            .expect("runtime source refresh sample should exist");
        assert_eq!(sample.kind, MetricKind::Counter);
        assert_eq!(sample.unit, MetricUnit::Count);
        assert_eq!(sample.value, 1.0);
    }

    #[test]
    fn durable_collectors_aggregate_ai_jobs_without_user_text_labels() {
        let connection = open_in_memory_database().expect("database should open");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should create");
        state
            .ingest_bankier_company_items(&[BankierCompanyItem {
                company_id: company.id,
                qualified_ticker: "GPW:CDR".to_owned(),
                title: "Wyniki finansowe QSr 1/2026".to_owned(),
                link: "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
                    .to_owned(),
                summary: "raporty okresowe".to_owned(),
                published_at: Some("2026-05-28T17:33:09".to_owned()),
                fetched_at: "2026-05-31T10:00:00Z".to_owned(),
                article_id: "9141553".to_owned(),
                pub_id: 3,
                dedupe_key: "bankier-company-komunikaty:article:9141553".to_owned(),
                duplicate_signature:
                    "official-secondary:GPW:CDR:wyniki-finansowe-qsr-1-2026:9141553".to_owned(),
                body_text: Some("Official Bankier report body.".to_owned()),
                attachments: Vec::new(),
                detail_fetch_attempted: true,
            }])
            .expect("feed item should ingest");
        let feed_item = state
            .list_feed_items()
            .expect("feed items should list")
            .pop()
            .expect("feed item should exist");

        state
            .create_ai_analysis_job(NewAiAnalysisJob {
                feed_item_id: feed_item.id,
                prompt_preset_id: Some("default_summary".to_owned()),
                custom_question: Some("Should not appear as metric label".to_owned()),
                provider_id: "test_sample".to_owned(),
                model: "test_sample_model".to_owned(),
                prompt_version: None,
            })
            .expect("job should create");

        let snapshot = state
            .local_metrics_snapshot(Path::new("/tmp"))
            .expect("metrics should collect");
        let sample = snapshot
            .samples
            .iter()
            .find(|sample| sample.name == "brawler_ai_analysis_jobs_total")
            .expect("ai job metric should exist");

        assert_eq!(sample.value, 1.0);
        assert!(sample.labels.iter().any(|label| label.key == "provider_id"));
        assert!(!sample
            .labels
            .iter()
            .any(|label| label.value.contains("Should not appear")));
    }
}
