import { CheckCircle2, Plus } from "lucide-react";
import type { Company } from "../../api/types";
import { Button } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { transcriptUrlValidationMessage } from "./transcriptHelpers";
import type { TranscriptsScreenProps } from "./transcriptTypes";

type TranscriptJobComposerProps = Pick<
  TranscriptsScreenProps,
  | "createTranscriptJob"
  | "selectTranscriptCompany"
  | "setTranscriptJobCreateError"
  | "setTranscriptJobForm"
  | "transcriptCompanySuggestions"
  | "transcriptJobCreateError"
  | "transcriptJobCreateState"
  | "transcriptJobForm"
>;

export function TranscriptJobComposer({
  createTranscriptJob,
  selectTranscriptCompany,
  setTranscriptJobCreateError,
  setTranscriptJobForm,
  transcriptCompanySuggestions,
  transcriptJobCreateError,
  transcriptJobCreateState,
  transcriptJobForm,
}: TranscriptJobComposerProps) {
  const { text } = useLocale();

  return (
    <form className="event-composer" onSubmit={createTranscriptJob} aria-label={text("Create transcript job")}>
      <div className="event-composer-header">
        <div>
          <h2>{text("New transcript job")}</h2>
          <p>{text("URL is required. Description and company are optional.")}</p>
        </div>
        <Button
          className="compact-button"
          disabled={transcriptJobCreateState === "refreshing" || !transcriptJobForm.url.trim()}
          title={transcriptJobForm.url.trim() ? text("Create transcript job") : text("URL is required")}
          type="submit"
          variant="primary"
        >
          {transcriptJobCreateState === "done" ? <CheckCircle2 size={15} /> : <Plus size={15} />}
          {transcriptJobCreateState === "refreshing" ? text("Creating") : text("Create job")}
        </Button>
      </div>
      <div className="event-composer-grid">
        <label className="event-composer-title">
          {text("URL")}
          <input
            aria-label={text("Transcript URL")}
            placeholder="https://www.youtube.com/watch?v=..."
            value={transcriptJobForm.url}
            onChange={(event) =>
              setTranscriptJobForm((current) => ({
                ...current,
                url: event.target.value,
              }))
            }
            onBlur={() => {
              if (transcriptJobForm.url.trim()) {
                const validationMessage = transcriptUrlValidationMessage(transcriptJobForm.url);
                setTranscriptJobCreateError(validationMessage ? text(validationMessage) : null);
              }
            }}
          />
        </label>
        <label className="event-composer-title">
          {text("Description")}
          <input
            aria-label={text("Transcript description")}
            placeholder={text("Optional, e.g. CDR Q2 investor conference")}
            value={transcriptJobForm.label}
            onChange={(event) =>
              setTranscriptJobForm((current) => ({
                ...current,
                label: event.target.value,
              }))
            }
          />
        </label>
        <label className="event-composer-title">
          {text("Company or ticker")}
          <input
            aria-label={text("Transcript company lookup")}
            placeholder={text("Optional, e.g. GPW:CDR, CDR, CD PROJEKT")}
            value={transcriptJobForm.companyQuery}
            onChange={(event) =>
              setTranscriptJobForm((current) => ({
                ...current,
                companyId: "",
                companyQuery: event.target.value,
              }))
            }
          />
        </label>
      </div>
      {transcriptJobForm.companyQuery || transcriptJobForm.companyId ? (
        <TranscriptCompanySuggestions
          selectedCompanyId={transcriptJobForm.companyId}
          suggestions={transcriptCompanySuggestions}
          selectTranscriptCompany={selectTranscriptCompany}
        />
      ) : null}
      {transcriptJobCreateError ? <p className="error-text">{text(transcriptJobCreateError)}</p> : null}
    </form>
  );
}

type TranscriptCompanySuggestionsProps = {
  selectedCompanyId: string;
  suggestions: Company[];
  selectTranscriptCompany: (company: Company) => void;
};

function TranscriptCompanySuggestions({
  selectedCompanyId,
  suggestions,
  selectTranscriptCompany,
}: TranscriptCompanySuggestionsProps) {
  const { text } = useLocale();

  return (
    <div className="company-registry-suggestions" aria-label={text("Transcript company suggestions")}>
      {suggestions.length > 0 ? (
        suggestions.map((company) => (
          <div key={company.id}>
            <button
              className={
                selectedCompanyId === company.id
                  ? "company-registry-suggestion company-registry-suggestion-selected"
                  : "company-registry-suggestion"
              }
              onClick={() => selectTranscriptCompany(company)}
              type="button"
            >
              <strong><TickerLabel value={company.qualifiedTicker} /></strong>
              <span>{company.displayName}</span>
              {company.isin ? <small>{company.isin}</small> : null}
            </button>
          </div>
        ))
      ) : (
        <span>{text("No tracked company matches. Leave company empty to keep this transcript unlinked.")}</span>
      )}
    </div>
  );
}
