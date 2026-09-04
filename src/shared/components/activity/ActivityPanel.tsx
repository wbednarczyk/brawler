import { useState, type RefObject } from "react";

import type { ActivityItem } from "../../../api/generated/ActivityItem";
import type { ActivityTarget } from "../../../api/generated/ActivityTarget";
import type { ActivityView } from "../../../api/generated/ActivityView";
import { ActionButton, EmptyState, ErrorText, ExpandableRow, Figure, Modal, SectionHeader, Skeleton, StatusChip } from "../../../ui";
import { useLocale, type LocaleCode } from "../../locale";
import { formatListTimestamp } from "../../format/datetime";
import { ACTIVE_FORMS, FAILED_FORMS, TASK_FORMS, pluralNoun } from "../../locale/plural";
import { TickerLabel } from "../TickerLabel";
import { familyLabel, statusLabel } from "./activityLabels";

export type ActivityPanelProps = {
  open: boolean;
  onClose: () => void;
  view: ActivityView | null;
  hydrated: boolean;
  error: string | null;
  onRetry: () => void;
  onNavigate: (target: ActivityTarget) => void;
  initialFocusRef?: RefObject<HTMLElement | null>;
};

// Subject is a document reference for these families (D1 table) — but a
// document's SUBJECT can be a filename ("Raport bieżący Q2 2026.pdf") or a
// human title ("Poranny przegląd"); only the former is mono (ADR 0104 dec.
// 2/6, sol diff R1 #15) — a human title renders in the ordinary UI face.
const DOCUMENT_SUBJECT_FAMILIES = new Set(["reportReading", "ownershipReading", "managementReading", "kpiIngest"]);
const FILENAME_PATTERN = /\.[a-z0-9]{2,5}$/i;

function isDocumentSubjectMono(item: ActivityItem): boolean {
  return DOCUMENT_SUBJECT_FAMILIES.has(item.family) && FILENAME_PATTERN.test(item.subject);
}

function toneForStatus(status: ActivityItem["status"]): "neutral" | "accent" | "ok" | "warn" | "danger" {
  switch (status) {
    case "queued":
      return "neutral";
    case "running":
      return "accent";
    case "stalled":
      return "warn";
    case "succeeded":
      return "ok";
    case "failed":
      return "danger";
    case "partial":
      return "warn";
    case "interrupted":
      return "danger";
  }
}

function destinationLabel(target: ActivityTarget, text: (value: string) => string): string {
  switch (target.kind) {
    case "company":
      return target.tool?.t === "dokumenty" ? text("Open document") : text("Open company");
    case "sources":
      return text("Open sources");
    case "today":
      return text("Open Today");
    case "transcripts":
      return text("Open transcripts");
  }
}

function latestFinishedAt(items: ActivityItem[]): string | null {
  return items.reduce<string | null>(
    (latest, item) => (item.finishedAt && (!latest || item.finishedAt > latest) ? item.finishedAt : latest),
    null,
  );
}

type Group = {
  key: string;
  qualifiedTicker: string | null;
  items: ActivityItem[];
};

// Grouped per company (`TickerLabel`), non-company items last under "Sources
// and system" — presentation only, never identity (ADR 0109 dec. 1). Group
// order follows first appearance in `items`; the system group always renders
// last, matching the approved storyboard.
function groupByCompany(items: ActivityItem[]): Group[] {
  const byCompany = new Map<string, Group>();
  const system: Group = { key: "__system__", qualifiedTicker: null, items: [] };
  for (const item of items) {
    if (item.companyId && item.qualifiedTicker) {
      let group = byCompany.get(item.companyId);
      if (!group) {
        group = { key: item.companyId, qualifiedTicker: item.qualifiedTicker, items: [] };
        byCompany.set(item.companyId, group);
      }
      group.items.push(item);
    } else {
      system.items.push(item);
    }
  }
  const groups = [...byCompany.values()];
  if (system.items.length > 0) groups.push(system);
  return groups;
}

// A company-keyed task (sweep, history fetch, company refresh, price history)
// carries the ticker as its raw subject; under the company group heading that
// would read twice, so the row shows the family alone (ADR 0104 dec. 5:
// detail never repeats what the heading already said).
function subjectRepeatsTicker(item: ActivityItem): boolean {
  const ticker = item.qualifiedTicker?.split(":").pop();
  return Boolean(ticker) && item.subject === ticker;
}

function ActivityRow({
  item,
  expanded,
  onToggle,
  onNavigate,
  locale,
  text,
}: {
  item: ActivityItem;
  expanded: boolean;
  onToggle: () => void;
  onNavigate: (target: ActivityTarget) => void;
  locale: LocaleCode;
  text: (value: string) => string;
}) {
  // A queued queue row carries its ORIGINAL enqueue time as `startedAt` (a
  // recurring id re-armed months later still says July) — showing it would
  // read as "waiting since July". Queued rows show no time; running rows show
  // when they started, terminal rows when they finished (live-drive 2026-09-04).
  const timestamp = item.status === "queued" ? null : (item.finishedAt ?? item.startedAt);
  const isDocumentSubject = isDocumentSubjectMono(item);
  const target = item.target;
  // sol diff R2 finding 8: the live harness needs the row's declared
  // target precise enough to tell Coverage/Overview/Documents apart and
  // catch a wrong document ID, not just "some company view opened".
  const documentId = target.kind === "company" && target.tool?.t === "dokumenty" ? target.tool.documentId : null;

  return (
    <div
      className="activity-item"
      data-activity-target={target.kind}
      data-activity-tool={target.kind === "company" ? (target.tool?.t ?? "overview") : ""}
      data-activity-document={documentId ?? ""}
    >
      <ExpandableRow
        label={`${familyLabel(item.family, text)} ${item.subject} ${statusLabel(item.status, text)}`.trim()}
        isExpanded={expanded}
        onToggle={onToggle}
        actions={
          <ActionButton kind="destination" onClick={() => onNavigate(item.target)}>
            {destinationLabel(item.target, text)}
          </ActionButton>
        }
        detail={
          <div className="activity-detail">
            {item.error ? <ErrorText as="span">{item.error}</ErrorText> : null}
            {item.progress && item.progress.failed > 0 ? (
              <span className="activity-detail-failed">
                <Figure kind="count" value={item.progress.failed} /> {pluralNoun(locale, item.progress.failed, FAILED_FORMS)}
              </span>
            ) : null}
            {item.members.length > 0 ? (
              <ul className="activity-detail-members">
                {item.members.map((member, index) => (
                  <li
                    key={`${item.id}-member-${index}`}
                    className={FILENAME_PATTERN.test(member) ? "activity-subject-mono" : undefined}
                  >
                    {member}
                  </li>
                ))}
              </ul>
            ) : null}
            {item.attempt > 1 ? (
              <span className="activity-detail-attempt">
                {text("Attempt")} <Figure kind="count" value={item.attempt} />
              </span>
            ) : null}
          </div>
        }
      >
        <span className="activity-line">
        <span className="activity-family">{familyLabel(item.family, text)}</span>
        {item.subject && !subjectRepeatsTicker(item) ? (
          <span className={isDocumentSubject ? "activity-subject activity-subject-mono" : "activity-subject"}>
            {item.subject}
          </span>
        ) : null}
        <StatusChip tone={toneForStatus(item.status)}>{statusLabel(item.status, text)}</StatusChip>
        {timestamp ? <Figure kind="datetime" value={timestamp} /> : null}
        {item.progress ? (
          <span className="activity-progress">
            <Figure kind="count" value={item.progress.done} />/<Figure kind="count" value={item.progress.total} />
          </span>
        ) : null}
        {item.inFlight !== null ? (
          <span className="activity-inflight">
            <Figure kind="count" value={item.inFlight} /> {text("in flight")}
          </span>
        ) : null}
        </span>
      </ExpandableRow>
    </div>
  );
}

function ActivityGroups({
  items,
  expandedIds,
  toggle,
  onNavigate,
  locale,
  text,
}: {
  items: ActivityItem[];
  expandedIds: Set<string>;
  toggle: (id: string) => void;
  onNavigate: (target: ActivityTarget) => void;
  locale: LocaleCode;
  text: (value: string) => string;
}) {
  return (
    <>
      {groupByCompany(items).map((group) => (
        <div className="activity-group" key={group.key}>
          <div className="activity-group-heading">
            {group.qualifiedTicker ? (
              <TickerLabel value={group.qualifiedTicker} />
            ) : (
              <span>{text("Sources and system")}</span>
            )}
          </div>
          {group.items.map((item) => (
            <ActivityRow
              key={item.id}
              item={item}
              expanded={expandedIds.has(item.id)}
              onToggle={() => toggle(item.id)}
              onNavigate={onNavigate}
              locale={locale}
              text={text}
            />
          ))}
        </div>
      ))}
    </>
  );
}

export function ActivityPanel({
  open,
  onClose,
  view,
  hydrated,
  error,
  onRetry,
  onNavigate,
  initialFocusRef,
}: ActivityPanelProps) {
  const { locale, text } = useLocale();
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  function toggle(id: string) {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function handleNavigate(target: ActivityTarget) {
    onNavigate(target);
    onClose();
  }

  const activeCount = view?.active.length ?? 0;
  const queuedCount = view?.queued.length ?? 0;
  const inProgress = view ? [...view.active, ...view.queued] : [];
  const recent = view?.recent ?? [];
  const isEmpty = hydrated && view !== null && inProgress.length === 0 && recent.length === 0;
  const lastFinishedAt = latestFinishedAt(recent);

  const emptyReason = lastFinishedAt
    ? `${text("Nothing is running in the background.")} ${text("Last finished")}: ${formatListTimestamp(lastFinishedAt, locale)}.`
    : text("Nothing is running in the background.");

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={text("Activity")}
      ariaLabel={text("Activity")}
      initialFocusRef={initialFocusRef}
      className="activity-modal"
    >
      <div className="activity-panel">
        {error ? (
          <div className="activity-error-strip">
            <ErrorText as="span">
              {text("Could not refresh the list. Showing the state from {time}.").replace(
                "{time}",
                view ? formatListTimestamp(view.generatedAt, locale) : "",
              )}
            </ErrorText>
            <ActionButton kind="control" onClick={onRetry}>
              {text("Try again")}
            </ActionButton>
          </div>
        ) : null}

        {!hydrated ? (
          <Skeleton variant="list-row" count={3} />
        ) : isEmpty ? (
          <EmptyState kind="quiet" reason={emptyReason} />
        ) : (
          <>
            <SectionHeader
              level="h3"
              title={text("In progress")}
              // "active"/"queued" are adjectives (agree with the implicit noun
              // "zadania"/tasks) — pluralNoun declines "active" (aktywne/aktywnych);
              // "queued" stays invariant ("w kolejce" needs no declension,
              // sol diff R1 #15).
              meta={`${activeCount} ${pluralNoun(locale, activeCount, ACTIVE_FORMS)} · ${queuedCount} ${text("queued")}`}
            />
            <ActivityGroups
              items={inProgress}
              expandedIds={expandedIds}
              toggle={toggle}
              onNavigate={handleNavigate}
              locale={locale}
              text={text}
            />
            <SectionHeader
              level="h3"
              title={text("Recent")}
              meta={`${text("7 days")} · ${recent.length} ${pluralNoun(locale, recent.length, TASK_FORMS)}`}
            />
            <ActivityGroups
              items={recent}
              expandedIds={expandedIds}
              toggle={toggle}
              onNavigate={handleNavigate}
              locale={locale}
              text={text}
            />
          </>
        )}
      </div>
    </Modal>
  );
}
