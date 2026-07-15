import { useEffect, useState } from "react";
import { getCompanySector, listCompanySectors, setCompanySector } from "../../api/companySector";
import { Button } from "./Button";
import { ErrorText, SectionHeader, TextField } from "../../ui";
import { useLocale } from "../locale";

export type CompanySectorFieldProps = {
  companyId: string;
};

/// Manual sector override (ADR 0067 Decision 3): the registry auto-populates a
/// company's sector, but a manual override survives later registry refreshes.
/// Self-contained: loads and saves its own value so it does not thread through
/// the company workspace state.
///
/// v0.53 M2 T3, reworked 2026-07-14 (owner report: a wall of ~90 taxonomy
/// chips is unusable): the registry-sourced taxonomy backs **type-to-filter
/// suggestions** — chips appear only while the typed value narrows the
/// taxonomy (capped, case-insensitive substring), never as a full list. The
/// field is auto-filled from the registry, so the default render is compact.
/// A suggestion only fills the field — Save still commits it, matching the
/// existing dirty/Save/Clear flow.

/// Cap on visible suggestions — enough to disambiguate, never a wall.
const MAX_SECTOR_SUGGESTIONS = 12;
export function CompanySectorField({ companyId }: CompanySectorFieldProps) {
  const { text } = useLocale();
  const [value, setValue] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [presets, setPresets] = useState<string[]>([]);

  useEffect(() => {
    let cancelled = false;
    void getCompanySector(companyId)
      .then((sector) => {
        if (cancelled) return;
        setValue(sector ?? "");
        setSaved(sector ?? null);
      })
      .catch((cause) => {
        if (!cancelled) setError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  // The taxonomy is registry-wide, not per-company — loaded once, independent
  // of `companyId`.
  useEffect(() => {
    let cancelled = false;
    void listCompanySectors()
      .then((sectors) => {
        if (!cancelled) setPresets(sectors);
      })
      .catch(() => {
        // Presets are a visual convenience; a load failure just leaves the
        // free-entry field usable with no chips above it.
        if (!cancelled) setPresets([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function save(next: string | null) {
    setBusy(true);
    setError(null);
    try {
      const result = await setCompanySector(companyId, next);
      setSaved(result);
      setValue(result ?? "");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  const trimmed = value.trim() || null;
  const dirty = trimmed !== saved;

  const query = value.trim().toLocaleLowerCase();
  const matches = query
    ? presets.filter((preset) => preset.toLocaleLowerCase().includes(query))
    : [];
  // An exact-only match means the field already holds a taxonomy value
  // (the registry-filled common case) — nothing left to suggest.
  const suggestions =
    matches.length === 1 && matches[0].toLocaleLowerCase() === query
      ? []
      : matches.slice(0, MAX_SECTOR_SUGGESTIONS);

  return (
    <section className="fundamentals-section" aria-label={text("Sector")}>
      <SectionHeader level="h4" title={text("Sector")} />
      <p className="ai-analysis-empty">
        {text(
          "Auto-filled from the registry. Override it here if it's wrong — a later registry refresh won't overwrite your choice.",
        )}
      </p>
      <div className="fundamentals-form-row">
        <TextField
          label={text("Sector")}
          aria-label={text("Sector")}
          onChange={(event) => setValue(event.target.value)}
          placeholder={text("e.g. Technology")}
          value={value}
        />
        <Button className="compact-button" disabled={busy || !dirty} onClick={() => void save(trimmed)}>
          {text("Save")}
        </Button>
        <Button className="compact-button" disabled={busy} onClick={() => void save(null)}>
          {text("Clear override")}
        </Button>
      </div>
      {suggestions.length > 0 ? (
        <div className="sector-preset-list" role="group" aria-label={text("Registry sectors")}>
          {suggestions.map((preset) => {
            const active = trimmed === preset;
            return (
              <button
                key={preset}
                type="button"
                className={["sector-preset-chip", active ? "sector-preset-chip-active" : ""]
                  .filter(Boolean)
                  .join(" ")}
                aria-pressed={active}
                onClick={() => setValue(preset)}
              >
                {preset}
              </button>
            );
          })}
        </div>
      ) : null}
      {error ? <ErrorText>{error}</ErrorText> : null}
    </section>
  );
}
