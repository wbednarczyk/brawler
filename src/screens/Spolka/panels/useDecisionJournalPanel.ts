import { useCallback, useEffect, useState } from "react";
import type { Company } from "../../../api/types";
import type { ResearchEvidenceItem } from "../../../api/researchTypes";
import * as decisionJournalApi from "../../../api/decisionJournal";
import type { DecisionEntry } from "../../../api/decisionJournal";
import * as researchApi from "../../../api/research";
import { listCompanyTimeline } from "../../../api/research";
import { formatLocalDate } from "../../../shared/format/datetime";

// The composer draft. `kind` is one of the four recorded judgments (ADR 0071);
// `decidedAt` is the decision's own date (the journal's chronology), distinct
// from the row's created_at. Immutable-once-saved: there is no edit form.
export type DecisionEntryForm = {
  kind: string;
  rationaleMd: string;
  decidedAt: string;
};

export const DECISION_KINDS = ["buy", "pass", "keep_watching", "sell_note"] as const;

function emptyForm(): DecisionEntryForm {
  return { kind: "buy", rationaleMd: "", decidedAt: formatLocalDate(new Date()) };
}

// Cockpit-native decision-journal state for one company (ADR 0071 / J3). Owns the
// entry list, the composer draft, the selected entry, and its evidence links via
// `api/decisionJournal` + the shared evidence-link machinery (`api/research`)
// directly — the company-scoped surface with no cross-screen coupling. Mirrors
// `useCompanyNotebookPanel` so the dashboard `decisionJournal` panel works for
// any company. Entries are IMMUTABLE: the only correction is a follow-up entry
// created via `startSupersede` (linked with `supersededByEntryId`).
export function useDecisionJournalPanel(company: Company) {
  const [entries, setEntries] = useState<DecisionEntry[]>([]);
  const [isComposerOpen, setComposerOpen] = useState(false);
  const [form, setForm] = useState<DecisionEntryForm>(emptyForm);
  // The entry a pending draft supersedes (the "Supersede" flow); null for a fresh
  // entry. Carried onto the created row as `supersededByEntryId`.
  const [supersedingEntryId, setSupersedingEntryId] = useState<string | null>(null);
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null);
  const [evidenceCandidates, setEvidenceCandidates] = useState<ResearchEvidenceItem[]>([]);
  const [linkedEvidenceKeys, setLinkedEvidenceKeys] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    decisionJournalApi
      .listDecisionEntries({ companyId: company.id })
      .then(setEntries)
      .catch((reason) => setError(String(reason)));
  }, [company.id]);

  useEffect(() => {
    setSelectedEntryId(null);
    setComposerOpen(false);
    setSupersedingEntryId(null);
    setForm(emptyForm());
    refresh();
  }, [company.id, refresh]);

  const selectedEntry = entries.find((entry) => entry.id === selectedEntryId) ?? null;
  const supersedingEntry = entries.find((entry) => entry.id === supersedingEntryId) ?? null;

  // Evidence candidates + already-linked keys track the selected entry (the
  // linking surface). The company timeline is the candidate pool; existing links
  // resolve to the "other" endpoint key so an item can't be linked twice.
  const refreshEvidence = useCallback(() => {
    if (!selectedEntryId) {
      setEvidenceCandidates([]);
      setLinkedEvidenceKeys(new Set());
      return;
    }
    listCompanyTimeline(company.id)
      .then((result) => setEvidenceCandidates(result.items))
      .catch((reason) => setError(String(reason)));
    researchApi
      .listEvidenceLinks({ endpointType: "decision_entry", endpointId: selectedEntryId })
      .then((links) => {
        const keys = new Set<string>();
        for (const link of links) {
          if (link.fromType === "decision_entry" && link.fromId === selectedEntryId) {
            keys.add(`${link.toType}:${link.toId}`);
          } else if (link.toType === "decision_entry" && link.toId === selectedEntryId) {
            keys.add(`${link.fromType}:${link.fromId}`);
          }
        }
        setLinkedEvidenceKeys(keys);
      })
      .catch((reason) => setError(String(reason)));
  }, [company.id, selectedEntryId]);

  useEffect(() => {
    refreshEvidence();
  }, [refreshEvidence]);

  function updateForm(field: keyof DecisionEntryForm, value: string) {
    setForm((current) => ({ ...current, [field]: value }));
  }

  function createEntry(event?: { preventDefault: () => void }) {
    event?.preventDefault();
    decisionJournalApi
      .createDecisionEntry({
        companyId: company.id,
        kind: form.kind,
        rationaleMd: form.rationaleMd,
        decidedAt: form.decidedAt,
        ...(supersedingEntryId ? { supersededByEntryId: supersedingEntryId } : {}),
      })
      .then((created) => {
        setForm(emptyForm());
        setComposerOpen(false);
        setSupersedingEntryId(null);
        setSelectedEntryId(created.id);
        setError(null);
        refresh();
      })
      .catch((reason) => setError(String(reason)));
  }

  // Open the composer to record a follow-up that supersedes `entry` (the only
  // "correction" affordance — entries themselves never change).
  function startSupersede(entry: DecisionEntry) {
    setSupersedingEntryId(entry.id);
    setForm({ kind: entry.kind, rationaleMd: "", decidedAt: formatLocalDate(new Date()) });
    setComposerOpen(true);
  }

  function cancelSupersede() {
    setSupersedingEntryId(null);
    setForm(emptyForm());
    setComposerOpen(false);
  }

  function linkEvidence(item: ResearchEvidenceItem) {
    if (!selectedEntryId) return;
    researchApi
      .createEvidenceLink({
        fromType: "decision_entry",
        fromId: selectedEntryId,
        toType: item.evidenceType,
        toId: item.sourceId,
        relationType: "cites",
      })
      .then(() => {
        setError(null);
        refreshEvidence();
      })
      .catch((reason) => setError(String(reason)));
  }

  return {
    company,
    entries,
    isComposerOpen,
    setComposerOpen,
    form,
    updateForm,
    createEntry,
    supersedingEntry,
    startSupersede,
    cancelSupersede,
    selectedEntry,
    setSelectedEntryId,
    evidenceCandidates,
    linkedEvidenceKeys,
    linkEvidence,
    error,
  };
}
