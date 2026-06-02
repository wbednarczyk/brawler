import { LocateFixed, Plus, Save, X } from "lucide-react";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { NotebookEntryEditor } from "./NotebookEntryEditor";
import type { NotebooksScreenProps } from "./notebookTypes";

export function NotebooksScreen({
  companies,
  notebookEntries,
  selectedNotebookScreenCompany,
  selectedNotebookScreenEntries,
  selectedNotebookScreenEntry,
  isNotebookScreenComposerOpen,
  isNotebookScreenEditMode,
  isNotebookScreenEditDirty,
  notebookScreenKindFilter,
  notebookScreenClaimStatusFilter,
  notebookScreenFollowUpFilter,
  notebookScreenTagFilter,
  notebookScreenForm,
  notebookScreenEditForm,
  notebookError,
  selectNotebookScreenCompany,
  showNotebookCompanyOpenClaims,
  showNotebookCompanyFollowUps,
  focusCompanyWorkspace,
  toggleNotebookScreenComposer,
  discardNotebookScreenDraft,
  createNotebookScreenEntry,
  toggleNotebookScreenEntry,
  saveNotebookScreenEntry,
  cancelNotebookScreenEdit,
  setNotebookScreenEditMode,
  setNotebookScreenKindFilter,
  setNotebookScreenClaimStatusFilter,
  setNotebookScreenFollowUpFilter,
  setNotebookScreenTagFilter,
  updateNotebookScreenForm,
  updateNotebookScreenEditForm,
  NotebookDateField,
  NotebookQuarterField,
  MarkdownNoteBody,
  renderNotebookOrigins,
}: NotebooksScreenProps) {
  return (
    <section className="feed-panel" aria-labelledby="notebooks-title">
      <div className="panel-header">
        <div>
          <h1 id="notebooks-title">Notebooks</h1>
          <p>Company-first research notes for daily notes work.</p>
        </div>
      </div>

      <div className="notebooks-screen" aria-label="Notebooks workspace">
        <div className="notebooks-company-nav" aria-label="Notebook companies">
          {companies.map((company) => {
            const companyNotes = notebookEntries.filter((entry) => entry.companyId === company.id);
            const openClaims = companyNotes.filter((entry) => entry.claimStatus === "open").length;
            const followUpScheduled = companyNotes.filter(
              (entry) => entry.followUpAfter || entry.followUpDate,
            ).length;

            return (
              <div
                className={[
                  "notebooks-company-row",
                  selectedNotebookScreenCompany?.id === company.id
                    ? "notebooks-company-row-selected"
                    : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                key={company.id}
              >
                <button
                  aria-label={`Open notebook company: ${company.qualifiedTicker}`}
                  className="notebooks-company-select"
                  onClick={() => selectNotebookScreenCompany(company)}
                  type="button"
                >
                  <span>
                    <strong>{company.qualifiedTicker}</strong>
                    <small>{company.displayName}</small>
                  </span>
                </button>
                <div className="notebooks-company-cues">
                  <span
                    className="notebooks-company-count"
                    aria-label={`${companyNotes.length} notebook entries for ${company.qualifiedTicker}`}
                  >
                    {companyNotes.length}
                  </span>
                  {openClaims > 0 ? (
                    <button
                      aria-label={`Show open claims for ${company.qualifiedTicker}`}
                      className="notebooks-company-cue notebooks-company-cue-button"
                      onClick={() => showNotebookCompanyOpenClaims(company)}
                      type="button"
                    >
                      {openClaims} open
                    </button>
                  ) : null}
                  {followUpScheduled > 0 ? (
                    <button
                      aria-label={`Show follow-ups for ${company.qualifiedTicker}`}
                      className="notebooks-company-cue notebooks-company-cue-button"
                      onClick={() => showNotebookCompanyFollowUps(company)}
                      type="button"
                    >
                      {followUpScheduled} follow-up
                    </button>
                  ) : null}
                </div>
                <button
                  aria-label={`Open company workspace: ${company.qualifiedTicker}`}
                  className="notebooks-company-action"
                  onClick={() => focusCompanyWorkspace(company.id)}
                  title={`Open ${company.qualifiedTicker} workspace`}
                  type="button"
                >
                  <LocateFixed size={14} />
                </button>
              </div>
            );
          })}
          {companies.length === 0 ? (
            <EmptyState>Add companies before using notebooks.</EmptyState>
          ) : null}
        </div>

        <div className="notebooks-main" aria-label="Notebook screen entries">
          <div className="notebooks-context-line">
            <div>
              <strong>{selectedNotebookScreenCompany?.qualifiedTicker ?? "No company selected"}</strong>
              <span>
                {selectedNotebookScreenEntries.length} visible note
                {selectedNotebookScreenEntries.length === 1 ? "" : "s"}
              </span>
            </div>
            <Button
              className="compact-button"
              disabled={
                !selectedNotebookScreenCompany ||
                (isNotebookScreenComposerOpen &&
                  (!notebookScreenForm.title.trim() || !notebookScreenForm.body.trim()))
              }
              form={isNotebookScreenComposerOpen ? "notebook-screen-create-form" : undefined}
              onClick={isNotebookScreenComposerOpen ? undefined : toggleNotebookScreenComposer}
              type={isNotebookScreenComposerOpen ? "submit" : "button"}
              variant="primary"
            >
              {isNotebookScreenComposerOpen ? <Save size={15} /> : <Plus size={15} />}
              {isNotebookScreenComposerOpen ? "Save" : "New note"}
            </Button>
          </div>
          <div className="filter-reset-row" aria-label="Notebook filter reset">
            <div className="inbox-review-summary" aria-label="Notebook follow-up summary">
              <span>
                <strong>{selectedNotebookScreenEntries.length}</strong> visible
              </span>
            </div>
            <Button
              className="compact-button"
              disabled={
                notebookScreenKindFilter === "all" &&
                notebookScreenClaimStatusFilter === "all" &&
                notebookScreenFollowUpFilter === "all" &&
                notebookScreenTagFilter.trim().length === 0
              }
              onClick={() => {
                setNotebookScreenKindFilter("all");
                setNotebookScreenClaimStatusFilter("all");
                setNotebookScreenFollowUpFilter("all");
                setNotebookScreenTagFilter("");
              }}
            >
              <X size={15} />
              Clear filters
            </Button>
          </div>
          <div className="notebooks-filter-row" aria-label="Notebook filters">
            <label>
              Kind
              <select
                aria-label="Notebook kind filter"
                value={notebookScreenKindFilter}
                onChange={(event) => setNotebookScreenKindFilter(event.target.value)}
              >
                <option value="all">All</option>
                <option value="manual">Manual</option>
                <option value="observation">Observation</option>
                <option value="claim">Claim</option>
                <option value="question">Question</option>
                <option value="follow_up">Follow-up</option>
              </select>
            </label>
            <label>
              Status
              <select
                aria-label="Notebook claim status filter"
                value={notebookScreenClaimStatusFilter}
                onChange={(event) => setNotebookScreenClaimStatusFilter(event.target.value)}
              >
                <option value="all">All</option>
                <option value="open">Open</option>
                <option value="delivered">Delivered</option>
                <option value="partially_delivered">Partially delivered</option>
                <option value="missed">Missed</option>
                <option value="unknown">Unknown</option>
                <option value="not_applicable">Not applicable</option>
              </select>
            </label>
            <label>
              Tag
              <input
                aria-label="Notebook tag filter"
                placeholder="tag"
                value={notebookScreenTagFilter}
                onChange={(event) => setNotebookScreenTagFilter(event.target.value)}
              />
            </label>
            <label>
              Follow-up
              <select
                aria-label="Notebook follow-up filter"
                value={notebookScreenFollowUpFilter}
                onChange={(event) => setNotebookScreenFollowUpFilter(event.target.value)}
              >
                <option value="all">All</option>
                <option value="has_follow_up">Has follow-up</option>
                <option value="no_follow_up">No follow-up</option>
              </select>
            </label>
          </div>

          <div className="notebooks-notes-list">
            {isNotebookScreenComposerOpen ? (
              <form
                id="notebook-screen-create-form"
                className="notebook-form notebooks-create-form"
                onSubmit={createNotebookScreenEntry}
              >
                <div className="notebooks-draft-header">
                  <Button onClick={discardNotebookScreenDraft} variant="minimal">
                    <X size={12} />
                    Discard
                  </Button>
                </div>
                <div className="notebook-form-grid">
                  <label>
                    Title
                    <input
                      aria-label="Notebook screen note title"
                      value={notebookScreenForm.title}
                      onChange={(event) => updateNotebookScreenForm("title", event.target.value)}
                    />
                  </label>
                  <label>
                    Kind
                    <select
                      aria-label="Notebook screen note kind"
                      value={notebookScreenForm.kind}
                      onChange={(event) => updateNotebookScreenForm("kind", event.target.value)}
                    >
                      <option value="manual">Manual</option>
                      <option value="observation">Observation</option>
                      <option value="claim">Claim</option>
                      <option value="question">Question</option>
                      <option value="follow_up">Follow-up</option>
                    </select>
                  </label>
                  <label>
                    Tags
                    <input
                      aria-label="Notebook screen note tags"
                      placeholder="comma, separated"
                      value={notebookScreenForm.tags}
                      onChange={(event) => updateNotebookScreenForm("tags", event.target.value)}
                    />
                  </label>
                  <label>
                    Claim status
                    <select
                      aria-label="Notebook screen note claim status"
                      value={notebookScreenForm.claimStatus}
                      onChange={(event) => updateNotebookScreenForm("claimStatus", event.target.value)}
                    >
                      <option value="">None</option>
                      <option value="open">Open</option>
                      <option value="delivered">Delivered</option>
                      <option value="partially_delivered">Partially delivered</option>
                      <option value="missed">Missed</option>
                      <option value="unknown">Unknown</option>
                      <option value="not_applicable">Not applicable</option>
                    </select>
                  </label>
                  <NotebookDateField
                    ariaLabel="Notebook screen note event date"
                    label="Event date"
                    value={notebookScreenForm.eventDate}
                    onChange={(value) => updateNotebookScreenForm("eventDate", value)}
                  />
                  <NotebookQuarterField
                    ariaLabel="Notebook screen note follow-up quarter"
                    label="Follow-up quarter"
                    value={notebookScreenForm.followUpAfter}
                    onChange={(value) => updateNotebookScreenForm("followUpAfter", value)}
                  />
                  <NotebookDateField
                    ariaLabel="Notebook screen note follow-up date"
                    label="Follow-up date"
                    value={notebookScreenForm.followUpDate}
                    onChange={(value) => updateNotebookScreenForm("followUpDate", value)}
                  />
                </div>
                <label className="notebook-body-field">
                  Body
                  <textarea
                    aria-label="Notebook screen note body"
                    value={notebookScreenForm.body}
                    onChange={(event) => updateNotebookScreenForm("body", event.target.value)}
                  />
                </label>
              </form>
            ) : null}

            {selectedNotebookScreenEntries.map((entry) => (
              <div className="notebook-row-block" key={entry.id}>
                <button
                  aria-label={`Select notebook screen entry: ${entry.title}`}
                  className={[
                    "notebook-row",
                    selectedNotebookScreenEntry?.id === entry.id ? "notebook-row-selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  onClick={() => toggleNotebookScreenEntry(entry)}
                  type="button"
                >
                  <div>
                    <div className="notebook-row-top">
                      <h3>{entry.title}</h3>
                      <span>{entry.kind.replace("_", " ")}</span>
                    </div>
                  </div>
                  <div className="notebook-row-meta">
                    {entry.claimStatus ? <span>{entry.claimStatus.replace("_", " ")}</span> : null}
                    {entry.followUpAfter ? <span>{entry.followUpAfter}</span> : null}
                    {entry.tags.slice(0, 2).map((tag) => (
                      <span key={tag}>{tag}</span>
                    ))}
                  </div>
                </button>

                {selectedNotebookScreenEntry?.id === entry.id ? (
                  <NotebookEntryEditor
                    selectedNotebookScreenEntry={selectedNotebookScreenEntry}
                    isNotebookScreenEditMode={isNotebookScreenEditMode}
                    isNotebookScreenEditDirty={isNotebookScreenEditDirty}
                    notebookScreenEditForm={notebookScreenEditForm}
                    saveNotebookScreenEntry={saveNotebookScreenEntry}
                    cancelNotebookScreenEdit={cancelNotebookScreenEdit}
                    setNotebookScreenEditMode={setNotebookScreenEditMode}
                    updateNotebookScreenEditForm={updateNotebookScreenEditForm}
                    NotebookDateField={NotebookDateField}
                    NotebookQuarterField={NotebookQuarterField}
                    MarkdownNoteBody={MarkdownNoteBody}
                    renderNotebookOrigins={renderNotebookOrigins}
                  />
                ) : null}
              </div>
            ))}
            {selectedNotebookScreenCompany && selectedNotebookScreenEntries.length === 0 ? (
              <EmptyState>No notes for {selectedNotebookScreenCompany.qualifiedTicker} yet.</EmptyState>
            ) : null}
          </div>
        </div>
      </div>
      {notebookError ? <p className="error-text">Notebook command failed: {notebookError}</p> : null}
    </section>
  );
}
