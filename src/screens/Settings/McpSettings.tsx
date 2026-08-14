import { useEffect, useState } from "react";
import { Copy, KeyRound, Trash2 } from "lucide-react";

import {
  kpiAcquisitionTokenStatus,
  mcpStatus as fetchMcpStatus,
  mcpTokenStatus,
  regenerateKpiAcquisitionToken,
  regenerateMcpToken,
  revokeKpiAcquisitionToken,
  revokeMcpToken,
  setMcpEnabled,
  type McpStatus,
} from "../../api/mcp";
import type { CredentialStatus, UserSettings } from "../../api/types";
import {
  ActionRow,
  Button,
  ErrorText,
  Hint,
  InlineConfirm,
  StatusPill,
  TextField,
} from "../../ui";
import { useLocale } from "../../shared/locale";

export type McpSettingsProps = {
  settings: UserSettings | null;
  onMcpPortChange: (port: number) => void;
  onMcpWritesEnabledChange: (enabled: boolean) => void;
  onKpiAcquisitionEnabledChange: (enabled: boolean) => void;
};

const MIN_PORT = 1024;
const MAX_PORT = 65535;
const DEFAULT_PORT = 8317;

function clampPort(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_PORT;
  return Math.min(MAX_PORT, Math.max(MIN_PORT, Math.trunc(value)));
}

/**
 * Settings section "MCP server" (M4, ADR 0078 decision 4). Self-contained like
 * `ProviderApiKeyForm`: the enable/token/status lifecycle runs through the
 * dedicated MCP commands (`api/mcp`), while the listen port — a persisted
 * setting — is committed through the shared settings controller on blur (never
 * per keystroke; docs/ui-authoring.md). The one-time plaintext token lives ONLY
 * in transient component state (`revealedToken`); leaving the section unmounts
 * the component and drops it (ADR 0078 G-3 — never persisted beyond the reveal).
 */
export function McpSettings({
  settings,
  onMcpPortChange,
  onMcpWritesEnabledChange,
  onKpiAcquisitionEnabledChange,
}: McpSettingsProps) {
  const { text } = useLocale();
  const configuredPort = settings?.mcp.port ?? DEFAULT_PORT;
  const writesEnabled = settings?.mcp.writesEnabled ?? false;
  const kpiAcquisitionEnabled = settings?.mcp.kpiAcquisitionEnabled ?? false;

  const [tokenStatus, setTokenStatus] = useState<CredentialStatus | null>(null);
  const [serverStatus, setServerStatus] = useState<McpStatus | null>(null);
  const [revealedToken, setRevealedToken] = useState<string | null>(null);
  const [kpiTokenStatus, setKpiTokenStatus] = useState<CredentialStatus | null>(
    null,
  );
  const [revealedKpiToken, setRevealedKpiToken] = useState<string | null>(null);
  const [kpiCopied, setKpiCopied] = useState(false);
  const [confirmingKpiRevoke, setConfirmingKpiRevoke] = useState(false);
  const [portDraft, setPortDraft] = useState(String(configuredPort));
  const [portHint, setPortHint] = useState(false);
  const [copied, setCopied] = useState(false);
  const [confirmingRevoke, setConfirmingRevoke] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load the current tokens + server status once on mount.
  useEffect(() => {
    let active = true;
    Promise.all([mcpTokenStatus(), kpiAcquisitionTokenStatus(), fetchMcpStatus()])
      .then(([token, kpiToken, status]) => {
        if (!active) return;
        setTokenStatus(token);
        setKpiTokenStatus(kpiToken);
        setServerStatus(status);
      })
      .catch((reason) => {
        if (active) setError(String(reason));
      });
    return () => {
      active = false;
    };
  }, []);

  // Every rotate/revoke restarts the listener (ADR 0099 dec. 2), so the token
  // response alone is stale server-wise — always refetch the live status
  // (revoking the primary token STOPS the server; the pill must say so).
  function refreshServerStatus() {
    fetchMcpStatus()
      .then((status) => setServerStatus(status))
      .catch((reason) => setError(String(reason)));
  }

  // Re-seed the port draft when the persisted value changes (e.g. after a
  // commit). During typing the setting is unchanged, so this does not clobber.
  useEffect(() => {
    setPortDraft(String(configuredPort));
  }, [configuredPort]);

  const running = serverStatus?.running ?? false;
  const configured = tokenStatus?.configured ?? false;

  // Acquisition credential state, composed EXPLICITLY from its three sources
  // (ADR 0099 dec. 2): credential status (incl. its error — an unreadable
  // keychain is NOT "no token yet"), the gate setting, and the listener's
  // effective availability (false on digest collision with the primary).
  const kpiConfigured = kpiTokenStatus?.configured ?? false;
  const kpiCredentialError = kpiTokenStatus?.error ?? null;
  const kpiEffective = serverStatus?.kpiAcquisitionConfigured ?? false;
  const kpiStateNote = kpiCredentialError
    ? `${text("Credential unavailable")}: ${kpiCredentialError}`
    : !kpiConfigured
      ? text(
          "No acquisition token yet — generate one to hand an ingest assistant its own limited key.",
        )
      : !kpiAcquisitionEnabled
        ? text("Configured, disabled — turn the switch on to activate it.")
        : running && kpiEffective
          ? text("Acquisition access is active.")
          : running
            ? text(
                "Configured but unavailable — the two tokens collide; rotate one of them.",
              )
            : text("Configured — it activates when the server starts.");

  function toggleEnabled(next: boolean) {
    setBusy(true);
    setError(null);
    setMcpEnabled(next)
      .then((status) => setServerStatus(status))
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  }

  function commitPort() {
    const parsed = Number.parseInt(portDraft, 10);
    const outOfRange =
      !Number.isInteger(parsed) || parsed < MIN_PORT || parsed > MAX_PORT;
    setPortHint(outOfRange);
    const clamped = clampPort(parsed);
    setPortDraft(String(clamped));
    if (clamped !== configuredPort) onMcpPortChange(clamped);
  }

  function generate() {
    setBusy(true);
    setError(null);
    setCopied(false);
    regenerateMcpToken()
      .then((generated) => {
        setRevealedToken(generated.token);
        setTokenStatus(generated.status);
        refreshServerStatus();
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  }

  function revoke() {
    setBusy(true);
    setError(null);
    setConfirmingRevoke(false);
    revokeMcpToken()
      .then((status) => {
        setTokenStatus(status);
        setRevealedToken(null);
        refreshServerStatus();
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  }

  function generateKpi() {
    setBusy(true);
    setError(null);
    setKpiCopied(false);
    regenerateKpiAcquisitionToken()
      .then((generated) => {
        setRevealedKpiToken(generated.token);
        setKpiTokenStatus(generated.status);
        refreshServerStatus();
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  }

  function revokeKpi() {
    setBusy(true);
    setError(null);
    setConfirmingKpiRevoke(false);
    revokeKpiAcquisitionToken()
      .then((status) => {
        setKpiTokenStatus(status);
        setRevealedKpiToken(null);
        refreshServerStatus();
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  }

  function copy(value: string) {
    setCopied(true);
    void navigator.clipboard?.writeText(value).catch(() => setCopied(false));
  }

  function copyKpi(value: string) {
    setKpiCopied(true);
    void navigator.clipboard?.writeText(value).catch(() => setKpiCopied(false));
  }

  // Connection snippets (example — verified plausible, marked as such). The port
  // interpolates live; the token stays a `<token>` placeholder unless it was
  // just generated (the one-time reveal), so a copied snippet never embeds a
  // stale token the app cannot show again.
  const tokenPlaceholder = revealedToken ?? "<token>";
  const httpSnippet = `claude mcp add --transport http brawler http://127.0.0.1:${configuredPort}/mcp --header "Authorization: Bearer ${tokenPlaceholder}"`;
  const stdioSnippet = `BRAWLER_MCP_TOKEN=${tokenPlaceholder} brawler-mcp-stdio`;

  return (
    <section className="settings-group" aria-labelledby="settings-mcp-title">
      <h2 id="settings-mcp-title">{text("MCP server")}</h2>
      <p className="settings-note">
        {text(
          "Let an AI assistant read your research through the Model Context Protocol (MCP). The server runs on your machine only and stays off until you turn it on.",
        )}
      </p>

      {/* Enable toggle + live status pill */}
      <div className="mcp-enable-row">
        <label className="settings-toggle">
          <input
            aria-label={text("Enable the server")}
            checked={running}
            disabled={busy}
            onChange={(event) => toggleEnabled(event.target.checked)}
            role="switch"
            type="checkbox"
          />
          <span aria-hidden="true" className="settings-toggle-track">
            <span />
          </span>
          <span className="settings-toggle-label">
            {text("Enable the server")}
          </span>
        </label>
        <span className="mcp-status" aria-label={text("Server status")}>
          <StatusPill tone={running ? "ok" : "neutral"}>
            {running ? text("Active") : text("Stopped")}
          </StatusPill>
        </span>
      </div>
      {serverStatus?.error ? <ErrorText>{serverStatus.error}</ErrorText> : null}

      {/* Write tier — off by default; the only path to enable agent writes.
          `update_settings` is MCP-excluded, so an assistant can never flip this
          itself (ADR 0088 M3). */}
      <div className="mcp-enable-row">
        <label className="settings-toggle">
          <input
            aria-label={text("Allow write tools")}
            checked={writesEnabled}
            onChange={(event) => onMcpWritesEnabledChange(event.target.checked)}
            role="switch"
            type="checkbox"
          />
          <span aria-hidden="true" className="settings-toggle-track">
            <span />
          </span>
          <span className="settings-toggle-label">
            {text("Allow write tools")}
          </span>
        </label>
        <span className="mcp-status">
          <StatusPill tone={writesEnabled ? "warn" : "neutral"}>
            {writesEnabled ? text("Writes on") : text("Read-only")}
          </StatusPill>
        </span>
      </div>
      <Hint>
        {text(
          "Write tools require citations; deletes and settings stay UI-only. Off by default — an assistant can never turn this on itself.",
        )}
      </Hint>

      {/* Acquisition scope — a second, limited credential (ADR 0099 dec. 2).
          Off = its token is rejected outright (reads included). */}
      <div className="mcp-enable-row">
        <label className="settings-toggle">
          <input
            aria-label={text("Allow acquisition access")}
            checked={kpiAcquisitionEnabled}
            onChange={(event) =>
              onKpiAcquisitionEnabledChange(event.target.checked)
            }
            role="switch"
            type="checkbox"
          />
          <span aria-hidden="true" className="settings-toggle-track">
            <span />
          </span>
          <span className="settings-toggle-label">
            {text("Allow acquisition access")}
          </span>
        </label>
        <span className="mcp-status">
          <StatusPill tone={kpiAcquisitionEnabled ? "warn" : "neutral"}>
            {kpiAcquisitionEnabled ? text("Enabled") : text("Off")}
          </StatusPill>
        </span>
      </div>
      <Hint>
        {text(
          "A separate key for a report-ingest assistant: it sees only the KPI-ingest workflow, never notes, settings or deletes. Off = the key stops working entirely. The ingest tools themselves arrive in a later update.",
        )}
      </Hint>

      {/* Listen port — committed on blur, clamped to [1024, 65535] */}
      <div className="mcp-port-field">
        <TextField
          aria-label={text("Listen port")}
          inputMode="numeric"
          label={text("Listen port")}
          max={MAX_PORT}
          min={MIN_PORT}
          onBlur={commitPort}
          onChange={(event) => setPortDraft(event.target.value)}
          type="number"
          value={portDraft}
        />
        <Button
          className="settings-link-button"
          disabled={configuredPort === DEFAULT_PORT}
          onClick={() => onMcpPortChange(DEFAULT_PORT)}
          variant="ghost"
        >
          {text("Reset to default port")}
        </Button>
      </div>
      {portHint ? (
        <Hint>
          {text("Port must be a whole number between 1024 and 65535.")}
        </Hint>
      ) : null}
      <Hint>
        {text("The new port applies the next time the server starts.")}
      </Hint>

      {/* Access token */}
      <h3>{text("Access token")}</h3>
      <p className="settings-note">
        {configured
          ? text("A token is configured.")
          : text("No token yet — generate one so an assistant can connect.")}
      </p>

      {revealedToken ? (
        <div className="mcp-token-reveal">
          <TextField
            aria-label={text("Access token")}
            label={text("Access token")}
            readOnly
            value={revealedToken}
          />
          <Button
            aria-label={text("Copy token")}
            onClick={() => copy(revealedToken)}
            variant="action"
          >
            <Copy size={14} />
            {copied ? text("Copied") : text("Copy token")}
          </Button>
          <Hint>
            {text(
              "This token is shown once. Copy it now — after you leave this screen it can only be revoked and regenerated, never shown again.",
            )}
          </Hint>
        </div>
      ) : null}

      <ActionRow className="mcp-token-actions">
        <Button disabled={busy} onClick={generate} variant="action">
          <KeyRound size={14} />
          {configured ? text("Regenerate token") : text("Generate token")}
        </Button>
        {configured && !confirmingRevoke ? (
          <Button
            disabled={busy}
            onClick={() => setConfirmingRevoke(true)}
            variant="ghost"
          >
            <Trash2 size={14} />
            {text("Revoke token")}
          </Button>
        ) : null}
      </ActionRow>
      {confirmingRevoke ? (
        <InlineConfirm
          confirmLabel={text("Revoke token")}
          disabled={busy}
          onCancel={() => setConfirmingRevoke(false)}
          onConfirm={revoke}
        >
          {text(
            "Revoke this token? Any assistant using it stops working until you generate a new one.",
          )}
        </InlineConfirm>
      ) : null}

      {tokenStatus?.devFallbackAvailable ? (
        <p className="settings-note">
          {text(
            "Development fallback is active through environment configuration.",
          )}
        </p>
      ) : null}

      {/* Acquisition token (ADR 0099 dec. 2) */}
      <h3>{text("Acquisition token")}</h3>
      <p className="settings-note">{kpiStateNote}</p>

      {revealedKpiToken ? (
        <div className="mcp-token-reveal">
          <TextField
            aria-label={text("Acquisition token")}
            label={text("Acquisition token")}
            readOnly
            value={revealedKpiToken}
          />
          <Button
            aria-label={text("Copy acquisition token")}
            onClick={() => copyKpi(revealedKpiToken)}
            variant="action"
          >
            <Copy size={14} />
            {kpiCopied ? text("Copied") : text("Copy token")}
          </Button>
          <Hint>
            {text(
              "This token is shown once. Copy it now — after you leave this screen it can only be revoked and regenerated, never shown again.",
            )}
          </Hint>
        </div>
      ) : null}

      <ActionRow className="mcp-token-actions">
        <Button disabled={busy} onClick={generateKpi} variant="action">
          <KeyRound size={14} />
          {kpiConfigured
            ? text("Regenerate acquisition token")
            : text("Generate acquisition token")}
        </Button>
        {kpiConfigured && !confirmingKpiRevoke ? (
          <Button
            disabled={busy}
            onClick={() => setConfirmingKpiRevoke(true)}
            variant="ghost"
          >
            <Trash2 size={14} />
            {text("Revoke acquisition token")}
          </Button>
        ) : null}
      </ActionRow>
      {confirmingKpiRevoke ? (
        <InlineConfirm
          confirmLabel={text("Revoke acquisition token")}
          disabled={busy}
          onCancel={() => setConfirmingKpiRevoke(false)}
          onConfirm={revokeKpi}
        >
          {text(
            "Revoke this token? Any ingest assistant using it stops working until you generate a new one.",
          )}
        </InlineConfirm>
      ) : null}

      {/* Connection snippets */}
      <h3>{text("Connect your assistant")}</h3>
      <Hint>
        {text(
          "Add the server in Claude running on the same machine as this app (Windows). From WSL it works only with mirrored networking.",
        )}
      </Hint>
      <Hint>
        {text(
          "Example — check the exact command for your assistant's version.",
        )}
      </Hint>
      {!revealedToken ? (
        <Hint>{text("Paste the token where <token> appears.")}</Hint>
      ) : null}

      <div className="mcp-snippet-block">
        <div className="mcp-snippet-head">
          <span className="mcp-snippet-label">
            {text("Claude Code (HTTP)")}
          </span>
          <Button
            aria-label={`${text("Copy")} — ${text("Claude Code (HTTP)")}`}
            className="compact-button"
            onClick={() => copy(httpSnippet)}
            variant="ghost"
          >
            <Copy size={13} />
            {text("Copy")}
          </Button>
        </div>
        <pre className="mcp-snippet" data-hscroll>
          <code>{httpSnippet}</code>
        </pre>
      </div>

      <div className="mcp-snippet-block">
        <div className="mcp-snippet-head">
          <span className="mcp-snippet-label">{text("Stdio adapter")}</span>
          <Button
            aria-label={`${text("Copy")} — ${text("Stdio adapter")}`}
            className="compact-button"
            onClick={() => copy(stdioSnippet)}
            variant="ghost"
          >
            <Copy size={13} />
            {text("Copy")}
          </Button>
        </div>
        <pre className="mcp-snippet" data-hscroll>
          <code>{stdioSnippet}</code>
        </pre>
      </div>

      {error ? (
        <ErrorText>
          {text("Server command failed")}: {error}
        </ErrorText>
      ) : null}
    </section>
  );
}
