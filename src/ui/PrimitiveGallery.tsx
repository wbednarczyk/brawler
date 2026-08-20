import { Plus } from "lucide-react";
import { useEffect } from "react";

import { ActionRow } from "./ActionRow";
import { Button, type ButtonVariant } from "./Button";
import { Checkbox } from "./Checkbox";
import { ChipList } from "./ChipList";
import { ClearButton } from "./ClearButton";
import { DenseRow } from "./DenseRow";
import { DetailSection } from "./DetailSection";
import { DonutChart, donutSwatchClass } from "./DonutChart";
import { MultiLineChart } from "./MultiLineChart";
import { RangeBarChart } from "./RangeBarChart";
import { EmptyState } from "./EmptyState";
import { ErrorText } from "./ErrorText";
import { FieldRow, SelectField } from "./Fields";
import { Hint } from "./Hint";
import { InfoGrid } from "./InfoGrid";
import { InlineConfirm } from "./InlineConfirm";
import { ListRow } from "./ListRow";
import { ProvenanceFigure } from "./ProvenanceFigure";
import { Panel, PanelHeader } from "./Panel";
import { SearchField } from "./SearchField";
import { SectionHeader } from "./SectionHeader";
import { SegmentedControl, SegmentedControlOption } from "./SegmentedControl";
import { StatusChip } from "./StatusChip";
import { StatusPill } from "./StatusPill";
import { DateField } from "./DateField";
import { TextField } from "./TextField";
import { TextareaField } from "./TextareaField";
import { ToastProvider, useToast } from "./Toast";

const noop = () => {};

// Toast has no static "closed" markup like InlineConfirm — it only renders
// once queued via `useToast().show()`. The gallery is a standalone dev-only
// preview root (src/gallery.tsx), not part of the app shell, so it mounts its
// own ToastProvider here purely to demonstrate the transient action-feedback
// variant (the only one, ADR 0097); this is not a second app-wide mount.
function ToastGalleryDemo() {
  return (
    <ToastProvider>
      <ToastGalleryTriggers />
    </ToastProvider>
  );
}

function ToastGalleryTriggers() {
  const { show } = useToast();
  useEffect(() => {
    show({ message: "Sources refreshed", tone: "positive" });
    show({ message: "View deleted", actionLabel: "Undo", onAction: noop });
    // `show` is a stable useCallback identity, so this fires once on mount —
    // the gallery/a11y snapshot always shows the queue.
  }, [show]);
  return null;
}

const BUTTON_VARIANTS: ButtonVariant[] = [
  "primary",
  "secondary",
  "action",
  "ghost",
  "minimal",
  "danger",
];

const CHIP_TONES = ["neutral", "accent", "ok", "warn", "danger"] as const;
const PILL_TONES = ["neutral", "ok", "warn", "danger"] as const;

// A rendered catalog of every commonly-used src/ui primitive and its variants.
// Triple duty: a canonical visual reference for agents/humans (see gallery.html),
// the surface the jest-axe accessibility smoke test renders, and a stable target
// for opt-in visual-regression. Keep it in sync when adding/changing a primitive.
export function PrimitiveGallery() {
  return (
    <main className="primitive-gallery" aria-label="UI primitive gallery">
      <h1>Brawler UI primitives</h1>

      <section aria-labelledby="g-headers">
        <SectionHeader title="SectionHeader (h2, default)" titleId="g-headers" />
        <SectionHeader title="SectionHeader (h3)" level="h3" description="With a description line." />
        <SectionHeader
          title="SectionHeader (h4, accent, meta + actions)"
          level="h4"
          variant="accent"
          meta={<StatusChip tone="ok">3</StatusChip>}
          actions={
            // ADR 0081 Q4 reference usage: the explicit experience-contract
            // primary action for this section is marked by the CALLER, not
            // inferred from the "primary" variant.
            <Button data-ux-primary-action="true" variant="primary" icon={<Plus size={14} />}>
              Add
            </Button>
          }
        />
      </section>

      <section aria-labelledby="g-buttons">
        <SectionHeader title="Button" titleId="g-buttons" level="h3" />
        <ActionRow ariaLabel="Button variants">
          {BUTTON_VARIANTS.map((variant) => (
            <Button key={variant} variant={variant}>
              {variant}
            </Button>
          ))}
          <Button variant="icon" aria-label="icon button">
            <Plus size={16} />
          </Button>
        </ActionRow>
      </section>

      <section aria-labelledby="g-status">
        <SectionHeader title="StatusChip (quiet) / StatusPill (bold)" titleId="g-status" level="h3" />
        <ChipList ariaLabel="Status chips">
          {CHIP_TONES.map((tone) => (
            <StatusChip key={tone} tone={tone}>
              {tone}
            </StatusChip>
          ))}
        </ChipList>
        {/* Issue #209: a chip in a slot narrower than its label clips inside
            its own box (min-width:0 + max-width:100% + overflow hidden) —
            robust to ±px font-metric variance across environments. */}
        <div className="ui-chip-constrained-demo">
          <StatusChip tone="warn">Awaiting verification</StatusChip>
        </div>
        <ChipList ariaLabel="Status pills">
          {PILL_TONES.map((tone) => (
            <StatusPill key={tone} tone={tone}>
              {tone}
            </StatusPill>
          ))}
        </ChipList>
      </section>

      <section aria-labelledby="g-fields">
        <SectionHeader title="Form fields" titleId="g-fields" level="h3" />
        <FieldRow>
          <TextField label="TextField" defaultValue="CD PROJEKT" />
          <SelectField label="SelectField" defaultValue="GPW">
            <option value="GPW">GPW</option>
            <option value="NC">NewConnect</option>
          </SelectField>
          <DateField label="DateField" defaultValue="2026-06-01" />
        </FieldRow>
        <TextareaField label="TextareaField" defaultValue="Multi-line note…" />
        <Checkbox label="Checkbox" defaultChecked />
        <SearchField
          ariaLabel="SearchField"
          placeholder="Search…"
          value=""
          onChange={noop}
          onClear={noop}
          clearLabel="Clear search"
        />
        <label>
          Field with clear
          <span className="field-with-clear">
            <TextField defaultValue="value" />
            <ClearButton label="Clear field" onClick={noop} />
          </span>
        </label>
      </section>

      <section aria-labelledby="g-feedback">
        <SectionHeader title="Feedback text" titleId="g-feedback" level="h3" />
        <ErrorText>Block error (role=alert).</ErrorText>
        <p>
          Inline: <ErrorText as="span">span error</ErrorText>
        </p>
        <Hint>Muted helper / hint text.</Hint>
        <EmptyState>Nothing here yet.</EmptyState>
      </section>

      <section aria-labelledby="g-toast">
        <SectionHeader
          title="Toast (action feedback, ADR 0097)"
          titleId="g-toast"
          level="h3"
          description="Bottom-left queue, role=status, auto-dismisses after 6s, max 3 stacked. Feedback for a direct user action only (undo, import applied) — ambient attention lives in Today."
        />
        <ToastGalleryDemo />
      </section>

      <section aria-labelledby="g-data">
        <SectionHeader title="Data + rows" titleId="g-data" level="h3" />
        <InfoGrid
          ariaLabel="Sample metadata"
          items={[
            { label: "Ticker", value: "CDR" },
            { label: "Exchange", value: "GPW" },
            { label: "ISIN", value: "PLOPTTC00011" },
          ]}
        />
        <ul className="ui-list-rows">
          <ListRow
            title="annual_report_2025.pdf"
            href="https://example.com/r.pdf"
            meta="Bankier"
            trailing={<StatusChip tone="ok">Stored</StatusChip>}
          />
        </ul>
        <DenseRow interactive selected>
          <span>Selectable dense row</span>
        </DenseRow>
        <ProvenanceFigure
          label="Zysk na akcję"
          value="3,49 zł"
          sourceTicket="ESPI · PSr 2026 · dziś"
        />
      </section>

      <section aria-labelledby="g-charts">
        <SectionHeader title="Donut chart" titleId="g-charts" level="h3" />
        <div className="ui-donut-wrap-demo">
          <DonutChart
            ariaLabel="Ownership structure by holder type"
            centerLabel={<span className="num-tabular">46,8%</span>}
            slices={[
              { key: "founder", label: "Founders", value: 41.4, kind: "founder" },
              { key: "ofe", label: "OFE", value: 11.3, kind: "ofe" },
              { key: "misc", label: "Treasury", value: 0.5, kind: "misc" },
              { key: "float", label: "Free float", value: 46.8, kind: "uncertain" },
            ]}
          />
          <ul className="ui-donut-legend-demo">
            <li>
              <span className={donutSwatchClass("founder")} /> Founders 41,4%
            </li>
            <li>
              <span className={donutSwatchClass("ofe")} /> OFE 11,3%
            </li>
            <li>
              <span className={donutSwatchClass("uncertain")} /> Free float 46,8%
            </li>
          </ul>
        </div>
        <SectionHeader title="Multi-line chart" titleId="g-multiline" level="h3" />
        <MultiLineChart
          ariaLabel="Top holders — capital % over time"
          series={[
            {
              key: "duch",
              label: "Jacek Duch",
              legendValue: "25,2%",
              markerLabel: "Jacek Duch — threshold crossing",
              points: [
                { label: "2024-12-31", value: 25.5 },
                // `marked` renders the event tick (a threshold crossing here).
                { label: "2025-12-31", value: 25.5, marked: true },
                { label: "2026-03-31", value: 25.2 },
              ],
            },
            {
              key: "nn",
              label: "NN PTE",
              legendValue: "6,1%",
              points: [
                { label: "2025-12-31", value: 6.0 },
                { label: "2026-03-31", value: 6.1 },
              ],
            },
          ]}
        />
        <SectionHeader title="Range bar chart (football field)" titleId="g-rangebar" level="h3" />
        <RangeBarChart
          ariaLabel="Implied fair-value ranges by method"
          rangeLegendLabel="implied range"
          markerLegendLabel="current price"
          marker={{ value: 132, label: "132 zł" }}
          rows={[
            { key: "pe", label: "P/E × median", low: 96, base: 122, high: 148, rangeText: "96–148 zł" },
            { key: "evebitda", label: "EV/EBITDA × median", low: 108, base: 131, high: 154, rangeText: "108–154 zł" },
            { key: "pbv", label: "P/BV × median", absentText: "too few peers" },
          ]}
        />
      </section>

      <section aria-labelledby="g-nav">
        <SectionHeader title="Segmented control + inline confirm" titleId="g-nav" level="h3" />
        <SegmentedControl ariaLabel="View mode">
          <SegmentedControlOption active onClick={noop}>
            List
          </SegmentedControlOption>
          <SegmentedControlOption onClick={noop}>Week</SegmentedControlOption>
        </SegmentedControl>
        <InlineConfirm onConfirm={noop} onCancel={noop}>
          Delete this item?
        </InlineConfirm>
      </section>

      <section aria-labelledby="g-containers">
        <SectionHeader title="Containers" titleId="g-containers" level="h3" />
        <Panel ariaLabelledBy="g-panel-title">
          <PanelHeader title="Panel + PanelHeader" titleId="g-panel-title" description="Top-level screen panel." />
          <DetailSection title="DetailSection" description="Fixed-width rail card.">
            <p>Body content.</p>
          </DetailSection>
        </Panel>
      </section>
    </main>
  );
}
