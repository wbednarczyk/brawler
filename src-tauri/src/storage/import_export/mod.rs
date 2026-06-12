use std::collections::{HashMap, HashSet};

use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::*;

mod export;
mod references;
mod research_import;
mod settings_import;
mod types;
mod util;

pub use types::{
    ExportPayload, ImportApplyResult, ImportApplySummary, ImportExportSummary, ImportPreview,
};

use references::*;
use types::*;
use util::*;

pub(super) fn export_research_data(connection: &Connection) -> StorageResult<ExportPayload> {
    export::export_research_data(connection)
}

pub(super) fn preview_research_import(
    connection: &Connection,
    contents: &str,
) -> StorageResult<ImportPreview> {
    research_import::preview_research_import(connection, contents)
}

pub(super) fn apply_research_import(
    connection: &mut Connection,
    contents: &str,
) -> StorageResult<ImportApplyResult> {
    research_import::apply_research_import(connection, contents)
}

pub(super) fn export_settings_data(connection: &Connection) -> StorageResult<ExportPayload> {
    export::export_settings_data(connection)
}

pub(super) fn preview_settings_import(contents: &str) -> StorageResult<ImportPreview> {
    settings_import::preview_settings_import(contents)
}

pub(super) fn apply_settings_import(
    connection: &Connection,
    contents: &str,
) -> StorageResult<ImportApplyResult> {
    settings_import::apply_settings_import(connection, contents)
}
