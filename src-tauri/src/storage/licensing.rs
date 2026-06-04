use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{StorageError, StorageResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredLicenseMetadata {
    pub status: String,
    pub reason: Option<String>,
    pub license_id: Option<String>,
    pub holder: Option<String>,
    pub channel: Option<String>,
    pub edition: Option<String>,
    pub features: Vec<String>,
    pub issued_at: Option<String>,
    pub expires_at: Option<String>,
    pub app_version_range: Option<String>,
    pub key_id: Option<String>,
    pub checked_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LicenseMetadataUpdate {
    pub status: String,
    pub reason: Option<String>,
    pub license_id: Option<String>,
    pub holder: Option<String>,
    pub channel: Option<String>,
    pub edition: Option<String>,
    pub features: Vec<String>,
    pub issued_at: Option<String>,
    pub expires_at: Option<String>,
    pub app_version_range: Option<String>,
    pub key_id: Option<String>,
}

pub(crate) fn get_license_metadata(
    connection: &Connection,
) -> StorageResult<Option<StoredLicenseMetadata>> {
    connection
        .query_row(
            "
            SELECT status,
                   reason,
                   license_id,
                   holder,
                   channel,
                   edition,
                   features_json,
                   issued_at,
                   expires_at,
                   app_version_range,
                   key_id,
                   checked_at,
                   updated_at
            FROM license_metadata
            WHERE id = 1
            ",
            [],
            map_license_metadata,
        )
        .optional()
        .map_err(StorageError::from)
}

pub(crate) fn upsert_license_metadata(
    connection: &Connection,
    input: LicenseMetadataUpdate,
) -> StorageResult<StoredLicenseMetadata> {
    validate_status(&input.status)?;
    validate_optional_value("license_id", input.license_id.as_deref())?;
    validate_optional_value("holder", input.holder.as_deref())?;
    validate_optional_identifier("channel", input.channel.as_deref())?;
    validate_optional_identifier("edition", input.edition.as_deref())?;
    validate_optional_identifier("key_id", input.key_id.as_deref())?;

    let features_json = serde_json::to_string(&input.features)?;

    connection.execute(
        "
        INSERT INTO license_metadata (
            id,
            status,
            reason,
            license_id,
            holder,
            channel,
            edition,
            features_json,
            issued_at,
            expires_at,
            app_version_range,
            key_id,
            checked_at,
            updated_at
        ) VALUES (
            1,
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7,
            ?8,
            ?9,
            ?10,
            ?11,
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        )
        ON CONFLICT(id) DO UPDATE SET
            status = excluded.status,
            reason = excluded.reason,
            license_id = excluded.license_id,
            holder = excluded.holder,
            channel = excluded.channel,
            edition = excluded.edition,
            features_json = excluded.features_json,
            issued_at = excluded.issued_at,
            expires_at = excluded.expires_at,
            app_version_range = excluded.app_version_range,
            key_id = excluded.key_id,
            checked_at = excluded.checked_at,
            updated_at = excluded.updated_at
        ",
        params![
            input.status,
            input.reason.as_deref().map(str::trim),
            input.license_id.as_deref().map(str::trim),
            input.holder.as_deref().map(str::trim),
            input.channel.as_deref().map(str::trim),
            input.edition.as_deref().map(str::trim),
            features_json,
            input.issued_at.as_deref().map(str::trim),
            input.expires_at.as_deref().map(str::trim),
            input.app_version_range.as_deref().map(str::trim),
            input.key_id.as_deref().map(str::trim),
        ],
    )?;

    get_license_metadata(connection)?.ok_or_else(|| StorageError::InvalidLicenseValue {
        key: "license_metadata",
        value: "missing after upsert".to_owned(),
    })
}

pub(crate) fn clear_license_metadata(connection: &Connection) -> StorageResult<()> {
    connection.execute("DELETE FROM license_metadata WHERE id = 1", [])?;
    Ok(())
}

fn map_license_metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredLicenseMetadata> {
    let features_json: String = row.get(6)?;
    let features = serde_json::from_str(&features_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(StoredLicenseMetadata {
        status: row.get(0)?,
        reason: row.get(1)?,
        license_id: row.get(2)?,
        holder: row.get(3)?,
        channel: row.get(4)?,
        edition: row.get(5)?,
        features,
        issued_at: row.get(7)?,
        expires_at: row.get(8)?,
        app_version_range: row.get(9)?,
        key_id: row.get(10)?,
        checked_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn validate_status(value: &str) -> StorageResult<()> {
    match value {
        "valid"
        | "missing"
        | "invalid"
        | "expired"
        | "wrong_version"
        | "unsupported_version"
        | "storage_error" => Ok(()),
        _ => Err(StorageError::InvalidLicenseValue {
            key: "status",
            value: value.to_owned(),
        }),
    }
}

fn validate_optional_value(key: &'static str, value: Option<&str>) -> StorageResult<()> {
    if let Some(value) = value {
        let value = value.trim();
        if value.is_empty() {
            return Err(StorageError::InvalidLicenseValue {
                key,
                value: value.to_owned(),
            });
        }
    }

    Ok(())
}

fn validate_optional_identifier(key: &'static str, value: Option<&str>) -> StorageResult<()> {
    if let Some(value) = value {
        let value = value.trim();
        let valid = !value.is_empty()
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            });
        if !valid {
            return Err(StorageError::InvalidLicenseValue {
                key,
                value: value.to_owned(),
            });
        }
    }

    Ok(())
}
