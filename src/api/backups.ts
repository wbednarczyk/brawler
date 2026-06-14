import { callCommand } from "./tauri";

export type BackupKind = "rotating" | "snapshot";

export type BackupEntry = {
  fileName: string;
  createdAt: string | null;
  kind: BackupKind;
  sizeBytes: number;
};

export type BackupStatus = {
  lastBackupAt: string | null;
  backupCount: number;
  backups: BackupEntry[];
};

export function backupStatus() {
  return callCommand<BackupStatus>("backup_status");
}

export function createBackup() {
  return callCommand<BackupStatus>("create_backup");
}

export function restoreBackup(fileName: string) {
  return callCommand<void>("restore_backup", { fileName });
}
