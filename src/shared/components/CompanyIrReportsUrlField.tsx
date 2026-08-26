import { useEffect, useState } from "react";
import { getCompanyIrReportsUrl, setCompanyIrReportsUrl } from "../../api/ir";
import { Button } from "./Button";
import { ErrorText, SectionHeader, TextField } from "../../ui";
import { useLocale } from "../locale";
import { useToolHost } from "../toolHost";

export type CompanyIrReportsUrlFieldProps = {
  companyId: string;
};

/// Durable per-company IR reports page URL (ADR 0029). Self-contained: loads and
/// saves its own value so it does not thread through the company workspace state.
export function CompanyIrReportsUrlField({ companyId }: CompanyIrReportsUrlFieldProps) {
  const { text } = useLocale();
  const [value, setValue] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void getCompanyIrReportsUrl(companyId)
      .then((url) => {
        if (cancelled) return;
        setValue(url ?? "");
        setSaved(url ?? null);
      })
      .catch((cause) => {
        if (!cancelled) setError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  async function save() {
    setBusy(true);
    setError(null);
    try {
      const next = await setCompanyIrReportsUrl(companyId, value.trim() ? value.trim() : null);
      setSaved(next);
      setValue(next ?? "");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  const dirty = (value.trim() || null) !== saved;

  // Registers this URL edit with the Spółka workshop's dirty gate (F3a S2,
  // ADR 0107, sol R1 finding 1) — a no-op when hosted outside it.
  const { register } = useToolHost();
  useEffect(() => {
    return register({ isDirty: () => dirty, discard: () => setValue(saved ?? "") });
  }, [register, dirty, saved]);

  return (
    <div role="group" className="fundamentals-section" aria-label={text("Investor relations reports page")}>
      <SectionHeader level="h4" title={text("Investor relations reports page")} />
      <p className="ai-analysis-empty">
        {text("Used to fetch reports when a filing has no attachment. The URL rarely changes.")}
      </p>
      <div className="fundamentals-form-row">
        <TextField
          label={text("IR reports page URL")}
          aria-label={text("IR reports page URL")}
          onChange={(event) => setValue(event.target.value)}
          placeholder="https://…/investors/reports"
          value={value}
        />
        <Button className="compact-button" disabled={busy || !dirty} onClick={() => void save()}>
          {text("Save")}
        </Button>
      </div>
      {error ? <ErrorText>{error}</ErrorText> : null}
    </div>
  );
}
