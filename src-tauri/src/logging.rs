use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use serde::Serialize;
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::observability::{redact_metadata, redact_text};
use crate::storage::LogSettings;

const LOG_FILE_NAME: &str = "brawler.log";

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct LogStatus {
    pub logs_dir: String,
    pub current_file_bytes: u64,
    pub rotated_file_count: usize,
    pub level: String,
    pub max_files: u32,
    pub max_file_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub file_name: String,
    pub line_number: usize,
    #[cfg_attr(feature = "ts-export", ts(type = "Record<string, unknown>"))]
    pub record: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLogSettings {
    pub level: String,
    pub max_files: u32,
    pub max_file_bytes: u64,
}

pub fn resolve_log_settings(settings: &LogSettings) -> ResolvedLogSettings {
    resolve_log_settings_with_env(settings, |key| std::env::var(key).ok())
}

fn resolve_log_settings_with_env(
    settings: &LogSettings,
    get_env: impl Fn(&str) -> Option<String>,
) -> ResolvedLogSettings {
    let level = get_env("BRAWLER_LOG_LEVEL")
        .and_then(|value| normalize_log_level(&value))
        .unwrap_or_else(|| settings.level.clone());
    let max_files = get_env("BRAWLER_LOG_MAX_FILES")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| (1..=20).contains(value))
        .unwrap_or(settings.max_files as u32);
    let max_file_bytes = get_env("BRAWLER_LOG_MAX_FILE_MEGABYTES")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .and_then(|value| value.checked_mul(1_048_576))
        .filter(|value| (1_048_576..=104_857_600).contains(value))
        .unwrap_or(settings.max_file_bytes as u64);

    ResolvedLogSettings {
        level,
        max_files,
        max_file_bytes,
    }
}

pub fn init_file_logger(
    app_data_dir: impl AsRef<Path>,
    settings: ResolvedLogSettings,
) -> Result<PathBuf, SetLoggerError> {
    let logs_dir = app_data_dir.as_ref().join("logs");
    let level = level_filter_from_str(&settings.level);
    let logger = JsonFileLogger::new(logs_dir.clone(), settings);

    static LOGGER: OnceLock<JsonFileLogger> = OnceLock::new();
    if LOGGER.set(logger).is_ok() {
        log::set_logger(LOGGER.get().expect("logger should be initialized"))?;
    }
    log::set_max_level(level);

    Ok(logs_dir)
}

pub fn logs_dir(app_data_dir: impl AsRef<Path>) -> PathBuf {
    app_data_dir.as_ref().join("logs")
}

pub fn log_status(
    app_data_dir: impl AsRef<Path>,
    settings: &ResolvedLogSettings,
) -> std::io::Result<LogStatus> {
    let logs_dir = logs_dir(app_data_dir);
    let current_file_bytes = fs::metadata(logs_dir.join(LOG_FILE_NAME))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let rotated_file_count = log_file_paths(&logs_dir, settings.max_files)
        .into_iter()
        .skip(1)
        .filter(|path| path.exists())
        .count();

    Ok(LogStatus {
        logs_dir: logs_dir.to_string_lossy().into_owned(),
        current_file_bytes,
        rotated_file_count,
        level: settings.level.clone(),
        max_files: settings.max_files,
        max_file_bytes: settings.max_file_bytes,
    })
}

pub fn read_log_entries(
    app_data_dir: impl AsRef<Path>,
    max_files: u32,
    limit: usize,
) -> std::io::Result<Vec<LogEntry>> {
    let logs_dir = logs_dir(app_data_dir);
    let bounded_limit = limit.clamp(1, 1_000);
    let mut entries = Vec::new();

    for path in log_file_paths(&logs_dir, max_files) {
        if !path.exists() {
            continue;
        }

        let file_name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| LOG_FILE_NAME.to_owned());
        let file = fs::File::open(&path)?;
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line?;
            let record = serde_json::from_str::<Value>(&line)
                .unwrap_or_else(|_| json!({ "message": redact_text(&line) }));
            entries.push(LogEntry {
                file_name: file_name.clone(),
                line_number: index + 1,
                record,
            });
        }
    }

    if entries.len() > bounded_limit {
        let start = entries.len() - bounded_limit;
        entries.drain(0..start);
    }

    Ok(entries)
}

fn normalize_log_level(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "error" | "warn" | "info" | "debug" | "trace" => {
            Some(value.trim().to_ascii_lowercase())
        }
        _ => None,
    }
}

fn level_filter_from_str(value: &str) -> LevelFilter {
    match value {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

fn log_file_paths(logs_dir: &Path, max_files: u32) -> Vec<PathBuf> {
    let mut paths = vec![logs_dir.join(LOG_FILE_NAME)];
    for index in 1..max_files.max(1) {
        paths.push(logs_dir.join(format!("{LOG_FILE_NAME}.{index}")));
    }
    paths
}

struct JsonFileLogger {
    level: LevelFilter,
    writer: Mutex<RotatingLogWriter>,
}

impl JsonFileLogger {
    fn new(logs_dir: PathBuf, settings: ResolvedLogSettings) -> Self {
        Self {
            level: level_filter_from_str(&settings.level),
            writer: Mutex::new(RotatingLogWriter::new(
                logs_dir,
                settings.max_files,
                settings.max_file_bytes,
            )),
        }
    }
}

impl Log for JsonFileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned());
        let metadata = redact_metadata(json!({
            "timestamp": timestamp,
            "level": record.level().as_str().to_ascii_lowercase(),
            "target": record.target(),
            "modulePath": record.module_path(),
            "file": record.file(),
            "line": record.line(),
            "message": redact_text(&record.args().to_string()),
        }));

        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_line(&metadata.to_string());
        }
    }

    fn flush(&self) {}
}

#[derive(Debug)]
struct RotatingLogWriter {
    logs_dir: PathBuf,
    max_files: u32,
    max_file_bytes: u64,
}

impl RotatingLogWriter {
    fn new(logs_dir: PathBuf, max_files: u32, max_file_bytes: u64) -> Self {
        Self {
            logs_dir,
            max_files: max_files.max(1),
            max_file_bytes: max_file_bytes.max(1),
        }
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        fs::create_dir_all(&self.logs_dir)?;
        let current_path = self.current_path();
        let line_len = line.len() as u64 + 1;
        let current_len = fs::metadata(&current_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        if current_len > 0 && current_len.saturating_add(line_len) > self.max_file_bytes {
            self.rotate()?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.current_path())?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()
    }

    fn rotate(&self) -> std::io::Result<()> {
        let current_path = self.current_path();
        if self.max_files <= 1 {
            if current_path.exists() {
                fs::remove_file(current_path)?;
            }
            return Ok(());
        }

        let oldest_path = self.rotated_path(self.max_files - 1);
        if oldest_path.exists() {
            fs::remove_file(oldest_path)?;
        }

        for index in (1..self.max_files - 1).rev() {
            let from = self.rotated_path(index);
            if from.exists() {
                fs::rename(from, self.rotated_path(index + 1))?;
            }
        }

        if current_path.exists() {
            fs::rename(current_path, self.rotated_path(1))?;
        }

        Ok(())
    }

    fn current_path(&self) -> PathBuf {
        self.logs_dir.join(LOG_FILE_NAME)
    }

    fn rotated_path(&self, index: u32) -> PathBuf {
        self.logs_dir.join(format!("{LOG_FILE_NAME}.{index}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn settings() -> LogSettings {
        LogSettings {
            level: "info".to_owned(),
            max_files: 5,
            max_file_bytes: 5_242_880,
        }
    }

    #[test]
    fn resolves_log_settings_from_persisted_defaults() {
        let resolved = resolve_log_settings_with_env(&settings(), |_| None);

        assert_eq!(
            resolved,
            ResolvedLogSettings {
                level: "info".to_owned(),
                max_files: 5,
                max_file_bytes: 5_242_880,
            }
        );
    }

    #[test]
    fn applies_valid_environment_overrides() {
        let resolved = resolve_log_settings_with_env(&settings(), |key| match key {
            "BRAWLER_LOG_LEVEL" => Some("DEBUG".to_owned()),
            "BRAWLER_LOG_MAX_FILES" => Some("9".to_owned()),
            "BRAWLER_LOG_MAX_FILE_MEGABYTES" => Some("10".to_owned()),
            _ => None,
        });

        assert_eq!(resolved.level, "debug");
        assert_eq!(resolved.max_files, 9);
        assert_eq!(resolved.max_file_bytes, 10_485_760);
    }

    #[test]
    fn ignores_invalid_environment_overrides() {
        let resolved = resolve_log_settings_with_env(&settings(), |key| match key {
            "BRAWLER_LOG_LEVEL" => Some("verbose".to_owned()),
            "BRAWLER_LOG_MAX_FILES" => Some("0".to_owned()),
            "BRAWLER_LOG_MAX_FILE_MEGABYTES" => Some("0".to_owned()),
            _ => None,
        });

        assert_eq!(resolved.level, "info");
        assert_eq!(resolved.max_files, 5);
        assert_eq!(resolved.max_file_bytes, 5_242_880);
    }

    #[test]
    fn rotating_writer_writes_json_lines_and_rotates() {
        let logs_dir = unique_test_dir("rotation");
        let mut writer = RotatingLogWriter::new(logs_dir.clone(), 3, 64);

        writer
            .write_line(r#"{"message":"first line that is intentionally long"}"#)
            .expect("first log line should write");
        writer
            .write_line(r#"{"message":"second line that rotates current file"}"#)
            .expect("second log line should write");

        assert!(logs_dir.join(LOG_FILE_NAME).exists());
        assert!(logs_dir.join(format!("{LOG_FILE_NAME}.1")).exists());
        assert!(!logs_dir.join(format!("{LOG_FILE_NAME}.3")).exists());

        let _ = fs::remove_dir_all(logs_dir);
    }

    #[test]
    fn json_file_logger_redacts_sensitive_message_fields() {
        let logs_dir = unique_test_dir("redaction");
        let logger = JsonFileLogger::new(
            logs_dir.clone(),
            ResolvedLogSettings {
                level: "info".to_owned(),
                max_files: 2,
                max_file_bytes: 4_096,
            },
        );

        let args = format_args!("apiKey=secret prompt=full sourceBody=body");
        let record = Record::builder()
            .args(args)
            .level(log::Level::Info)
            .target("brawler::test")
            .module_path(Some("brawler::test"))
            .build();

        logger.log(&record);

        let content =
            fs::read_to_string(logs_dir.join(LOG_FILE_NAME)).expect("log file should be readable");
        let value: Value = serde_json::from_str(content.trim()).expect("log line should be JSON");

        assert_eq!(value["level"], "info");
        assert_eq!(value["target"], "brawler::test");
        assert_eq!(
            value["message"],
            "apiKey=[redacted] prompt=[redacted] sourceBody=[redacted]"
        );

        let _ = fs::remove_dir_all(logs_dir);
    }

    #[test]
    fn reports_log_status_from_app_data_dir() {
        let app_data_dir = unique_test_dir("status");
        let logs_dir = logs_dir(&app_data_dir);
        fs::create_dir_all(&logs_dir).expect("logs dir should be created");
        fs::write(logs_dir.join(LOG_FILE_NAME), "line\n").expect("current log should be written");
        fs::write(logs_dir.join(format!("{LOG_FILE_NAME}.1")), "old\n")
            .expect("rotated log should be written");

        let settings = ResolvedLogSettings {
            level: "debug".to_owned(),
            max_files: 5,
            max_file_bytes: 5_242_880,
        };
        let status = log_status(&app_data_dir, &settings).expect("status should resolve");

        assert_eq!(status.current_file_bytes, 5);
        assert_eq!(status.rotated_file_count, 1);
        assert_eq!(status.level, "debug");
        assert_eq!(status.max_files, 5);

        let _ = fs::remove_dir_all(app_data_dir);
    }

    #[test]
    fn reads_bounded_log_entries_from_app_data_dir() {
        let app_data_dir = unique_test_dir("read");
        let logs_dir = logs_dir(&app_data_dir);
        fs::create_dir_all(&logs_dir).expect("logs dir should be created");
        fs::write(
            logs_dir.join(LOG_FILE_NAME),
            "{\"message\":\"one\"}\n{\"message\":\"two\"}\n",
        )
        .expect("current log should be written");
        fs::write(
            logs_dir.join(format!("{LOG_FILE_NAME}.1")),
            "{\"message\":\"old\"}\n",
        )
        .expect("rotated log should be written");

        let entries = read_log_entries(&app_data_dir, 2, 2).expect("entries should read");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].record["message"], "two");
        assert_eq!(entries[1].record["message"], "old");

        let _ = fs::remove_dir_all(app_data_dir);
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("brawler-{name}-{suffix}"))
    }
}
