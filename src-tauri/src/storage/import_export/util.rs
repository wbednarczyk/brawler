use super::*;

pub(super) fn normalize_qualified_ticker(exchange: &str, ticker: &str) -> String {
    format!(
        "{}:{}",
        exchange.trim().to_uppercase(),
        ticker.trim().to_uppercase()
    )
}

pub(super) fn now_rfc3339() -> StorageResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| StorageError::InvalidSettingValue {
            key: "import_export",
            value: error.to_string(),
        })
}
