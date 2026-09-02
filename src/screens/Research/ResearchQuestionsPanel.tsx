import { Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import type {
  EvidenceLink,
  ResearchQuestion,
  ResearchQuestionStatus,
} from "../../api/researchTypes";
import { useFocusAfterRemove } from "../../shared/focus/focusAfterRemove";
import { ActionButton, EmptyState, Figure, InlineConfirm, SectionHeader } from "../../ui";
import { formatEvidenceType, formatQuestionStatus } from "./researchFormatters";

type ResearchQuestionsPanelProps = {
  questions: ResearchQuestion[];
  selectedQuestion: ResearchQuestion | null;
  selectedQuestionId: string | null;
  questionLinks: EvidenceLink[];
  /** Linked evidence's own title, keyed `${evidenceType}:${sourceId}` (sol
   * fix1 item 6) — the removal chip's accessible name reads the evidence's
   * TITLE, not its type, so two linked items of the same type never collide
   * on accessible name. Falls back to the type label if a link's evidence
   * item isn't in the current timeline scope. */
  evidenceTitleByKey: Map<string, string>;
  canAdd: boolean;
  questionInFlight: boolean;
  onAdd: () => void;
  setSelectedQuestionId: (questionId: string | null) => void;
  updateQuestionStatus: (questionId: string, status: ResearchQuestionStatus) => void;
  deleteQuestion: (questionId: string) => void;
  unlinkEvidence: (linkId: string) => void;
  text: (value: string) => string;
};

export function ResearchQuestionsPanel({
  questions,
  selectedQuestion,
  selectedQuestionId,
  questionLinks,
  evidenceTitleByKey,
  canAdd,
  questionInFlight,
  onAdd,
  setSelectedQuestionId,
  updateQuestionStatus,
  deleteQuestion,
  unlinkEvidence,
  text,
}: ResearchQuestionsPanelProps) {
  // Cascading (ADR 0076 D5): deleting a question drops its evidence links, so
  // confirm in place rather than via a native dialog.
  const [confirmDeleteQuestionId, setConfirmDeleteQuestionId] = useState<string | null>(null);
  // Deleting a question row moves focus to the next row's select control rather
  // than dropping it on <body> (ADR 0076 D9). The row div is not focusable, so
  // land on its primary select button.
  const { listRef } = useFocusAfterRemove<HTMLDivElement>(
    questions.map((question) => question.id),
    { rowSelector: ".research-question-row", focusSelector: ".research-question-row-main" },
  );
  const addQuestionButton = (
    <ActionButton className="compact-button" disabled={!canAdd} onClick={onAdd} verb="add">
      <Plus size={15} />
      {text("Add question")}
    </ActionButton>
  );

  return (
    <div role="group" className="research-questions" aria-label={text("Research questions")}>
      <div className="research-question-strip">
        <SectionHeader
          className="research-section-questions"
          description={text("Open questions you are actively tracking.")}
          meta={<Figure value={questions.length} />}
          title={text("Research questions")}
          variant="accent"
        />
        {questions.length > 0 ? addQuestionButton : null}
      </div>
      {/* A DELIBERATE horizontal card strip (grid-auto-flow: column) — it scrolls
          inside its own bounded container; data-hscroll exempts it from the
          panel-overflow layout gate. */}
      <div className="research-question-card-list" data-hscroll ref={listRef}>
        {questions.map((question) => (
          <div
            className={question.id === selectedQuestionId ? "research-question-row selected" : "research-question-row"}
            key={question.id}
          >
            <ActionButton
              className="research-question-row-main"
              kind="control"
              onClick={() => setSelectedQuestionId(question.id)}
            >
              <strong>{question.title}</strong>
              <span>{text(formatQuestionStatus(question.status))}</span>
            </ActionButton>
            {confirmDeleteQuestionId === question.id ? (
              <InlineConfirm
                verb="remove"
                cancelLabel={text("Cancel")}
                confirmLabel={text("Remove")}
                disabled={questionInFlight}
                onCancel={() => setConfirmDeleteQuestionId(null)}
                onConfirm={() => {
                  setConfirmDeleteQuestionId(null);
                  deleteQuestion(question.id);
                }}
              >
                {text("Remove question")}
              </InlineConfirm>
            ) : (
              <ActionButton
                aria-label={`${text("Remove question")}: ${question.title}`}
                className="compact-button"
                disabled={questionInFlight}
                onClick={() => setConfirmDeleteQuestionId(question.id)}
                verb="remove"
              >
                <Trash2 size={14} />
                {text("Remove question")}
              </ActionButton>
            )}
          </div>
        ))}
        {questions.length === 0 ? (
          <EmptyState
            kind="invitation"
            title={text("No research questions yet.")}
            source={text("Questions capture what you still need to find out about this company.")}
            action={addQuestionButton}
          />
        ) : null}
      </div>
      {selectedQuestion ? (
        <div className="research-active-question">
          <div className="research-active-question-main">
            <span>{text("Active question")}</span>
            <strong>{selectedQuestion.title}</strong>
            {selectedQuestion.body ? <p>{selectedQuestion.body}</p> : null}
            <div className="research-linked-evidence">
              <span>{text("Linked evidence")}</span>
              {questionLinks.map((link) => {
                const evidenceType = link.fromType === "research_question" ? link.toType : link.fromType;
                const evidenceId = link.fromType === "research_question" ? link.toId : link.fromId;
                // The evidence's own title (sol fix1 item 6) — falls back to
                // its type only when the item isn't in the current timeline
                // scope (should not happen in practice; never blank a name).
                const linkTitle =
                  evidenceTitleByKey.get(`${evidenceType}:${evidenceId}`) ?? text(formatEvidenceType(evidenceType));
                return (
                  <ActionButton
                    aria-label={`${text("Remove")}: ${linkTitle}`}
                    key={link.id}
                    onClick={() => unlinkEvidence(link.id)}
                    verb="remove"
                  >
                    <X size={13} />
                    {text("Remove")}
                  </ActionButton>
                );
              })}
              {questionLinks.length === 0 ? (
                <EmptyState kind="quiet" reason={text("No linked evidence yet.")} />
              ) : null}
            </div>
          </div>
          <div className="research-question-actions">
            <ActionButton
              className="compact-button"
              disabled={questionInFlight || selectedQuestion.status === "answered"}
              onClick={() => updateQuestionStatus(selectedQuestion.id, "answered")}
              verb="markAs"
            >
              {text("Mark as answered")}
            </ActionButton>
            <ActionButton
              className="compact-button"
              disabled={questionInFlight || selectedQuestion.status === "closed"}
              onClick={() => updateQuestionStatus(selectedQuestion.id, "closed")}
              verb="markAs"
            >
              {text("Mark as closed")}
            </ActionButton>
            <ActionButton
              className="compact-button"
              disabled={questionInFlight || selectedQuestion.status === "open"}
              onClick={() => updateQuestionStatus(selectedQuestion.id, "open")}
              verb="resume"
            >
              {text("Reopen")}
            </ActionButton>
            <ActionButton className="compact-button" kind="control" onClick={() => setSelectedQuestionId(null)}>
              {text("Clear selection")}
            </ActionButton>
          </div>
        </div>
      ) : null}
    </div>
  );
}
