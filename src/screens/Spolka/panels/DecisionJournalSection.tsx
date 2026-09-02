import { openUrl } from "@tauri-apps/plugin-opener";
import { Plus, Save, X } from "lucide-react";
import type { Company } from "../../../api/types";
import type { ResearchEvidenceItem } from "../../../api/researchTypes";
import type { DecisionEntry } from "../../../api/decisionJournal";
import type { DecisionEntryForm } from "./useDecisionJournalPanel";
import { TickerLabel } from "../../../shared/components/TickerLabel";
import { MarkdownNoteBody } from "../../../shared/components/MarkdownNoteBody";
import { NotebookDateField } from "../../../shared/components/NotebookDateField";
import { useLocale } from "../../../shared/locale";
import { pluralNoun } from "../../../shared/locale/plural";
import {
  Button,
  EmptyState,
  ErrorText,
  Hint,
  SectionHeader,
  SelectField,
  StatusChip,
  StatusPill,
  TextareaField,
} from "../../../ui";
import { EvidenceRow } from "../../Research/EvidenceRow";

// The four recorded judgments (ADR 0071) — actions/judgments the investor took,
// never advice the app gives. Their labels are shared with the global panel.
export function decisionKindLabel(kind: string, text: (value: string) => string): string {
  switch (kind) {
    case "buy":
      return text("Buy");
    case "pass":
      return text("Pass");
    case "keep_watching":
      return text("Keep watching");
    case "sell_note":
      return text("Sell note");
    default:
      return kind;
  }
}

export type DecisionJournalSectionProps = {
  company: Company;
  entries: DecisionEntry[];
  isComposerOpen: boolean;
  form: DecisionEntryForm;
  supersedingEntry: DecisionEntry | null;
  selectedEntry: DecisionEntry | null;
  evidenceCandidates: ResearchEvidenceItem[];
  linkedEvidenceKeys: Set<string>;
  error: string | null;
  setComposerOpen: (open: boolean) => void;
  updateForm: (field: keyof DecisionEntryForm, value: string) => void;
  createEntry: (event?: { preventDefault: () => void }) => void;
  startSupersede: (entry: DecisionEntry) => void;
  cancelSupersede: () => void;
  setSelectedEntryId: (id: string | null) => void;
  linkEvidence: (item: ResearchEvidenceItem) => void;
};

// The company-scoped decision-journal composer + immutable entry list (ADR 0071,
// J3). Entries never change: there is no edit or delete affordance anywhere. A
// correction is a follow-up entry created via "Supersede" (linked back). The
// selected entry exposes an evidence picker over the company timeline, reusing
// the research `EvidenceRow` link pattern with `fromType: "decision_entry"`.
export function DecisionJournalSection({
  company,
  entries,
  isComposerOpen,
  form,
  supersedingEntry,
  selectedEntry,
  evidenceCandidates,
  linkedEvidenceKeys,
  error,
  setComposerOpen,
  updateForm,
  createEntry,
  startSupersede,
  cancelSupersede,
  setSelectedEntryId,
  linkEvidence,
}: DecisionJournalSectionProps) {
  const { text, locale } = useLocale();
  const noun = pluralNoun(locale, entries.length, {
    en: ["decision", "decisions"],
    pl: ["decyzja", "decyzje", "decyzji"],
  });

  return (
    <div className="company-tab-panel decision-journal-panel" aria-label={text("Decision journal")}>
      <SectionHeader
        level="h3"
        paneLead
        title={text("Decision journal")}
        description={
          <>
            {entries.length} {noun} {text("for")} <TickerLabel value={company.qualifiedTicker} />
          </>
        }
        actions={
          <Button
            className="compact-button"
            onClick={() => (isComposerOpen ? cancelSupersede() : setComposerOpen(true))}
            variant="primary"
          >
            {isComposerOpen ? <X size={15} /> : <Plus size={15} />}
            {isComposerOpen ? text("Hide form") : text("New entry")}
          </Button>
        }
      />

      {isComposerOpen ? (
        <form className="decision-journal-form" onSubmit={createEntry}>
          {supersedingEntry ? (
            <Hint>
              {text("Superseding the entry from")} {supersedingEntry.decidedAt} (
              {decisionKindLabel(supersedingEntry.kind, text)})
            </Hint>
          ) : null}
          <div className="decision-journal-form-grid">
            <SelectField
              label={text("Decision")}
              aria-label={text("Decision kind")}
              value={form.kind}
              onChange={(event) => updateForm("kind", event.target.value)}
            >
              <option value="buy">{text("Buy")}</option>
              <option value="pass">{text("Pass")}</option>
              <option value="keep_watching">{text("Keep watching")}</option>
              <option value="sell_note">{text("Sell note")}</option>
            </SelectField>
            <NotebookDateField
              ariaLabel={text("Decision date")}
              label={text("Decided on")}
              value={form.decidedAt}
              onChange={(value) => updateForm("decidedAt", value)}
            />
            <Button
              className="compact-button decision-journal-submit-button"
              disabled={!form.rationaleMd.trim() || !form.decidedAt.trim()}
              type="submit"
              variant="primary"
            >
              <Save size={15} />
              {text("Save")}
            </Button>
          </div>
          <TextareaField
            className="decision-journal-body-field"
            label={text("Rationale")}
            aria-label={text("Decision rationale")}
            placeholder={text("Why this decision? Markdown supported.")}
            value={form.rationaleMd}
            onChange={(event) => updateForm("rationaleMd", event.target.value)}
          />
        </form>
      ) : null}

      <div
        className="decision-journal-workspace"
        {...(selectedEntry ? { "data-detail-open": "" } : {})}
      >
        <div className="decision-journal-list" aria-label={text("Decision entries")}>
          {entries.map((entry) => (
            <button
              aria-label={`${text("Select decision entry")}: ${decisionKindLabel(entry.kind, text)} ${entry.decidedAt}`}
              className={[
                "decision-journal-row",
                selectedEntry?.id === entry.id ? "decision-journal-row-selected" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              key={entry.id}
              onClick={() => setSelectedEntryId(entry.id)}
              type="button"
            >
              <div className="decision-journal-row-top">
                <StatusPill>{decisionKindLabel(entry.kind, text)}</StatusPill>
                {entry.supersededByEntryId ? (
                  <StatusChip tone="accent">{text("Follow-up")}</StatusChip>
                ) : null}
              </div>
              <time className="decision-journal-row-date num-tabular" dateTime={entry.decidedAt}>
                {entry.decidedAt}
              </time>
            </button>
          ))}
          {entries.length === 0 ? (
            <EmptyState>
              {text("No decisions recorded for")} <TickerLabel value={company.qualifiedTicker} />{" "}
              {text("yet.")}
            </EmptyState>
          ) : null}
        </div>

        <div className="decision-journal-detail" aria-label={text("Decision entry detail")}>
          {selectedEntry ? (
            <>
              <div className="decision-journal-entry-header">
                <div>
                  <span className="eyebrow">{decisionKindLabel(selectedEntry.kind, text)}</span>
                  <time className="num-tabular" dateTime={selectedEntry.decidedAt}>
                    {selectedEntry.decidedAt}
                  </time>
                </div>
                {/* Immutable (ADR 0071): the ONLY correction affordance is a
                    follow-up entry — never an edit or delete. */}
                <Button
                  className="compact-button"
                  onClick={() => startSupersede(selectedEntry)}
                  variant="secondary"
                >
                  {text("Supersede")}
                </Button>
              </div>
              <MarkdownNoteBody
                ariaLabel={text("Decision rationale")}
                body={selectedEntry.rationaleMd}
              />
              <div
                role="group"
                className="decision-journal-evidence"
                aria-label={text("Link supporting evidence")}
              >
                <SectionHeader
                  level="h4"
                  title={text("Supporting evidence")}
                  description={text("Link feed items, notes, claims, or events to this decision.")}
                />
                {evidenceCandidates.map((item) => (
                  <EvidenceRow
                    key={item.id}
                    linkLabel="Link to decision"
                    item={item}
                    changed={false}
                    canLink={!linkedEvidenceKeys.has(`${item.evidenceType}:${item.sourceId}`)}
                    onLink={linkEvidence}
                    onOpen={(candidate) =>
                      candidate.sourceUrl ? void openUrl(candidate.sourceUrl) : undefined
                    }
                    onOpenUrl={(url) => void openUrl(url)}
                    text={text}
                  />
                ))}
                {evidenceCandidates.length === 0 ? (
                  <EmptyState>{text("No evidence to link yet.")}</EmptyState>
                ) : null}
              </div>
            </>
          ) : (
            <EmptyState>{text("Select a decision to review it.")}</EmptyState>
          )}
        </div>
      </div>

      {error ? (
        <ErrorText>
          {text("Decision journal command failed")}: {error}
        </ErrorText>
      ) : null}
    </div>
  );
}
