import type { Company, Watchlist } from "../../api/types";
import type { ResearchEvidenceType } from "../../api/researchTypes";
import type { ResearchMode } from "../../app/useResearchController";
import { ActionButton, Checkbox, SegmentedControl, SegmentedControlOption, SelectField } from "../../ui";

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
    <div role="group" className="research-toolbar" aria-label={text("Research filters")}>
      <SegmentedControl ariaLabel={text("Research mode")}>
        <SegmentedControlOption
          active={mode === "company"}
          data-action-kind="control"
          onClick={() => setMode("company")}
        >
          {text("Company")}
        </SegmentedControlOption>
        <SegmentedControlOption
          active={mode === "watchlist"}
          data-action-kind="control"
          onClick={() => setMode("watchlist")}
        >
          {text("Watchlist")}
        </SegmentedControlOption>
      </SegmentedControl>

      {mode === "company" ? (
        <SelectField
          label={<span>{text("Company")}</span>}
          value={selectedCompanyId ?? ""}
          onChange={(event) => setSelectedCompanyId(event.target.value || null)}
        >
          {companies.length === 0 ? <option value="">{text("No companies tracked yet.")}</option> : null}
          {companies.map((company) => (
            <option key={company.id} value={company.id}>
              {company.qualifiedTicker} - {company.displayName}
            </option>
          ))}
        </SelectField>
      ) : (
        <SelectField
          label={<span>{text("Watchlist")}</span>}
          value={selectedWatchlistId ?? ""}
          onChange={(event) => setSelectedWatchlistId(event.target.value || null)}
        >
          {watchlists.length === 0 ? <option value="">{text("No watchlists yet.")}</option> : null}
          {watchlists.map((watchlist) => (
            <option key={watchlist.id} value={watchlist.id}>
              {watchlist.name}
            </option>
          ))}
        </SelectField>
      )}

      <div className="research-filter-group" aria-label={text("Evidence type filters")}>
        {evidenceTypeOptions.map((option) => (
          <ActionButton
            className={selectedEvidenceTypeSet.has(option.value) ? "research-filter active" : "research-filter"}
            key={option.value}
            kind="control"
            onClick={() => toggleEvidenceType(option.value)}
          >
            {text(option.label)}
          </ActionButton>
        ))}
        {selectedEvidenceTypes.length > 0 ? (
          <ActionButton
            className="research-filter research-filter-clear"
            kind="control"
            onClick={clearEvidenceTypes}
          >
            {text("All evidence")}
          </ActionButton>
        ) : null}
      </div>

      <Checkbox
        className="research-toggle"
        checked={changedOnly}
        onChange={(event) => setChangedOnly(event.target.checked)}
        label={<span>{text("Changed since review")}</span>}
      />
      {mode === "watchlist" ? (
        <Checkbox
          className="research-toggle"
          checked={cascadeToCompanies}
          onChange={(event) => setCascadeToCompanies(event.target.checked)}
          label={<span>{text("Also mark member companies reviewed")}</span>}
        />
      ) : null}
    </div>
  );
}
