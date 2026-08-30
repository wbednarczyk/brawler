import type { Company } from "../../api/types";
import { ActionButton, ErrorText, TextField } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { transcriptUrlValidationMessage } from "./transcriptHelpers";
import type { TranscriptPrimary } from "./transcriptPrimary";
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
  | "openSettings"
> & {
  primary: TranscriptPrimary;
  geminiConfigured: boolean;
};

// F4b S2 (#430a, mockup docs/mockups/frontend-v2-f4/transcripts.html plansza
// 1/2/4): the composer's OWN grid (`.transcript-composer` in transcripts.css)
// — never `.event-composer-grid` — three columns at L/M
// (`minmax(0,1fr) minmax(200px,280px) auto`), one column ≤ 640px. Two fields
// only (recording link, optional company) — the legacy "Description" field
// (the source of a transcript's manual title) is retired from this composer
// per the mockup; `updateTranscriptJobDescription` stays wired for later
// reuse (deviation noted in the S2 handover report).
export function TranscriptJobComposer({
  createTranscriptJob,
  selectTranscriptCompany,
  setTranscriptJobCreateError,
  setTranscriptJobForm,
  transcriptCompanySuggestions,
  transcriptJobCreateError,
  transcriptJobCreateState,
  transcriptJobForm,
  openSettings,
  primary,
  geminiConfigured,
}: TranscriptJobComposerProps) {
  const { text } = useLocale();
  const fetching = transcriptJobCreateState === "refreshing";

  return (
    <form className="transcript-composer" onSubmit={createTranscriptJob} aria-label={text("New transcript")}>
      <TextField
        className="transcript-composer-url"
        label={text("Recording link")}
        aria-label={text("Recording link")}
        placeholder="https://www.youtube.com/watch?v=…"
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
      <TextField
        label={text("Company (optional)")}
        aria-label={text("Company (optional)")}
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
      <ActionButton
        className="transcript-fetch-button"
        verb="fetch"
        type="submit"
        variant={primary === "fetch" ? "primary" : "secondary"}
        data-ux-primary-action={primary === "fetch" ? "true" : undefined}
        disabled={fetching || !transcriptJobForm.url.trim()}
      >
        {fetching ? text("Fetching…") : text("Fetch transcript")}
      </ActionButton>
      {geminiConfigured ? (
        <p className="transcript-source-line">
          <span className="transcript-source-dot" aria-hidden="true" />
          {text("Gemini · key configured")}
          {" · "}
          <ActionButton kind="destination" onClick={openSettings} variant="minimal">
            {text("Settings")}
          </ActionButton>
        </p>
      ) : null}
      {transcriptJobForm.companyQuery || transcriptJobForm.companyId ? (
        <TranscriptCompanySuggestions
          selectedCompanyId={transcriptJobForm.companyId}
          suggestions={transcriptCompanySuggestions}
          selectTranscriptCompany={selectTranscriptCompany}
        />
      ) : null}
      {transcriptJobCreateError ? <ErrorText>{text(transcriptJobCreateError)}</ErrorText> : null}
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
    <div className="company-registry-suggestions transcript-composer-suggestions" aria-label={text("Transcript company suggestions")}>
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
