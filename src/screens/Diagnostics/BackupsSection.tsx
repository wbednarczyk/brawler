import { ChevronDown, DatabaseBackup, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import * as backupsApi from "../../api/backups";
import type { BackupStatus } from "../../api/backups";
import { ActionRow, Button, EmptyState, ErrorText, InfoGrid, InlineConfirm } from "../../ui";
import { useLocale } from "../../shared/locale";

export function BackupsSection() {
  const { text } = useLocale();
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [inFlight, setInFlight] = useState(false);
  // Irreversible/multi-consequence (ADR 0076 D5): restore replaces all data on the
  // next launch, so it confirms in place via InlineConfirm rather than a native dialog.
  const [confirmRestoreFile, setConfirmRestoreFile] = useState<string | null>(null);

  function refreshStatus() {
    backupsApi
      .backupStatus()
      .then((next) => {
        setStatus(next);
        setError(null);
      })
      .catch((cause) => setError(String(cause)));
  }

  useEffect(() => {
    refreshStatus();
  }, []);

  function createBackup() {
    setInFlight(true);
    setNotice(null);
    backupsApi
      .createBackup()
      .then((next) => {
        setStatus(next);
        setError(null);
        setNotice(text("Backup created."));
      })
      .catch((cause) => setError(String(cause)))
      .finally(() => setInFlight(false));
  }

  function restore(fileName: string) {
    setConfirmRestoreFile(null);
    setInFlight(true);
    setNotice(null);
    backupsApi
      .restoreBackup(fileName)
      .then(() => {
        setError(null);
        setNotice(text("Restore staged. Restart the app to apply it."));
      })
      .catch((cause) => setError(String(cause)))
      .finally(() => setInFlight(false));
  }

  return (
    <section className="diagnostics-section" aria-labelledby="diagnostics-backups-title">
      <button
        aria-expanded={open}
        className="diagnostics-section-header"
        onClick={() => setOpen((value) => !value)}
        type="button"
      >
        <span>
          <h2 id="diagnostics-backups-title">{text("Backups")}</h2>
          <small>{text("Local data backups. Restore is applied on the next app launch.")}</small>
        </span>
        <span className="diagnostics-section-meta">
          {status?.backupCount ?? 0}
          <ChevronDown
            className={open ? "section-chevron section-chevron-open" : "section-chevron"}
            size={16}
          />
        </span>
      </button>
      {open ? (
        <div className="diagnostics-section-body">
          <div className="diagnostics-section-toolbar">
            <InfoGrid
              className="settings-grid diagnostics-log-status"
              items={[
                { label: text("Last backup"), value: status?.lastBackupAt ?? text("None yet") },
                { label: text("Backups kept"), value: status?.backupCount ?? 0 },
              ]}
            />
            <ActionRow className="diagnostics-actions">
              <Button className="compact-button" disabled={inFlight} onClick={createBackup}>
                <DatabaseBackup size={15} />
                {inFlight ? text("Working") : text("Create backup")}
              </Button>
              <Button className="compact-button" disabled={inFlight} onClick={refreshStatus}>
                <RefreshCw size={15} />
                {text("Refresh")}
              </Button>
            </ActionRow>
          </div>
          {notice ? <p className="settings-note">{notice}</p> : null}
          {error ? <ErrorText>{error}</ErrorText> : null}
          <div className="diagnostics-backups-list" aria-label={text("Backups")}>
            {status && status.backups.length > 0 ? (
              status.backups.map((backup) => (
                <article className="diagnostics-backup-row" key={backup.fileName}>
                  <div>
                    <h3>{backup.fileName}</h3>
                    <small>
                      {backup.kind === "snapshot"
                        ? text("Pre-migration snapshot")
                        : text("Automatic backup")}
                      {backup.createdAt ? ` · ${backup.createdAt}` : ""}
                    </small>
                  </div>
                  {confirmRestoreFile === backup.fileName ? (
                    <InlineConfirm
                      cancelLabel={text("Cancel")}
                      confirmLabel={text("Restore")}
                      disabled={inFlight}
                      onCancel={() => setConfirmRestoreFile(null)}
                      onConfirm={() => restore(backup.fileName)}
                    >
                      {text("Restore this backup? It is applied when the app restarts and replaces current data.")}
                    </InlineConfirm>
                  ) : (
                    <Button
                      className="compact-button"
                      disabled={inFlight}
                      onClick={() => setConfirmRestoreFile(backup.fileName)}
                    >
                      {text("Restore")}
                    </Button>
                  )}
                </article>
              ))
            ) : (
              <EmptyState>{text("No backups yet.")}</EmptyState>
            )}
          </div>
        </div>
      ) : null}
    </section>
  );
}
