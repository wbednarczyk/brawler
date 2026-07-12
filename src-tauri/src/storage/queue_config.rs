//! Worker-pool and per-provider concurrency tuning for the durable job queue
//! ([ADR 0059](../../../docs/adr/0059-worker-pools-and-queue-fairness.md)).
//!
//! Read from the `settings` KV table with safe-range clamping and default
//! fallback (the `db_max_connections` pattern), so an absent or invalid value can
//! never stop the queue from starting. Worker counts apply at startup (the lanes
//! are spawned once); the per-provider concurrency limit bounds concurrent AI
//! provider calls shared across the `autopilot` and `ai` lanes.

use super::*;

const DEFAULT_SOURCES_WORKERS: i64 = 2;
const DEFAULT_AUTOPILOT_WORKERS: i64 = 3;
const DEFAULT_AI_WORKERS: i64 = 2;
const DEFAULT_AI_PROVIDER_CONCURRENCY: i64 = 2;

const MIN_WORKERS: i64 = 1;
const MAX_WORKERS: i64 = 16;
const MIN_PROVIDER_CONCURRENCY: i64 = 1;
const MAX_PROVIDER_CONCURRENCY: i64 = 10;

pub(super) const SOURCES_WORKERS_KEY: &str = "sources_workers";
pub(super) const AUTOPILOT_WORKERS_KEY: &str = "autopilot_workers";
pub(super) const AI_WORKERS_KEY: &str = "ai_workers";
pub(super) const AI_PROVIDER_CONCURRENCY_KEY: &str = "ai_provider_concurrency";

/// Resolved queue tuning: lane worker counts + the per-AI-provider concurrency
/// limit. Copyable snapshot read once when the lanes are spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueConfig {
    pub sources_workers: i64,
    pub autopilot_workers: i64,
    pub ai_workers: i64,
    pub ai_provider_concurrency: i64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            sources_workers: DEFAULT_SOURCES_WORKERS,
            autopilot_workers: DEFAULT_AUTOPILOT_WORKERS,
            ai_workers: DEFAULT_AI_WORKERS,
            ai_provider_concurrency: DEFAULT_AI_PROVIDER_CONCURRENCY,
        }
    }
}

pub(super) fn clamp_workers(value: i64) -> i64 {
    value.clamp(MIN_WORKERS, MAX_WORKERS)
}

pub(super) fn clamp_provider_concurrency(value: i64) -> i64 {
    value.clamp(MIN_PROVIDER_CONCURRENCY, MAX_PROVIDER_CONCURRENCY)
}

fn read_setting_i64(connection: &Connection, key: &str) -> Option<i64> {
    connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok())
}

pub(super) fn read_queue_config(connection: &Connection) -> QueueConfig {
    let defaults = QueueConfig::default();
    QueueConfig {
        sources_workers: read_setting_i64(connection, SOURCES_WORKERS_KEY)
            .map(clamp_workers)
            .unwrap_or(defaults.sources_workers),
        autopilot_workers: read_setting_i64(connection, AUTOPILOT_WORKERS_KEY)
            .map(clamp_workers)
            .unwrap_or(defaults.autopilot_workers),
        ai_workers: read_setting_i64(connection, AI_WORKERS_KEY)
            .map(clamp_workers)
            .unwrap_or(defaults.ai_workers),
        ai_provider_concurrency: read_setting_i64(connection, AI_PROVIDER_CONCURRENCY_KEY)
            .map(clamp_provider_concurrency)
            .unwrap_or(defaults.ai_provider_concurrency),
    }
}
