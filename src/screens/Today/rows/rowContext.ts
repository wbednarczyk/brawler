import type { CompanyWorkspaceTab } from "../../Companies/companyTypes";
import type { Company, FeedItem } from "../../../api/types";
import type { ReportSeasonEntry } from "../../../api/generated/ReportSeasonEntry";
import type { AlertRule, AttentionEvent } from "../../../api/attention";
import type { AutopilotRun } from "../../../api/autopilot";
import type { LocaleCode } from "../../../shared/locale";
import type { PulseClaim } from "../useTodayPulse";

/**
 * The per-category render payloads carried by a `StreamItem` (ADR 0087). The
 * discriminant `kind` mirrors the item's `StreamCategory`, so a renderer narrows
 * on it to reach the strongly-typed underlying object.
 */
export type StreamPayload =
  | { kind: "autopilot"; run: AutopilotRun }
  | { kind: "attention"; event: AttentionEvent }
  | { kind: "verify"; claim: PulseClaim }
  | { kind: "changed"; item: FeedItem }
  | { kind: "upcoming"; entry: ReportSeasonEntry };

/** The autopilot-run controls a row/member needs (from `useTodayPulse`). */
export type AutopilotControls = {
  dismissAutopilotRun: (runId: string) => void;
  undoAutopilotRun: (runId: string) => Promise<void>;
  undoneAutopilotRuns: Record<string, number>;
};

/** The attention-event controls a row needs (from `useTodayPulse`). */
export type AttentionControls = {
  attentionRulesById: Map<string, AlertRule>;
  dismissAttentionEventRow: (id: string) => void;
  markAttentionEventSeenRow: (id: string) => void;
};

/** Everything a category row/member needs to render and act, passed once. */
export type RowContext = {
  text: (key: string) => string;
  locale: LocaleCode;
  companyById: Map<string, Company>;
  companyByTicker: Map<string, Company>;
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
  openInbox: () => void;
  autopilot: AutopilotControls;
  attention: AttentionControls;
};
