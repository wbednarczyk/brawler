import { useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { ArrowLeft, LocateFixed, Plus, Save, X } from "lucide-react";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useFocusAfterRemove } from "../../shared/focus/focusAfterRemove";
import { useLocale } from "../../shared/locale";
import { useNotebooksViewModel } from "../../app/state/screenViewModels";
import {
  Button,
  ClearButton,
  DenseRow,
  EmptyState,
  ErrorText,
  PanelHeader,
  SelectField,
  TextareaField,
  TextField,
} from "../../ui";
import { NotebookEntryEditor } from "./NotebookEntryEditor";

export function NotebooksScreen() {
  const {
  companies,
  totalCompanyCount,
  watchlists,
  notebookEntries,
  selectedNotebookScreenCompany,
  selectedNotebookScreenEntries,
  selectedNotebookScreenEntry,
  isNotebookScreenComposerOpen,
  isNotebookScreenEditMode,
  isNotebookScreenEditDirty,
  notebookScreenKindFilter,
  notebookScreenWatchlistFilter,
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
  deleteNotebookScreenEntry,
  cancelNotebookScreenEdit,
  setNotebookScreenEditMode,
  setNotebookScreenKindFilter,
  setNotebookScreenWatchlistFilter,
  setNotebookScreenClaimStatusFilter,
  setNotebookScreenFollowUpFilter,
  setNotebookScreenTagFilter,
  updateNotebookScreenForm,
  updateNotebookScreenEditForm,
  NotebookDateField,
  NotebookQuarterField,
  MarkdownNoteBody,
  renderNotebookOrigins,
  } = useNotebooksViewModel();
  const { t, text } = useLocale();
  // Deleting the selected note (from the detail-pane editor) clears selection;
  // move focus to the next note row instead of stranding it on <body>
  // (ADR 0076 D9). The hook auto-detects the removed row from the list keys.
  const { listRef: notesListRef } = useFocusAfterRemove<HTMLDivElement>(
    selectedNotebookScreenEntries.map((entry) => entry.id),
    { rowSelector: ".notebook-row" },
  );
  const workspaceRef = useRef<HTMLDivElement | null>(null);
  const resizeStartRef = useRef<{
    handle: "companies" | "notes";
    clientX: number;
    companyWidth: number;
    notesWidth: number;
  } | null>(null);
  const [companyPanelWidth, setCompanyPanelWidth] = useState(280);
  const [notesPanelWidth, setNotesPanelWidth] = useState(420);

  function clampPanelWidth(width: number, minWidth: number, maxShare: number) {
    const workspaceWidth = workspaceRef.current?.getBoundingClientRect().width ?? 0;
    const maxWidth = workspaceWidth > 0 ? Math.max(minWidth, workspaceWidth * maxShare) : minWidth * 2;

    return Math.round(Math.min(Math.max(width, minWidth), maxWidth));
  }

  function startNotebookResize(handle: "companies" | "notes", event: PointerEvent<HTMLDivElement>) {
    resizeStartRef.current = {
      handle,
      clientX: event.clientX,
      companyWidth: companyPanelWidth,
      notesWidth: notesPanelWidth,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function resizeNotebookPanels(event: PointerEvent<HTMLDivElement>) {
    const resizeStart = resizeStartRef.current;

    if (!resizeStart || !event.currentTarget.hasPointerCapture(event.pointerId)) {
      return;
    }

    const delta = event.clientX - resizeStart.clientX;

    if (resizeStart.handle === "companies") {
      setCompanyPanelWidth(clampPanelWidth(resizeStart.companyWidth + delta, 220, 0.42));
      return;
    }

    setNotesPanelWidth(clampPanelWidth(resizeStart.notesWidth + delta, 280, 0.52));
  }

  function stopNotebookResize(event: PointerEvent<HTMLDivElement>) {
    resizeStartRef.current = null;

    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function resizeNotebookPanelWithKeyboard(
    handle: "companies" | "notes",
    event: KeyboardEvent<HTMLDivElement>,
  ) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return;
    }

    event.preventDefault();

    const delta = event.key === "ArrowRight" ? 24 : -24;

    if (handle === "companies") {
      setCompanyPanelWidth((current) => clampPanelWidth(current + delta, 220, 0.42));
      return;
    }

    setNotesPanelWidth((current) => clampPanelWidth(current + delta, 280, 0.52));
  }

  const notebookWorkspaceStyle = {
    "--notebooks-company-panel-width": `${companyPanelWidth}px`,
    "--notebooks-notes-panel-width": `${notesPanelWidth}px`,
  } as CSSProperties;

  return (
    <section className="feed-panel" aria-labelledby="notebooks-title">
      <PanelHeader
        paneLead
        title={t("notebooks.title")}
        description={t("notebooks.description")}
        titleId="notebooks-title"
      />

      <div
        className="notebooks-screen"
        aria-label={text("Notebooks workspace")}
        ref={workspaceRef}
        style={notebookWorkspaceStyle}
        {...(isNotebookScreenComposerOpen || selectedNotebookScreenEntry
          ? { "data-detail-open": "" }
          : {})}
      >
        <div className="notebooks-company-nav" aria-label={text("Notebook companies")}>
          {companies.map((company) => {
            const companyNotes = notebookEntries.filter((entry) => entry.companyId === company.id);
            const openClaims = companyNotes.filter((entry) => entry.claimStatus === "open").length;
            const followUpScheduled = companyNotes.filter(
              (entry) => entry.followUpAfter || entry.followUpDate,
            ).length;

            return (
              <DenseRow
                className={[
                  "notebooks-company-row",
                  selectedNotebookScreenCompany?.id === company.id
                    ? "notebooks-company-row-selected"
                    : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                interactive={false}
                key={company.id}
                selected={selectedNotebookScreenCompany?.id === company.id}
              >
                <button
                  aria-label={`${text("Open notebook company")}: ${company.qualifiedTicker}`}
                  className="notebooks-company-select"
                  onClick={() => selectNotebookScreenCompany(company)}
                  type="button"
                >
                  <span>
                    <strong className="notebooks-company-name">{company.displayName}</strong>
                    <small className="notebooks-company-ticker">
                      <TickerLabel value={company.qualifiedTicker} />
                    </small>
                  </span>
                </button>
                <div className="notebooks-company-cues">
                  <span
                    className="notebooks-company-count"
                    aria-label={`${companyNotes.length} ${text("notebook entries for")} ${company.qualifiedTicker}`}
                  >
                    {companyNotes.length}
                  </span>
                  {openClaims > 0 ? (
                    <button
                      aria-label={`${text("Show open claims for")} ${company.qualifiedTicker}`}
                      className="notebooks-company-cue notebooks-company-cue-button"
                      onClick={() => showNotebookCompanyOpenClaims(company)}
                      type="button"
                    >
                      {openClaims} {text("open")}
                    </button>
                  ) : null}
                  {followUpScheduled > 0 ? (
                    <button
                      aria-label={`${text("Show follow-ups for")} ${company.qualifiedTicker}`}
                      className="notebooks-company-cue notebooks-company-cue-button"
                      onClick={() => showNotebookCompanyFollowUps(company)}
                      type="button"
                    >
                      {followUpScheduled} {text("Follow-up")}
                    </button>
                  ) : null}
                </div>
                <button
                  aria-label={`${text("Open company workspace")}: ${company.qualifiedTicker}`}
                  className="notebooks-company-action"
                  onClick={() => focusCompanyWorkspace(company.id)}
                  title={`${text("Open")} ${company.qualifiedTicker} ${text("workspace")}`}
                  type="button"
                >
                  <LocateFixed size={14} />
                </button>
              </DenseRow>
            );
          })}
          {companies.length === 0 ? (
            <EmptyState>
              {totalCompanyCount === 0
                ? text("Add companies before using notebooks.")
                : text("No notebook companies match this watchlist.")}
            </EmptyState>
          ) : null}
        </div>

        <div
          aria-label={text("Resize notebook company list")}
          aria-orientation="vertical"
          aria-valuemax={520}
          aria-valuemin={220}
          aria-valuenow={companyPanelWidth}
          className="notebooks-resizer"
          onKeyDown={(event) => resizeNotebookPanelWithKeyboard("companies", event)}
          onPointerDown={(event) => startNotebookResize("companies", event)}
          onPointerMove={resizeNotebookPanels}
          onPointerUp={stopNotebookResize}
          role="separator"
          tabIndex={0}
          title={text("Drag to resize notebook company list")}
        />

        <div className="notebooks-main" aria-label={text("Notebook screen entries")}>
          <div className="notebooks-context-line">
            <div>
              <strong>
                {selectedNotebookScreenCompany ? (
                  <TickerLabel value={selectedNotebookScreenCompany.qualifiedTicker} />
                ) : (
                  text("No company selected")
                )}
              </strong>
              <span>
                {selectedNotebookScreenEntries.length} {text(selectedNotebookScreenEntries.length === 1 ? "visible note" : "visible notes")}
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
              {isNotebookScreenComposerOpen ? text("Save") : text("New note")}
            </Button>
          </div>
          <div className="filter-reset-row" aria-label={text("Notebook filter reset")}>
            <div className="inbox-review-summary" aria-label={text("Notebook follow-up summary")}>
              <span>
                <strong>{selectedNotebookScreenEntries.length}</strong> {text("visible")}
              </span>
            </div>
            <Button
              className="compact-button"
              disabled={
                notebookScreenWatchlistFilter === "all" &&
                notebookScreenKindFilter === "all" &&
                notebookScreenClaimStatusFilter === "all" &&
                notebookScreenFollowUpFilter === "all" &&
                notebookScreenTagFilter.trim().length === 0
              }
              onClick={() => {
                setNotebookScreenWatchlistFilter("all");
                setNotebookScreenKindFilter("all");
                setNotebookScreenClaimStatusFilter("all");
                setNotebookScreenFollowUpFilter("all");
                setNotebookScreenTagFilter("");
              }}
            >
              <X size={15} />
              {text("Clear filters")}
            </Button>
          </div>
          <div className="notebooks-filter-row" aria-label={text("Notebook filters")}>
            <SelectField
              label={text("Watchlist")}
              aria-label={text("Notebook watchlist filter")}
              value={notebookScreenWatchlistFilter}
              onChange={(event) => setNotebookScreenWatchlistFilter(event.target.value)}
            >
              <option value="all">{text("All watchlists")}</option>
              {watchlists.map((watchlist) => (
                <option key={watchlist.id} value={watchlist.id}>
                  {watchlist.name}
                </option>
              ))}
            </SelectField>
            <SelectField
              label={text("Kind")}
              aria-label={text("Notebook kind filter")}
              value={notebookScreenKindFilter}
              onChange={(event) => setNotebookScreenKindFilter(event.target.value)}
            >
              <option value="all">{text("All")}</option>
              <option value="manual">{text("Manual")}</option>
              <option value="observation">{text("Observation")}</option>
              <option value="claim">{text("Claim")}</option>
              <option value="question">{text("Question")}</option>
              <option value="follow_up">{text("Follow-up")}</option>
            </SelectField>
            <SelectField
              label={text("Status")}
              aria-label={text("Notebook claim status filter")}
              value={notebookScreenClaimStatusFilter}
              onChange={(event) => setNotebookScreenClaimStatusFilter(event.target.value)}
            >
              <option value="all">{text("All")}</option>
              <option value="open">{text("Status open")}</option>
              <option value="delivered">{text("Delivered")}</option>
              <option value="partially_delivered">{text("Partially delivered")}</option>
              <option value="missed">{text("Missed")}</option>
              <option value="unknown">{text("Unknown")}</option>
              <option value="not_applicable">{text("Not applicable")}</option>
            </SelectField>
            <label>
              {text("Tag")}
              <span className="field-with-clear">
                <TextField
                  aria-label={text("Notebook tag filter")}
                  placeholder={text("tag")}
                  value={notebookScreenTagFilter}
                  onChange={(event) => setNotebookScreenTagFilter(event.target.value)}
                />
                {notebookScreenTagFilter.trim().length > 0 ? (
                  <ClearButton label={text("Clear notebook tag filter")} onClick={() => setNotebookScreenTagFilter("")} />
                ) : null}
              </span>
            </label>
            <SelectField
              label={text("Follow-up")}
              aria-label={text("Notebook follow-up filter")}
              value={notebookScreenFollowUpFilter}
              onChange={(event) => setNotebookScreenFollowUpFilter(event.target.value)}
            >
              <option value="all">{text("All")}</option>
              <option value="has_follow_up">{text("Has follow-up")}</option>
              <option value="no_follow_up">{text("No follow-up")}</option>
            </SelectField>
          </div>

          <div className="notebooks-notes-list" aria-label={text("Notebook note list")} ref={notesListRef}>
            {selectedNotebookScreenEntries.map((entry) => (
              <div className="notebook-row-block" key={entry.id} data-notebook-entry-id={entry.id}>
                <button
                  aria-label={`${text("Select notebook screen entry")}: ${entry.title}`}
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
                      <span>{text(entry.kind.replace("_", " "))}</span>
                    </div>
                  </div>
                  <div className="notebook-row-meta">
                    {entry.claimStatus ? <span>{text(entry.claimStatus.replace("_", " "))}</span> : null}
                    {entry.followUpAfter ? <span>{entry.followUpAfter}</span> : null}
                    {entry.tags.slice(0, 2).map((tag) => (
                      <span key={tag}>{tag}</span>
                    ))}
                  </div>
                </button>
              </div>
            ))}
            {selectedNotebookScreenCompany && selectedNotebookScreenEntries.length === 0 ? (
              <EmptyState>
                {text("No notes for")} <TickerLabel value={selectedNotebookScreenCompany.qualifiedTicker} /> {text("yet.")}
              </EmptyState>
            ) : null}
          </div>
        </div>

        <div
          aria-label={text("Resize notebook note list")}
          aria-orientation="vertical"
          aria-valuemax={640}
          aria-valuemin={280}
          aria-valuenow={notesPanelWidth}
          className="notebooks-resizer"
          onKeyDown={(event) => resizeNotebookPanelWithKeyboard("notes", event)}
          onPointerDown={(event) => startNotebookResize("notes", event)}
          onPointerMove={resizeNotebookPanels}
          onPointerUp={stopNotebookResize}
          role="separator"
          tabIndex={0}
          title={text("Drag to resize notebook note list")}
        />

        <aside className="notebooks-detail-pane" aria-label={text("Notebook selected entry")}>
          {isNotebookScreenComposerOpen || selectedNotebookScreenEntry ? (
            <Button
              className="notebooks-back-to-list"
              type="button"
              variant="minimal"
              onClick={() => {
                if (isNotebookScreenComposerOpen) {
                  discardNotebookScreenDraft();
                } else if (selectedNotebookScreenEntry) {
                  toggleNotebookScreenEntry(selectedNotebookScreenEntry);
                }
              }}
              aria-label={text("Back to note list")}
            >
              <ArrowLeft size={15} />
              {text("Back")}
            </Button>
          ) : null}
          {isNotebookScreenComposerOpen ? (
            <form
              id="notebook-screen-create-form"
              className="notebook-form notebooks-create-form"
              onSubmit={createNotebookScreenEntry}
            >
              <div className="notebooks-draft-header">
                <div>
                  <span className="eyebrow">{text("New note")}</span>
                  <h2>
                    {selectedNotebookScreenCompany ? (
                      <TickerLabel value={selectedNotebookScreenCompany.qualifiedTicker} />
                    ) : (
                      text("Notebook")
                    )}
                  </h2>
                </div>
                <Button onClick={discardNotebookScreenDraft} variant="minimal">
                  <X size={12} />
                  {text("Discard")}
                </Button>
              </div>
              <div className="notebook-form-grid">
                <TextField
                  label={text("Title")}
                  aria-label={text("Notebook screen note title")}
                  value={notebookScreenForm.title}
                  onChange={(event) => updateNotebookScreenForm("title", event.target.value)}
                />
                <SelectField
                  label={text("Kind")}
                  aria-label={text("Notebook screen note kind")}
                  value={notebookScreenForm.kind}
                  onChange={(event) => updateNotebookScreenForm("kind", event.target.value)}
                >
                  <option value="manual">{text("Manual")}</option>
                  <option value="observation">{text("Observation")}</option>
                  <option value="claim">{text("Claim")}</option>
                  <option value="question">{text("Question")}</option>
                  <option value="follow_up">{text("Follow-up")}</option>
                </SelectField>
                <TextField
                  label={text("Tags")}
                  aria-label={text("Notebook screen note tags")}
                  placeholder={text("comma, separated")}
                  value={notebookScreenForm.tags}
                  onChange={(event) => updateNotebookScreenForm("tags", event.target.value)}
                />
                <SelectField
                  label={text("Claim status")}
                  aria-label={text("Notebook screen note claim status")}
                  value={notebookScreenForm.claimStatus}
                  onChange={(event) => updateNotebookScreenForm("claimStatus", event.target.value)}
                >
                  <option value="">{text("None")}</option>
                  <option value="open">{text("Status open")}</option>
                  <option value="delivered">{text("Delivered")}</option>
                  <option value="partially_delivered">{text("Partially delivered")}</option>
                  <option value="missed">{text("Missed")}</option>
                  <option value="unknown">{text("Unknown")}</option>
                  <option value="not_applicable">{text("Not applicable")}</option>
                </SelectField>
                <NotebookDateField
                  ariaLabel={text("Notebook screen note event date")}
                  label={text("Event date")}
                  value={notebookScreenForm.eventDate}
                  onChange={(value) => updateNotebookScreenForm("eventDate", value)}
                />
                <NotebookQuarterField
                  ariaLabel={text("Notebook screen note follow-up quarter")}
                  label={text("Follow-up quarter")}
                  value={notebookScreenForm.followUpAfter}
                  onChange={(value) => updateNotebookScreenForm("followUpAfter", value)}
                />
                <NotebookDateField
                  ariaLabel={text("Notebook screen note follow-up date")}
                  label={text("Follow-up date")}
                  value={notebookScreenForm.followUpDate}
                  onChange={(value) => updateNotebookScreenForm("followUpDate", value)}
                />
              </div>
              <TextareaField
                className="notebook-body-field"
                label={text("Body")}
                aria-label={text("Notebook screen note body")}
                value={notebookScreenForm.body}
                onChange={(event) => updateNotebookScreenForm("body", event.target.value)}
              />
            </form>
          ) : null}

          {!isNotebookScreenComposerOpen && selectedNotebookScreenEntry ? (
            <NotebookEntryEditor
              selectedNotebookScreenEntry={selectedNotebookScreenEntry}
              isNotebookScreenEditMode={isNotebookScreenEditMode}
              isNotebookScreenEditDirty={isNotebookScreenEditDirty}
              notebookScreenEditForm={notebookScreenEditForm}
              saveNotebookScreenEntry={saveNotebookScreenEntry}
              deleteNotebookScreenEntry={deleteNotebookScreenEntry}
              cancelNotebookScreenEdit={cancelNotebookScreenEdit}
              setNotebookScreenEditMode={setNotebookScreenEditMode}
              updateNotebookScreenEditForm={updateNotebookScreenEditForm}
              NotebookDateField={NotebookDateField}
              NotebookQuarterField={NotebookQuarterField}
              MarkdownNoteBody={MarkdownNoteBody}
              renderNotebookOrigins={renderNotebookOrigins}
            />
          ) : null}

          {!isNotebookScreenComposerOpen && !selectedNotebookScreenEntry ? (
            <EmptyState>{text("Select a note to read or edit it.")}</EmptyState>
          ) : null}
        </aside>
      </div>
      {notebookError ? <ErrorText>{text("Notebook command failed")}: {notebookError}</ErrorText> : null}
    </section>
  );
}
