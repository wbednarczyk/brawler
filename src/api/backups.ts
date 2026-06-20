import { callCommand } from "./tauri";
import type { BackupStatus } from "./generated/BackupStatus";

// BackupEntry/BackupStatus GENERATED from src-tauri/src/storage/backup.rs via
// ts-rs (ADR 0048); BackupEntry.kind carries the inline "rotating" | "snapshot"
// union. BackupKind stays a hand-written alias for callers that name it.
export type BackupKind = "rotating" | "snapshot";
export type { BackupEntry } from "./generated/BackupEntry";
export type { BackupStatus } from "./generated/BackupStatus";

export function backupStatus() {
  return callCommand<BackupStatus>("backup_status");
}

export function createBackup() {
  return callCommand<BackupStatus>("create_backup");
}

export function restoreBackup(fileName: string) {
  return callCommand<void>("restore_backup", { fileName });
}
