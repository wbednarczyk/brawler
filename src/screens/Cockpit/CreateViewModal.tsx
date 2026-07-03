import { useState } from "react";

import { Button, Modal, RangeField, TextField } from "../../ui";
import { useLocale } from "../../shared/locale";

export type CreateViewSpec = { name: string; cols: number; rows: number };

export type CreateViewModalProps = {
  open: boolean;
  onClose: () => void;
  onCreate: (spec: CreateViewSpec) => void;
};

const MIN = 1;
const MAX = 6;
const PRESETS: ReadonlyArray<{ cols: number; rows: number }> = [
  { cols: 2, rows: 2 },
  { cols: 2, rows: 3 },
  { cols: 3, rows: 3 },
];

const clamp = (value: number) => Math.max(MIN, Math.min(MAX, Number.isFinite(value) ? value : MIN));

/** A flex (not grid-template) preview so cols×rows is fully dynamic without inline
 * styles. Reused for the preset thumbnails and the live preview. */
function GridPreview({ cols, rows, className }: { cols: number; rows: number; className?: string }) {
  return (
    <span className={["view-grid-preview", className].filter(Boolean).join(" ")} aria-hidden="true">
      {Array.from({ length: rows }).map((_, row) => (
        <span className="view-grid-preview-row" key={row}>
          {Array.from({ length: cols }).map((_, col) => (
            <span className="view-grid-preview-cell" key={col} />
          ))}
        </span>
      ))}
    </span>
  );
}

/// New-view creator (ADR 0057): name + grid size, visual-first per ui-authoring.md
/// — clickable presets, a slider two-way bound to an exact number field, and a
/// live preview. Returns the chosen name + cols/rows; wiring to a saved layout is
/// the caller's job.
export function CreateViewModal({ open, onClose, onCreate }: CreateViewModalProps) {
  const { text } = useLocale();
  const [name, setName] = useState("");
  const [cols, setCols] = useState(2);
  const [rows, setRows] = useState(2);

  function reset() {
    setName("");
    setCols(2);
    setRows(2);
  }

  function close() {
    reset();
    onClose();
  }

  function create() {
    const trimmed = name.trim();
    if (!trimmed) return;
    onCreate({ name: trimmed, cols, rows });
    reset();
  }

  return (
    <Modal
      open={open}
      onClose={close}
      title={text("New view")}
      ariaLabel={text("New view")}
      className="create-view-modal"
      footer={
        <>
          <Button onClick={close} type="button" variant="ghost">
            {text("Cancel")}
          </Button>
          <Button onClick={create} type="button" disabled={!name.trim()}>
            {text("Create view")}
          </Button>
        </>
      }
    >
      <div className="create-view-form">
        <TextField
          label={text("View name")}
          aria-label={text("View name")}
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={text("e.g. Earnings day")}
          autoFocus
        />

        <div className="create-view-presets" role="group" aria-label={text("Grid presets")}>
          {PRESETS.map((preset) => {
            const active = preset.cols === cols && preset.rows === rows;
            return (
              <button
                key={`${preset.cols}x${preset.rows}`}
                type="button"
                aria-pressed={active}
                className={["create-view-preset", active ? "is-active" : ""].filter(Boolean).join(" ")}
                onClick={() => {
                  setCols(preset.cols);
                  setRows(preset.rows);
                }}
              >
                <GridPreview cols={preset.cols} rows={preset.rows} className="is-mini" />
                <span>
                  {preset.cols}×{preset.rows}
                </span>
              </button>
            );
          })}
        </div>

        <div className="create-view-custom">
          <div className="create-view-dim">
            <RangeField
              label={text("Columns")}
              min={MIN}
              max={MAX}
              step={1}
              value={cols}
              onChange={(event) => setCols(clamp(Number(event.target.value)))}
            />
            <TextField
              aria-label={text("Columns")}
              type="number"
              min={MIN}
              max={MAX}
              value={String(cols)}
              onChange={(event) => setCols(clamp(Number(event.target.value)))}
              className="create-view-dim-input"
            />
          </div>
          <div className="create-view-dim">
            <RangeField
              label={text("Rows")}
              min={MIN}
              max={MAX}
              step={1}
              value={rows}
              onChange={(event) => setRows(clamp(Number(event.target.value)))}
            />
            <TextField
              aria-label={text("Rows")}
              type="number"
              min={MIN}
              max={MAX}
              value={String(rows)}
              onChange={(event) => setRows(clamp(Number(event.target.value)))}
              className="create-view-dim-input"
            />
          </div>
        </div>

        <div className="create-view-live" aria-label={text("Preview")}>
          <GridPreview cols={cols} rows={rows} className="is-live" />
        </div>
      </div>
    </Modal>
  );
}
