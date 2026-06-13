import { useEffect, useState } from "react";
import { getCompanyIrReportsUrl, setCompanyIrReportsUrl } from "../../api/ir";
import { Button } from "./Button";
import { useLocale } from "../locale";

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

  return (
    <section className="fundamentals-section" aria-label={text("Investor relations reports page")}>
      <div className="section-heading">
        <h4>{text("Investor relations reports page")}</h4>
      </div>
      <p className="ai-analysis-empty">
        {text("Used to fetch reports when a filing has no attachment. The URL rarely changes.")}
      </p>
      <div className="fundamentals-form-row">
        <label>
          {text("IR reports page URL")}
          <input
            aria-label={text("IR reports page URL")}
            onChange={(event) => setValue(event.target.value)}
            placeholder="https://…/investors/reports"
            value={value}
          />
        </label>
        <Button className="compact-button" disabled={busy || !dirty} onClick={() => void save()}>
          {text("Save")}
        </Button>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  );
}
