import { CheckCircle2, ChevronDown, ExternalLink, Plus, RefreshCw } from "lucide-react";
import type { CompanyRegistryEntry, SourceAdapter, SourceRefreshTrigger } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import {
  ActionButton,
  ActionRow,
  ChipList,
  DenseRow,
  EmptyState,
  ErrorText,
  Figure,
  InfoGrid,
  SearchField,
  StatusChip,
  StatusPill,
} from "../../ui";
import {
  formatSourceHealth,
  formatSourceSubtitle,
  formatSourceTrigger,
  sourceLastResultLabel,
} from "./sourceHelpers";

// F4b S4 (contract § Sources telemetry pass): the fetch-result sentence
// interpolates several counts, so each one renders through `Figure
// kind="count"` instead of the plain string `formatSourceLastResult` used to
// build (retired — text() can't round-trip an interpolated string, and a raw
// number loses the num-tabular UI face, ADR 0104 dec. 2 amendment).
function SourceLastResultText({ adapter }: { adapter: SourceAdapter }) {
  const { text } = useLocale();
  if (adapter.lastItemsFetched === null) {
    return <>{text("None")}</>;
  }
  if (adapter.sourceType === "company_registry") {
    return (
      <>
        <Figure value={adapter.lastItemsFetched} /> {text("directory entries")} ·{" "}
        <Figure value={adapter.lastItemsCreated ?? 0} /> {text("refreshed or updated")}
      </>
    );
  }
  return (
    <>
      {text("Fetched")} <Figure value={adapter.lastItemsFetched} /> · {text("created")}{" "}
      <Figure value={adapter.lastItemsCreated ?? 0} /> · {text("matched")}{" "}
      <Figure value={adapter.lastItemsMatched ?? 0} /> · {text("unmatched")}{" "}
      <Figure value={adapter.lastItemsUnmatched ?? 0} />
      {adapter.lastDetailItemsAttempted !== null ? (
        <>
          {" "}
          · {text("details")} <Figure value={adapter.lastDetailItemsStored ?? 0} />/
          <Figure value={adapter.lastDetailItemsAttempted} /> {text("saved")} ·{" "}
          <Figure value={adapter.lastDetailItemsFailed ?? 0} /> {text("failed")}
        </>
      ) : null}
    </>
  );
}

// A nullable timestamp as a `Figure kind="datetime"` (contract: "timestamps →
// Figure datetime") without threading a new formatter prop through
// AppStateRoot (out of this slice's allowed diff there, F4b S4 contract note)
// — the row already has the raw ISO value on `adapter`.
function TimestampFigure({ value, emptyLabel }: { value: string | null; emptyLabel: string }) {
  return value ? <Figure kind="datetime" value={value} /> : <>{emptyLabel}</>;
}

type SourceAdapterRowProps = {
  adapter: SourceAdapter;
  addingRegistryTicker: string | null;
  companyRegistryEntries: CompanyRegistryEntry[];
  companyRegistryEntriesError: string | null;
  companyRegistrySearch: string;
  filteredCompanyRegistryEntries: CompanyRegistryEntry[];
  isCompanyRegistryListExpanded: boolean;
  registryRefreshError: string | null;
  registryRefreshResult: { entriesFetched: number; entriesUpserted: number } | null;
  registryRefreshState: string;
  selected: boolean;
  sourceAdapterRefreshInFlight: string | null;
  sourceRefreshError: string | null;
  addCompanyFromRegistry: (entry: CompanyRegistryEntry) => void;
  formatNextRefresh: (adapter: SourceAdapter) => string;
  formatSourceScheduler: (adapter: SourceAdapter) => string;
  openExternalUrl: (url: string) => void;
  refreshCompanyRegistry: (trigger: SourceRefreshTrigger) => void;
  setSourceEnabled: (adapter: SourceAdapter, enabled: boolean) => void;
  setCompanyRegistrySearch: (value: string) => void;
  toggleCompanyRegistryList: () => void;
  toggleSourceAdapter: (adapterId: string) => void;
};

export function SourceAdapterRow({
  adapter,
  addingRegistryTicker,
  companyRegistryEntries,
  companyRegistryEntriesError,
  companyRegistrySearch,
  filteredCompanyRegistryEntries,
  isCompanyRegistryListExpanded,
  registryRefreshError,
  registryRefreshResult,
  registryRefreshState,
  selected,
  sourceAdapterRefreshInFlight,
  sourceRefreshError,
  addCompanyFromRegistry,
  formatNextRefresh,
  formatSourceScheduler,
  openExternalUrl,
  refreshCompanyRegistry,
  setSourceEnabled,
  setCompanyRegistrySearch,
  toggleCompanyRegistryList,
  toggleSourceAdapter,
}: SourceAdapterRowProps) {
  const { text } = useLocale();
  const isCompanyDirectorySource = adapter.sourceType === "company_registry";

  return (
    <div className="source-row-block">
      <DenseRow
        className={["source-row", adapter.enabled ? "" : "source-row-disabled", selected ? "source-row-selected" : ""]
          .filter(Boolean)
          .join(" ")}
        disabled={!adapter.enabled}
        interactive={false}
        selected={selected}
      >
        {/* Primary action is a real <button>, not a role="button" on the row: the
            enable toggle below is then a sibling control, not a nested interactive
            inside a clickable row (ADR 0076 D9). */}
        <button
          type="button"
          aria-label={`${text("Open source")}: ${adapter.displayName}`}
          className="source-row-main"
          data-action-kind="control"
          onClick={() => toggleSourceAdapter(adapter.id)}
          title={`${text("Open")} ${adapter.displayName} ${text("details")}`}
        >
          <div className="source-title-line">
            <span
              className={adapter.enabled ? "status-dot status-ok" : "status-dot status-warn"}
              title={adapter.enabled ? text("Enabled") : text("Disabled")}
            />
            <h2>{adapter.displayName}</h2>
            {/* Witness role (ADR 0069 D2, plan v0.55 T3): a health mechanism, not a
                feed — it reconciles its listings against the primary source. */}
            {adapter.role === "witness" ? (
              <StatusChip tone="accent">{text("Witness")}</StatusChip>
            ) : null}
          </div>
          <p>{text(formatSourceSubtitle(adapter))}</p>
          <ChipList ariaLabel={`${text("Markets for")} ${adapter.displayName}`}>
            {adapter.markets.map((market) => (
              <StatusPill key={market}>{market}</StatusPill>
            ))}
            {adapter.markets.length === 0 ? (
              <EmptyState kind="quiet" className="membership-empty" reason={text("No markets")} />
            ) : null}
          </ChipList>
        </button>
        <div className="source-row-status">
          <span className={`source-health source-health-${adapter.healthStatus}`}>
            <span aria-hidden="true" />
            {text(formatSourceHealth(adapter))}
          </span>
          {adapter.userConfigurable ? (
            <label className="source-enable-control">
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
        {/* Density contract (ADR 0076 D6): the schedule/settings summary folds at
            S/short and shows inline from M up; the last-fetch diagnostics summary
            is an extra column at L only. Both fold via container queries
            (sources.css) — the detail values remain available in the expanded
            detail panel below. */}
        <div className="source-row-schedule" aria-label={text("Schedule")}>
          <span className="source-row-meta-value">{formatSourceScheduler(adapter)}</span>
          <span className="source-row-meta-value source-row-meta-muted">{formatNextRefresh(adapter)}</span>
        </div>
        <div className="source-row-diagnostics" aria-label={text("Last fetch")}>
          <span className="source-row-meta-value">
            <TimestampFigure value={adapter.lastAttemptAt} emptyLabel={text("Never")} />
          </span>
          <span className="source-row-meta-value source-row-meta-muted">
            <SourceLastResultText adapter={adapter} />
          </span>
        </div>
      </DenseRow>
      {selected ? (
        <div className="source-detail-panel" aria-label={text("Source details")}>
          {sourceRefreshError && sourceAdapterRefreshInFlight === null ? (
            <ErrorText as="span">{text("Source refresh failed")}: {sourceRefreshError}</ErrorText>
          ) : null}
          <InfoGrid
            className="source-status-grid source-status-detail"
            items={[
              { label: text("Refreshing"), value: formatSourceScheduler(adapter) },
              { label: text("Next refresh"), value: formatNextRefresh(adapter) },
              {
                label: text("Last attempt"),
                value: <TimestampFigure value={adapter.lastAttemptAt} emptyLabel={text("Never")} />,
              },
              { label: text("Last run by"), value: text(formatSourceTrigger(adapter)) },
              {
                label: text("Last success"),
                value: <TimestampFigure value={adapter.lastSuccessAt} emptyLabel={text("Never")} />,
              },
              {
                label: text("Last error"),
                value: <TimestampFigure value={adapter.lastErrorAt} emptyLabel={text("None")} />,
              },
              { label: text(sourceLastResultLabel(adapter)), value: <SourceLastResultText adapter={adapter} /> },
              ...(isCompanyDirectorySource
                ? []
                : [{ label: text("Detail warning"), value: adapter.lastDetailWarning ?? text("None") }]),
              { label: text("Status"), value: text(formatSourceHealth(adapter)) },
              {
                label: text("Source page"),
                value: (
                  <ActionButton
                    verb="open"
                    className="source-page-link"
                    onClick={() => openExternalUrl(adapter.sourceUrl)}
                    variant="minimal"
                  >
                    <ExternalLink size={14} />
                    {text("Open source page")}
                  </ActionButton>
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
        <ActionButton
          verb="refresh"
          className="compact-button"
          disabled={registryRefreshState === "refreshing"}
          onClick={() => refreshCompanyRegistry("manual")}
        >
          {registryRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
          {registryRefreshState === "refreshing" ? text("Refreshing") : text("Refresh company directory")}
        </ActionButton>
        {registryRefreshResult ? (
          <span>
            <Figure value={registryRefreshResult.entriesUpserted} />/
            <Figure value={registryRefreshResult.entriesFetched} /> {text("saved entries")}
          </span>
        ) : null}
        {registryRefreshError ? (
          <ErrorText as="span">{text("Company directory refresh failed")}: {registryRefreshError}</ErrorText>
        ) : null}
      </ActionRow>
      <div className="source-collapsible-panel" aria-label={text("Company directory entries")}>
        <button
          aria-expanded={isCompanyRegistryListExpanded}
          aria-label={text("Companies")}
          className="source-collapsible-header"
          data-action-kind="control"
          onClick={toggleCompanyRegistryList}
          type="button"
        >
          <span>{text("Companies")}</span>
          <span className="source-collapsible-header-meta">
            <strong><Figure value={companyRegistryEntries.length} /></strong>
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
              <Figure value={filteredCompanyRegistryEntries.length} />/
              <Figure value={companyRegistryEntries.length} /> {text("companies")}
            </span>
            {filteredCompanyRegistryEntries.map((entry) => (
              <div className="source-registry-row" key={entry.qualifiedTicker}>
                <span><TickerLabel value={entry.qualifiedTicker} /></span>
                <strong title={entry.displayName}>{entry.displayName}</strong>
                <small>{entry.isin ?? text("No ISIN")}</small>
                {entry.tracked ? (
                  <span
                    className="compact-button source-registry-added-chip"
                    title={`${entry.qualifiedTicker} ${text("already added")}`}
                  >
                    <CheckCircle2 size={14} />
                    {text("Added")}
                  </span>
                ) : (
                  <ActionButton
                    verb="add"
                    className="compact-button"
                    disabled={addingRegistryTicker === entry.qualifiedTicker}
                    onClick={() => addCompanyFromRegistry(entry)}
                    title={`${text("Add")} ${entry.qualifiedTicker}`}
                  >
                    <Plus size={14} />
                    {text("Add")}
                  </ActionButton>
                )}
              </div>
            ))}
            {companyRegistryEntries.length === 0 ? (
              <EmptyState
                kind="quiet"
                className="membership-empty"
                reason={text("No companies available yet. Refresh the company directory first.")}
              />
            ) : null}
            {companyRegistryEntries.length > 0 && filteredCompanyRegistryEntries.length === 0 ? (
              <EmptyState
                kind="quiet"
                className="membership-empty"
                reason={text("No company directory entries match this search.")}
              />
            ) : null}
            {companyRegistryEntriesError ? (
              <ErrorText as="span">{text("Company directory list failed")}: {companyRegistryEntriesError}</ErrorText>
            ) : null}
          </div>
        ) : null}
      </div>
    </>
  );
}
