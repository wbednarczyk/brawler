import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ExternalLink, X } from "lucide-react";

import { listCompanies } from "../../api/companies";
import { getCompanySector } from "../../api/companySector";
import { listKpiDefinitions } from "../../api/financials";
import { getKpiComparison } from "../../api/comparison";
import type { KpiComparison, KpiComparisonCell, KpiComparisonSeries } from "../../api/comparison";
import { listWatchlistMemberships, listWatchlists } from "../../api/watchlists";
import type { Company, Watchlist, WatchlistMembership } from "../../api/types";
import type { KpiDefinition } from "../../api/financialsTypes";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatFixedDecimal, formatFixedPercent, groupFormat } from "../../shared/format/financialValue";
import { useLocale, type LocaleCode } from "../../shared/locale";
import { localizedKpiLabel } from "../../shared/locale/kpiLabels";
import { COMPANY_FORMS, pluralNoun } from "../../shared/locale/plural";
import {
  Button,
  EmptyState,
  ErrorText,
  Hint,
  MultiLineChart,
  PanelHeader,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  Skeleton,
  StatusChip,
} from "../../ui";
import type { MultiLineSeries } from "../../ui";
import { COMPARE_SERIES_SLOTS, seriesSlotClass } from "./seriesSlots";
import { CompareValuation } from "./CompareValuation";

type Granularity = "annual" | "quarterly";
type CompareView = "profil" | "trend";
type LoadState = "idle" | "loading" | "error";

// Reactive compute (§A7): the comparison recomputes on every zestaw / metric /
// period / view change — no "Porównaj" button. A short debounce coalesces rapid
// changes (typing through the pickers, adding companies in a burst) into one read.
const COMPARE_DEBOUNCE_MS = 180;

type CompareScreenProps = {
  // Evidence drill-in (⧉): open the fact's company on its cockpit dashboard,
  // where the Fundamentals + report surfaces host the source evidence. Wired to
  // `openCompanyWorkspaceById` in AppStateRoot (the same idiom Today uses).
  openCompanyWorkspace?: (companyId: string) => void;
};

function isRatioKind(valueKind: string | null): boolean {
  return valueKind === "ratio" || valueKind === "percentage";
}

function isPerShare(def: KpiDefinition | undefined): boolean {
  return def?.unit === "per_share";
}

/** Monetary display: grouped, up to 2 dp (PLN-comparable value). */
function formatValue(locale: LocaleCode, decimal: string): string {
  const parsed = Number(decimal);
  if (!Number.isFinite(parsed)) return decimal;
  return groupFormat(parsed, locale, 2);
}

/** Ratio/percentage display: native value; percentages carry a `%` suffix. */
function formatPointsValue(locale: LocaleCode, decimal: string, valueKind: string | null): string {
  const parsed = Number(decimal);
  if (!Number.isFinite(parsed)) return decimal;
  if (valueKind === "percentage") return formatFixedPercent(parsed, locale, 1);
  return formatFixedDecimal(parsed, locale, 2);
}

function formatDelta(locale: LocaleCode, decimal: string, valueKind: string | null): string {
  const parsed = Number(decimal);
  if (!Number.isFinite(parsed)) return decimal;
  const sign = parsed > 0 ? "+" : "";
  const magnitude = groupFormat(parsed, locale, 1);
  const unit = isRatioKind(valueKind) ? " p.p." : "%";
  return `${sign}${magnitude}${unit}`;
}

/** Comparative magnitude between two monetary values, e.g. "9.0×". */
function formatMultiple(locale: LocaleCode, ratio: number): string {
  return `${formatFixedDecimal(ratio, locale, 1)}×`;
}

// One resolved Profil cell: a comparable number to display, a typed gap, or
// nothing (the metric has no coordinate for this company/period).
type ProfileCellView =
  | { kind: "value"; text: string; comparable: number; factId: string | null }
  | { kind: "gap"; flag: string }
  | { kind: "none" };

const GAP_FLAGS = new Set(["no_fact", "fx_missing", "currency_unknown"]);

/**
 * Resolve a Profil cell into what the table shows. Monetary/per-share values are
 * only ever shown PLN-comparable (an unconverted native figure is a typed flag,
 * never a misleading number — ADR 0089 dec. 8). Ratio/percentage KPIs are not
 * converted: their native value IS the comparable number. The read model no
 * longer emits `currency_unknown` for ratio/percentage cells (card b875e69), so
 * the same typed-gap check applies to both kinds — no points-only special case.
 */
function profileCellView(
  cell: KpiComparisonCell | null,
  valueKind: string | null,
): ProfileCellView {
  if (!cell) return { kind: "none" };
  const gapFlag = cell.flags.find((flag) => GAP_FLAGS.has(flag));
  if (gapFlag) return { kind: "gap", flag: gapFlag };
  if (isRatioKind(valueKind)) {
    // Ratios read their native value directly (never FX-converted).
    if (cell.value !== null && Number.isFinite(Number(cell.value))) {
      return { kind: "value", text: cell.value, comparable: Number(cell.value), factId: cell.factId };
    }
    return { kind: "none" };
  }
  if (cell.valuePln !== null && Number.isFinite(Number(cell.valuePln))) {
    return { kind: "value", text: cell.valuePln, comparable: Number(cell.valuePln), factId: cell.factId };
  }
  return { kind: "none" };
}

// The Różnica cell for exactly two companies: a signed p.p. delta for ratios, a
// magnitude multiple for monetary values, or a typed "—" where the pair is
// incomparable (per-share, an absent value, or a zero base).
type ProfileDelta = { text: string; tone: "up" | "dn" | "none" };
const NO_DELTA: ProfileDelta = { text: "—", tone: "none" };

function profileDelta(
  a: ProfileCellView,
  b: ProfileCellView,
  def: KpiDefinition | undefined,
  valueKind: string | null,
  locale: LocaleCode,
): ProfileDelta {
  if (isPerShare(def)) return NO_DELTA;
  if (a.kind !== "value" || b.kind !== "value") return NO_DELTA;
  if (isRatioKind(valueKind)) {
    const diff = a.comparable - b.comparable;
    return {
      text: formatDelta(locale, String(diff), valueKind),
      tone: diff > 0 ? "up" : diff < 0 ? "dn" : "none",
    };
  }
  const magA = Math.abs(a.comparable);
  const magB = Math.abs(b.comparable);
  if (magA === 0 || magB === 0) return NO_DELTA;
  return { text: formatMultiple(locale, Math.max(magA, magB) / Math.min(magA, magB)), tone: "none" };
}

export function CompareScreen({ openCompanyWorkspace }: CompareScreenProps = {}) {
  const { t, locale } = useLocale();

  const [companies, setCompanies] = useState<Company[]>([]);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
  const [memberships, setMemberships] = useState<WatchlistMembership[]>([]);
  const [kpiDefs, setKpiDefs] = useState<KpiDefinition[]>([]);
  const [sectorByCompany, setSectorByCompany] = useState<Record<string, string | null>>({});

  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [view, setView] = useState<CompareView>("profil");
  const [metricKey, setMetricKey] = useState<string>("");
  const [granularity, setGranularity] = useState<Granularity>("annual");
  const [period, setPeriod] = useState<string>("");

  const [comparison, setComparison] = useState<KpiComparison | null>(null);
  const [resultView, setResultView] = useState<CompareView | null>(null);
  const [status, setStatus] = useState<LoadState>("idle");
  const requestSeq = useRef(0);

  const companyById = useMemo(() => {
    const map = new Map<string, Company>();
    for (const company of companies) map.set(company.id, company);
    return map;
  }, [companies]);

  const defsByKey = useMemo(() => {
    const map = new Map<string, KpiDefinition>();
    for (const def of kpiDefs) map.set(def.metricKey, def);
    return map;
  }, [kpiDefs]);

  // Profil pivots over the whole canonical catalog; the read model already takes
  // a metricKeys list, so no Rust change is needed (§A7).
  const allMetricKeys = useMemo(() => kpiDefs.map((def) => def.metricKey), [kpiDefs]);

  // Load the selection substrate once. Sectors are fetched lazily (below) — only
  // once a company is chosen — so the empty entry screen costs no extra reads.
  useEffect(() => {
    let live = true;
    void (async () => {
      const [companyList, watchlistList, membershipList, defs] = await Promise.all([
        listCompanies(),
        listWatchlists(),
        listWatchlistMemberships(),
        listKpiDefinitions({}),
      ]);
      if (!live) return;
      setCompanies(companyList);
      setWatchlists(watchlistList);
      setMemberships(membershipList);
      setKpiDefs(defs);
      setMetricKey((current) => current || defs[0]?.metricKey || "");
    })();
    return () => {
      live = false;
    };
  }, []);

  // Sector map for the "peers in sector" helper (frame 2). Fetched once, the
  // first time any company is selected — the Company shape carries no sector, so
  // this is the per-company read the helper needs to count sector siblings.
  const sectorsLoaded = useRef(false);
  useEffect(() => {
    if (selectedIds.length === 0 || sectorsLoaded.current || companies.length === 0) return;
    sectorsLoaded.current = true;
    let live = true;
    void (async () => {
      const entries = await Promise.all(
        companies.map(async (company) => [company.id, await getCompanySector(company.id)] as const),
      );
      if (!live) return;
      setSectorByCompany(Object.fromEntries(entries));
    })();
    return () => {
      live = false;
    };
  }, [selectedIds.length, companies]);

  const runCompare = useCallback(
    async (ids: string[], metrics: string[], gran: Granularity, forView: CompareView) => {
      if (ids.length < 2 || metrics.length === 0) return;
      const seq = (requestSeq.current += 1);
      setStatus("loading");
      try {
        const result = await getKpiComparison({
          companyIds: ids,
          metricKeys: metrics,
          granularity: gran,
        });
        if (seq !== requestSeq.current) return;
        setComparison(result);
        setResultView(forView);
        setStatus("idle");
      } catch {
        if (seq !== requestSeq.current) return;
        setStatus("error");
      }
    },
    [],
  );

  // The request the current view needs: Profil aligns the whole catalog at annual
  // (period picker selects the column); Trend aligns one metric at the chosen
  // granularity.
  const requestMetrics = view === "profil" ? allMetricKeys : metricKey ? [metricKey] : [];
  const requestGranularity: Granularity = view === "profil" ? "annual" : granularity;
  const requestKey = requestMetrics.join(",");

  // Reactive compute (§A7): recompute whenever the set / metric / granularity /
  // view changes and ≥2 companies are chosen; a set that drops below 2 falls back
  // to the invite without a stale table. Debounced to coalesce bursts; removing a
  // chip re-aligns within one debounce tick.
  useEffect(() => {
    if (selectedIds.length < 2) {
      setComparison(null);
      setResultView(null);
      setStatus("idle");
      return;
    }
    if (requestMetrics.length === 0) return;
    setStatus("loading");
    const handle = setTimeout(() => {
      void runCompare(selectedIds, requestMetrics, requestGranularity, view);
    }, COMPARE_DEBOUNCE_MS);
    return () => clearTimeout(handle);
    // requestMetrics is derived from view+metricKey+allMetricKeys; requestKey
    // captures its identity for the dependency array.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedIds, view, requestKey, requestGranularity, runCompare]);

  const addCompany = useCallback((id: string) => {
    if (!id) return;
    setSelectedIds((prev) => (prev.includes(id) ? prev : [...prev, id]));
  }, []);

  const removeCompany = useCallback((id: string) => {
    setSelectedIds((prev) => prev.filter((existing) => existing !== id));
  }, []);

  const addWatchlist = useCallback(
    (watchlistId: string) => {
      if (!watchlistId) return;
      const ids = memberships
        .filter((membership) => membership.watchlistId === watchlistId)
        .map((membership) => membership.companyId);
      setSelectedIds((prev) => {
        const next = [...prev];
        for (const id of ids) if (!next.includes(id)) next.push(id);
        return next;
      });
    },
    [memberships],
  );

  const unselectedCompanies = useMemo(
    () =>
      companies
        .filter((company) => !selectedIds.includes(company.id))
        .sort((a, b) => a.qualifiedTicker.localeCompare(b.qualifiedTicker)),
    [companies, selectedIds],
  );

  // Peers helper: sector of the first selected company, and the tracked siblings
  // sharing it that are not yet in the set.
  const sectorPeers = useMemo(() => {
    const first = selectedIds[0];
    if (!first) return null;
    const sector = sectorByCompany[first];
    if (!sector) return null;
    const peers = companies
      .filter(
        (company) =>
          !selectedIds.includes(company.id) && sectorByCompany[company.id] === sector,
      )
      .map((company) => company.id);
    if (peers.length === 0) return null;
    return { sector, peers };
  }, [selectedIds, sectorByCompany, companies]);

  const metricLabel = useMemo(() => {
    const def = defsByKey.get(metricKey);
    return def ? localizedKpiLabel(def, locale) : metricKey;
  }, [defsByKey, metricKey, locale]);

  const axis = useMemo(() => comparison?.axis ?? [], [comparison]);

  // Profil defaults to (and stays on) the latest complete period; a change of set
  // that reshapes the axis re-pins it to the latest available period.
  useEffect(() => {
    if (view !== "profil") return;
    const keys = axis.map((entry) => entry.key);
    if (keys.length === 0) return;
    setPeriod((prev) => (keys.includes(prev) ? prev : keys[keys.length - 1]));
  }, [axis, view]);

  const seriesByCompany = useMemo(() => {
    const map = new Map<string, KpiComparisonSeries>();
    for (const series of comparison?.series ?? []) {
      if (series.metricKey === metricKey) map.set(series.companyId, series);
    }
    return map;
  }, [comparison, metricKey]);

  const flagLabel = (flag: string): string => {
    switch (flag) {
      case "no_fact":
        return t("compare.flag.noFact");
      case "fx_missing":
        return t("compare.flag.fxMissing");
      case "currency_unknown":
        return t("compare.flag.currencyUnknown");
      default:
        return flag;
    }
  };

  // ---- Profil pivot ------------------------------------------------------
  // Rows = every canonical KPI with data for ≥1 selected company at the chosen
  // period; columns = companies; Różnica only for exactly 2 companies.
  const profileCompare = comparison !== null && resultView === "profil";
  const profileRows = useMemo(() => {
    if (view !== "profil" || comparison === null) return [];
    const periodIndex = axis.findIndex((entry) => entry.key === period);
    if (periodIndex < 0) return [];
    const seriesIndex = new Map<string, KpiComparisonSeries>();
    for (const series of comparison.series) {
      seriesIndex.set(`${series.companyId}::${series.metricKey}`, series);
    }
    return allMetricKeys.flatMap((key) => {
      const def = defsByKey.get(key);
      const valueKind = def?.valueKind ?? null;
      const cells = selectedIds.map((id) => {
        const series = seriesIndex.get(`${id}::${key}`);
        const cell = series?.cells[periodIndex] ?? null;
        return { id, view: profileCellView(cell, valueKind), currency: cell?.currency ?? null };
      });
      // Omit a row only when no selected company has a comparable value.
      if (!cells.some((entry) => entry.view.kind === "value")) return [];
      const difference =
        selectedIds.length === 2
          ? profileDelta(cells[0].view, cells[1].view, def, valueKind, locale)
          : null;
      return [{ key, label: def ? localizedKpiLabel(def, locale) : key, cells, difference }];
    });
  }, [view, comparison, axis, period, allMetricKeys, selectedIds, defsByKey, locale]);

  // Per-company FX chip in the Profil header when that company's selected-period
  // figure was converted from a non-PLN currency.
  const profileFxByCompany = useMemo(() => {
    const map = new Map<string, string>();
    if (view !== "profil" || comparison === null) return map;
    const periodKey = period;
    for (const series of comparison.series) {
      const cell = series.cells.find((entry) => `${entry.fiscalYear}:${entry.periodType}` === periodKey);
      const currency = cell?.currency;
      if (
        currency &&
        currency !== "PLN" &&
        !cell?.flags.includes("currency_unknown") &&
        !map.has(series.companyId)
      ) {
        map.set(series.companyId, currency);
      }
    }
    return map;
  }, [view, comparison, period]);

  // ---- Trend overlay (§A4) ------------------------------------------------
  const chartSeries = useMemo<MultiLineSeries[]>(
    () =>
      selectedIds.slice(0, COMPARE_SERIES_SLOTS).map((id) => {
        const company = companyById.get(id);
        const cells = seriesByCompany.get(id)?.cells ?? [];
        const points = axis.flatMap((axisPeriod, cellIndex) => {
          const cell = cells[cellIndex];
          const gapFlag = cell?.flags.find((flag) => GAP_FLAGS.has(flag));
          const raw = gapFlag ? null : (cell?.valuePln ?? cell?.value ?? null);
          if (raw === null) return [];
          const value = Number(raw);
          if (!Number.isFinite(value)) return [];
          return [{ label: axisPeriod.key, value }];
        });
        return { key: id, label: company?.qualifiedTicker ?? id, points };
      }),
    [selectedIds, companyById, seriesByCompany, axis],
  );
  const chartCapped = selectedIds.length > COMPARE_SERIES_SLOTS;

  const periodOptions = useMemo(
    () =>
      axis.map((entry) => ({
        key: entry.key,
        label: entry.periodType === "FY" ? String(entry.fiscalYear) : `${entry.fiscalYear} ${entry.periodType}`,
      })),
    [axis],
  );

  const trendCompare = comparison !== null && resultView === "trend";
  const showProfil = status === "idle" && profileCompare && axis.length > 0 && profileRows.length > 0;
  const showTrend = status === "idle" && trendCompare && axis.length > 0;
  const showNoData =
    status === "idle" &&
    comparison !== null &&
    resultView === view &&
    (view === "profil" ? axis.length === 0 || profileRows.length === 0 : axis.length === 0);

  const retry = () =>
    void runCompare(selectedIds, requestMetrics, requestGranularity, view);

  return (
    <section className="compare-screen feed-panel" aria-labelledby="compare-title">
      <PanelHeader
        title={t("compare.title")}
        description={t("compare.description")}
        titleId="compare-title"
      />
      <div className="compare-layout">
        {/* Companies — the zestaw-spółek selector (frames 1/2/6). */}
        <section className="compare-companies" aria-label={t("compare.section.companies")}>
          <SectionHeader
            level="h3"
            title={
              selectedIds.length > 0
                ? `${t("compare.section.companies")} (${selectedIds.length})`
                : t("compare.section.companies")
            }
          />
          <div className="compare-chip-row">
            {selectedIds.map((id, index) => {
              const company = companyById.get(id);
              return (
                <span key={id} className="compare-chip">
                  <span className={`compare-dot ${seriesSlotClass(index)}`} aria-hidden="true" />
                  {company ? (
                    <TickerLabel value={company.qualifiedTicker} />
                  ) : (
                    <span>{id}</span>
                  )}
                  <Button
                    variant="icon"
                    className="compare-chip-remove"
                    icon={<X size={12} aria-hidden="true" />}
                    aria-label={`${t("compare.removeCompany")} ${company?.qualifiedTicker ?? id}`}
                    onClick={() => removeCompany(id)}
                  />
                </span>
              );
            })}
            {sectorPeers ? (
              <Button
                variant="ghost"
                className="compare-peers-chip"
                onClick={() =>
                  setSelectedIds((prev) => [
                    ...prev,
                    ...sectorPeers.peers.filter((id) => !prev.includes(id)),
                  ])
                }
              >
                {`${t("compare.peers.prefix")} ${sectorPeers.sector} (${sectorPeers.peers.length} ${pluralNoun(locale, sectorPeers.peers.length, COMPANY_FORMS)})`}
              </Button>
            ) : null}
          </div>
          <div className="compare-selectors">
            {/* Primary action for the entry state (§A7): adding a company is the
                one marked primary; the result surface is informational, no submit. */}
            <SelectField
              label={t("compare.addCompany")}
              value=""
              data-ux-primary-action="true"
              onChange={(event) => {
                addCompany(event.target.value);
                event.target.value = "";
              }}
            >
              <option value="">{t("compare.addCompany")}</option>
              {unselectedCompanies.map((company) => (
                <option key={company.id} value={company.id}>
                  {company.qualifiedTicker} — {company.displayName}
                </option>
              ))}
            </SelectField>
            <SelectField
              label={t("compare.fromWatchlist")}
              value=""
              onChange={(event) => {
                addWatchlist(event.target.value);
                event.target.value = "";
              }}
            >
              <option value="">{t("compare.fromWatchlist")}</option>
              {watchlists.map((watchlist) => (
                <option key={watchlist.id} value={watchlist.id}>
                  {watchlist.name}
                </option>
              ))}
            </SelectField>
          </div>
        </section>

        {/* View toggle + per-view controls (§A7 frame "Widok Profil"). Profil is
            the entry default; Trend hosts the one-KPI picker + granularity. */}
        <section className="compare-controls" aria-label={t("compare.section.kpi")}>
          <SegmentedControl ariaLabel={t("compare.view.label")}>
            <SegmentedControlOption active={view === "profil"} onClick={() => setView("profil")}>
              {t("compare.view.profil")}
            </SegmentedControlOption>
            <SegmentedControlOption active={view === "trend"} onClick={() => setView("trend")}>
              {t("compare.view.trend")}
            </SegmentedControlOption>
          </SegmentedControl>

          {view === "profil" && axis.length > 0 ? (
            <SelectField
              label={t("compare.period.label")}
              value={period}
              onChange={(event) => setPeriod(event.target.value)}
            >
              {periodOptions.map((option) => (
                <option key={option.key} value={option.key}>
                  {option.label}
                </option>
              ))}
            </SelectField>
          ) : null}

          {view === "trend" ? (
            <>
              <SelectField
                label={t("compare.kpi.label")}
                value={metricKey}
                onChange={(event) => setMetricKey(event.target.value)}
              >
                {kpiDefs.map((def) => (
                  <option key={def.id} value={def.metricKey}>
                    {localizedKpiLabel(def, locale)}
                  </option>
                ))}
              </SelectField>
              <SegmentedControl ariaLabel={t("compare.granularity.label")}>
                <SegmentedControlOption
                  active={granularity === "annual"}
                  onClick={() => setGranularity("annual")}
                >
                  {t("compare.granularity.annual")}
                </SegmentedControlOption>
                <SegmentedControlOption
                  active={granularity === "quarterly"}
                  onClick={() => setGranularity("quarterly")}
                >
                  {t("compare.granularity.quarterly")}
                </SegmentedControlOption>
              </SegmentedControl>
            </>
          ) : null}
        </section>

        {/* Result region: invite → loading → error → no-data → table (frames 3/4/5). */}
        {status === "loading" ? (
          <section className="compare-result" aria-busy="true" aria-label={t("compare.section.table")}>
            <SectionHeader level="h3" title={t("compare.section.table")} />
            <Skeleton variant="list-row" count={3} />
          </section>
        ) : null}

        {status === "error" ? (
          <section className="compare-result">
            <div className="compare-error">
              <ErrorText>{t("compare.error.load")}</ErrorText>
              <Button variant="secondary" onClick={retry}>
                {t("compare.retry")}
              </Button>
            </div>
          </section>
        ) : null}

        {status === "idle" && comparison === null ? (
          <EmptyState className="compare-invite">{t("compare.invite")}</EmptyState>
        ) : null}

        {showNoData ? <EmptyState>{t("compare.emptyNoData")}</EmptyState> : null}

        {/* Profil: all-KPI × companies at one period, Różnica for a pair. */}
        {showProfil ? (
          <section className="compare-result" aria-label={t("compare.section.table")}>
            <SectionHeader level="h3" title={t("compare.view.profil")} />
            <div className="compare-table-scroll" data-hscroll>
              <table className="compare-table">
                <thead>
                  <tr>
                    <th scope="col">{t("compare.table.kpi")}</th>
                    {selectedIds.map((id, index) => {
                      const company = companyById.get(id);
                      const fx = profileFxByCompany.get(id);
                      return (
                        <th key={id} scope="col">
                          <span className="compare-col-head">
                            <span
                              className={`compare-dot ${seriesSlotClass(index)}`}
                              aria-hidden="true"
                            />
                            {company ? (
                              <TickerLabel value={company.qualifiedTicker} />
                            ) : (
                              <span>{id}</span>
                            )}
                            {fx ? (
                              <StatusChip
                                tone="accent"
                                className="compare-fx-chip"
                                title={t("compare.fx.tooltip")}
                              >
                                {`${fx}→PLN`}
                              </StatusChip>
                            ) : null}
                          </span>
                        </th>
                      );
                    })}
                    {selectedIds.length === 2 ? (
                      <th scope="col">{t("compare.table.difference")}</th>
                    ) : null}
                  </tr>
                </thead>
                <tbody>
                  {profileRows.map((row) => (
                    <tr key={row.key}>
                      <th scope="row" className="compare-row-head">
                        {row.label}
                      </th>
                      {row.cells.map((entry) => {
                        const company = companyById.get(entry.id);
                        return (
                          <td key={entry.id}>
                            {entry.view.kind === "value" ? (
                              <span className="compare-value">
                                {defsByKey.get(row.key) && isRatioKind(defsByKey.get(row.key)!.valueKind)
                                  ? formatPointsValue(locale, entry.view.text, defsByKey.get(row.key)!.valueKind)
                                  : formatValue(locale, entry.view.text)}
                                {entry.view.factId ? (
                                  <Button
                                    variant="icon"
                                    className="compare-evidence"
                                    icon={<ExternalLink size={12} aria-hidden="true" />}
                                    aria-label={`${t("compare.evidence.open")} ${company?.qualifiedTicker ?? entry.id}`}
                                    onClick={() => openCompanyWorkspace?.(entry.id)}
                                  />
                                ) : null}
                              </span>
                            ) : entry.view.kind === "gap" ? (
                              <StatusChip tone="warn" className="compare-flag">
                                {flagLabel(entry.view.flag)}
                              </StatusChip>
                            ) : (
                              <span className="compare-empty-cell">—</span>
                            )}
                          </td>
                        );
                      })}
                      {row.difference ? (
                        <td>
                          <span
                            className={
                              row.difference.tone === "up"
                                ? "compare-delta-up"
                                : row.difference.tone === "dn"
                                  ? "compare-delta-dn"
                                  : "compare-empty-cell"
                            }
                          >
                            {row.difference.text}
                          </span>
                        </td>
                      ) : null}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <Hint>{t("compare.evidence.foot")}</Hint>
          </section>
        ) : null}

        {/* Trend: one KPI across the aligned period axis + the multi-series overlay. */}
        {showTrend ? (
          <section className="compare-result" aria-label={t("compare.section.table")}>
            <SectionHeader
              level="h3"
              title={`${metricLabel} — ${
                granularity === "annual"
                  ? t("compare.granularity.annual")
                  : t("compare.granularity.quarterly")
              }`}
            />
            <div className="compare-table-scroll" data-hscroll>
              <table className="compare-table">
                <thead>
                  <tr>
                    <th scope="col">{t("compare.table.company")}</th>
                    {axis.map((axisPeriod) => (
                      <th key={axisPeriod.key} scope="col">
                        {axisPeriod.periodType === "FY"
                          ? axisPeriod.fiscalYear
                          : `${axisPeriod.fiscalYear} ${axisPeriod.periodType}`}
                      </th>
                    ))}
                    <th scope="col">{t("compare.table.deltaYoY")}</th>
                    {granularity === "quarterly" ? (
                      <th scope="col">{t("compare.table.deltaQoQ")}</th>
                    ) : null}
                  </tr>
                </thead>
                <tbody>
                  {selectedIds.map((id, index) => {
                    const company = companyById.get(id);
                    const series = seriesByCompany.get(id);
                    const cells = series?.cells ?? [];
                    const valueKind = series?.valueKind ?? null;
                    const nonPlnCurrency = cells.find(
                      (cell) =>
                        cell.currency !== null &&
                        cell.currency !== "PLN" &&
                        !cell.flags.includes("currency_unknown"),
                    )?.currency;
                    const latest = cells[cells.length - 1];
                    return (
                      <tr key={id}>
                        <th scope="row" className="compare-row-head">
                          <span className="compare-row-head-inner">
                            <span
                              className={`compare-dot ${seriesSlotClass(index)}`}
                              aria-hidden="true"
                            />
                            {company ? (
                              <TickerLabel value={company.qualifiedTicker} />
                            ) : (
                              <span>{id}</span>
                            )}
                            {nonPlnCurrency ? (
                              <StatusChip
                                tone="accent"
                                className="compare-fx-chip"
                                title={t("compare.fx.tooltip")}
                              >
                                {`${nonPlnCurrency}→PLN`}
                              </StatusChip>
                            ) : null}
                          </span>
                        </th>
                        {axis.map((axisPeriod, cellIndex) => {
                          const cell = cells[cellIndex];
                          const gapFlag = cell?.flags.find((flag) => GAP_FLAGS.has(flag));
                          // Only a PLN-comparable value is rendered as a number;
                          // an unconverted native value (fx_missing / unknown
                          // currency) shows its typed flag, never a misleading
                          // number (ADR 0089 dec. 8 "no unconverted comparables").
                          const shown = gapFlag ? null : (cell?.valuePln ?? null);
                          return (
                            <td key={axisPeriod.key}>
                              {shown !== null && cell ? (
                                <span className="compare-value">
                                  {formatValue(locale, shown)}
                                  {cell.factId ? (
                                    <Button
                                      variant="icon"
                                      className="compare-evidence"
                                      icon={<ExternalLink size={12} aria-hidden="true" />}
                                      aria-label={`${t("compare.evidence.open")} ${company?.qualifiedTicker ?? id}`}
                                      onClick={() => openCompanyWorkspace?.(id)}
                                    />
                                  ) : null}
                                </span>
                              ) : gapFlag ? (
                                <StatusChip tone="warn" className="compare-flag">
                                  {flagLabel(gapFlag)}
                                </StatusChip>
                              ) : (
                                <span className="compare-empty-cell">—</span>
                              )}
                            </td>
                          );
                        })}
                        <td>{renderDelta(locale, latest?.deltaYoY ?? null, latest?.flags.includes("delta_yoy_undefined") ?? false, valueKind, t("compare.flag.deltaUndefined"))}</td>
                        {granularity === "quarterly" ? (
                          <td>{renderDelta(locale, latest?.deltaQoQ ?? null, latest?.flags.includes("delta_qoq_undefined") ?? false, valueKind, t("compare.flag.deltaUndefined"))}</td>
                        ) : null}
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <Hint>{t("compare.evidence.foot")}</Hint>
            {/* Trend overlay (§A4) — SVG width-bound inside a min-width:0 container
                so it stacks under the table at narrow widths without overflow.
                Series colours share the selection-order slot tokens with the chips
                + table row dots. */}
            <section
              className="compare-chart"
              data-compare-chart-mount
              aria-label={t("compare.section.chart")}
            >
              <SectionHeader level="h3" title={t("compare.section.chart")} />
              <MultiLineChart
                series={chartSeries}
                ariaLabel={`${t("compare.section.chart")} — ${metricLabel}`}
                formatValue={(value) => groupFormat(value, locale, 2)}
              />
              {chartCapped ? (
                <Hint>
                  {`${t("compare.chart.cap")} ${selectedIds.length} ${pluralNoun(
                    locale,
                    selectedIds.length,
                    COMPANY_FORMS,
                  )}`}
                </Hint>
              ) : null}
            </section>
          </section>
        ) : null}

        {/* Comparative valuation L1 (§B3, storyboard frame 4 bottom + frame 6).
            Scoped to one company among the zestaw; independent reactive reads,
            so it renders alongside the comparison whenever a set exists. */}
        {selectedIds.length >= 2 ? (
          <CompareValuation companyIds={selectedIds} companyById={companyById} />
        ) : null}
      </div>
    </section>
  );
}

function renderDelta(
  locale: LocaleCode,
  delta: string | null,
  undefinedFlag: boolean,
  valueKind: string | null,
  undefinedTitle: string,
) {
  if (undefinedFlag || delta === null) {
    return (
      <span className="compare-empty-cell" title={undefinedFlag ? undefinedTitle : undefined}>
        —
      </span>
    );
  }
  const parsed = Number(delta);
  const direction = parsed > 0 ? "compare-delta-up" : parsed < 0 ? "compare-delta-dn" : "";
  return <span className={direction}>{formatDelta(locale, delta, valueKind)}</span>;
}
