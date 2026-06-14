import { Plus } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { createKpiDefinition, listKpiDefinitions } from "../../api/financials";
import type { KpiDefinition } from "../../api/financialsTypes";
import { Button } from "./Button";
import { useLocale } from "../locale";

export type CustomKpiManagerProps = {
  companyId: string;
  // Lifts the company-scoped definitions up so the panel can merge them into the
  // matrix and the add-fact KPI dropdown.
  onDefinitionsChange: (definitions: KpiDefinition[]) => void;
};

const VALUE_KINDS = ["monetary", "percentage", "count", "ratio"];

export function CustomKpiManager({ companyId, onDefinitionsChange }: CustomKpiManagerProps) {
  const { text } = useLocale();
  const [definitions, setDefinitions] = useState<KpiDefinition[]>([]);
  const [metricKey, setMetricKey] = useState("");
  const [label, setLabel] = useState("");
  const [valueKind, setValueKind] = useState("monetary");
  const [unit, setUnit] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const company = await listKpiDefinitions({ scope: "company", companyId });
    setDefinitions(company);
    onDefinitionsChange(company);
  }, [companyId, onDefinitionsChange]);

  useEffect(() => {
    void refresh().catch((cause) => setError(String(cause)));
  }, [refresh]);

  const create = useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      setBusy(true);
      setError(null);
      try {
        await createKpiDefinition({
          scope: "company",
          companyId,
          metricKey: metricKey.trim(),
          label: label.trim(),
          valueKind,
          unit: unit.trim() || undefined,
          computation: "reported",
        });
        setMetricKey("");
        setLabel("");
        setUnit("");
        setValueKind("monetary");
        await refresh();
      } catch (cause) {
        setError(String(cause));
      } finally {
        setBusy(false);
      }
    },
    [companyId, label, metricKey, refresh, unit, valueKind]
  );

  return (
    <section className="fundamentals-section custom-kpi-manager" aria-label={text("Custom KPIs")}>
      <div className="section-heading">
        <h4>{text("Custom KPIs")}</h4>
      </div>

      {definitions.length > 0 ? (
        <div className="periods-list" aria-label={text("Custom KPI list")}>
          {definitions.map((definition) => (
            <span className="period-item" key={definition.id} title={definition.metricKey}>
              <span className="period-label">{definition.label}</span>
            </span>
          ))}
        </div>
      ) : (
        <p className="ai-analysis-empty">{text("No custom KPIs for this company yet.")}</p>
      )}

      <form className="fundamentals-form" onSubmit={create}>
        <div className="fundamentals-form-grid">
          <label>
            {text("Label")}
            <input
              aria-label={text("Custom KPI label")}
              onChange={(event) => setLabel(event.target.value)}
              placeholder={text("Subscribers")}
              value={label}
            />
          </label>
          <label>
            {text("Metric key")}
            <input
              aria-label={text("Custom KPI metric key")}
              onChange={(event) => setMetricKey(event.target.value)}
              placeholder="subscribers"
              value={metricKey}
            />
          </label>
          <label>
            {text("Value kind")}
            <select
              aria-label={text("Custom KPI value kind")}
              onChange={(event) => setValueKind(event.target.value)}
              value={valueKind}
            >
              {VALUE_KINDS.map((kind) => (
                <option key={kind} value={kind}>
                  {text(kind)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {text("Unit")}
            <input
              aria-label={text("Custom KPI unit")}
              onChange={(event) => setUnit(event.target.value)}
              placeholder="per_share"
              value={unit}
            />
          </label>
          <Button
            className="compact-button"
            disabled={busy || !metricKey.trim() || !label.trim()}
            type="submit"
            variant="primary"
          >
            <Plus size={15} />
            {text("Add KPI")}
          </Button>
        </div>
      </form>

      {error ? <p className="error-text">{text("Failed to create custom KPI")}: {error}</p> : null}
    </section>
  );
}
