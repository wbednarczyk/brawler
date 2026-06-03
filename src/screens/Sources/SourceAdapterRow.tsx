import { CheckCircle2, ChevronDown, ExternalLink, Plus, RefreshCw, Search } from "lucide-react";
import type { CompanyRegistryEntry, SourceAdapter, SourceRefreshTrigger, UnmatchedSourceItem } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { StatusPill } from "../../shared/components/StatusPill";
import { useLocale } from "../../shared/locale";
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
  const { text } = useLocale();

  return (
    <div className="source-row-block">
      <article
        aria-label={`${text("Open source adapter")}: ${adapter.displayName}`}
        className={["source-row", selected ? "source-row-selected" : ""]
          .filter(Boolean)
          .join(" ")}
        onClick={() => toggleSourceAdapter(adapter.id)}
        onKeyDown={toggleSourceAdapterFromKeyboard}
        role="button"
        tabIndex={0}
        title={`${text("Open")} ${adapter.displayName} ${text("details")}`}
      >
        <div className="source-row-main">
          <div className="source-title-line">
            <span
              className={adapter.enabled ? "status-dot status-ok" : "status-dot status-warn"}
              title={adapter.enabled ? text("Enabled") : text("Disabled")}
            />
            <h2>{adapter.displayName}</h2>
            <span className="source-id">{adapter.id}</span>
          </div>
          <p>{text(formatSourceSubtitle(adapter))}</p>
          <div className="source-chip-list" aria-label={`${text("Markets for")} ${adapter.displayName}`}>
            {adapter.markets.map((market) => (
              <StatusPill key={market}>{market}</StatusPill>
            ))}
            {adapter.markets.length === 0 ? (
              <span className="membership-empty">{text("No markets")}</span>
            ) : null}
          </div>
        </div>
        <div className="source-row-status">
          <span>{adapter.lastError ?? (adapter.enabled ? text("Ready") : text("Disabled"))}</span>
        </div>
      </article>
      {selected ? (
        <div className="source-detail-panel" aria-label={text("Source adapter details")}>
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
              title={adapter.enabled ? `${text("Refresh source")}: ${adapter.displayName}` : `${adapter.displayName} ${text("is disabled")}`}
            >
              {adapter.id === gpwRegistryAdapterId && registryRefreshState === "done" ? (
                <CheckCircle2 size={15} />
              ) : (
                <RefreshCw size={15} />
              )}
              {adapter.id === gpwRegistryAdapterId
                ? registryRefreshState === "refreshing"
                  ? text("Refreshing")
                  : text("Refresh source")
                : sourceAdapterRefreshInFlight === adapter.id
                  ? text("Refreshing")
                  : text("Refresh source")}
            </Button>
            {sourceRefreshError && sourceAdapterRefreshInFlight === null ? (
              <span className="error-text">{text("Source refresh failed")}: {sourceRefreshError}</span>
            ) : null}
          </div>
          <dl className="source-status-grid source-status-detail">
            <div>
              <dt>{text("Scheduler")}</dt>
              <dd>{text(formatSourceScheduler(adapter))}</dd>
            </div>
            <div>
              <dt>{text("Next poll")}</dt>
              <dd>{formatNextRefresh(adapter)}</dd>
            </div>
            <div>
              <dt>{text("Last attempt")}</dt>
              <dd>{formatTimestamp(adapter.lastAttemptAt, text("Never"))}</dd>
            </div>
            <div>
              <dt>{text("Last trigger")}</dt>
              <dd>{text(formatSourceTrigger(adapter))}</dd>
            </div>
            <div>
              <dt>{text("Last success")}</dt>
              <dd>{formatTimestamp(adapter.lastSuccessAt, text("Never"))}</dd>
            </div>
            <div>
              <dt>{text("Last error")}</dt>
              <dd>{formatTimestamp(adapter.lastErrorAt, text("None"))}</dd>
            </div>
            <div>
              <dt>{text(sourceLastResultLabel(adapter))}</dt>
              <dd>{text(formatSourceLastResult(adapter))}</dd>
            </div>
            {adapter.id === gpwRegistryAdapterId ? null : (
              <div>
                <dt>{text("Detail warning")}</dt>
                <dd>{adapter.lastDetailWarning ?? text("None")}</dd>
              </div>
            )}
            <div>
              <dt>{text("Status")}</dt>
              <dd>{adapter.lastError ?? (adapter.enabled ? text("Ready") : text("Disabled"))}</dd>
            </div>
            <div>
              <dt>{text("Access")}</dt>
              <dd>{text(formatSourceAccess(adapter))}</dd>
            </div>
            <div>
              <dt>{text(sourcePolicyLabel(adapter))}</dt>
              <dd>{adapter.rateLimitPolicy}</dd>
            </div>
            <div>
              <dt>{text("Source page")}</dt>
              <dd>
                <Button
                  aria-label={`${text("Open source page for")} ${adapter.displayName}`}
                  className="source-page-link"
                  onClick={() => openExternalUrl(adapter.sourceUrl)}
                  variant="minimal"
                >
                  <ExternalLink size={14} />
                  {text("Open source page")}
                </Button>
              </dd>
            </div>
            <div>
              <dt>{text("Policy")}</dt>
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
  const { text } = useLocale();

  return (
    <>
      <div className="source-registry-actions" aria-label={text("Company registry refresh")}>
        <Button
          className="compact-button"
          disabled={registryRefreshState === "refreshing"}
          onClick={() => refreshCompanyRegistry("manual")}
        >
          {registryRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
          {registryRefreshState === "refreshing" ? text("Refreshing") : text("Refresh registry")}
        </Button>
        {registryRefreshResult ? (
          <span>
            {registryRefreshResult.entriesUpserted}/{registryRefreshResult.entriesFetched} {text("cached")}
          </span>
        ) : null}
        {registryRefreshError ? (
          <span className="error-text">{text("Registry refresh failed")}: {registryRefreshError}</span>
        ) : null}
      </div>
      <div className="source-collapsible-panel" aria-label={text("GPW company registry entries")}>
        <button
          aria-expanded={isCompanyRegistryListExpanded}
          className="source-collapsible-header"
          onClick={toggleCompanyRegistryList}
          type="button"
        >
          <span>{text("Companies")}</span>
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
                aria-label={text("Search GPW company registry")}
                onChange={(event) => setCompanyRegistrySearch(event.target.value)}
                placeholder={text("Search ticker, company, ISIN")}
                type="search"
                value={companyRegistrySearch}
              />
            </label>
            <span className="source-registry-count">
              {filteredCompanyRegistryEntries.length}/{companyRegistryEntries.length} {text("companies")}
            </span>
            {filteredCompanyRegistryEntries.map((entry) => (
              <div className="source-registry-row" key={entry.qualifiedTicker}>
                <span>{entry.qualifiedTicker}</span>
                <strong title={entry.displayName}>{entry.displayName}</strong>
                <small>{entry.isin ?? text("No ISIN")}</small>
                <Button
                  className="compact-button"
                  disabled={entry.tracked || addingRegistryTicker === entry.qualifiedTicker}
                  onClick={() => addCompanyFromRegistry(entry)}
                  title={entry.tracked ? `${entry.qualifiedTicker} ${text("already added")}` : `${text("Add")} ${entry.qualifiedTicker}`}
                >
                  {entry.tracked ? <CheckCircle2 size={14} /> : <Plus size={14} />}
                  {entry.tracked ? text("Added") : text("Add")}
                </Button>
              </div>
            ))}
            {companyRegistryEntries.length === 0 ? (
              <span className="membership-empty">{text("No cached companies yet. Refresh registry first.")}</span>
            ) : null}
            {companyRegistryEntries.length > 0 && filteredCompanyRegistryEntries.length === 0 ? (
              <span className="membership-empty">{text("No registry companies match this search.")}</span>
            ) : null}
            {companyRegistryEntriesError ? (
              <span className="error-text">{text("Company registry list failed")}: {companyRegistryEntriesError}</span>
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
  const { text } = useLocale();

  return (
    <div className="source-collapsible-panel" aria-label={text("Unmatched source item diagnostics")}>
      <button
        aria-expanded={Boolean(expandedUnmatchedAdapters[adapterId])}
        className="source-collapsible-header"
        onClick={() => toggleUnmatchedSourceItems(adapterId)}
        type="button"
      >
        <span>{text("Unmatched")}</span>
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
              <small>{formatTimestamp(item.publishedAt || item.fetchedAt, text("Unknown"))}</small>
            </a>
          ))}
          {(unmatchedSourceItems[adapterId] ?? []).length === 0 ? (
            <span className="membership-empty">{text("No unmatched items stored.")}</span>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
