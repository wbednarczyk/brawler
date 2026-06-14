import { ChevronDown, ClipboardCopy, FolderOpen, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import * as diagnosticsApi from "../../api/diagnostics";
import * as logsApi from "../../api/logs";
import * as metricsApi from "../../api/metrics";
import * as sourcesApi from "../../api/sources";
import type {
  DiagnosticEvent,
  DiagnosticSeverity,
  LocalMetricsSnapshot,
  LogEntry,
  LogStatus,
  MetricSample,
  SourceAdapter,
} from "../../api/types";
import { ActionRow, Button, EmptyState, FilterToolbar, InfoGrid, PanelHeader } from "../../ui";
import { useLocale } from "../../shared/locale";
import { BackupsSection } from "./BackupsSection";

const eventLimit = 200;
const severityOptions: Array<DiagnosticSeverity | "all"> = [
  "all",
  "debug",
  "info",
  "warning",
  "error",
];

type DiagnosticsScreenProps = {
  developerMode: boolean;
  onDisableDeveloperMode: () => void;
};

export function DiagnosticsScreen({
  developerMode,
  onDisableDeveloperMode,
}: DiagnosticsScreenProps) {
  const { t, text } = useLocale();
  const [events, setEvents] = useState<DiagnosticEvent[]>([]);
  const [moduleFilter, setModuleFilter] = useState("all");
  const [severityFilter, setSeverityFilter] = useState<DiagnosticSeverity | "all">("all");
  const [expandedEventId, setExpandedEventId] = useState<string | null>(null);
  const [expandedLogEntryId, setExpandedLogEntryId] = useState<string | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [diagnosticsStatus, setDiagnosticsStatus] = useState<string | null>(null);
  const [logStatus, setLogStatus] = useState<LogStatus | null>(null);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [metricsSnapshot, setMetricsSnapshot] = useState<LocalMetricsSnapshot | null>(null);
  const [developerSources, setDeveloperSources] = useState<SourceAdapter[]>([]);
  const [inFlight, setInFlight] = useState(false);
  const [logsInFlight, setLogsInFlight] = useState(false);
  const [metricsInFlight, setMetricsInFlight] = useState(false);
  const [eventsSectionOpen, setEventsSectionOpen] = useState(true);
  const [sourcesSectionOpen, setSourcesSectionOpen] = useState(false);
  const [metricsSectionOpen, setMetricsSectionOpen] = useState(true);
  const [logsSectionOpen, setLogsSectionOpen] = useState(true);

  function refreshDiagnostics() {
    if (!developerMode) {
      setEvents([]);
      return;
    }

    setInFlight(true);
    diagnosticsApi.listDiagnosticEvents({ limit: eventLimit })
      .then((response) => {
        setEvents(response);
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      })
      .finally(() => {
        setInFlight(false);
      });
  }

  function refreshLogs() {
    if (!developerMode) {
      setLogStatus(null);
      setLogEntries([]);
      return;
    }

    setLogsInFlight(true);
    Promise.all([
      logsApi.getLogStatus(),
      logsApi.listLogEntries({ limit: 300 }),
    ])
      .then(([status, entries]) => {
        setLogStatus(status);
        setLogEntries(entries);
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      })
      .finally(() => {
        setLogsInFlight(false);
      });
  }

  function refreshMetrics() {
    if (!developerMode) {
      setMetricsSnapshot(null);
      return;
    }

    setMetricsInFlight(true);
    metricsApi.getLocalMetricsSnapshot()
      .then((snapshot) => {
        setMetricsSnapshot(snapshot);
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      })
      .finally(() => {
        setMetricsInFlight(false);
      });
  }

  function refreshDeveloperSources() {
    if (!developerMode) {
      setDeveloperSources([]);
      return;
    }

    sourcesApi.listSourceAdapters({ includeDeveloperOnly: true })
      .then((adapters) => {
        setDeveloperSources(adapters.filter((adapter) => adapter.visibility === "developer"));
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      });
  }

  useEffect(() => {
    refreshDiagnostics();
    refreshMetrics();
    refreshLogs();
    refreshDeveloperSources();
  }, [developerMode]);

  const modules = useMemo(() => {
    return Array.from(new Set(events.map((event) => event.module))).sort();
  }, [events]);

  const filteredEvents = events.filter((event) => {
    return (
      (moduleFilter === "all" || event.module === moduleFilter) &&
      (severityFilter === "all" || event.severity === severityFilter)
    );
  });

  function clearDiagnostics() {
    if (!developerMode) {
      return;
    }

    setInFlight(true);
    diagnosticsApi.clearDiagnosticEvents()
      .then((result) => {
        setEvents([]);
        setExpandedEventId(null);
        setDiagnosticsStatus(`${result.eventsDeleted} ${text("diagnostic events cleared")}`);
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      })
      .finally(() => {
        setInFlight(false);
      });
  }

  function copySummary() {
    if (!developerMode) {
      return;
    }

    setInFlight(true);
    diagnosticsApi.getDiagnosticSummary({ limit: eventLimit })
      .then((result) => navigator.clipboard.writeText(result.summary).then(() => result))
      .then((result) => {
        setDiagnosticsStatus(`${result.eventCount} ${text("diagnostic events copied")}`);
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      })
      .finally(() => {
        setInFlight(false);
      });
  }

  function copyLogs() {
    if (!developerMode) {
      return;
    }

    setLogsInFlight(true);
    logsApi.listLogEntries({ limit: 300 })
      .then((entries) => {
        const summary = entries.map((entry) => JSON.stringify(entry.record)).join("\n");
        return navigator.clipboard.writeText(summary).then(() => entries);
      })
      .then((entries) => {
        setDiagnosticsStatus(`${entries.length} ${text("log entries copied")}`);
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      })
      .finally(() => {
        setLogsInFlight(false);
      });
  }

  function openLogsDirectory() {
    if (!developerMode) {
      return;
    }

    setLogsInFlight(true);
    logsApi.openLogsDirectory()
      .then(() => {
        setDiagnosticsStatus(text("Logs folder opened"));
        setDiagnosticsError(null);
      })
      .catch((error) => {
        setDiagnosticsError(String(error));
      })
      .finally(() => {
        setLogsInFlight(false);
      });
  }

  const metricGroups = useMemo(() => groupMetricSamples(metricsSnapshot?.samples ?? []), [metricsSnapshot]);

  return (
    <section className="feed-panel diagnostics-panel" aria-labelledby="diagnostics-title">
      <PanelHeader
        title={t("diagnostics.title")}
        description={t("diagnostics.description")}
        titleId="diagnostics-title"
        actions={
          <ActionRow className="diagnostics-actions">
            <Button className="compact-button" disabled={inFlight} onClick={onDisableDeveloperMode} variant="ghost">
              <ShieldAlert size={15} />
              {text("Disable Developer mode")}
            </Button>
          </ActionRow>
        }
      />
      <p className="settings-note">
        {text("Developer mode is active. Diagnostics remain local-only.")}
      </p>

      {diagnosticsStatus ? <p className="settings-note">{diagnosticsStatus}</p> : null}
      {diagnosticsError ? <p className="error-text">{text("Diagnostics command failed")}: {diagnosticsError}</p> : null}

      <div className="diagnostics-sections">
        <section className="diagnostics-section" aria-labelledby="diagnostics-events-title">
          <button
            aria-expanded={eventsSectionOpen}
            className="diagnostics-section-header"
            onClick={() => setEventsSectionOpen((open) => !open)}
            type="button"
          >
            <span>
              <h2 id="diagnostics-events-title">{text("Diagnostic events")}</h2>
              <small>{text("Structured developer-mode module events.")}</small>
            </span>
            <span className="diagnostics-section-meta">
              {filteredEvents.length}/{events.length}
              <ChevronDown className={eventsSectionOpen ? "section-chevron section-chevron-open" : "section-chevron"} size={16} />
            </span>
          </button>
          {eventsSectionOpen ? (
            <div className="diagnostics-section-body">
              <div className="diagnostics-section-toolbar">
                <FilterToolbar ariaLabel={text("Diagnostic filters")} className="diagnostics-filter-toolbar">
                  <label>
                    {text("Module")}
                    <select value={moduleFilter} onChange={(event) => setModuleFilter(event.target.value)}>
                      <option value="all">{text("All modules")}</option>
                      {modules.map((moduleId) => (
                        <option key={moduleId} value={moduleId}>
                          {moduleId}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    {text("Severity")}
                    <select
                      value={severityFilter}
                      onChange={(event) => setSeverityFilter(event.target.value as DiagnosticSeverity | "all")}
                    >
                      {severityOptions.map((severity) => (
                        <option key={severity} value={severity}>
                          {severity === "all" ? text("All severities") : severity}
                        </option>
                      ))}
                    </select>
                  </label>
                </FilterToolbar>
                <ActionRow className="diagnostics-actions">
                  <Button className="compact-button" disabled={inFlight} onClick={refreshDiagnostics}>
                    <RefreshCw size={15} />
                    {inFlight ? text("Loading") : text("Refresh")}
                  </Button>
                  <Button className="compact-button" disabled={inFlight} onClick={copySummary} variant="ghost">
                    <ClipboardCopy size={15} />
                    {text("Copy summary")}
                  </Button>
                  <Button className="compact-button" disabled={inFlight || events.length === 0} onClick={clearDiagnostics} variant="ghost">
                    <Trash2 size={15} />
                    {text("Clear")}
                  </Button>
                </ActionRow>
              </div>
              <div className="diagnostics-list" aria-label={text("Diagnostic events")}>
                {filteredEvents.map((event) => {
                  const expanded = expandedEventId === event.id;
                  return (
                    <article className="diagnostic-event" key={event.id}>
                      <button
                        className="diagnostic-event-main"
                        onClick={() => setExpandedEventId(expanded ? null : event.id)}
                        type="button"
                      >
                        <span className={`diagnostic-severity diagnostic-severity-${event.severity}`}>
                          {event.severity}
                        </span>
                        <span className="diagnostic-event-stage">
                          <strong>{event.module}</strong>
                          <span>{event.stage}</span>
                        </span>
                        <span className="diagnostic-event-message">{event.message}</span>
                        <time dateTime={event.occurredAt}>{event.occurredAt}</time>
                      </button>
                      {expanded ? (
                        <div className="diagnostic-event-detail">
                          <InfoGrid
                            className="settings-grid"
                            items={[
                              { label: text("Scope"), value: formatScope(event) },
                              { label: text("Created"), value: event.createdAt },
                            ]}
                          />
                          <pre>{JSON.stringify(event.metadata, null, 2)}</pre>
                        </div>
                      ) : null}
                    </article>
                  );
                })}
                {filteredEvents.length === 0 ? <EmptyState>{text("No diagnostic events recorded.")}</EmptyState> : null}
              </div>
            </div>
          ) : null}
        </section>

        <section className="diagnostics-section" aria-labelledby="diagnostics-sources-title">
          <button
            aria-expanded={sourcesSectionOpen}
            className="diagnostics-section-header"
            onClick={() => setSourcesSectionOpen((open) => !open)}
            type="button"
          >
            <span>
              <h2 id="diagnostics-sources-title">{text("Source candidates")}</h2>
              <small>{text("Registered source candidates and developer-only source details.")}</small>
            </span>
            <span className="diagnostics-section-meta">
              {developerSources.length}
              <ChevronDown className={sourcesSectionOpen ? "section-chevron section-chevron-open" : "section-chevron"} size={16} />
            </span>
          </button>
          {sourcesSectionOpen ? (
            <div className="diagnostics-section-body">
              <div className="diagnostics-list" aria-label={text("Source candidates")}>
                {developerSources.map((adapter) => (
                  <article className="diagnostic-event" key={adapter.id}>
                    <div className="diagnostic-event-main">
                      <span className="diagnostic-severity diagnostic-severity-info">
                        {adapter.enabled ? "enabled" : "disabled"}
                      </span>
                      <span className="diagnostic-event-stage">
                        <strong>{adapter.id}</strong>
                        <span>{adapter.sourceType} · {adapter.fetchMode}</span>
                      </span>
                      <span className="diagnostic-event-message">{adapter.displayName}</span>
                      <time>{adapter.healthStatus}</time>
                    </div>
                    <div className="diagnostic-event-detail">
                      <InfoGrid
                        className="settings-grid"
                        items={[
                          { label: text("URL"), value: adapter.sourceUrl },
                          { label: text("Policy"), value: adapter.policyNote },
                        ]}
                      />
                    </div>
                  </article>
                ))}
                {developerSources.length === 0 ? <EmptyState>{text("No source candidates registered.")}</EmptyState> : null}
              </div>
            </div>
          ) : null}
        </section>

        <section className="diagnostics-section" aria-labelledby="diagnostics-metrics-title">
          <button
            aria-expanded={metricsSectionOpen}
            className="diagnostics-section-header"
            onClick={() => setMetricsSectionOpen((open) => !open)}
            type="button"
          >
            <span>
              <h2 id="diagnostics-metrics-title">{text("Metrics")}</h2>
              <small>{text("Local operational counters, gauges, and durations.")}</small>
            </span>
            <span className="diagnostics-section-meta">
              {metricsSnapshot?.samples.length ?? 0}
              <ChevronDown className={metricsSectionOpen ? "section-chevron section-chevron-open" : "section-chevron"} size={16} />
            </span>
          </button>
          {metricsSectionOpen ? (
            <div className="diagnostics-section-body">
              <div className="diagnostics-section-toolbar">
                <InfoGrid
                  className="settings-grid diagnostics-log-status"
                  items={[
                    { label: text("Collected"), value: metricsSnapshot?.collectedAt ?? text("Unknown") },
                    { label: text("Samples"), value: metricsSnapshot?.samples.length ?? 0 },
                  ]}
                />
                <ActionRow className="diagnostics-actions">
                  <Button className="compact-button" disabled={metricsInFlight} onClick={refreshMetrics}>
                    <RefreshCw size={15} />
                    {metricsInFlight ? text("Loading") : text("Refresh metrics")}
                  </Button>
                </ActionRow>
              </div>
              <div className="diagnostics-metrics-list" aria-label={text("Local metrics")}>
                {metricGroups.map((group) => (
                  <article className="diagnostics-metric-group" key={group.name}>
                    <div>
                      <h3>{group.name}</h3>
                      <small>{group.samples[0]?.description}</small>
                    </div>
                    <div className="diagnostics-metric-samples">
                      {group.samples.map((sample) => (
                        <MetricSampleRow key={metricSampleKey(sample)} sample={sample} />
                      ))}
                    </div>
                  </article>
                ))}
                {metricGroups.length === 0 ? <EmptyState>{text("No local metrics recorded.")}</EmptyState> : null}
              </div>
            </div>
          ) : null}
        </section>

        <section className="diagnostics-section" aria-labelledby="diagnostics-logs-title">
          <button
            aria-expanded={logsSectionOpen}
            className="diagnostics-section-header"
            onClick={() => setLogsSectionOpen((open) => !open)}
            type="button"
          >
            <span>
              <h2 id="diagnostics-logs-title">{text("Runtime logs")}</h2>
              <small>{text("Local JSON log files under the app data directory.")}</small>
            </span>
            <span className="diagnostics-section-meta">
              {logEntries.length}
              <ChevronDown className={logsSectionOpen ? "section-chevron section-chevron-open" : "section-chevron"} size={16} />
            </span>
          </button>
          {logsSectionOpen ? (
            <div className="diagnostics-section-body">
              <div className="diagnostics-section-toolbar">
                <InfoGrid
                  className="settings-grid diagnostics-log-status"
                  items={[
                    { label: text("Directory"), value: logStatus?.logsDir ?? text("Unknown") },
                    { label: text("Level"), value: logStatus?.level ?? "info" },
                    { label: text("Current file"), value: formatBytes(logStatus?.currentFileBytes ?? 0) },
                    {
                      label: text("Rotation"),
                      value: logStatus ? `${logStatus.maxFiles} x ${formatBytes(logStatus.maxFileBytes)}` : text("Unknown"),
                    },
                  ]}
                />
                <ActionRow className="diagnostics-actions">
                  <Button className="compact-button" disabled={logsInFlight} onClick={refreshLogs}>
                    <RefreshCw size={15} />
                    {logsInFlight ? text("Loading") : text("Refresh logs")}
                  </Button>
                  <Button className="compact-button" disabled={logsInFlight || logEntries.length === 0} onClick={copyLogs} variant="ghost">
                    <ClipboardCopy size={15} />
                    {text("Copy logs")}
                  </Button>
                  <Button className="compact-button" disabled={logsInFlight} onClick={openLogsDirectory} variant="ghost">
                    <FolderOpen size={15} />
                    {text("Open logs folder")}
                  </Button>
                </ActionRow>
              </div>
              <div className="diagnostics-log-viewer" aria-label={text("Runtime log entries")}>
                {logEntries.map((entry) => (
                  <LogEntryRow
                    entry={entry}
                    expanded={expandedLogEntryId === logEntryId(entry)}
                    key={logEntryId(entry)}
                    onToggle={() => {
                      const entryId = logEntryId(entry);
                      setExpandedLogEntryId((current) => current === entryId ? null : entryId);
                    }}
                  />
                ))}
                {logEntries.length === 0 ? <EmptyState>{text("No runtime log entries recorded.")}</EmptyState> : null}
              </div>
            </div>
          ) : null}
        </section>

        <BackupsSection />
      </div>
    </section>
  );
}

type MetricSampleGroup = {
  name: string;
  samples: MetricSample[];
};

function groupMetricSamples(samples: MetricSample[]): MetricSampleGroup[] {
  const groups = new Map<string, MetricSample[]>();

  for (const sample of samples) {
    const existing = groups.get(sample.name) ?? [];
    existing.push(sample);
    groups.set(sample.name, existing);
  }

  return Array.from(groups.entries())
    .map(([name, samples]) => ({ name, samples }))
    .sort((left, right) => left.name.localeCompare(right.name));
}

function MetricSampleRow({ sample }: { sample: MetricSample }) {
  return (
    <div className="diagnostics-metric-sample">
      <span className="diagnostics-metric-value">{formatMetricValue(sample)}</span>
      <span className="diagnostics-metric-kind">{sample.kind} · {sample.unit}</span>
      <span className="diagnostics-metric-labels">{formatMetricLabels(sample)}</span>
    </div>
  );
}

function metricSampleKey(sample: MetricSample) {
  return `${sample.name}:${sample.labels.map((label) => `${label.key}=${label.value}`).join(",")}`;
}

function formatMetricLabels(sample: MetricSample) {
  if (sample.labels.length === 0) {
    return "global";
  }

  return sample.labels.map((label) => `${label.key}=${label.value}`).join(" · ");
}

function formatMetricValue(sample: MetricSample) {
  if (sample.unit === "bytes") {
    return formatBytes(sample.value);
  }

  if (sample.unit === "seconds") {
    return `${sample.value.toFixed(3)}s`;
  }

  if (Number.isInteger(sample.value)) {
    return String(sample.value);
  }

  return sample.value.toFixed(2);
}

type LogEntryRowProps = {
  entry: LogEntry;
  expanded: boolean;
  onToggle: () => void;
};

function LogEntryRow({ entry, expanded, onToggle }: LogEntryRowProps) {
  const message = String(entry.record.message ?? JSON.stringify(entry.record));

  return (
    <article className="diagnostic-log-entry">
      <button className="diagnostic-log-entry-main" onClick={onToggle} type="button">
        <span>
          <strong>{String(entry.record.level ?? "info")}</strong>
          <small>{String(entry.record.timestamp ?? "")}</small>
          <small>{entry.fileName}:{entry.lineNumber}</small>
        </span>
        <span>{message}</span>
        <ChevronDown className={expanded ? "section-chevron section-chevron-open" : "section-chevron"} size={16} />
      </button>
      {expanded ? <pre>{JSON.stringify(entry.record, null, 2)}</pre> : null}
    </article>
  );
}

function logEntryId(entry: LogEntry) {
  return `${entry.fileName}:${entry.lineNumber}`;
}

function formatBytes(value: number) {
  if (value >= 1_048_576) {
    return `${Math.round(value / 1_048_576)} MiB`;
  }

  if (value >= 1024) {
    return `${Math.round(value / 1024)} KiB`;
  }

  return `${value} B`;
}

function formatScope(event: DiagnosticEvent) {
  if (!event.scope) {
    return "global";
  }

  if (!event.scope.id) {
    return event.scope.type;
  }

  return `${event.scope.type}:${event.scope.id}`;
}
