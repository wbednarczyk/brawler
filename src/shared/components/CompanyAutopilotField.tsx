import { useEffect, useState } from "react";
import { getCompanyAutopilot, setCompanyAutopilot, type AutopilotMode } from "../../api/autopilot";
import { ErrorText, SectionHeader, SelectField } from "../../ui";
import { useLocale } from "../locale";

export type CompanyAutopilotFieldProps = {
  companyId: string;
};

/// Per-company autopilot trust-ladder control (North Star, v0.49.0, ADR 0055).
/// Self-contained: loads and saves its own mode so it does not thread through the
/// company workspace state. Setting a mode is the entry point to the autonomous
/// report pipeline; the global confirm-before-commit default is never changed.
export function CompanyAutopilotField({ companyId }: CompanyAutopilotFieldProps) {
  const { text } = useLocale();
  const [mode, setMode] = useState<AutopilotMode>("off");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void getCompanyAutopilot(companyId)
      .then((status) => {
        if (!cancelled) setMode((status.mode as AutopilotMode) ?? "off");
      })
      .catch((cause) => {
        if (!cancelled) setError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  async function change(next: AutopilotMode) {
    const previous = mode;
    setBusy(true);
    setError(null);
    setMode(next); // optimistic
    try {
      await setCompanyAutopilot({ companyId, mode: next });
    } catch (cause) {
      setMode(previous);
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div role="group" className="fundamentals-section" aria-label={text("Autopilot")}>
      <SectionHeader level="h4" title={text("Autopilot")} />
      <p className="ai-analysis-empty">
        {text(
          "Automatically process this company's new reports on the next source refresh. Off keeps everything manual; Assist auto-fetches and extracts but you confirm each value; Autopilot also auto-confirms extracted values as unreviewed (cited and reversible).",
        )}
      </p>
      <SelectField
        label={text("Autopilot mode")}
        aria-label={text("Autopilot mode")}
        value={mode}
        disabled={busy}
        onChange={(event) => void change(event.target.value as AutopilotMode)}
      >
        <option value="off">{text("Off — manual")}</option>
        <option value="assist">{text("Assist — auto-extract, you confirm")}</option>
        <option value="autopilot">{text("Autopilot — auto-confirm (unreviewed)")}</option>
      </SelectField>
      {error ? <ErrorText>{error}</ErrorText> : null}
    </div>
  );
}
