import type { Company, Watchlist } from "../../api/types";
import type { ResearchEvidenceType } from "../../api/researchTypes";
import type { ResearchMode } from "../../app/useResearchController";

type ResearchScopeBarProps = {
  companies: Company[];
  watchlists: Watchlist[];
  mode: ResearchMode;
  selectedCompanyId: string | null;
  selectedWatchlistId: string | null;
  selectedEvidenceTypes: ResearchEvidenceType[];
  changedOnly: boolean;
  cascadeToCompanies: boolean;
  setMode: (mode: ResearchMode) => void;
  setSelectedCompanyId: (companyId: string | null) => void;
  setSelectedWatchlistId: (watchlistId: string | null) => void;
  setChangedOnly: (changedOnly: boolean) => void;
  setCascadeToCompanies: (cascade: boolean) => void;
  toggleEvidenceType: (evidenceType: ResearchEvidenceType) => void;
  clearEvidenceTypes: () => void;
  text: (value: string) => string;
};

const evidenceTypeOptions: Array<{ value: ResearchEvidenceType; label: string }> = [
  { value: "feed_item", label: "Feed items" },
  { value: "notebook_entry", label: "Notes" },
  { value: "claim", label: "Claims" },
  { value: "company_event", label: "Events" },
  { value: "transcript_segment", label: "Transcripts" },
  { value: "ai_analysis", label: "AI analysis" },
  { value: "company_signal", label: "Signals" },
];

export function ResearchScopeBar({
  companies,
  watchlists,
  mode,
  selectedCompanyId,
  selectedWatchlistId,
  selectedEvidenceTypes,
  changedOnly,
  cascadeToCompanies,
  setMode,
  setSelectedCompanyId,
  setSelectedWatchlistId,
  setChangedOnly,
  setCascadeToCompanies,
  toggleEvidenceType,
  clearEvidenceTypes,
  text,
}: ResearchScopeBarProps) {
  const selectedEvidenceTypeSet = new Set(selectedEvidenceTypes);

  return (
    <section className="research-toolbar" aria-label={text("Research filters")}>
      <div className="research-mode-switch" aria-label={text("Research mode")}>
        <button
          className={mode === "company" ? "research-mode-option active" : "research-mode-option"}
          type="button"
          onClick={() => setMode("company")}
        >
          {text("Company")}
        </button>
        <button
          className={mode === "watchlist" ? "research-mode-option active" : "research-mode-option"}
          type="button"
          onClick={() => setMode("watchlist")}
        >
          {text("Watchlist")}
        </button>
      </div>

      {mode === "company" ? (
        <label className="research-company-picker">
          <span>{text("Company")}</span>
          <select
            value={selectedCompanyId ?? ""}
            onChange={(event) => setSelectedCompanyId(event.target.value || null)}
          >
            {companies.length === 0 ? <option value="">{text("No companies tracked yet.")}</option> : null}
            {companies.map((company) => (
              <option key={company.id} value={company.id}>
                {company.qualifiedTicker} - {company.displayName}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <label className="research-company-picker">
          <span>{text("Watchlist")}</span>
          <select
            value={selectedWatchlistId ?? ""}
            onChange={(event) => setSelectedWatchlistId(event.target.value || null)}
          >
            {watchlists.length === 0 ? <option value="">{text("No watchlists yet.")}</option> : null}
            {watchlists.map((watchlist) => (
              <option key={watchlist.id} value={watchlist.id}>
                {watchlist.name}
              </option>
            ))}
          </select>
        </label>
      )}

      <div className="research-filter-group" aria-label={text("Evidence type filters")}>
        {evidenceTypeOptions.map((option) => (
          <button
            className={selectedEvidenceTypeSet.has(option.value) ? "research-filter active" : "research-filter"}
            key={option.value}
            type="button"
            onClick={() => toggleEvidenceType(option.value)}
          >
            {text(option.label)}
          </button>
        ))}
        {selectedEvidenceTypes.length > 0 ? (
          <button className="research-filter research-filter-clear" type="button" onClick={clearEvidenceTypes}>
            {text("All evidence")}
          </button>
        ) : null}
      </div>

      <label className="research-toggle">
        <input
          checked={changedOnly}
          type="checkbox"
          onChange={(event) => setChangedOnly(event.target.checked)}
        />
        <span>{text("Changed since review")}</span>
      </label>
      {mode === "watchlist" ? (
        <label className="research-toggle">
          <input
            checked={cascadeToCompanies}
            type="checkbox"
            onChange={(event) => setCascadeToCompanies(event.target.checked)}
          />
          <span>{text("Also mark member companies reviewed")}</span>
        </label>
      ) : null}
    </section>
  );
}
