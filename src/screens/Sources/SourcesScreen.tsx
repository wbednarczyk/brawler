import { CheckCircle2, RefreshCw } from "lucide-react";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { useLocale } from "../../shared/locale";
import { groupSourceAdapters } from "./sourceHelpers";
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
  const { t, text } = useLocale();
  const groupedAdapters = groupSourceAdapters(sourceAdapters);

  return (
    <section className="feed-panel" aria-labelledby="sources-title">
      <div className="panel-header">
        <div>
          <h1 id="sources-title">{t("sources.title")}</h1>
          <p>{t("sources.description")}</p>
        </div>
        <Button
          className="compact-button"
          disabled={sourceRefreshState === "refreshing"}
          onClick={() => refreshSources("manual")}
        >
          {sourceRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
          {sourceRefreshState === "refreshing" ? t("action.refreshing") : t("action.refreshSources")}
        </Button>
      </div>

      <div className="sources-layout" aria-label={text("Source list")}>
        {sourceRefreshResult ? (
          <dl className="source-status-grid source-refresh-summary" aria-label={text("Last source refresh summary")}>
            <div>
              <dt>{text("Fetched")}</dt>
              <dd aria-label={text("Fetched source items")}>{sourceRefreshResult.itemsFetched}</dd>
            </div>
            <div>
              <dt>{text("Created")}</dt>
              <dd aria-label={text("Created source items")}>{sourceRefreshResult.itemsCreated}</dd>
            </div>
            <div>
              <dt>{text("Matched")}</dt>
              <dd aria-label={text("Matched source items")}>{sourceRefreshResult.itemsMatched}</dd>
            </div>
            <div>
              <dt>{text("Unmatched")}</dt>
              <dd aria-label={text("Unmatched source items")}>{sourceRefreshResult.itemsUnmatched}</dd>
            </div>
            <div>
              <dt>{text("Details")}</dt>
              <dd aria-label={text("Stored source detail bodies")}>
                {sourceRefreshResult.detailItemsStored}/{sourceRefreshResult.detailItemsAttempted}
              </dd>
            </div>
            <div>
              <dt>{text("Detail failures")}</dt>
              <dd aria-label={text("Failed source detail bodies")}>{sourceRefreshResult.detailItemsFailed}</dd>
            </div>
          </dl>
        ) : null}
        {groupedAdapters.map((group) => (
          <section className="source-group" key={group.id} aria-label={text(group.label)}>
            <div className="source-group-header">
              <div>
                <h2>{text(group.label)}</h2>
                <p>{text(group.description)}</p>
              </div>
              <span>{group.adapters.length}</span>
            </div>
            <div className="source-group-list">
              {group.adapters.map((adapter) => (
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
            </div>
          </section>
        ))}
        {sourceAdapters.length === 0 ? <EmptyState>{t("empty.noSourceAdapters")}</EmptyState> : null}
        {sourceAdaptersError ? <p className="error-text">{t("error.sourceCommandFailed")}: {sourceAdaptersError}</p> : null}
        {sourceRefreshError ? <p className="error-text">{t("error.sourceRefreshFailed")}: {sourceRefreshError}</p> : null}
        {unmatchedSourceItemsError ? (
          <p className="error-text">{t("error.unmatchedSourceDiagnosticsFailed")}: {unmatchedSourceItemsError}</p>
        ) : null}
      </div>
    </section>
  );
}
