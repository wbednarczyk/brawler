import { Plus } from "lucide-react";

import { ActionRow } from "./ActionRow";
import { Button, type ButtonVariant } from "./Button";
import { Checkbox } from "./Checkbox";
import { ChipList } from "./ChipList";
import { ClearButton } from "./ClearButton";
import { DenseRow } from "./DenseRow";
import { DetailSection } from "./DetailSection";
import { EmptyState } from "./EmptyState";
import { ErrorText } from "./ErrorText";
import { FieldRow, SelectField } from "./Fields";
import { Hint } from "./Hint";
import { InfoGrid } from "./InfoGrid";
import { InlineConfirm } from "./InlineConfirm";
import { ListRow } from "./ListRow";
import { Panel, PanelHeader } from "./Panel";
import { SearchField } from "./SearchField";
import { SectionHeader } from "./SectionHeader";
import { SegmentedControl, SegmentedControlOption } from "./SegmentedControl";
import { StatusChip } from "./StatusChip";
import { StatusPill } from "./StatusPill";
import { DateField } from "./DateField";
import { TextField } from "./TextField";
import { TextareaField } from "./TextareaField";

const noop = () => {};

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
          actions={<Button variant="primary" icon={<Plus size={14} />}>Add</Button>}
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
