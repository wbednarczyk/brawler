use super::companies::company_directories_are_stale;
use super::migrations::{apply_migrations, database_status, expected_migration_count};
use super::sources::{media_duplicate_signature, MediaMatchCompany};
use super::*;

const PORTAL_ANALIZ_ADAPTER_ID: &str = "portal-analiz";

mod ai_analysis;
mod common;
mod companies;
mod diagnostics;
mod events;
mod feed_sources;
mod import_export;
mod licensing;
mod notebooks;
mod research;
mod research_briefs;
mod schema;
mod settings;
mod source_registry;
mod transcripts;
mod watchlists;
