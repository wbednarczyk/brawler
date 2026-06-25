import { useEffect, useMemo, useState } from "react";

import { listCompanyAutopilotModes, setCompaniesAutopilot, type AutopilotMode } from "../../api/autopilot";
import type { Company, Watchlist, WatchlistMembership } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { Checkbox, EmptyState, ErrorText, SearchField, SectionHeader, SelectField } from "../../ui";

export type CompanySettingsManagerProps = {
  companies: Company[];
  watchlists: Watchlist[];
  membershipsByCompany: Record<string, WatchlistMembership[]>;
};

const AUTOPILOT_MODES: AutopilotMode[] = ["off", "assist", "autopilot"];

/// Master-detail per-company settings surface (ADR 0056): the scalable home for
/// per-company settings. Left = company multi-select (with watchlist-scope
/// selection); right = grouped settings applied to the whole selection. v1 ships
/// the Automation group → autopilot mode (bulk); pinned and watchlist membership
/// are the next groups. Self-contained: loads and writes its own autopilot state.
export function CompanySettingsManager({
  companies,
  watchlists,
  membershipsByCompany,
}: CompanySettingsManagerProps) {
  const { text } = useLocale();
  const [modeByCompany, setModeByCompany] = useState<Record<string, AutopilotMode>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void listCompanyAutopilotModes()
      .then((rows) => {
        if (cancelled) return;
        const next: Record<string, AutopilotMode> = {};
        for (const row of rows) next[row.companyId] = row.mode as AutopilotMode;
        setModeByCompany(next);
      })
      .catch((cause) => {
        if (!cancelled) setError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const modeOf = (companyId: string): AutopilotMode => modeByCompany[companyId] ?? "off";

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase();
    if (!needle) return companies;
    return companies.filter(
      (company) =>
        company.displayName.toLowerCase().includes(needle) ||
        company.qualifiedTicker.toLowerCase().includes(needle),
    );
  }, [companies, search]);

  function toggle(companyId: string) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(companyId)) next.delete(companyId);
      else next.add(companyId);
      return next;
    });
  }

  function selectAll(ids: string[]) {
    setSelected(new Set(ids));
  }

  function selectWatchlist(watchlistId: string) {
    if (!watchlistId) return;
    const ids = companies
      .filter((company) =>
        (membershipsByCompany[company.id] ?? []).some((m) => m.watchlistId === watchlistId),
      )
      .map((company) => company.id);
    setSelected(new Set(ids));
  }

  const selectedIds = useMemo(() => [...selected], [selected]);
  const selectedModes = selectedIds.map(modeOf);
  const commonMode =
    selectedModes.length > 0 && selectedModes.every((m) => m === selectedModes[0])
      ? selectedModes[0]
      : ""; // "" => mixed

  async function applyAutopilotMode(mode: AutopilotMode) {
    if (selectedIds.length === 0) return;
    setBusy(true);
    setError(null);
    const previous = modeByCompany;
    setModeByCompany((current) => {
      const next = { ...current };
      for (const id of selectedIds) next[id] = mode;
      return next;
    });
    try {
      await setCompaniesAutopilot({ companyIds: selectedIds, mode });
    } catch (cause) {
      setModeByCompany(previous);
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="company-settings" aria-label={text("Company settings")}>
      <div className="company-settings-master">
        <SectionHeader level="h3" title={text("Companies")} />
        <SearchField
          ariaLabel={text("Search companies")}
          onChange={(value) => setSearch(value)}
          placeholder={text("Search companies")}
          value={search}
        />
        <div className="company-settings-select-row">
          <Checkbox
            className="company-settings-select-all"
            checked={filtered.length > 0 && filtered.every((company) => selected.has(company.id))}
            onChange={(event) =>
              event.target.checked
                ? selectAll(filtered.map((company) => company.id))
                : setSelected(new Set())
            }
            aria-label={text("Select all")}
            label={text("Select all")}
          />
          <SelectField
            aria-label={text("Select watchlist members")}
            label={text("By watchlist")}
            value=""
            onChange={(event) => selectWatchlist(event.target.value)}
          >
            <option value="">{text("Select a watchlist…")}</option>
            {watchlists.map((watchlist) => (
              <option key={watchlist.id} value={watchlist.id}>
                {watchlist.name}
              </option>
            ))}
          </SelectField>
        </div>
        <ul className="company-settings-list" aria-label={text("Company selection")}>
          {filtered.length === 0 ? (
            <EmptyState>{text("No companies match.")}</EmptyState>
          ) : (
            filtered.map((company) => (
              <li className="company-settings-row" key={company.id}>
                <Checkbox
                  checked={selected.has(company.id)}
                  onChange={() => toggle(company.id)}
                  label={
                    <span className="company-settings-row-label">
                      <span className="company-settings-row-name">{company.displayName}</span>
                      <span className="company-settings-row-meta">
                        <TickerLabel value={company.qualifiedTicker} />
                        <span className="company-settings-mode-badge">{modeOf(company.id)}</span>
                      </span>
                    </span>
                  }
                />
              </li>
            ))
          )}
        </ul>
      </div>

      <div className="company-settings-detail">
        <SectionHeader
          level="h3"
          title={text("Settings")}
          actions={
            <span className="company-settings-count">
              {text("{n} selected").replace("{n}", String(selectedIds.length))}
            </span>
          }
        />
        {selectedIds.length === 0 ? (
          <EmptyState>{text("Select companies on the left to edit their settings.")}</EmptyState>
        ) : (
          <section className="company-settings-group" aria-label={text("Automation")}>
            <SectionHeader level="h4" title={text("Automation")} />
            <SelectField
              aria-label={text("Autopilot mode")}
              label={text("Autopilot mode")}
              value={commonMode}
              disabled={busy}
              onChange={(event) => void applyAutopilotMode(event.target.value as AutopilotMode)}
            >
              {commonMode === "" ? <option value="">{text("— Mixed —")}</option> : null}
              {AUTOPILOT_MODES.map((mode) => (
                <option key={mode} value={mode}>
                  {mode === "off"
                    ? text("Off — manual")
                    : mode === "assist"
                      ? text("Assist — auto-extract, you confirm")
                      : text("Autopilot — auto-confirm (unreviewed)")}
                </option>
              ))}
            </SelectField>
            <p className="ai-analysis-empty">
              {text("Applies to all selected companies. Pinned and watchlist settings arrive here next.")}
            </p>
          </section>
        )}
        {error ? <ErrorText>{error}</ErrorText> : null}
      </div>
    </section>
  );
}
