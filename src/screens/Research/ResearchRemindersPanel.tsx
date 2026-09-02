import { Check, Clock3, Plus, RotateCcw, Trash2 } from "lucide-react";
import type { ResearchReminder } from "../../api/researchTypes";
import { useFocusAfterRemove } from "../../shared/focus/focusAfterRemove";
import { ActionButton, ActionRow, EmptyState, Figure, SectionHeader } from "../../ui";
import { formatReminderKind, formatReminderStatus } from "./researchFormatters";

type ResearchRemindersPanelProps = {
  reminders: ResearchReminder[];
  canAdd: boolean;
  reminderInFlight: boolean;
  onAdd: () => void;
  completeReminder: (reminderId: string) => void;
  snoozeReminder: (reminderId: string) => void;
  reopenReminder: (reminderId: string) => void;
  deleteReminder: (reminderId: string) => void;
  text: (value: string) => string;
};

export function ResearchRemindersPanel({
  reminders,
  canAdd,
  reminderInFlight,
  onAdd,
  completeReminder,
  snoozeReminder,
  reopenReminder,
  deleteReminder,
  text,
}: ResearchRemindersPanelProps) {
  const visibleReminders = reminders.slice(0, 6);
  // After deleting a reminder, keep focus in the queue by landing on the next
  // row's leading action button (ADR 0076 D9); the row article is not focusable.
  const { listRef } = useFocusAfterRemove<HTMLDivElement>(
    visibleReminders.map((reminder) => reminder.id),
    { rowSelector: ".research-reminder-row", focusSelector: ".compact-button" },
  );
  const addReminderButton = (
    <ActionButton className="compact-button" disabled={reminderInFlight || !canAdd} onClick={onAdd} verb="add">
      <Plus size={14} />
      {text("Add reminder")}
    </ActionButton>
  );

  return (
    <div role="group" className="research-reminders" aria-label={text("Research reminders")}>
      <SectionHeader
        actions={reminders.length > 0 ? addReminderButton : undefined}
        className="research-section-review"
        description={text("Items that need a concrete follow-up action.")}
        meta={<Figure value={reminders.length} />}
        title={text("Review queue")}
        variant="accent"
      />
      <div className="research-reminder-list" ref={listRef}>
        {visibleReminders.map((reminder) => (
          <article className="research-reminder-row" key={reminder.id}>
            <div>
              <span>{text(formatReminderKind(reminder.reminderKind))}</span>
              <strong>{reminder.title}</strong>
              {reminder.dueAt ? (
                <time dateTime={reminder.dueAt}>
                  <Figure kind="datetime" value={reminder.dueAt} />
                </time>
              ) : null}
              {reminder.status !== "open" ? <em>{text(formatReminderStatus(reminder.status))}</em> : null}
            </div>
            <ActionRow className="research-reminder-actions">
              {reminder.status === "open" ? (
                <>
                  <ActionButton
                    aria-label={`${text("Mark as done")}: ${reminder.title}`}
                    className="compact-button"
                    disabled={reminderInFlight}
                    onClick={() => completeReminder(reminder.id)}
                    verb="markAs"
                  >
                    <Check size={15} />
                    {text("Mark as done")}
                  </ActionButton>
                  <ActionButton
                    aria-label={`${text("Snooze")}: ${reminder.title}`}
                    className="compact-button"
                    disabled={reminderInFlight}
                    onClick={() => snoozeReminder(reminder.id)}
                    verb="snooze"
                  >
                    <Clock3 size={15} />
                    {text("Snooze")}
                  </ActionButton>
                </>
              ) : (
                <ActionButton
                  aria-label={`${text("Reopen")}: ${reminder.title}`}
                  className="compact-button"
                  disabled={reminderInFlight}
                  onClick={() => reopenReminder(reminder.id)}
                  verb="resume"
                >
                  <RotateCcw size={15} />
                  {text("Reopen")}
                </ActionButton>
              )}
              <ActionButton
                aria-label={`${text("Remove reminder")}: ${reminder.title}`}
                className="compact-button"
                disabled={reminderInFlight}
                onClick={() => deleteReminder(reminder.id)}
                verb="remove"
              >
                <Trash2 size={15} />
                {text("Remove reminder")}
              </ActionButton>
            </ActionRow>
          </article>
        ))}
        {reminders.length === 0 ? (
          <EmptyState
            kind="invitation"
            title={text("No research reminders.")}
            source={text("Reminders track follow-ups on claims, questions, and reviews.")}
            action={addReminderButton}
          />
        ) : null}
      </div>
    </div>
  );
}
