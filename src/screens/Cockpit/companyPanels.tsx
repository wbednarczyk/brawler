import { useEffect } from "react";
import type { Company } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { useToolHost } from "../../shared/toolHost";
import { NotebookDateField } from "../../shared/components/NotebookDateField";
import { NotebookQuarterField } from "../../shared/components/NotebookQuarterField";
import { MarkdownNoteBody } from "../../shared/components/MarkdownNoteBody";
import { CompanyNotebookSection } from "../Companies/CompanyNotebookSection";
import { FundamentalsPanel } from "../Companies/FundamentalsPanel";
import { formatDetailTimestamp } from "../../shared/format/datetime";
import { emptyNotebookForm } from "../../app/notebookForms";
import { useCockpitFundamentals } from "./useCockpitFundamentals";
import { useCockpitCompanyNotebook } from "./useCockpitCompanyNotebook";
import { useCockpitDecisionJournal } from "./useCockpitDecisionJournal";
import { useCockpitShortPositions } from "./useCockpitShortPositions";
import { useCockpitRedFlags } from "./useCockpitRedFlags";
import { useCockpitAnalystRecommendations } from "./useCockpitAnalystRecommendations";
import { DecisionJournalSection } from "./DecisionJournalSection";
import { ShortPositionsSection } from "./ShortPositionsSection";
import { RedFlagsSection } from "./RedFlagsSection";
import { AnalystRecommendationsSection } from "../../shared/components/AnalystRecommendationsSection";

// Company-scoped panel wrappers shared by the cockpit's `renderPinned` (ADR
// 0057, frozen per F3a) and the Spółka workshop's `toolRegistry` (F3a S2, ADR
// 0107) — extracted from CockpitScreen.tsx so both hosts render the SAME
// component/props instead of diverging copies (file-size ratchet, ADR 0103).

// The full, editable Fundamentals panel (ADR 0053 phase 4b). It reuses the real
// `FundamentalsPanel` from the Companies screen — the caller owns the state via
// `useCockpitFundamentals` (which calls api/financials directly), so editing
// works for any company with no host-specific coupling.
export function CockpitFundamentalsPanel({
  companyId,
  qualifiedTicker,
  revision,
  onOpenRecommendations,
}: {
  companyId: string;
  // Card #307: the fact-detail modal's header line. Omitted when the company
  // isn't (yet) resolvable in the host's company map.
  qualifiedTicker?: string;
  // Bumped by a sibling report-documents extraction; forces a facts refetch.
  revision: number;
  // Opens/pins the analyst-recommendations panel from the "vs target" readout.
  onOpenRecommendations?: () => void;
}) {
  const props = useCockpitFundamentals(companyId, revision);
  return (
    <FundamentalsPanel
      {...props}
      qualifiedTicker={qualifiedTicker}
      onOpenRecommendations={onOpenRecommendations}
    />
  );
}

// Company-scoped notebook panel (ADR 0057/0107). Reuses the real
// `CompanyNotebookSection` with caller-owned state (`useCockpitCompanyNotebook`).
// Origins render read-only (label + external source link) — the cross-screen
// "open origin feed item" nav belongs to the Inbox, not a self-contained panel.
export function CockpitCompanyNotebookPanel({ company }: { company: Company }) {
  const { text } = useLocale();
  const notebook = useCockpitCompanyNotebook(company);

  // Register the notebook composer/edit draft with the Spółka workshop's
  // dirty gate (F3a S2, ADR 0107) — a no-op when hosted outside it.
  const { register } = useToolHost();
  useEffect(() => {
    return register({
      isDirty: () =>
        (notebook.isComposerOpen && JSON.stringify(notebook.notebookForm) !== JSON.stringify(emptyNotebookForm())) ||
        (notebook.editMode && notebook.isEditDirty),
      discard: () => {
        notebook.setComposerOpen(false);
        notebook.cancelNotebookEdit();
      },
    });
  }, [register, notebook.isComposerOpen, notebook.notebookForm, notebook.editMode, notebook.isEditDirty]); // eslint-disable-line react-hooks/exhaustive-deps -- setComposerOpen/cancelNotebookEdit are stable setters from the hook, intentionally excluded

  return (
    <CompanyNotebookSection
      company={company}
      notebookEntries={notebook.entries}
      isComposerOpen={notebook.isComposerOpen}
      notebookForm={notebook.notebookForm}
      selectedNotebookEntry={notebook.selectedEntry}
      notebookEditMode={notebook.editMode}
      notebookEditForm={notebook.editForm}
      isNotebookEditDirty={notebook.isEditDirty}
      notebookError={notebook.error}
      setComposerOpen={notebook.setComposerOpen}
      updateNotebookForm={notebook.updateNotebookForm}
      createNotebookEntry={notebook.createNotebookEntry}
      setSelectedNotebookEntryId={notebook.setSelectedEntryId}
      saveNotebookEntry={notebook.saveNotebookEntry}
      cancelNotebookEdit={notebook.cancelNotebookEdit}
      setNotebookEditMode={notebook.setEditMode}
      updateNotebookEditForm={notebook.updateNotebookEditForm}
      NotebookDateField={NotebookDateField}
      NotebookQuarterField={NotebookQuarterField}
      MarkdownNoteBody={MarkdownNoteBody}
      renderNotebookOrigins={(origins) =>
        origins.length === 0 ? (
          <span className="membership-empty">None</span>
        ) : (
          <div className="origin-link-list">
            {origins.map((origin) => (
              <div className="origin-link" key={origin.id}>
                <span>{origin.label ?? origin.sourceType.replace("_", " ")}</span>
                {origin.sourceUrl ? (
                  <a
                    className="secondary-button compact-button"
                    href={origin.sourceUrl}
                    rel="noreferrer"
                    target="_blank"
                  >
                    {text("Source")}
                  </a>
                ) : null}
              </div>
            ))}
          </div>
        )
      }
    />
  );
}

export function CockpitShortPositionsPanel({ company }: { company: Company }) {
  const { view, error } = useCockpitShortPositions(company);
  return <ShortPositionsSection company={company} view={view} error={error} />;
}

// `onOpenEvidence` selects the underlying feed item — the caller owns where
// that navigates (cockpit selection vs. the Spółka feed-item tool).
export function CockpitRedFlagsPanel({
  company,
  onOpenEvidence,
}: {
  company: Company;
  onOpenEvidence?: (feedItemId: string) => void;
}) {
  const { view, error, acknowledge } = useCockpitRedFlags(company);
  return (
    <RedFlagsSection
      company={company}
      view={view}
      error={error}
      onAcknowledge={acknowledge}
      onOpenEvidence={onOpenEvidence}
    />
  );
}

// Analyst-recommendations panel (v0.58 A3, ADR 0073).
export function CockpitAnalystRecommendationsPanel({ company }: { company: Company }) {
  const { view, error, loading, lastClose, currency, reload } =
    useCockpitAnalystRecommendations(company);
  return (
    <AnalystRecommendationsSection
      company={company}
      view={view}
      error={error}
      loading={loading}
      onRetry={reload}
      lastClose={lastClose}
      currency={currency}
    />
  );
}

export function CockpitDecisionJournalPanel({ company }: { company: Company }) {
  const journal = useCockpitDecisionJournal(company);

  // Register the decision-entry composer draft with the Spółka workshop's
  // dirty gate (F3a S2, ADR 0107) — a no-op when hosted outside it. Entries
  // are immutable (only a fresh composer draft exists — no edit form).
  const { register } = useToolHost();
  useEffect(() => {
    return register({
      isDirty: () => journal.isComposerOpen && journal.form.rationaleMd.trim() !== "",
      discard: () => journal.setComposerOpen(false),
    });
  }, [register, journal.isComposerOpen, journal.form.rationaleMd]); // eslint-disable-line react-hooks/exhaustive-deps -- setComposerOpen is a stable setter from the hook, intentionally excluded

  return (
    <DecisionJournalSection
      company={company}
      entries={journal.entries}
      isComposerOpen={journal.isComposerOpen}
      form={journal.form}
      supersedingEntry={journal.supersedingEntry}
      selectedEntry={journal.selectedEntry}
      evidenceCandidates={journal.evidenceCandidates}
      linkedEvidenceKeys={journal.linkedEvidenceKeys}
      error={journal.error}
      setComposerOpen={journal.setComposerOpen}
      updateForm={journal.updateForm}
      createEntry={journal.createEntry}
      startSupersede={journal.startSupersede}
      cancelSupersede={journal.cancelSupersede}
      setSelectedEntryId={journal.setSelectedEntryId}
      linkEvidence={journal.linkEvidence}
      formatTimestamp={formatDetailTimestamp}
    />
  );
}
