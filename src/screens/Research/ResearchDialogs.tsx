import { X } from "lucide-react";
import { ActionButton, TextField, TextareaField } from "../../ui";

type AddQuestionDialogProps = {
  questionTitle: string;
  questionBody: string;
  questionInFlight: boolean;
  setQuestionTitle: (title: string) => void;
  setQuestionBody: (body: string) => void;
  onCancel: () => void;
  onSubmit: () => void;
  text: (value: string) => string;
};

export function AddQuestionDialog({
  questionTitle,
  questionBody,
  questionInFlight,
  setQuestionTitle,
  setQuestionBody,
  onCancel,
  onSubmit,
  text,
}: AddQuestionDialogProps) {
  return (
    <div className="research-dialog-backdrop" role="presentation">
      <form
        aria-label={text("Add research question")}
        className="research-question-dialog"
        role="dialog"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
      >
        <div className="research-dialog-header">
          <h2>{text("Add research question")}</h2>
          <ActionButton
            aria-label={text("Close")}
            className="research-question-delete"
            kind="control"
            onClick={onCancel}
          >
            <X size={14} />
          </ActionButton>
        </div>
        <TextField
          label={<span>{text("Question title")}</span>}
          aria-label={text("Question title")}
          placeholder={text("Question title")}
          value={questionTitle}
          onChange={(event) => setQuestionTitle(event.target.value)}
        />
        <TextareaField
          label={<span>{text("Question context")}</span>}
          aria-label={text("Question context")}
          placeholder={text("Question context")}
          value={questionBody}
          onChange={(event) => setQuestionBody(event.target.value)}
        />
        <div className="research-dialog-actions">
          <ActionButton className="compact-button" kind="control" type="button" onClick={onCancel}>
            {text("Cancel")}
          </ActionButton>
          <ActionButton
            className="compact-button"
            disabled={questionInFlight || !questionTitle.trim()}
            type="submit"
            verb="save"
          >
            {text("Save question")}
          </ActionButton>
        </div>
      </form>
    </div>
  );
}

type AddReminderDialogProps = {
  reminderTitle: string;
  reminderBody: string;
  reminderDueAt: string;
  reminderInFlight: boolean;
  setReminderTitle: (title: string) => void;
  setReminderBody: (body: string) => void;
  setReminderDueAt: (dueAt: string) => void;
  onCancel: () => void;
  onSubmit: () => void;
  text: (value: string) => string;
};

export function AddReminderDialog({
  reminderTitle,
  reminderBody,
  reminderDueAt,
  reminderInFlight,
  setReminderTitle,
  setReminderBody,
  setReminderDueAt,
  onCancel,
  onSubmit,
  text,
}: AddReminderDialogProps) {
  return (
    <div className="research-dialog-backdrop" role="presentation">
      <form
        aria-label={text("Add research reminder")}
        className="research-question-dialog"
        role="dialog"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
      >
        <div className="research-dialog-header">
          <h2>{text("Add research reminder")}</h2>
          <ActionButton
            aria-label={text("Close")}
            className="research-question-delete"
            kind="control"
            onClick={onCancel}
          >
            <X size={14} />
          </ActionButton>
        </div>
        <TextField
          label={<span>{text("Reminder title")}</span>}
          aria-label={text("Reminder title")}
          placeholder={text("Reminder title")}
          value={reminderTitle}
          onChange={(event) => setReminderTitle(event.target.value)}
        />
        <TextareaField
          label={<span>{text("Reminder notes")}</span>}
          aria-label={text("Reminder notes")}
          placeholder={text("Reminder notes")}
          value={reminderBody}
          onChange={(event) => setReminderBody(event.target.value)}
        />
        <TextField
          label={<span>{text("Due date")}</span>}
          aria-label={text("Due date")}
          type="datetime-local"
          value={reminderDueAt}
          onChange={(event) => setReminderDueAt(event.target.value)}
        />
        <div className="research-dialog-actions">
          <ActionButton className="compact-button" kind="control" type="button" onClick={onCancel}>
            {text("Cancel")}
          </ActionButton>
          <ActionButton
            className="compact-button"
            disabled={reminderInFlight || !reminderTitle.trim()}
            type="submit"
            verb="save"
          >
            {text("Save reminder")}
          </ActionButton>
        </div>
      </form>
    </div>
  );
}
