# ADR 0015: Developer Mode And Local Observability

Status: Accepted

## Context

Brawler needs enough observability to debug local source ingestion, provider calls, background jobs, storage, credentials, and future modules. The app also stores personal research data and should not introduce telemetry, hosted diagnostics, or hidden reporting in v1.

Developer diagnostics, runtime logs, and metrics serve different purposes. Combining them into one ad hoc debug feature would make privacy rules, retention, and UI expectations harder to reason about.

## Decision

Brawler v1 observability remains local-only:

- no telemetry
- no remote error reporting
- no remote log shipping
- no hosted metrics
- no hosted tracing

Developer mode is an intentional local mode for trusted users. It is off by default and enabled only through local developer mechanisms. Startup activation uses `BRAWLER_DEVELOPER_MODE=1`, `true`, `yes`, or `on`. Runtime author unlock may enable Developer mode after the app is already running only when `BRAWLER_DEVELOPER_UNLOCK_CODE` is present in the app process environment and the submitted passphrase matches it. Normal Settings must not expose an always-visible enable toggle, and runtime unlock must not be registered as a configurable shortcut. Once Developer mode is active, the Diagnostics panel may show active status and a disable action.

Developer diagnostics are structured SQLite-backed events recorded only while Developer mode is enabled. Diagnostic events use a shared typed contract with timestamp, module, scope/entity ID, stage, severity, message, and redacted metadata. They are shown in a dedicated developer-only Diagnostics panel.

Runtime logs are a separate local append-only file framework with conservative defaults, rotation, and shared redaction rules. Runtime logging uses the Rust `log` facade with a local JSON Lines file backend. Logs are written under the app data logs directory, default to `info`, and can be configured through Settings and local environment overrides. Rotation is configurable and defaults to five files of five MiB each. The Diagnostics panel may expose a developer-only full in-app log viewer and log-folder open action, while Settings may expose always-visible log configuration because those settings affect local files only.

Metrics are separate local operational health signals exposed only in Developer mode. They are not product analytics or user behavior tracking. The metrics implementation should keep collection, internal metric representation, and presentation/export adapters separate. M16 should expose an in-app Developer-mode view through a typed command, while keeping the internal metric model compatible with future Prometheus-style adapters and other metrics ecosystems. M16 metrics are collected as on-demand snapshots from durable local state plus explicit in-memory runtime counters that reset when the app restarts. Future adapters may render the same internal samples into Prometheus text format, OpenTelemetry mapping, files, or another local integration, but adding any new exposure surface still requires a separate decision.

OpenTelemetry-compatible naming and structure are acceptable where cheap, but v1 must not add OpenTelemetry dependencies, exporters, remote reporting, or compatibility-only code unless a later implementation proves the overhead is low.

## Consequences

- M14 implements Developer mode and structured diagnostics before logs and metrics.
- M15 implements local runtime logs separately using the `log` crate, JSON Lines, configurable rotation, Settings-visible log configuration, and a Developer-mode Diagnostics log viewer.
- M16 implements modest local metrics separately with a pluggable collector and adapter boundary.
- Diagnostics, logs, and metrics share redaction rules but remain different surfaces.
- Events are not recorded while Developer mode is disabled.
- Logs, diagnostics, and metrics must not include API keys, full prompts, full source bodies, full transcript text, raw provider responses, license private material, or full license secrets by default.
- Telemetry, hosted observability, remote crash reporting, remote log shipping, hosted metrics, hosted tracing, a Prometheus endpoint, or an OpenTelemetry exporter require a future ADR.
