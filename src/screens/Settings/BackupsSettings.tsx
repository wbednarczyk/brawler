import { DatabaseBackup, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import * as backupsApi from "../../api/backups";
import type { BackupEntry, BackupStatus } from "../../api/backups";
import { ActionButton, ActionRow, EmptyState, ErrorText, Figure, InfoGrid, InlineConfirm, ListRow } from "../../ui";
import { useLocale, type LocaleCode } from "../../shared/locale";
import { formatListTimestamp } from "../../shared/format/datetime";

export function BackupsSettings() {
  const { locale, text } = useLocale();
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

  const hasBackups = (status?.backups.length ?? 0) > 0;
  // ADR 0104 dec. 4: exactly one primary Create-backup action lives in the
  // section at a time — the empty-state invitation while there is nothing to
  // show, the toolbar once at least one backup exists — never both.
  const createButton = (
    <ActionButton disabled={inFlight} onClick={createBackup} verb="create">
      <DatabaseBackup size={15} />
      {inFlight ? text("Working") : text("Create backup")}
    </ActionButton>
  );

  return (
    <section className="settings-group" aria-labelledby="settings-backups-title">
      <h2 id="settings-backups-title">{text("Backups")}</h2>
      <p className="settings-note">
        {text("Local copies of your data. A restore is applied when the app restarts.")}
      </p>
      <InfoGrid
        className="settings-grid"
        items={[
          {
            label: text("Last backup"),
            value: (
              <span className="num-tabular">
                {formatListTimestamp(status?.lastBackupAt ?? null, locale, text("None yet"))}
              </span>
            ),
          },
          { label: text("Backups kept"), value: <Figure value={status?.backupCount ?? 0} /> },
        ]}
      />
      <ActionRow>
        {hasBackups ? createButton : null}
        <ActionButton disabled={inFlight} kind="control" onClick={refreshStatus}>
          <RefreshCw size={15} />
          {text("Refresh")}
        </ActionButton>
      </ActionRow>
      {notice ? <p className="settings-note">{notice}</p> : null}
      {error ? <ErrorText>{error}</ErrorText> : null}
      {hasBackups ? (
        <ul aria-label={text("Backups")} className="ui-list-rows">
          {status!.backups.map((backup) => (
            <ListRow
              key={backup.fileName}
              title={backupTitle(backup, text, locale)}
              meta={<span className="settings-backup-filename">{backup.fileName}</span>}
              trailing={
                confirmRestoreFile === backup.fileName ? (
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
                  <ActionButton
                    disabled={inFlight}
                    kind="control"
                    onClick={() => setConfirmRestoreFile(backup.fileName)}
                  >
                    {text("Restore")}
                  </ActionButton>
                )
              }
            />
          ))}
        </ul>
      ) : (
        <EmptyState
          action={createButton}
          kind="invitation"
          source={text("Stored on this computer.")}
          title={text("Local copies of your data.")}
        />
      )}
    </section>
  );
}

// Human title first (ADR 0104 dec. 6): "Backup · <date>" / "Copy before
// upgrade · <date>" for a pre-migration snapshot; the file name stays
// secondary metadata (the ListRow `meta` slot). `formatListTimestamp` is the
// shared list-row date formatter (ADR 0076 D4) — never a raw `toLocaleString`.
function backupTitle(backup: BackupEntry, text: (value: string) => string, locale: LocaleCode) {
  const label = backup.kind === "snapshot" ? text("Copy before upgrade") : text("Backup");
  if (!backup.createdAt) {
    return label;
  }
  return (
    <>
      {label} · <span className="num-tabular">{formatListTimestamp(backup.createdAt, locale)}</span>
    </>
  );
}
