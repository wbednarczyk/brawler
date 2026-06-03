import { ClipboardCopy, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import * as diagnosticsApi from "../../api/diagnostics";
import type { DiagnosticEvent, DiagnosticSeverity } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { useLocale } from "../../shared/locale";

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
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [diagnosticsStatus, setDiagnosticsStatus] = useState<string | null>(null);
  const [inFlight, setInFlight] = useState(false);

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

  useEffect(() => {
    refreshDiagnostics();
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

  return (
    <section className="feed-panel diagnostics-panel" aria-labelledby="diagnostics-title">
      <div className="panel-header">
        <div>
          <h1 id="diagnostics-title">{t("diagnostics.title")}</h1>
          <p>{t("diagnostics.description")}</p>
        </div>
        <div className="diagnostics-actions">
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
          <Button className="compact-button" disabled={inFlight} onClick={onDisableDeveloperMode} variant="ghost">
            <ShieldAlert size={15} />
            {text("Disable Developer mode")}
          </Button>
        </div>
      </div>
      <p className="settings-note">
        {text("Developer mode is active. Diagnostics remain local-only.")}
      </p>

      <div className="filter-toolbar diagnostics-filter-toolbar" aria-label={text("Diagnostic filters")}>
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
      </div>

      {diagnosticsStatus ? <p className="settings-note">{diagnosticsStatus}</p> : null}
      {diagnosticsError ? <p className="error-text">{text("Diagnostics command failed")}: {diagnosticsError}</p> : null}

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
                  <dl className="settings-grid">
                    <div>
                      <dt>{text("Scope")}</dt>
                      <dd>{formatScope(event)}</dd>
                    </div>
                    <div>
                      <dt>{text("Created")}</dt>
                      <dd>{event.createdAt}</dd>
                    </div>
                  </dl>
                  <pre>{JSON.stringify(event.metadata, null, 2)}</pre>
                </div>
              ) : null}
            </article>
          );
        })}
        {filteredEvents.length === 0 ? <EmptyState>{text("No diagnostic events recorded.")}</EmptyState> : null}
      </div>
    </section>
  );
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
