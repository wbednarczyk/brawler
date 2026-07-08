import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import * as backupsApi from "../../api/backups";
import { BackupsSection } from "./BackupsSection";

vi.mock("../../api/backups");

const sampleStatus: backupsApi.BackupStatus = {
  lastBackupAt: "2026-06-14T10:00:00.000Z",
  backupCount: 1,
  backups: [
    {
      fileName: "backup-123.sqlite3",
      createdAt: "2026-06-14T10:00:00.000Z",
      kind: "rotating",
      sizeBytes: 2048,
    },
  ],
};

beforeEach(() => {
  vi.mocked(backupsApi.backupStatus).mockResolvedValue(sampleStatus);
  vi.mocked(backupsApi.createBackup).mockResolvedValue(sampleStatus);
  vi.mocked(backupsApi.restoreBackup).mockResolvedValue(undefined);
});

describe("Diagnostics backups section", () => {
  it("lists backups, creates a backup, and stages a restore", async () => {
    const user = userEvent.setup();

    render(<BackupsSection />);

    await user.click(screen.getByRole("button", { name: /Backups/ }));

    expect(await screen.findByText("backup-123.sqlite3")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Create backup" }));
    expect(backupsApi.createBackup).toHaveBeenCalled();
    expect(await screen.findByText("Backup created.")).toBeInTheDocument();

    // Irreversible/multi-consequence (ADR 0076 D5): confirm in place, no native
    // dialog. Restore fires only after the InlineConfirm is confirmed.
    await user.click(screen.getByRole("button", { name: "Restore" }));
    expect(backupsApi.restoreBackup).not.toHaveBeenCalled();
    expect(
      screen.getByText("Restore this backup? It is applied when the app restarts and replaces current data."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Restore" }));
    expect(backupsApi.restoreBackup).toHaveBeenCalledWith("backup-123.sqlite3");
    await waitFor(() => {
      expect(screen.getByText("Restore staged. Restart the app to apply it.")).toBeInTheDocument();
    });
  });
});
