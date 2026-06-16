import { CheckCircle2, RefreshCw } from "lucide-react";
import { useLocale } from "../../shared/locale";
import { Button, EmptyState, InfoGrid, PanelHeader, SectionHeader } from "../../ui";
import { groupSourceAdapters } from "./sourceHelpers";
import { SourceAdapterRow } from "./SourceAdapterRow";
import type { SourcesScreenProps } from "./sourceTypes";

export function SourcesScreen({
  sourceAdapters,
  sourceAdaptersError,
  developerMode,
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
  refreshSources,
  refreshCompanyRegistry,
  setSourceEnabled,
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
      <PanelHeader
        title={t("sources.title")}
        description={t("sources.description")}
        titleId="sources-title"
        actions={
          <Button
            className="compact-button"
            disabled={sourceRefreshState === "refreshing"}
            onClick={() => refreshSources("manual")}
          >
            {sourceRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
            {sourceRefreshState === "refreshing" ? t("action.refreshing") : t("action.refreshSources")}
          </Button>
        }
      />

      <div className="sources-layout" aria-label={text("Source list")}>
        {groupedAdapters.map((group) => (
          <section className="source-group" key={group.id} aria-label={text(group.label)}>
            <SectionHeader
              className="source-group-header"
              title={text(group.label)}
              description={text(group.description)}
              meta={group.adapters.length}
            />
            <div className="source-group-list">
              {group.adapters.map((adapter) => (
                <SourceAdapterRow
                  adapter={adapter}
                  addingRegistryTicker={addingRegistryTicker}
                  companyRegistryEntries={
                    adapter.sourceType === "company_registry"
                      ? companyRegistryEntries.filter((entry) => entry.sourceAdapterId === adapter.id)
                      : companyRegistryEntries
                  }
                  companyRegistryEntriesError={companyRegistryEntriesError}
                  companyRegistrySearch={companyRegistrySearch}
                  expandedUnmatchedAdapters={expandedUnmatchedAdapters}
                  filteredCompanyRegistryEntries={
                    adapter.sourceType === "company_registry"
                      ? filteredCompanyRegistryEntries.filter((entry) => entry.sourceAdapterId === adapter.id)
                      : filteredCompanyRegistryEntries
                  }
                  isCompanyRegistryListExpanded={isCompanyRegistryListExpanded}
                  key={adapter.id}
                  registryRefreshError={registryRefreshError}
                  registryRefreshResult={registryRefreshResult}
                  registryRefreshState={registryRefreshState}
                  selected={selectedSourceAdapterId === adapter.id}
                  sourceAdapterRefreshInFlight={sourceAdapterRefreshInFlight}
                  sourceRefreshError={sourceRefreshError}
                  unmatchedSourceItems={unmatchedSourceItems}
                  addCompanyFromRegistry={addCompanyFromRegistry}
                  formatNextRefresh={formatNextRefresh}
                  formatSourceScheduler={formatSourceScheduler}
                  formatTimestamp={formatTimestamp}
                  openExternalUrl={openExternalUrl}
                  refreshCompanyRegistry={refreshCompanyRegistry}
                  setSourceEnabled={setSourceEnabled}
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
        {developerMode && sourceRefreshResult ? (
          <section className="source-developer-summary" aria-label={text("Developer source refresh summary")}>
            <SectionHeader
              className="source-developer-summary-header"
              title={text("Developer refresh summary")}
              description={text("Latest manual or scheduled source run")}
            />
            <InfoGrid
              ariaLabel={text("Last source refresh summary")}
              className="source-status-grid source-refresh-summary"
              items={[
                {
                  label: text("Fetched"),
                  value: sourceRefreshResult.itemsFetched,
                  valueAriaLabel: text("Fetched source items"),
                },
                {
                  label: text("Created"),
                  value: sourceRefreshResult.itemsCreated,
                  valueAriaLabel: text("Created source items"),
                },
                {
                  label: text("Matched"),
                  value: sourceRefreshResult.itemsMatched,
                  valueAriaLabel: text("Matched source items"),
                },
                {
                  label: text("Unmatched"),
                  value: sourceRefreshResult.itemsUnmatched,
                  valueAriaLabel: text("Unmatched source items"),
                },
                {
                  label: text("Details"),
                  value: `${sourceRefreshResult.detailItemsStored}/${sourceRefreshResult.detailItemsAttempted}`,
                  valueAriaLabel: text("Stored source detail bodies"),
                },
                {
                  label: text("Detail failures"),
                  value: sourceRefreshResult.detailItemsFailed,
                  valueAriaLabel: text("Failed source detail bodies"),
                },
              ]}
            />
          </section>
        ) : null}
      </div>
    </section>
  );
}
