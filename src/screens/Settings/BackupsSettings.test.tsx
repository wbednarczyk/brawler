import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import * as backupsApi from "../../api/backups";
import { BackupsSettings } from "./BackupsSettings";
import { LocaleContext, makeTextTranslator, makeTranslator } from "../../shared/locale";
import { formatListTimestamp } from "../../shared/format/datetime";

vi.mock("../../api/backups");

// A different-year, non-midnight timestamp so `formatListTimestamp` always
// takes its stable "month day, year" branch regardless of the real run date
// (sol R2 correction: raw `createdAt`/`toLocaleString` are banned — the row
// title and the "Last backup" figure both go through the shared list-context
// formatter, ADR 0076 D4).
const ROTATING_CREATED_AT = "2020-03-15T10:00:00.000Z";
const SNAPSHOT_CREATED_AT = "2020-06-08T09:30:00.000Z";
// Distinct from both row timestamps above so the "Last backup" figure and
// the row titles never render identical text (that would make the
// text-content assertions ambiguous, not proof of a distinct render site).
const LAST_BACKUP_AT = "2019-11-01T08:00:00.000Z";

const sampleStatus: backupsApi.BackupStatus = {
  lastBackupAt: LAST_BACKUP_AT,
  backupCount: 3,
  backups: [
    {
      fileName: "backup-123.sqlite3",
      createdAt: ROTATING_CREATED_AT,
      kind: "rotating",
      sizeBytes: 2048,
    },
    {
      fileName: "brawler-2026-06-08.snapshot.sqlite",
      createdAt: SNAPSHOT_CREATED_AT,
      kind: "snapshot",
      sizeBytes: 4096,
    },
    {
      // A backup with no known creation time — the title has no date suffix.
      fileName: "backup-legacy.sqlite3",
      createdAt: null,
      kind: "rotating",
      sizeBytes: 1024,
    },
  ],
};

const emptyStatus: backupsApi.BackupStatus = { lastBackupAt: null, backupCount: 0, backups: [] };

function rowFor(fileName: string) {
  return screen.getByText(fileName).closest(".ui-list-row") as HTMLElement;
}

// The row title splits "<label> · <date>" across a text node and a nested
// `.num-tabular` span (the date goes through `Figure`/`formatListTimestamp`
// styling) — RTL's default text matcher only tests a node's own direct text,
// so match by full `textContent` instead.
function rowTitle(row: HTMLElement, expected: string) {
  return within(row).getByText(
    (_content, element) => element?.classList.contains("ui-list-row-title") === true &&
      element.textContent === expected,
  );
}

function renderPl(children: React.ReactElement) {
  return render(
    <LocaleContext.Provider value={{ locale: "pl", t: makeTranslator("pl"), text: makeTextTranslator("pl") }}>
      {children}
    </LocaleContext.Provider>,
  );
}

beforeEach(() => {
  vi.mocked(backupsApi.backupStatus).mockResolvedValue(sampleStatus);
  vi.mocked(backupsApi.createBackup).mockResolvedValue(sampleStatus);
  vi.mocked(backupsApi.restoreBackup).mockResolvedValue(undefined);
});

describe("Settings › Data storage — Backups (#451)", () => {
  it("is a plain, always-visible settings group — no developer gate, no disclosure toggle", async () => {
    render(<BackupsSettings />);

    // Product feature: reachable at rest, not behind an accordion. #451.
    expect(await screen.findByRole("heading", { name: "Backups" })).toBeInTheDocument();
    expect(screen.getByText("backup-123.sqlite3")).toBeInTheDocument();
  });

  it("leads each row with a human title through the shared list formatter, EN", async () => {
    render(<BackupsSettings />);
    await screen.findByText("backup-123.sqlite3");

    const expectedRotating = formatListTimestamp(ROTATING_CREATED_AT, "en");
    const expectedSnapshot = formatListTimestamp(SNAPSHOT_CREATED_AT, "en");

    // Human title first (ADR 0104 dec. 6); the file name stays secondary
    // metadata. "Pre-migration snapshot" is retired implementation
    // vocabulary — retiredKeys.test.ts pins it shut.
    expect(rowTitle(rowFor("backup-123.sqlite3"), `Backup · ${expectedRotating}`)).toBeInTheDocument();
    expect(
      rowTitle(rowFor("brawler-2026-06-08.snapshot.sqlite"), `Copy before upgrade · ${expectedSnapshot}`),
    ).toBeInTheDocument();
    // No known creation time — no dangling "· " suffix.
    expect(rowTitle(rowFor("backup-legacy.sqlite3"), "Backup")).toBeInTheDocument();
  });

  it("leads each row with a human title through the shared list formatter, PL", async () => {
    renderPl(<BackupsSettings />);
    await screen.findByText("backup-123.sqlite3");

    const expectedRotating = formatListTimestamp(ROTATING_CREATED_AT, "pl");

    expect(rowTitle(rowFor("backup-123.sqlite3"), `Kopia · ${expectedRotating}`)).toBeInTheDocument();
    expect(screen.getByText("Kopie zapasowe")).toBeInTheDocument();
  });

  it("shows the last-backup figure through the shared list formatter, never toLocaleString", async () => {
    render(<BackupsSettings />);
    await screen.findByText("backup-123.sqlite3");

    expect(screen.getByText(formatListTimestamp(LAST_BACKUP_AT, "en"))).toBeInTheDocument();
  });

  it("renders 'None yet' through the same formatter call when no backup has ever run", async () => {
    vi.mocked(backupsApi.backupStatus).mockResolvedValue({ ...sampleStatus, lastBackupAt: null });
    render(<BackupsSettings />);
    await screen.findByText("backup-123.sqlite3");

    expect(screen.getByText("None yet")).toBeInTheDocument();
  });

  it("lists backups, creates a backup, and stages a restore", async () => {
    const user = userEvent.setup();

    render(<BackupsSettings />);

    expect(await screen.findByText("backup-123.sqlite3")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Create backup" }));
    expect(backupsApi.createBackup).toHaveBeenCalled();
    expect(await screen.findByText("Backup created.")).toBeInTheDocument();

    const rotatingRow = rowFor("backup-123.sqlite3");

    // Irreversible/multi-consequence (ADR 0076 D5): confirm in place, no native
    // dialog. Restore fires only after the InlineConfirm is confirmed.
    await user.click(within(rotatingRow).getByRole("button", { name: "Restore" }));
    expect(backupsApi.restoreBackup).not.toHaveBeenCalled();
    expect(
      screen.getByText("Restore this backup? It is applied when the app restarts and replaces current data."),
    ).toBeInTheDocument();
    await user.click(within(rotatingRow).getByRole("button", { name: "Restore" }));
    expect(backupsApi.restoreBackup).toHaveBeenCalledWith("backup-123.sqlite3");
    await waitFor(() => {
      expect(screen.getByText("Restore staged. Restart the app to apply it.")).toBeInTheDocument();
    });
  });

  it("renders a three-beat invitation with the sole Create-backup action when there are no backups (ADR 0104 dec. 4)", async () => {
    vi.mocked(backupsApi.backupStatus).mockResolvedValue(emptyStatus);
    const user = userEvent.setup();

    render(<BackupsSettings />);

    const invitation = await screen.findByText("Local copies of your data.");
    expect(screen.getByText("Stored on this computer.")).toBeInTheDocument();
    // Exactly one Create backup control exists — inside the invitation, not
    // also duplicated in the toolbar.
    expect(screen.getAllByRole("button", { name: "Create backup" })).toHaveLength(1);
    expect(invitation.closest('[data-empty-kind="invitation"]')).not.toBeNull();

    vi.mocked(backupsApi.createBackup).mockResolvedValue(sampleStatus);
    await user.click(screen.getByRole("button", { name: "Create backup" }));
    expect(backupsApi.createBackup).toHaveBeenCalled();
    // Backups exist now — the invitation is gone and the row list takes over.
    expect(await screen.findByText("backup-123.sqlite3")).toBeInTheDocument();
    expect(screen.queryByText("Local copies of your data.")).not.toBeInTheDocument();
  });

  it("marks Create backup as the section's one primary action", async () => {
    renderPl(<BackupsSettings />);
    const create = await screen.findByRole("button", { name: "Utwórz kopię" });
    expect(create).toHaveAttribute("data-ux-primary-action", "true");
    expect(document.querySelectorAll('[data-ux-primary-action="true"]')).toHaveLength(1);
  });

  it("shows neither the list nor the invitation while the status is still loading", () => {
    vi.mocked(backupsApi.backupStatus).mockReturnValue(new Promise(() => {}));
    renderPl(<BackupsSettings />);
    expect(screen.queryByRole("list", { name: "Kopie zapasowe" })).toBeNull();
    expect(screen.queryByText("Lokalne kopie Twoich danych.")).toBeNull();
  });

  it("keeps the last-known list when a refresh fails instead of showing an empty state", async () => {
    const user = userEvent.setup();
    renderPl(<BackupsSettings />);
    await screen.findByRole("list", { name: "Kopie zapasowe" });
    vi.mocked(backupsApi.backupStatus).mockRejectedValueOnce(new Error("disk unreadable"));
    await user.click(screen.getByRole("button", { name: "Odśwież" }));
    await screen.findByText("Error: disk unreadable");
    expect(screen.getByRole("list", { name: "Kopie zapasowe" })).toBeInTheDocument();
    expect(screen.queryByText("Lokalne kopie Twoich danych.")).toBeNull();
  });
});
