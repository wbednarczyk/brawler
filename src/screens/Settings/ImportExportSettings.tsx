import { useRef, useState, type MutableRefObject } from "react";
import { downloadDir, join } from "@tauri-apps/api/path";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import {
  applyResearchImport,
  applySettingsImport,
  exportResearchData,
  exportSettingsData,
  previewResearchImport,
  previewSettingsImport,
  type ImportApplyResult,
  type ImportApplySummary,
  type ImportPreview,
} from "../../api/importExport";
import { ActionRow, Button, ErrorText, InfoGrid, useToast } from "../../ui";
import { useLocale } from "../../shared/locale";

type ImportKind = "research" | "settings";

type ImportState = {
  contents: string;
  preview: ImportPreview | null;
  result: ImportApplyResult | null;
  error: string | null;
  inFlight: boolean;
};

type ImportExportSettingsProps = {
  onImportApplied: () => void;
};

const emptyImportState: ImportState = {
  contents: "",
  preview: null,
  result: null,
  error: null,
  inFlight: false,
};

export function ImportExportSettings({ onImportApplied }: ImportExportSettingsProps) {
  const { text } = useLocale();
  const toast = useToast();
  const [researchState, setResearchState] = useState<ImportState>(emptyImportState);
  const [settingsState, setSettingsState] = useState<ImportState>(emptyImportState);
  const [exportError, setExportError] = useState<string | null>(null);
  const researchInputRef = useRef<HTMLInputElement | null>(null);
  const settingsInputRef = useRef<HTMLInputElement | null>(null);

  const stateFor = (kind: ImportKind) => kind === "research" ? researchState : settingsState;
  const setStateFor = (kind: ImportKind, state: ImportState) => {
    if (kind === "research") {
      setResearchState(state);
    } else {
      setSettingsState(state);
    }
  };

  const exportData = async (kind: ImportKind) => {
    setExportError(null);
    try {
      const payload = kind === "research" ? await exportResearchData() : await exportSettingsData();
      await saveExportFile(
        payload.fileName,
        payload.contents,
        kind === "research"
          ? { name: "Research data", extensions: ["json"], defaultExtension: "json" }
          : { name: "Settings", extensions: ["yaml", "yml"], defaultExtension: "yaml" },
      );
    } catch (error) {
      setExportError(String(error));
    }
  };

  const previewFile = async (kind: ImportKind, file: File | null) => {
    if (!file) {
      return;
    }
    const baseState = stateFor(kind);
    setStateFor(kind, { ...baseState, inFlight: true, error: null, result: null });

    try {
      const contents = await readFileText(file);
      const preview = kind === "research"
        ? await previewResearchImport(contents)
        : await previewSettingsImport(contents);
      setStateFor(kind, { contents, preview, result: null, error: null, inFlight: false });
    } catch (error) {
      setStateFor(kind, { contents: "", preview: null, result: null, error: String(error), inFlight: false });
    }
  };

  const applyImport = async (kind: ImportKind) => {
    const current = stateFor(kind);
    if (!current.preview?.valid || !current.contents) {
      return;
    }

    setStateFor(kind, { ...current, inFlight: true, error: null });
    try {
      const result = kind === "research"
        ? await applyResearchImport(current.contents)
        : await applySettingsImport(current.contents);
      setStateFor(kind, { ...current, result, error: null, inFlight: false });
      // v0.54 T6: transient success confirmation on the shared Toast surface;
      // the detailed result summary grid below stays as the persistent record.
      toast.show({ message: text("Import applied"), tone: "positive" });
      onImportApplied();
    } catch (error) {
      setStateFor(kind, { ...current, result: null, error: String(error), inFlight: false });
    }
  };

  return (
    <section className="settings-group" aria-labelledby="settings-import-export-title">
      <h2 id="settings-import-export-title">{text("Import and export")}</h2>

      <div className="import-export-grid">
        <WorkflowPanel
          applyLabel={text("Apply import")}
          acceptedFileTypes=".json"
          description={text("Companies, watchlists, and notebooks.")}
          exportLabel={text("Export")}
          fileInputAriaLabel={text("Choose research data file")}
          importLabel={text("Import")}
          inputRef={researchInputRef}
          state={researchState}
          title={text("Research data")}
          onApply={() => void applyImport("research")}
          onExport={() => void exportData("research")}
          onFileSelected={(file) => void previewFile("research", file)}
        />

        <WorkflowPanel
          applyLabel={text("Apply import")}
          acceptedFileTypes=".yaml,.yml"
          description={text("Safe preferences only. Keys and licenses are excluded.")}
          exportLabel={text("Export")}
          fileInputAriaLabel={text("Choose settings file")}
          importLabel={text("Import")}
          inputRef={settingsInputRef}
          state={settingsState}
          title={text("Settings")}
          onApply={() => void applyImport("settings")}
          onExport={() => void exportData("settings")}
          onFileSelected={(file) => void previewFile("settings", file)}
        />
      </div>

      {exportError ? <ErrorText>{text("Export failed")}: {exportError}</ErrorText> : null}
    </section>
  );
}

type WorkflowPanelProps = {
  title: string;
  description: string;
  exportLabel: string;
  importLabel: string;
  fileInputAriaLabel: string;
  applyLabel: string;
  acceptedFileTypes: string;
  state: ImportState;
  inputRef: MutableRefObject<HTMLInputElement | null>;
  onExport: () => void;
  onFileSelected: (file: File | null) => void;
  onApply: () => void;
};

function WorkflowPanel({
  title,
  description,
  exportLabel,
  importLabel,
  fileInputAriaLabel,
  applyLabel,
  acceptedFileTypes,
  state,
  inputRef,
  onExport,
  onFileSelected,
  onApply,
}: WorkflowPanelProps) {
  const { text } = useLocale();

  return (
    <section className="import-export-panel" aria-label={title}>
      <div>
        <h3>{title}</h3>
        <p className="settings-note">{description}</p>
      </div>

      <ActionRow className="import-export-actions">
        <Button className="compact-button" onClick={onExport} variant="primary">{exportLabel}</Button>
        <Button className="compact-button" onClick={() => inputRef.current?.click()}>{importLabel}</Button>
        <input
          ref={inputRef}
          accept={acceptedFileTypes}
          aria-label={fileInputAriaLabel}
          className="visually-hidden"
          type="file"
          onChange={(event) => onFileSelected(event.target.files?.[0] ?? null)}
        />
      </ActionRow>

      {state.inFlight ? <p className="settings-note">{text("Working")}</p> : null}
      {state.preview ? <PreviewSummary preview={state.preview} /> : null}
      {state.result ? <ResultSummary result={state.result} /> : null}
      {state.error ? <ErrorText>{text("Import failed")}: {state.error}</ErrorText> : null}

      {!state.preview && !state.result && !state.error && !state.inFlight ? (
        <p className="settings-note import-export-empty">{text("Choose a file to preview import.")}</p>
      ) : null}

      {state.preview ? (
        <Button
          className="import-export-apply compact-button"
          disabled={!state.preview.valid || state.inFlight}
          onClick={onApply}
          variant="secondary"
        >
          {applyLabel}
        </Button>
      ) : null}
    </section>
  );
}

function PreviewSummary({ preview }: { preview: ImportPreview }) {
  const { text } = useLocale();

  return (
    <div className="import-export-summary" aria-label={text("Import preview")}>
      <SummaryGrid summary={preview.summary} />
      {preview.valid ? <p className="settings-note">{text("Ready to import")}</p> : null}
      {preview.errors.length > 0 ? (
        <ul className="import-export-messages">
          {preview.errors.map((error) => <li key={error}>{error}</li>)}
        </ul>
      ) : null}
      {preview.warnings.length > 0 ? (
        <ul className="import-export-messages">
          {preview.warnings.map((warning) => <li key={warning}>{warning}</li>)}
        </ul>
      ) : null}
    </div>
  );
}

function ResultSummary({ result }: { result: ImportApplyResult }) {
  const { text } = useLocale();

  return (
    <div className="import-export-summary" aria-label={text("Import result")}>
      <p className="settings-note">{text("Import complete")}</p>
      <SummaryGrid summary={result.summary} />
      {result.warnings.length > 0 ? (
        <ul className="import-export-messages">
          {result.warnings.map((warning) => <li key={warning}>{warning}</li>)}
        </ul>
      ) : null}
    </div>
  );
}

function SummaryGrid({ summary }: { summary: ImportApplySummary }) {
  const { text } = useLocale();
  const items = [
    [text("Companies created"), summary.companiesCreated],
    [text("Companies merged"), summary.companiesMerged],
    [text("Watchlists created"), summary.watchlistsCreated],
    [text("Watchlists merged"), summary.watchlistsMerged],
    [text("Memberships created"), summary.membershipsCreated],
    [text("Notebook entries created"), summary.notebookEntriesCreated],
    [text("Notebook entries skipped"), summary.notebookEntriesSkipped],
    [text("Research questions created"), summary.researchQuestionsCreated],
    [text("Research questions merged"), summary.researchQuestionsMerged],
    [text("Evidence links created"), summary.evidenceLinksCreated],
    [text("Evidence links skipped"), summary.evidenceLinksSkipped],
    [text("Research reminders created"), summary.researchRemindersCreated],
    [text("Research reminders skipped"), summary.researchRemindersSkipped],
    [text("Settings updated"), summary.settingsUpdated],
  ] as const;

  return (
    <InfoGrid
      className="import-export-summary-grid"
      items={items
        .filter(([, value]) => value > 0)
        .map(([label, value]) => ({ label, value }))}
    />
  );
}

async function readFileText(file: File) {
  if ("text" in file) {
    return file.text();
  }

  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () => resolve(String(reader.result ?? "")));
    reader.addEventListener("error", () => reject(reader.error ?? new Error("File read failed")));
    reader.readAsText(file);
  });
}

async function saveExportFile(
  fileName: string,
  contents: string,
  filter: { name: string; extensions: string[]; defaultExtension: string },
) {
  const defaultPath = await join(await downloadDir(), ensureFileExtension(fileName, filter.defaultExtension));
  const selectedPath = await save({
    title: "Export file",
    defaultPath,
    filters: [{ name: filter.name, extensions: filter.extensions }],
    canCreateDirectories: true,
  });

  if (!selectedPath) {
    return;
  }

  await writeTextFile(ensureAllowedExtension(selectedPath, filter.extensions, filter.defaultExtension), contents);
}

function ensureFileExtension(fileName: string, extension: string) {
  const normalizedExtension = extension.replace(/^\./, "");
  return fileName.toLowerCase().endsWith(`.${normalizedExtension}`)
    ? fileName
    : `${fileName}.${normalizedExtension}`;
}

function ensureAllowedExtension(path: string, allowedExtensions: string[], defaultExtension: string) {
  const lowerPath = path.toLowerCase();
  const hasAllowedExtension = allowedExtensions.some((extension) =>
    lowerPath.endsWith(`.${extension.toLowerCase().replace(/^\./, "")}`),
  );

  return hasAllowedExtension ? path : `${path}.${defaultExtension.replace(/^\./, "")}`;
}
