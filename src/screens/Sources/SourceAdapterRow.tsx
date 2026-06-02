import { CheckCircle2, ChevronDown, ExternalLink, Plus, RefreshCw, Search } from "lucide-react";
import type { CompanyRegistryEntry, SourceAdapter, SourceRefreshTrigger, UnmatchedSourceItem } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { StatusPill } from "../../shared/components/StatusPill";
import {
  formatSourceAccess,
  formatSourceLastResult,
  formatSourceSubtitle,
  formatSourceTrigger,
  sourceLastResultLabel,
  sourcePolicyLabel,
} from "./sourceHelpers";

type SourceAdapterRowProps = {
  adapter: SourceAdapter;
  addingRegistryTicker: string | null;
  companyRegistryEntries: CompanyRegistryEntry[];
  companyRegistryEntriesError: string | null;
  companyRegistrySearch: string;
  expandedUnmatchedAdapters: Record<string, boolean>;
  filteredCompanyRegistryEntries: CompanyRegistryEntry[];
  gpwRegistryAdapterId: string;
  isCompanyRegistryListExpanded: boolean;
  registryRefreshError: string | null;
  registryRefreshResult: { entriesFetched: number; entriesUpserted: number } | null;
  registryRefreshState: string;
  selected: boolean;
  sourceAdapterRefreshInFlight: string | null;
  sourceRefreshError: string | null;
  sourceRefreshState: string;
  unmatchedSourceItems: Record<string, UnmatchedSourceItem[]>;
  addCompanyFromRegistry: (entry: CompanyRegistryEntry) => void;
  formatNextRefresh: (adapter: SourceAdapter) => string;
  formatSourceScheduler: (adapter: SourceAdapter) => string;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  openExternalUrl: (url: string) => void;
  refreshCompanyRegistry: (trigger: SourceRefreshTrigger) => void;
  refreshSingleSource: (adapter: SourceAdapter, trigger: SourceRefreshTrigger) => void;
  setCompanyRegistrySearch: (value: string) => void;
  toggleCompanyRegistryList: () => void;
  toggleSourceAdapter: (adapterId: string) => void;
  toggleSourceAdapterFromKeyboard: React.KeyboardEventHandler<HTMLElement>;
  toggleUnmatchedSourceItems: (adapterId: string) => void;
};

export function SourceAdapterRow({
  adapter,
  addingRegistryTicker,
  companyRegistryEntries,
  companyRegistryEntriesError,
  companyRegistrySearch,
  expandedUnmatchedAdapters,
  filteredCompanyRegistryEntries,
  gpwRegistryAdapterId,
  isCompanyRegistryListExpanded,
  registryRefreshError,
  registryRefreshResult,
  registryRefreshState,
  selected,
  sourceAdapterRefreshInFlight,
  sourceRefreshError,
  sourceRefreshState,
  unmatchedSourceItems,
  addCompanyFromRegistry,
  formatNextRefresh,
  formatSourceScheduler,
  formatTimestamp,
  openExternalUrl,
  refreshCompanyRegistry,
  refreshSingleSource,
  setCompanyRegistrySearch,
  toggleCompanyRegistryList,
  toggleSourceAdapter,
  toggleSourceAdapterFromKeyboard,
  toggleUnmatchedSourceItems,
}: SourceAdapterRowProps) {
  return (
    <div className="source-row-block">
      <article
        aria-label={`Open source adapter: ${adapter.displayName}`}
        className={["source-row", selected ? "source-row-selected" : ""]
          .filter(Boolean)
          .join(" ")}
        onClick={() => toggleSourceAdapter(adapter.id)}
        onKeyDown={toggleSourceAdapterFromKeyboard}
        role="button"
        tabIndex={0}
        title={`Open ${adapter.displayName} details`}
      >
        <div className="source-row-main">
          <div className="source-title-line">
            <span
              className={adapter.enabled ? "status-dot status-ok" : "status-dot status-warn"}
              title={adapter.enabled ? "Enabled" : "Disabled"}
            />
            <h2>{adapter.displayName}</h2>
            <span className="source-id">{adapter.id}</span>
          </div>
          <p>{formatSourceSubtitle(adapter)}</p>
          <div className="source-chip-list" aria-label={`Markets for ${adapter.displayName}`}>
            {adapter.markets.map((market) => (
              <StatusPill key={market}>{market}</StatusPill>
            ))}
            {adapter.markets.length === 0 ? (
              <span className="membership-empty">No markets</span>
            ) : null}
          </div>
        </div>
        <div className="source-row-status">
          <span>{adapter.lastError ?? (adapter.enabled ? "Ready" : "Disabled")}</span>
        </div>
      </article>
      {selected ? (
        <div className="source-detail-panel" aria-label="Source adapter details">
          <div className="source-detail-actions">
            <Button
              className="compact-button"
              disabled={
                !adapter.enabled ||
                sourceRefreshState === "refreshing" ||
                (adapter.id === gpwRegistryAdapterId
                  ? registryRefreshState === "refreshing"
                  : sourceAdapterRefreshInFlight !== null)
              }
              onClick={() => refreshSingleSource(adapter, "manual")}
              title={adapter.enabled ? `Refresh ${adapter.displayName}` : `${adapter.displayName} is disabled`}
            >
              {adapter.id === gpwRegistryAdapterId && registryRefreshState === "done" ? (
                <CheckCircle2 size={15} />
              ) : (
                <RefreshCw size={15} />
              )}
              {adapter.id === gpwRegistryAdapterId
                ? registryRefreshState === "refreshing"
                  ? "Refreshing"
                  : "Refresh source"
                : sourceAdapterRefreshInFlight === adapter.id
                  ? "Refreshing"
                  : "Refresh source"}
            </Button>
            {sourceRefreshError && sourceAdapterRefreshInFlight === null ? (
              <span className="error-text">Source refresh failed: {sourceRefreshError}</span>
            ) : null}
          </div>
          <dl className="source-status-grid source-status-detail">
            <div>
              <dt>Scheduler</dt>
              <dd>{formatSourceScheduler(adapter)}</dd>
            </div>
            <div>
              <dt>Next poll</dt>
              <dd>{formatNextRefresh(adapter)}</dd>
            </div>
            <div>
              <dt>Last attempt</dt>
              <dd>{formatTimestamp(adapter.lastAttemptAt, "Never")}</dd>
            </div>
            <div>
              <dt>Last trigger</dt>
              <dd>{formatSourceTrigger(adapter)}</dd>
            </div>
            <div>
              <dt>Last success</dt>
              <dd>{formatTimestamp(adapter.lastSuccessAt, "Never")}</dd>
            </div>
            <div>
              <dt>Last error</dt>
              <dd>{formatTimestamp(adapter.lastErrorAt, "None")}</dd>
            </div>
            <div>
              <dt>{sourceLastResultLabel(adapter)}</dt>
              <dd>{formatSourceLastResult(adapter)}</dd>
            </div>
            {adapter.id === gpwRegistryAdapterId ? null : (
              <div>
                <dt>Detail warning</dt>
                <dd>{adapter.lastDetailWarning ?? "None"}</dd>
              </div>
            )}
            <div>
              <dt>Status</dt>
              <dd>{adapter.lastError ?? (adapter.enabled ? "Ready" : "Disabled")}</dd>
            </div>
            <div>
              <dt>Access</dt>
              <dd>{formatSourceAccess(adapter)}</dd>
            </div>
            <div>
              <dt>{sourcePolicyLabel(adapter)}</dt>
              <dd>{adapter.rateLimitPolicy}</dd>
            </div>
            <div>
              <dt>Source page</dt>
              <dd>
                <Button
                  aria-label={`Open source page for ${adapter.displayName}`}
                  className="source-page-link"
                  onClick={() => openExternalUrl(adapter.sourceUrl)}
                  variant="minimal"
                >
                  <ExternalLink size={14} />
                  Open source page
                </Button>
              </dd>
            </div>
            <div>
              <dt>Policy</dt>
              <dd>{adapter.policyNote}</dd>
            </div>
          </dl>
          {adapter.id === gpwRegistryAdapterId ? (
            <RegistrySourcePanel
              addingRegistryTicker={addingRegistryTicker}
              companyRegistryEntries={companyRegistryEntries}
              companyRegistryEntriesError={companyRegistryEntriesError}
              companyRegistrySearch={companyRegistrySearch}
              filteredCompanyRegistryEntries={filteredCompanyRegistryEntries}
              isCompanyRegistryListExpanded={isCompanyRegistryListExpanded}
              registryRefreshError={registryRefreshError}
              registryRefreshResult={registryRefreshResult}
              registryRefreshState={registryRefreshState}
              addCompanyFromRegistry={addCompanyFromRegistry}
              refreshCompanyRegistry={refreshCompanyRegistry}
              setCompanyRegistrySearch={setCompanyRegistrySearch}
              toggleCompanyRegistryList={toggleCompanyRegistryList}
            />
          ) : (
            <UnmatchedSourcePanel
              adapterId={adapter.id}
              expandedUnmatchedAdapters={expandedUnmatchedAdapters}
              unmatchedSourceItems={unmatchedSourceItems}
              formatTimestamp={formatTimestamp}
              toggleUnmatchedSourceItems={toggleUnmatchedSourceItems}
            />
          )}
        </div>
      ) : null}
    </div>
  );
}

type RegistrySourcePanelProps = Pick<
  SourceAdapterRowProps,
  | "addingRegistryTicker"
  | "companyRegistryEntries"
  | "companyRegistryEntriesError"
  | "companyRegistrySearch"
  | "filteredCompanyRegistryEntries"
  | "isCompanyRegistryListExpanded"
  | "registryRefreshError"
  | "registryRefreshResult"
  | "registryRefreshState"
  | "addCompanyFromRegistry"
  | "refreshCompanyRegistry"
  | "setCompanyRegistrySearch"
  | "toggleCompanyRegistryList"
>;

function RegistrySourcePanel({
  addingRegistryTicker,
  companyRegistryEntries,
  companyRegistryEntriesError,
  companyRegistrySearch,
  filteredCompanyRegistryEntries,
  isCompanyRegistryListExpanded,
  registryRefreshError,
  registryRefreshResult,
  registryRefreshState,
  addCompanyFromRegistry,
  refreshCompanyRegistry,
  setCompanyRegistrySearch,
  toggleCompanyRegistryList,
}: RegistrySourcePanelProps) {
  return (
    <>
      <div className="source-registry-actions" aria-label="Company registry refresh">
        <Button
          className="compact-button"
          disabled={registryRefreshState === "refreshing"}
          onClick={() => refreshCompanyRegistry("manual")}
        >
          {registryRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
          {registryRefreshState === "refreshing" ? "Refreshing" : "Refresh registry"}
        </Button>
        {registryRefreshResult ? (
          <span>
            {registryRefreshResult.entriesUpserted}/{registryRefreshResult.entriesFetched} cached
          </span>
        ) : null}
        {registryRefreshError ? (
          <span className="error-text">Registry refresh failed: {registryRefreshError}</span>
        ) : null}
      </div>
      <div className="source-collapsible-panel" aria-label="GPW company registry entries">
        <button
          aria-expanded={isCompanyRegistryListExpanded}
          className="source-collapsible-header"
          onClick={toggleCompanyRegistryList}
          type="button"
        >
          <span>Companies</span>
          <span className="source-collapsible-header-meta">
            <strong>{companyRegistryEntries.length}</strong>
            <ChevronDown className={isCompanyRegistryListExpanded ? "chevron-open" : ""} size={15} />
          </span>
        </button>
        {isCompanyRegistryListExpanded ? (
          <div className="source-registry-list">
            <label className="registry-search-field">
              <Search size={15} />
              <input
                aria-label="Search GPW company registry"
                onChange={(event) => setCompanyRegistrySearch(event.target.value)}
                placeholder="Search ticker, company, ISIN"
                type="search"
                value={companyRegistrySearch}
              />
            </label>
            <span className="source-registry-count">
              {filteredCompanyRegistryEntries.length}/{companyRegistryEntries.length} companies
            </span>
            {filteredCompanyRegistryEntries.map((entry) => (
              <div className="source-registry-row" key={entry.qualifiedTicker}>
                <span>{entry.qualifiedTicker}</span>
                <strong title={entry.displayName}>{entry.displayName}</strong>
                <small>{entry.isin ?? "No ISIN"}</small>
                <Button
                  className="compact-button"
                  disabled={entry.tracked || addingRegistryTicker === entry.qualifiedTicker}
                  onClick={() => addCompanyFromRegistry(entry)}
                  title={entry.tracked ? `${entry.qualifiedTicker} already added` : `Add ${entry.qualifiedTicker}`}
                >
                  {entry.tracked ? <CheckCircle2 size={14} /> : <Plus size={14} />}
                  {entry.tracked ? "Added" : "Add"}
                </Button>
              </div>
            ))}
            {companyRegistryEntries.length === 0 ? (
              <span className="membership-empty">No cached companies yet. Refresh registry first.</span>
            ) : null}
            {companyRegistryEntries.length > 0 && filteredCompanyRegistryEntries.length === 0 ? (
              <span className="membership-empty">No registry companies match this search.</span>
            ) : null}
            {companyRegistryEntriesError ? (
              <span className="error-text">Company registry list failed: {companyRegistryEntriesError}</span>
            ) : null}
          </div>
        ) : null}
      </div>
    </>
  );
}

type UnmatchedSourcePanelProps = {
  adapterId: string;
  expandedUnmatchedAdapters: Record<string, boolean>;
  unmatchedSourceItems: Record<string, UnmatchedSourceItem[]>;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  toggleUnmatchedSourceItems: (adapterId: string) => void;
};

function UnmatchedSourcePanel({
  adapterId,
  expandedUnmatchedAdapters,
  unmatchedSourceItems,
  formatTimestamp,
  toggleUnmatchedSourceItems,
}: UnmatchedSourcePanelProps) {
  return (
    <div className="source-collapsible-panel" aria-label="Unmatched source item diagnostics">
      <button
        aria-expanded={Boolean(expandedUnmatchedAdapters[adapterId])}
        className="source-collapsible-header"
        onClick={() => toggleUnmatchedSourceItems(adapterId)}
        type="button"
      >
        <span>Unmatched</span>
        <span className="source-collapsible-header-meta">
          <strong>{unmatchedSourceItems[adapterId]?.length ?? 0}</strong>
          <ChevronDown className={expandedUnmatchedAdapters[adapterId] ? "chevron-open" : ""} size={15} />
        </span>
      </button>
      {expandedUnmatchedAdapters[adapterId] ? (
        <div className="source-unmatched-list">
          {(unmatchedSourceItems[adapterId] ?? []).map((item) => (
            <a
              className="source-unmatched-row"
              href={item.sourceUrl}
              key={item.id}
              rel="noreferrer"
              target="_blank"
              title={item.title}
            >
              <span>{item.companyName}</span>
              <strong>{item.title}</strong>
              <small>{formatTimestamp(item.publishedAt || item.fetchedAt, "Unknown")}</small>
            </a>
          ))}
          {(unmatchedSourceItems[adapterId] ?? []).length === 0 ? (
            <span className="membership-empty">No unmatched items stored.</span>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
