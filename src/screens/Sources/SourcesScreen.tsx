import { CheckCircle2, RefreshCw } from "lucide-react";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { SourceAdapterRow } from "./SourceAdapterRow";
import type { SourcesScreenProps } from "./sourceTypes";

export function SourcesScreen({
  sourceAdapters,
  sourceAdaptersError,
  selectedSourceAdapterId,
  sourceRefreshState,
  sourceRefreshResult,
  sourceRefreshError,
  sourceAdapterRefreshInFlight,
  registryRefreshState,
  registryRefreshResult,
  registryRefreshError,
  companyRegistryEntries,
  filteredCompanyRegistryEntries,
  companyRegistryEntriesError,
  isCompanyRegistryListExpanded,
  companyRegistrySearch,
  addingRegistryTicker,
  unmatchedSourceItems,
  unmatchedSourceItemsError,
  expandedUnmatchedAdapters,
  gpwRegistryAdapterId,
  refreshSources,
  refreshSingleSource,
  refreshCompanyRegistry,
  toggleSourceAdapter,
  toggleSourceAdapterFromKeyboard,
  toggleCompanyRegistryList,
  toggleUnmatchedSourceItems,
  setCompanyRegistrySearch,
  addCompanyFromRegistry,
  openExternalUrl,
  formatSourceScheduler,
  formatNextRefresh,
  formatTimestamp,
}: SourcesScreenProps) {
  return (
    <section className="feed-panel" aria-labelledby="sources-title">
      <div className="panel-header">
        <div>
          <h1 id="sources-title">Sources</h1>
          <p>Local source adapter status before remote ingestion is wired.</p>
        </div>
        <Button
          className="compact-button"
          disabled={sourceRefreshState === "refreshing"}
          onClick={() => refreshSources("manual")}
        >
          {sourceRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
          {sourceRefreshState === "refreshing" ? "Refreshing" : "Refresh sources"}
        </Button>
      </div>

      <div className="sources-layout" aria-label="Source adapters">
        {sourceRefreshResult ? (
          <dl className="source-status-grid source-refresh-summary" aria-label="Last source refresh summary">
            <div>
              <dt>Fetched</dt>
              <dd aria-label="Fetched source items">{sourceRefreshResult.itemsFetched}</dd>
            </div>
            <div>
              <dt>Created</dt>
              <dd aria-label="Created source items">{sourceRefreshResult.itemsCreated}</dd>
            </div>
            <div>
              <dt>Matched</dt>
              <dd aria-label="Matched source items">{sourceRefreshResult.itemsMatched}</dd>
            </div>
            <div>
              <dt>Unmatched</dt>
              <dd aria-label="Unmatched source items">{sourceRefreshResult.itemsUnmatched}</dd>
            </div>
            <div>
              <dt>Details</dt>
              <dd aria-label="Stored source detail bodies">
                {sourceRefreshResult.detailItemsStored}/{sourceRefreshResult.detailItemsAttempted}
              </dd>
            </div>
            <div>
              <dt>Detail failures</dt>
              <dd aria-label="Failed source detail bodies">{sourceRefreshResult.detailItemsFailed}</dd>
            </div>
          </dl>
        ) : null}
        {sourceAdapters.map((adapter) => (
          <SourceAdapterRow
            adapter={adapter}
            addingRegistryTicker={addingRegistryTicker}
            companyRegistryEntries={companyRegistryEntries}
            companyRegistryEntriesError={companyRegistryEntriesError}
            companyRegistrySearch={companyRegistrySearch}
            expandedUnmatchedAdapters={expandedUnmatchedAdapters}
            filteredCompanyRegistryEntries={filteredCompanyRegistryEntries}
            gpwRegistryAdapterId={gpwRegistryAdapterId}
            isCompanyRegistryListExpanded={isCompanyRegistryListExpanded}
            key={adapter.id}
            registryRefreshError={registryRefreshError}
            registryRefreshResult={registryRefreshResult}
            registryRefreshState={registryRefreshState}
            selected={selectedSourceAdapterId === adapter.id}
            sourceAdapterRefreshInFlight={sourceAdapterRefreshInFlight}
            sourceRefreshError={sourceRefreshError}
            sourceRefreshState={sourceRefreshState}
            unmatchedSourceItems={unmatchedSourceItems}
            addCompanyFromRegistry={addCompanyFromRegistry}
            formatNextRefresh={formatNextRefresh}
            formatSourceScheduler={formatSourceScheduler}
            formatTimestamp={formatTimestamp}
            openExternalUrl={openExternalUrl}
            refreshCompanyRegistry={refreshCompanyRegistry}
            refreshSingleSource={refreshSingleSource}
            setCompanyRegistrySearch={setCompanyRegistrySearch}
            toggleCompanyRegistryList={toggleCompanyRegistryList}
            toggleSourceAdapter={toggleSourceAdapter}
            toggleSourceAdapterFromKeyboard={(event) =>
              toggleSourceAdapterFromKeyboard(event, adapter.id)
            }
            toggleUnmatchedSourceItems={toggleUnmatchedSourceItems}
          />
        ))}
        {sourceAdapters.length === 0 ? <EmptyState>No source adapters configured.</EmptyState> : null}
        {sourceAdaptersError ? <p className="error-text">Source command failed: {sourceAdaptersError}</p> : null}
        {sourceRefreshError ? <p className="error-text">Source refresh failed: {sourceRefreshError}</p> : null}
        {unmatchedSourceItemsError ? (
          <p className="error-text">Unmatched source diagnostics failed: {unmatchedSourceItemsError}</p>
        ) : null}
      </div>
    </section>
  );
}
