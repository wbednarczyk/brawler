//! Task identity for the Activity center (ADR 0109 dec. 1, #133).
//!
//! One [`ActivityIdentity`] per background task, resolved from a queue job's
//! `kind`/`payload`/`id` (or, for awaited work, constructed directly by the
//! caller with a synthetic payload carrying the same shape a queue job would).
//! `activity_key` is the identity the read model collapses on — distinct from
//! the queue's own (reusable) job id. A registered kind with a malformed
//! payload resolves to `family: Corrupted` rather than being silently dropped;
//! an unregistered (retired) kind resolves to `None` and its rows are excluded.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// One family of background work. Exhaustive over the 15 registered queue
/// kinds plus every instrumented awaited path (ADR 0109 dec. 1); `Corrupted`
/// is the explicit malformed-payload item, never a silent drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub enum ActivityFamily {
    SourceRefresh,
    CompanyRefresh,
    RegistryRefresh,
    FxPull,
    FundamentalsPull,
    Briefing,
    HistoryFetch,
    ReportSweep,
    Reextraction,
    ReportReading,
    OwnershipReading,
    ManagementReading,
    PriceHistory,
    KpiIngest,
    Transcript,
    Corrupted,
}

/// A navigation target expressed in the Spółka `Tool` union
/// (`src/screens/Spolka/route.ts`) — structurally `{ t: "dokumenty", documentId }`
/// etc. Only the tools an Activity item can land on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(tag = "t", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ActivityTool {
    Feed,
    Pokrycie,
    Dokumenty { document_id: String },
}

/// The typed navigation target an Activity item lands on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActivityTarget {
    Company {
        company_id: String,
        tool: Option<ActivityTool>,
    },
    Sources,
    Today,
    Transcripts,
}

/// The resolved identity of one background task (ADR 0109 dec. 1): the task
/// identity the read model collapses on, its family, an optional company
/// scope, a raw subject (never composed prose), and its navigation target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityIdentity {
    pub activity_key: String,
    pub family: ActivityFamily,
    pub company_id: Option<String>,
    pub subject: String,
    pub target: ActivityTarget,
}

impl ActivityIdentity {
    fn corrupted(job_id: &str) -> Self {
        Self {
            activity_key: format!("corrupted:{job_id}"),
            family: ActivityFamily::Corrupted,
            company_id: None,
            subject: job_id.to_owned(),
            target: ActivityTarget::Today,
        }
    }
}

/// A source adapter's display name, via the adapter registry (falls back to
/// the raw id for an unregistered/legacy adapter — a source proper noun,
/// never silently blanked). Mirrors `storage::attention::adapter_display_name`.
fn adapter_display_name(adapter_id: &str) -> String {
    crate::source_adapters::registry::descriptor(adapter_id)
        .map(|descriptor| descriptor.display_name.to_owned())
        .unwrap_or_else(|| adapter_id.to_owned())
}

/// A company's ticker for use as a raw subject, or the id itself if the
/// company is gone (never a silent blank).
fn company_ticker(connection: &Connection, company_id: &str) -> String {
    crate::storage::activity_reads::ticker(connection, company_id)
}

/// A report document's title for use as a raw subject, or the id itself if
/// the document is gone or untitled.
fn document_title(connection: &Connection, document_id: &str) -> String {
    crate::storage::activity_reads::document_title_bound(connection, document_id)
}

/// Identity for [`crate::jobs::scheduler::SOURCE_REFRESH_KIND`], connection-
/// free (`adapter_display_name` is a static registry lookup) — the SAME
/// value [`identity_for_job`]'s own arm for this kind produces. A `_direct`
/// wrapper builds this directly instead of losing registration to a checkout
/// failure it never actually needed to survive (sol diff R2 #4: registration
/// must never depend on database health for a kind whose identity never
/// touched the database in the first place).
pub fn source_refresh_identity(adapter_id: &str) -> ActivityIdentity {
    ActivityIdentity {
        activity_key: format!("source-refresh:{adapter_id}"),
        family: ActivityFamily::SourceRefresh,
        company_id: None,
        subject: adapter_display_name(adapter_id),
        target: ActivityTarget::Sources,
    }
}

/// Identity for [`crate::jobs::scheduler::REGISTRY_REFRESH_KIND`],
/// connection-free (sol diff R2 #4, mirrors [`source_refresh_identity`]).
pub fn registry_refresh_identity() -> ActivityIdentity {
    ActivityIdentity {
        activity_key: "registry-refresh".to_owned(),
        family: ActivityFamily::RegistryRefresh,
        company_id: None,
        // sol diff R2 #6 (backend): empty, never composed prose — the
        // family label carries it (mirrors the briefing/system subjects).
        subject: String::new(),
        target: ActivityTarget::Sources,
    }
}

/// Identity for
/// [`crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND`],
/// connection-free (sol diff R2 #4, mirrors [`source_refresh_identity`]).
pub fn aggregator_fundamentals_pull_identity() -> ActivityIdentity {
    ActivityIdentity {
        activity_key: "fundamentals-pull".to_owned(),
        family: ActivityFamily::FundamentalsPull,
        company_id: None,
        subject: crate::source_adapters::biznesradar_fundamentals::DISPLAY_NAME.to_owned(),
        target: ActivityTarget::Sources,
    }
}

/// Fallback identity for [`crate::jobs::backfill::COMPANY_BACKFILL_KIND`]
/// when no checkout is available to resolve the company's ticker (sol diff
/// R2 #4): the raw `company_id` substitutes as the subject so the direct
/// wrapper still ALWAYS registers instead of running the core unrecorded.
pub fn company_backfill_identity_fallback(company_id: &str) -> ActivityIdentity {
    ActivityIdentity {
        activity_key: format!("history-fetch:{company_id}"),
        family: ActivityFamily::HistoryFetch,
        subject: company_id.to_owned(),
        target: ActivityTarget::Company {
            company_id: company_id.to_owned(),
            tool: Some(ActivityTool::Pokrycie),
        },
        company_id: Some(company_id.to_owned()),
    }
}

/// The company scoping a `history_sweeps` row, if the row exists.
fn sweep_company(connection: &Connection, sweep_id: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT company_id FROM history_sweeps WHERE id = ?1",
            [sweep_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
}

/// The company scoping a `pipeline_reextraction_batches` row, if it exists.
fn batch_company(connection: &Connection, batch_id: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT company_id FROM pipeline_reextraction_batches WHERE id = ?1",
            [batch_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
}

/// `(company_id, report_document_id, sweep_id)` for an `autopilot_run`, if it
/// exists.
fn autopilot_run_scope(
    connection: &Connection,
    run_id: &str,
) -> Option<(String, String, Option<String>)> {
    connection
        .query_row(
            "SELECT company_id, report_document_id, sweep_id FROM autopilot_run WHERE id = ?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()
}

/// `(company_id, report_document_id)` for a `kpi_ingest_runs` row, if it
/// exists.
fn kpi_run_scope(connection: &Connection, run_id: &str) -> Option<(String, String)> {
    connection
        .query_row(
            "SELECT company_id, report_document_id FROM kpi_ingest_runs WHERE id = ?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok()
}

fn json_value(payload: &str) -> Option<serde_json::Value> {
    serde_json::from_str(payload).ok()
}

fn str_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(str::to_owned)
}

/// Resolve one background task's identity from a queue job's `kind` +
/// `payload` + its own `id` (ADR 0109 dec. 1). `None` ONLY for an
/// unregistered (retired) kind — never for a registered kind with a malformed
/// payload, which resolves to an explicit `Corrupted` item instead (subject =
/// `job_id`, target = the family default = Today) so a broken row is a visible
/// task, never a silent drop.
pub fn identity_for_job(
    kind: &str,
    job_id: &str,
    payload: &str,
    connection: &Connection,
) -> Option<ActivityIdentity> {
    use crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND;
    use crate::jobs::autopilot::AUTOPILOT_STAGE_KIND;
    use crate::jobs::backfill::COMPANY_BACKFILL_KIND;
    use crate::jobs::fx_daily_pull::FX_DAILY_PULL_KIND;
    use crate::jobs::history_sweep::HISTORY_SWEEP_KIND;
    use crate::jobs::kpi_ingest_queue::{KPI_INGEST_COMMIT_KIND, KPI_INGEST_VALIDATE_KIND};
    use crate::jobs::management_holdings_extraction::MANAGEMENT_EXTRACTION_KIND;
    use crate::jobs::morning_briefing::MORNING_BRIEFING_KIND;
    use crate::jobs::ownership_extraction::OWNERSHIP_EXTRACTION_KIND;
    use crate::jobs::pipeline_reextraction::PIPELINE_REEXTRACTION_KIND;
    use crate::jobs::quote_backfill::QUOTE_BACKFILL_KIND;
    use crate::jobs::scheduler::{REGISTRY_REFRESH_KIND, SOURCE_REFRESH_KIND};
    use crate::jobs::source_refresh::SOURCE_COMPANY_REFRESH_KIND;

    let value = json_value(payload);

    Some(match kind {
        SOURCE_REFRESH_KIND => {
            let Some(adapter_id) = value.as_ref().and_then(|v| str_field(v, "adapterId")) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            source_refresh_identity(&adapter_id)
        }
        SOURCE_COMPANY_REFRESH_KIND => {
            let company_id = value.as_ref().and_then(|v| str_field(v, "companyId"));
            let Some(company_id) = company_id else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            ActivityIdentity {
                activity_key: format!("company-refresh:{company_id}"),
                family: ActivityFamily::CompanyRefresh,
                subject: company_ticker(connection, &company_id),
                target: ActivityTarget::Company {
                    company_id: company_id.clone(),
                    tool: Some(ActivityTool::Feed),
                },
                company_id: Some(company_id),
            }
        }
        REGISTRY_REFRESH_KIND => registry_refresh_identity(),
        FX_DAILY_PULL_KIND => ActivityIdentity {
            activity_key: "fx-pull".to_owned(),
            family: ActivityFamily::FxPull,
            company_id: None,
            subject: crate::source_adapters::nbp_fx::DISPLAY_NAME.to_owned(),
            target: ActivityTarget::Sources,
        },
        AGGREGATOR_FUNDAMENTALS_PULL_KIND => aggregator_fundamentals_pull_identity(),
        MORNING_BRIEFING_KIND => ActivityIdentity {
            activity_key: "briefing".to_owned(),
            family: ActivityFamily::Briefing,
            company_id: None,
            // sol diff R1 #17: no composed backend prose in `subject` (ADR
            // 0087 dec. 4 / contracts.md: raw source data only) — a fixed
            // Polish string here rendered untranslated for an English user.
            // The briefing/system families have no raw subject of their
            // own; the frontend renders the family label instead.
            subject: String::new(),
            target: ActivityTarget::Today,
        },
        COMPANY_BACKFILL_KIND => {
            let Some(company_id) = value.as_ref().and_then(|v| str_field(v, "companyId")) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            ActivityIdentity {
                activity_key: format!("history-fetch:{company_id}"),
                family: ActivityFamily::HistoryFetch,
                subject: company_ticker(connection, &company_id),
                target: ActivityTarget::Company {
                    company_id: company_id.clone(),
                    tool: Some(ActivityTool::Pokrycie),
                },
                company_id: Some(company_id),
            }
        }
        QUOTE_BACKFILL_KIND => {
            let Some(company_id) = value.as_ref().and_then(|v| str_field(v, "companyId")) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            ActivityIdentity {
                activity_key: format!("price-history:{company_id}"),
                family: ActivityFamily::PriceHistory,
                subject: company_ticker(connection, &company_id),
                target: ActivityTarget::Company {
                    company_id: company_id.clone(),
                    tool: None,
                },
                company_id: Some(company_id),
            }
        }
        HISTORY_SWEEP_KIND => {
            let Some(sweep_id) = value.as_ref().and_then(|v| str_field(v, "sweep_id")) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            let company_id = sweep_company(connection, &sweep_id);
            let subject = company_id
                .as_deref()
                .map(|id| company_ticker(connection, id))
                .unwrap_or_else(|| sweep_id.clone());
            ActivityIdentity {
                activity_key: format!("report-sweep:{sweep_id}"),
                family: ActivityFamily::ReportSweep,
                subject,
                target: match &company_id {
                    Some(company_id) => ActivityTarget::Company {
                        company_id: company_id.clone(),
                        tool: Some(ActivityTool::Pokrycie),
                    },
                    None => ActivityTarget::Sources,
                },
                company_id,
            }
        }
        PIPELINE_REEXTRACTION_KIND => {
            let Some(batch_id) = value.as_ref().and_then(|v| str_field(v, "batch_id")) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            let company_id = batch_company(connection, &batch_id);
            let subject = company_id
                .as_deref()
                .map(|id| company_ticker(connection, id))
                .unwrap_or_else(|| batch_id.clone());
            ActivityIdentity {
                activity_key: format!("reextraction:{batch_id}"),
                family: ActivityFamily::Reextraction,
                subject,
                target: match &company_id {
                    Some(company_id) => ActivityTarget::Company {
                        company_id: company_id.clone(),
                        tool: Some(ActivityTool::Pokrycie),
                    },
                    None => ActivityTarget::Sources,
                },
                company_id,
            }
        }
        OWNERSHIP_EXTRACTION_KIND | MANAGEMENT_EXTRACTION_KIND => {
            let company_id = value.as_ref().and_then(|v| str_field(v, "companyId"));
            let document_id = value
                .as_ref()
                .and_then(|v| str_field(v, "reportDocumentId"));
            let (Some(company_id), Some(document_id)) = (company_id, document_id) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            let prefix = if kind == OWNERSHIP_EXTRACTION_KIND {
                "ownership-reading"
            } else {
                "management-reading"
            };
            let family = if kind == OWNERSHIP_EXTRACTION_KIND {
                ActivityFamily::OwnershipReading
            } else {
                ActivityFamily::ManagementReading
            };
            ActivityIdentity {
                activity_key: format!("{prefix}:{document_id}"),
                family,
                subject: document_title(connection, &document_id),
                target: ActivityTarget::Company {
                    company_id: company_id.clone(),
                    tool: Some(ActivityTool::Dokumenty { document_id }),
                },
                company_id: Some(company_id),
            }
        }
        AUTOPILOT_STAGE_KIND => {
            let Some(run_id) = value.as_ref().and_then(|v| str_field(v, "run_id")) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            let Some((company_id, report_document_id, sweep_id)) =
                autopilot_run_scope(connection, &run_id)
            else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            match sweep_id {
                Some(sweep_id) => ActivityIdentity {
                    activity_key: format!("report-sweep:{sweep_id}"),
                    family: ActivityFamily::ReportSweep,
                    subject: company_ticker(connection, &company_id),
                    target: ActivityTarget::Company {
                        company_id: company_id.clone(),
                        tool: Some(ActivityTool::Pokrycie),
                    },
                    company_id: Some(company_id),
                },
                None => ActivityIdentity {
                    activity_key: format!("report-reading:{run_id}"),
                    family: ActivityFamily::ReportReading,
                    subject: document_title(connection, &report_document_id),
                    target: ActivityTarget::Company {
                        company_id: company_id.clone(),
                        tool: Some(ActivityTool::Dokumenty {
                            document_id: report_document_id,
                        }),
                    },
                    company_id: Some(company_id),
                },
            }
        }
        KPI_INGEST_VALIDATE_KIND | KPI_INGEST_COMMIT_KIND => {
            // sol diff R1 #9: `run_id` is authoritative from the CLAIMED
            // job id (`kpi_ingest_queue::parse_job_id`, the SAME parser the
            // KPI subsystem itself treats as the single authority for
            // terminalization) — never the payload's duplicated `runId`,
            // which a tampered/mismatched payload could otherwise use to
            // misattribute the occurrence to the wrong run before preflight
            // validation ever runs. The payload's `runId` is only a
            // coherence check (logged, never authoritative).
            let Some(parsed) = crate::jobs::kpi_ingest_queue::parse_job_id(job_id) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            let run_id = parsed.run_id();
            if let Some(payload_run_id) = value.as_ref().and_then(|v| str_field(v, "runId")) {
                if payload_run_id != run_id {
                    log::warn!(
                        "activity identity: job {job_id} payload runId {payload_run_id:?} \
                         disagrees with the id-derived run {run_id:?} — identity taken from the \
                         id (sol diff R1 #9)"
                    );
                }
            }
            let Some((company_id, report_document_id)) = kpi_run_scope(connection, run_id) else {
                return Some(ActivityIdentity::corrupted(job_id));
            };
            ActivityIdentity {
                activity_key: format!("kpi-ingest:{run_id}"),
                family: ActivityFamily::KpiIngest,
                subject: document_title(connection, &report_document_id),
                target: ActivityTarget::Company {
                    company_id: company_id.clone(),
                    tool: Some(ActivityTool::Dokumenty {
                        document_id: report_document_id,
                    }),
                },
                company_id: Some(company_id),
            }
        }
        _ => return None,
    })
}

/// The identity of a video transcript job (awaited work, direct-activity
/// registry — never a `job_queue` row). Subject is the video title, falling
/// back to its source URL when untitled.
pub fn identity_for_transcript(job: &crate::storage::TranscriptJob) -> ActivityIdentity {
    ActivityIdentity {
        activity_key: format!("transcript:{}", job.id),
        family: ActivityFamily::Transcript,
        company_id: job.company_id.clone(),
        subject: job
            .source_label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| job.source_url.clone()),
        target: ActivityTarget::Transcripts,
    }
}

#[cfg(test)]
mod tests;
