import { CheckCircle2, ChevronDown, ExternalLink, Plus, RefreshCw } from "lucide-react";
import type { CompanyRegistryEntry, SourceAdapter, SourceRefreshTrigger, UnmatchedSourceItem } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { ActionRow, Button, ChipList, DenseRow, InfoGrid, SearchField, StatusPill } from "../../ui";
import {
  formatSourceHealth,
  formatSourceLastResult,
  formatSourceSubtitle,
  formatSourceTrigger,
  sourceLastResultLabel,
} from "./sourceHelpers";

type SourceAdapterRowProps = {
  adapter: SourceAdapter;
  addingRegistryTicker: string | null;
  companyRegistryEntries: CompanyRegistryEntry[];
  companyRegistryEntriesError: string | null;
  companyRegistrySearch: string;
  expandedUnmatchedAdapters: Record<string, boolean>;
  filteredCompanyRegistryEntries: CompanyRegistryEntry[];
  isCompanyRegistryListExpanded: boolean;
  registryRefreshError: string | null;
  registryRefreshResult: { entriesFetched: number; entriesUpserted: number } | null;
  registryRefreshState: string;
  selected: boolean;
  sourceAdapterRefreshInFlight: string | null;
  sourceRefreshError: string | null;
  unmatchedSourceItems: Record<string, UnmatchedSourceItem[]>;
  addCompanyFromRegistry: (entry: CompanyRegistryEntry) => void;
  formatNextRefresh: (adapter: SourceAdapter) => string;
  formatSourceScheduler: (adapter: SourceAdapter) => string;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  openExternalUrl: (url: string) => void;
  refreshCompanyRegistry: (trigger: SourceRefreshTrigger) => void;
  setSourceEnabled: (adapter: SourceAdapter, enabled: boolean) => void;
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
  isCompanyRegistryListExpanded,
  registryRefreshError,
  registryRefreshResult,
  registryRefreshState,
  selected,
  sourceAdapterRefreshInFlight,
  sourceRefreshError,
  unmatchedSourceItems,
  addCompanyFromRegistry,
  formatNextRefresh,
  formatSourceScheduler,
  formatTimestamp,
  openExternalUrl,
  refreshCompanyRegistry,
  setSourceEnabled,
  setCompanyRegistrySearch,
  toggleCompanyRegistryList,
  toggleSourceAdapter,
  toggleSourceAdapterFromKeyboard,
  toggleUnmatchedSourceItems,
}: SourceAdapterRowProps) {
  const { text } = useLocale();
  const isCompanyDirectorySource = adapter.sourceType === "company_registry";

  return (
    <div className="source-row-block">
      <DenseRow
        aria-label={`${text("Open source")}: ${adapter.displayName}`}
        className={["source-row", adapter.enabled ? "" : "source-row-disabled", selected ? "source-row-selected" : ""]
          .filter(Boolean)
          .join(" ")}
        disabled={!adapter.enabled}
        onClick={() => toggleSourceAdapter(adapter.id)}
        onKeyDown={toggleSourceAdapterFromKeyboard}
        role="button"
        selected={selected}
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
          </div>
          <p>{text(formatSourceSubtitle(adapter))}</p>
          <ChipList ariaLabel={`${text("Markets for")} ${adapter.displayName}`}>
            {adapter.markets.map((market) => (
              <StatusPill key={market}>{market}</StatusPill>
            ))}
            {adapter.markets.length === 0 ? (
              <span className="membership-empty">{text("No markets")}</span>
            ) : null}
          </ChipList>
        </div>
        <div className="source-row-status">
          <span className={`source-health source-health-${adapter.healthStatus}`}>
            <span aria-hidden="true" />
            {text(formatSourceHealth(adapter))}
          </span>
          {adapter.userConfigurable ? (
            <label className="source-enable-control" onClick={(event) => event.stopPropagation()}>
              <input
                aria-label={`${adapter.enabled ? text("Turn off") : text("Turn on")} ${adapter.displayName}`}
                checked={adapter.enabled}
                onChange={(event) => setSourceEnabled(adapter, event.target.checked)}
                role="switch"
                type="checkbox"
              />
              <span aria-hidden="true" className="source-enable-track">
                <span />
              </span>
            </label>
          ) : null}
        </div>
      </DenseRow>
      {selected ? (
        <div className="source-detail-panel" aria-label={text("Source details")}>
          {sourceRefreshError && sourceAdapterRefreshInFlight === null ? (
            <span className="error-text">{text("Source refresh failed")}: {sourceRefreshError}</span>
          ) : null}
          <InfoGrid
            className="source-status-grid source-status-detail"
            items={[
              { label: text("Scheduler"), value: text(formatSourceScheduler(adapter)) },
              { label: text("Next poll"), value: formatNextRefresh(adapter) },
              { label: text("Last attempt"), value: formatTimestamp(adapter.lastAttemptAt, text("Never")) },
              { label: text("Last trigger"), value: text(formatSourceTrigger(adapter)) },
              { label: text("Last success"), value: formatTimestamp(adapter.lastSuccessAt, text("Never")) },
              { label: text("Last error"), value: formatTimestamp(adapter.lastErrorAt, text("None")) },
              { label: text(sourceLastResultLabel(adapter)), value: text(formatSourceLastResult(adapter)) },
              ...(isCompanyDirectorySource
                ? []
                : [{ label: text("Detail warning"), value: adapter.lastDetailWarning ?? text("None") }]),
              { label: text("Status"), value: text(formatSourceHealth(adapter)) },
              {
                label: text("Source page"),
                value: (
                  <Button
                    aria-label={`${text("Open source page for")} ${adapter.displayName}`}
                    className="source-page-link"
                    onClick={() => openExternalUrl(adapter.sourceUrl)}
                    variant="minimal"
                  >
                    <ExternalLink size={14} />
                    {text("Open source page")}
                  </Button>
                ),
              },
            ]}
          />
          {isCompanyDirectorySource ? (
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
          ) : null}
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
      <ActionRow className="source-registry-actions" ariaLabel={text("Company directory refresh")}>
        <Button
          className="compact-button"
          disabled={registryRefreshState === "refreshing"}
          onClick={() => refreshCompanyRegistry("manual")}
        >
          {registryRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
          {registryRefreshState === "refreshing" ? text("Refreshing") : text("Refresh company directory")}
        </Button>
        {registryRefreshResult ? (
          <span>
            {registryRefreshResult.entriesUpserted}/{registryRefreshResult.entriesFetched} {text("saved entries")}
          </span>
        ) : null}
        {registryRefreshError ? (
          <span className="error-text">{text("Company directory refresh failed")}: {registryRefreshError}</span>
        ) : null}
      </ActionRow>
      <div className="source-collapsible-panel" aria-label={text("Company directory entries")}>
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
            <SearchField
              ariaLabel={text("Search company directory")}
              className="registry-search-field"
              clearLabel={text("Clear company directory search")}
              onChange={setCompanyRegistrySearch}
              onClear={() => setCompanyRegistrySearch("")}
              placeholder={text("Search ticker, company, ISIN")}
              value={companyRegistrySearch}
            />
            <span className="source-registry-count">
              {filteredCompanyRegistryEntries.length}/{companyRegistryEntries.length} {text("companies")}
            </span>
            {filteredCompanyRegistryEntries.map((entry) => (
              <div className="source-registry-row" key={entry.qualifiedTicker}>
                <span><TickerLabel value={entry.qualifiedTicker} /></span>
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
              <span className="membership-empty">{text("No companies available yet. Refresh the company directory first.")}</span>
            ) : null}
            {companyRegistryEntries.length > 0 && filteredCompanyRegistryEntries.length === 0 ? (
              <span className="membership-empty">{text("No company directory entries match this search.")}</span>
            ) : null}
            {companyRegistryEntriesError ? (
              <span className="error-text">{text("Company directory list failed")}: {companyRegistryEntriesError}</span>
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
