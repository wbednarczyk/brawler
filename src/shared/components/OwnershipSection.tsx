import { useState } from "react";
import { Pencil } from "lucide-react";

import { useLocale, type LocaleCode } from "../locale";
import { pluralNoun, type PluralForms } from "../locale/plural";
import { formatFinancialValue } from "../format/financialValue";
import {
  Button,
  DonutChart,
  donutSwatchClass,
  EmptyState,
  ErrorText,
  Hint,
  MULTI_LINE_MAX_SERIES,
  MultiLineChart,
  SectionHeader,
  SelectField,
  Skeleton,
  StatusChip,
  type DonutSlice,
  type DonutSliceKind,
  type MultiLineSeries,
} from "../../ui";
import type { OwnershipHolder, OwnershipHolderSeries, OwnershipOverview } from "../../api/ownership";

// Ownership ("Akcjonariat") section of the Basic Info panel (v0.56 T6, ADR 0072,
// storyboard docs/mockups/v056-ownership-section-storyboard.html). Pure-render
// against the OwnershipOverview DTO; the host (CompanyBasicInfoPanel) owns the
// IPC fetch + mutations and passes callbacks. Decision-support only.

export type OwnershipSectionProps = {
  /** `null` while loading. */
  data: OwnershipOverview | null;
  error?: string | null;
  /** A backfill/mutation is in flight (disables actions, shows progress). */
  busy?: boolean;
  onBackfill: () => void;
  /** Manually (re)classify a holder — the only classification path (ADR 0084). */
  onSetHolderType: (holderKey: string, holderType: string | null) => void;
  className?: string;
};

// The nine holder types (ADR 0072 §3) → the five validated donut hues. Hue is
// bound to holder TYPE, never cycled; >4 coloured groups fold into "misc".
const TYPE_KIND: Record<string, DonutSliceKind> = {
  founder_insider: "founder",
  ofe_pension: "ofe",
  tfi: "tfi",
  other_institutional: "tfi",
  family_foundation: "misc",
  state_treasury: "misc",
  parent_company: "misc",
  treasury_shares: "misc",
  free_float_rest: "uncertain",
};

const KIND_ORDER: DonutSliceKind[] = ["founder", "ofe", "tfi", "misc", "uncertain"];

// Holders classified as free float never appear as a separate legend row — the
// derived free-float slice already carries them.
function kindFor(holderType: string | null | undefined): DonutSliceKind | null {
  if (!holderType) return null;
  return TYPE_KIND[holderType] ?? "misc";
}

function parsePct(value: string | null | undefined): number {
  if (value === null || value === undefined) return 0;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function formatPct(value: string | null | undefined, locale: LocaleCode): string {
  if (value === null || value === undefined) return "—";
  return formatFinancialValue({ valueNumeric: value, valueKind: "percentage" }, locale);
}

const REPORT_FORMS: PluralForms = {
  en: ["report", "reports"],
  pl: ["raport", "raporty", "raportów"],
};

export function OwnershipSection({
  data,
  error,
  busy = false,
  onBackfill,
  onSetHolderType,
  className,
}: OwnershipSectionProps) {
  const { text, locale } = useLocale();
  // Inline re-type editor: which holder is open, and the drafted new type.
  const [editingHolder, setEditingHolder] = useState<string | null>(null);
  const [draftType, setDraftType] = useState<string>("");
  // Undo affordance after a manual re-type (survives the data refresh).
  const [justChanged, setJustChanged] = useState<{
    holderKey: string;
    name: string;
    previousType: string | null;
    newType: string;
  } | null>(null);

  const typeLabels: Record<string, string> = {
    founder_insider: text("Founder/insider"),
    family_foundation: text("Family foundation"),
    tfi: text("Fund manager (TFI)"),
    ofe_pension: text("Pension fund (OFE)"),
    state_treasury: text("State Treasury"),
    parent_company: text("Parent company"),
    treasury_shares: text("Treasury shares"),
    other_institutional: text("Other institutional"),
    free_float_rest: text("Free float"),
  };
  const kindGroupLabel: Record<DonutSliceKind, string> = {
    founder: text("Founders/insiders"),
    ofe: text("Pension funds (OFE)"),
    tfi: text("Funds/institutions"),
    misc: text("Other"),
    uncertain: text("Free float"),
    unknown: text("Unclassified"),
  };
  const typeLabel = (holderType: string | null | undefined): string =>
    holderType ? (typeLabels[holderType] ?? holderType) : text("Unclassified");

  // Ownership basis (ADR 0072 amended 2026-07-16): stakes can come from several
  // sources — label each honestly, falling back to the raw value for anything
  // unrecognised.
  const sourceLabels: Record<string, string> = {
    report_document: text("periodic report"),
    espi_filing: text("ESPI filing"),
    aggregator: text("BiznesRadar"),
    manual: text("manual entry"),
  };
  const sourceLabel = (source: string): string => sourceLabels[source] ?? source;

  const heading = (
    <SectionHeader
      level="h4"
      title={text("Ownership")}
      meta={
        data?.asOf ? (
          <span className="ownership-as-of">
            {text("as of")} {data.asOf}
            {data.source ? ` · ${sourceLabel(data.source)}` : ""}
          </span>
        ) : undefined
      }
    />
  );

  const wrap = (children: React.ReactNode) => (
    <div
      role="group"
      aria-label={text("Ownership")}
      className={["ownership-section", className].filter(Boolean).join(" ")}
    >
      {heading}
      {children}
    </div>
  );

  if (error) {
    return wrap(<ErrorText>{text("Failed to load ownership")}: {error}</ErrorText>);
  }
  if (!data) {
    return wrap(<Skeleton variant="list-row" count={3} label={text("Loading…")} />);
  }

  const hasHolders = data.holders.length > 0;

  // Empty state: no disclosed stakes and no residual yet → offer the backfill CTA.
  if (!hasHolders && data.residuals.length === 0) {
    return wrap(
      busy ? (
        <div className="ownership-progress" role="status">
          {text("Reading shareholder tables from the reports…")}
        </div>
      ) : (
        <EmptyState
          actions={
            <Button className="compact-button" data-ux-primary-action="true" onClick={onBackfill}>
              {text("Extract from reports")}
            </Button>
          }
        >
          {text("No ownership disclosures yet. I can read the saved periodic reports.")}
        </EmptyState>
      ),
    );
  }

  // ----- Donut slices grouped by kind (with single-type labelling) -----
  const kindAgg = new Map<DonutSliceKind, { value: number; types: Set<string> }>();
  // Not-yet-classified holders get their own honest slice (real-data harvest
  // 2026-07-16: dropping them silently let the donut renormalize and overstate
  // every classified share). They share the "misc" hue and vanish once typed.
  let unclassified = 0;
  for (const holder of data.holders) {
    const kind = kindFor(holder.holderType);
    if (kind === "uncertain") continue; // free_float folds into the derived slice
    if (!kind) {
      unclassified += parsePct(holder.capitalPct);
      continue;
    }
    const entry = kindAgg.get(kind) ?? { value: 0, types: new Set() };
    entry.value += parsePct(holder.capitalPct);
    if (holder.holderType) entry.types.add(holder.holderType);
    kindAgg.set(kind, entry);
  }
  const freeFloat = parsePct(data.freeFloatPct);
  const legendLabelFor = (kind: DonutSliceKind, types: Set<string>): string =>
    types.size === 1 ? typeLabel([...types][0]) : kindGroupLabel[kind];

  const slices: (DonutSlice & { display: string })[] = [];
  for (const kind of KIND_ORDER) {
    if (kind === "uncertain") continue;
    const entry = kindAgg.get(kind);
    if (!entry || entry.value <= 0) continue;
    const label = legendLabelFor(kind, entry.types);
    slices.push({ key: kind, kind, label, value: entry.value, display: label });
  }
  if (unclassified > 0) {
    slices.push({
      key: "unclassified",
      kind: "unknown",
      label: text("Unclassified"),
      value: unclassified,
      display: text("Unclassified"),
    });
  }
  if (freeFloat > 0) {
    slices.push({
      key: "free_float",
      kind: "uncertain",
      label: kindGroupLabel.uncertain,
      value: freeFloat,
      display: kindGroupLabel.uncertain,
    });
  }

  // ----- Top-holder capital trajectories (one LineChart per top holder) -----
  const sortedHolders = [...data.holders].sort(
    (a, b) => parsePct(b.capitalPct) - parsePct(a.capitalPct),
  );
  const seriesByKey = new Map<string, OwnershipHolderSeries>(
    data.history.map((serie) => [serie.holderKey, serie]),
  );
  // One merged chart, top holders by current capital (owner dogfooding
  // 2026-07-16: separate per-holder charts with independent y-scales hid the
  // relative picture). MULTI_LINE_MAX_SERIES caps the series count.
  const trajectories: MultiLineSeries[] = sortedHolders
    .map((holder) => ({ holder, serie: seriesByKey.get(holder.holderKey) }))
    .filter(
      (entry): entry is { holder: OwnershipHolder; serie: OwnershipHolderSeries } =>
        Boolean(entry.serie) &&
        (entry.serie?.points.filter((p) => p.capitalPct !== null).length ?? 0) >= 2,
    )
    .slice(0, MULTI_LINE_MAX_SERIES)
    .map(({ holder, serie }) => ({
      key: holder.holderKey,
      label: holder.name,
      legendValue: formatPct(holder.capitalPct ?? "0", locale),
      markerLabel: `${holder.name} — ${text("threshold crossing")}`,
      // An ESPI filing is compelled only when a holder crosses a statutory band,
      // so an `espi_filing`-sourced point IS the threshold-crossing event ADR 0072
      // decision 5 asks to mark. Aggregator and periodic-report points are ordinary
      // samples and stay unmarked.
      points: serie.points
        .filter((p) => p.capitalPct !== null)
        .map((p) => ({
          label: p.asOf,
          value: parsePct(p.capitalPct),
          marked: p.source === "espi_filing",
        })),
    }));
  // The derived free float joins the chart as a NEUTRAL dashed series (owner
  // dogfooding round 3) — concentration trend at a glance, visually consistent
  // with the donut's neutral float language.
  if (data.freeFloatHistory.length >= 2 && trajectories.length > 0) {
    trajectories.push({
      key: "free_float",
      label: text("Free float"),
      legendValue: formatPct(data.freeFloatPct, locale),
      neutral: true,
      points: data.freeFloatHistory.map((p) => ({ label: p.asOf, value: parsePct(p.pct) })),
    });
  }

  const renderTypeChip = (holder: OwnershipHolder) => {
    const kind = kindFor(holder.holderType);
    if (!holder.holderType) {
      return <span className="ownership-type-chip ownership-type-chip-unknown">{text("Unclassified")}</span>;
    }
    return (
      <span className={`ownership-type-chip ownership-type-chip-${kind ?? "misc"}`}>
        {typeLabel(holder.holderType)}
      </span>
    );
  };

  // Skin-in-the-game badge: a holder corroborated by a parsed management-holdings
  // row or an insider transaction — by person name, or as the vehicle a founder
  // holds through (ADR 0083 D6). Sits in the fixed trailing chip slot so it never
  // shifts the row layout (ui-authoring: trailing chips in fixed slots).
  const renderSkinBadge = (holder: OwnershipHolder) => {
    const skin = holder.skinInTheGame;
    if (!skin) return null;
    const detail = skin.via
      ? `${text("Corroborated by")}: ${skin.person} (${text("via")} ${skin.via})`
      : `${text("Corroborated by")}: ${skin.person}`;
    return (
      <span className="ownership-skin-badge" title={detail}>
        <StatusChip tone="ok">{text("skin in the game")}</StatusChip>
      </span>
    );
  };

  return wrap(
    <>
      <div className="ownership-overview-row">
        <div className="ownership-donut-wrap">
          <DonutChart
            ariaLabel={text("Ownership structure by holder type")}
            size={132}
            tooltip={text(
              "Free float is derived: 100% − sum of disclosed stakes; stakes below the 5% disclosure threshold stay hidden in it.",
            )}
            slices={slices}
            centerLabel={
              <>
                <span className="ownership-donut-center-value num-tabular">
                  {formatPct(data.freeFloatPct, locale)}
                </span>
                <span className="ownership-donut-center-label">{text("free float")}</span>
              </>
            }
          />
          <ul className="ownership-legend">
            {slices.map((slice) => (
              <li key={slice.key}>
                <span className={donutSwatchClass(slice.kind)} aria-hidden="true" />
                <span className="ownership-legend-label">{slice.display}</span>
                <b className="num-tabular">{formatPct(String(slice.value), locale)}</b>
              </li>
            ))}
          </ul>
        </div>
        {trajectories.length > 0 ? (
          <div className="ownership-trajectories">
            <span className="eyebrow">{text("Stakes over time")}</span>
            <MultiLineChart
              ariaLabel={text("Top holders — capital % over time")}
              height={120}
              series={trajectories}
              formatValue={(value) => formatPct(String(value), locale)}
            />
          </div>
        ) : null}
      </div>
      <ul className="ownership-holders">
        {sortedHolders.map((holder) => {
          const isEditing = editingHolder === holder.holderKey;
          return (
            <li key={holder.holderKey} className="ownership-holder-row">
              <div className="ownership-holder-main">
                <span className="ownership-holder-name">{holder.name}</span>
                {renderTypeChip(holder)}
                {renderSkinBadge(holder)}
              </div>
              <div className="ownership-holder-meta">
                <span className="num-tabular">
                  {formatPct(holder.capitalPct, locale)} {text("cap.")} ·{" "}
                  {formatPct(holder.votesPct, locale)} {text("votes")}
                </span>
                <Button
                  className="compact-button"
                  variant="minimal"
                  aria-label={`${text("Change type")}: ${holder.name}`}
                  disabled={busy}
                  onClick={() => {
                    setEditingHolder(holder.holderKey);
                    setDraftType(holder.holderType ?? "other_institutional");
                  }}
                >
                  <Pencil size={13} />
                </Button>
              </div>
              {isEditing ? (
                <div className="ownership-retype">
                  <SelectField
                    label={text("Change type to")}
                    value={draftType}
                    onChange={(event) => setDraftType(event.target.value)}
                  >
                    {Object.entries(typeLabels).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
                      </option>
                    ))}
                  </SelectField>
                  <Button
                    className="compact-button"
                    variant="secondary"
                    disabled={busy}
                    onClick={() => {
                      setJustChanged({
                        holderKey: holder.holderKey,
                        name: holder.name,
                        previousType: holder.holderType ?? null,
                        newType: draftType,
                      });
                      setEditingHolder(null);
                      onSetHolderType(holder.holderKey, draftType);
                    }}
                  >
                    {text("Save")}
                  </Button>
                  <Button className="compact-button" variant="minimal" onClick={() => setEditingHolder(null)}>
                    {text("Cancel")}
                  </Button>
                </div>
              ) : null}
            </li>
          );
        })}
      </ul>

      {justChanged ? (
        <div className="ownership-okbox" role="status">
          <span>
            {text("Changed to")} „{typeLabel(justChanged.newType)}" ({text("manually")}).
          </span>
          <Button
            className="compact-button"
            variant="minimal"
            onClick={() => {
              onSetHolderType(justChanged.holderKey, justChanged.previousType);
              setJustChanged(null);
            }}
          >
            {text("Undo")}
          </Button>
        </div>
      ) : null}

      {/* Residual warnbox: documents the deterministic parser could not read.
          The tier-4 OCR pass is retired (ADR 0084 decision 4), so there is no
          action to offer and no OCR lifecycle to report — the gap is stated
          honestly and stays flagged until a deterministic tier can read it. */}
      {data.residuals.length > 0 ? (
        <div className="ownership-warnbox" role="status">
          <p>
            {text(
              "Some report has an unreadable text layer (non-standard font), so its shareholder table could not be read. The gap is flagged here rather than guessed.",
            )}
          </p>
          <div className="ownership-warnbox-action">
            <span className="ownership-warnbox-count num-tabular">
              {data.residuals.length} {pluralNoun(locale, data.residuals.length, REPORT_FORMS)}
            </span>
          </div>
        </div>
      ) : null}

      <Hint>{text("A manual type always wins — automation never overwrites it.")}</Hint>
    </>,
  );
}
