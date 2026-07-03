import { ChipList, StatusChip } from "../../ui";
import { useLocale } from "../locale";

/** The clean label diff behind a "structure changed" drift (ADR 0061 dec. 3). */
export type ParsedDrift = {
  addedLabels: string[];
  removedLabels: string[];
  unitChanged: [string, string] | null;
};

/**
 * Parses the opaque `driftJson` blob carried on a fact's provenance row (ADR
 * 0061) or an autopilot run's `kpiDeltaJson` (ADR 0055/0061 wave 2). Both are
 * a serialized Rust `DriftReport` (default serde → snake_case keys), stored as
 * TEXT rather than a ts-rs contract type. Parses defensively: a null,
 * malformed, or empty-diff blob yields no card.
 */
export function parseDrift(driftJson: string | null | undefined): ParsedDrift | null {
  if (!driftJson) return null;
  try {
    const raw = JSON.parse(driftJson) as {
      added_labels?: unknown;
      removed_labels?: unknown;
      unit_changed?: unknown;
    };
    const asStrings = (value: unknown): string[] =>
      Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === "string") : [];
    const addedLabels = asStrings(raw.added_labels);
    const removedLabels = asStrings(raw.removed_labels);
    const unitChanged =
      Array.isArray(raw.unit_changed) && raw.unit_changed.length === 2
        ? ([String(raw.unit_changed[0]), String(raw.unit_changed[1])] as [string, string])
        : null;
    if (addedLabels.length === 0 && removedLabels.length === 0 && !unitChanged) return null;
    return { addedLabels, removedLabels, unitChanged };
  } catch {
    return null;
  }
}

/**
 * The label-diff body of a "structure changed" drift card: new/missing report
 * lines as chip groups, plus a reporting-unit-changed line. Domain-shared
 * (ADR 0061 / Radicle 4fde931) — rendered inside `FundamentalsPanel`'s fact
 * detail drift section and Today's autopilot-run drift block, so it owns only
 * the body; callers keep their own section wrapper, heading, and aria-label.
 */
export function DriftDiff({ drift }: { drift: ParsedDrift }) {
  const { text } = useLocale();

  return (
    <div className="drift-diff">
      {drift.addedLabels.length > 0 ? (
        <div className="drift-diff-group">
          <span className="eyebrow">{text("New lines")}</span>
          <ChipList ariaLabel={text("New lines")}>
            {drift.addedLabels.map((label) => (
              <StatusChip key={label} tone="accent">
                {label}
              </StatusChip>
            ))}
          </ChipList>
        </div>
      ) : null}
      {drift.removedLabels.length > 0 ? (
        <div className="drift-diff-group">
          <span className="eyebrow">{text("Missing lines")}</span>
          <ChipList ariaLabel={text("Missing lines")}>
            {drift.removedLabels.map((label) => (
              <StatusChip key={label} tone="neutral">
                {label}
              </StatusChip>
            ))}
          </ChipList>
        </div>
      ) : null}
      {drift.unitChanged ? (
        <span className="eyebrow">
          {text("Reporting unit changed")}: {drift.unitChanged[0]} → {drift.unitChanged[1]}
        </span>
      ) : null}
    </div>
  );
}
