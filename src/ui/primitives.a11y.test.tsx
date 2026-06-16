import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { axe } from "jest-axe";

import {
  Button,
  Checkbox,
  ErrorText,
  Hint,
  ListRow,
  SectionHeader,
  SelectField,
  StatusChip,
  StatusPill,
  TextField,
  TextareaField,
} from "./index";

// Axe smoke test over a representative composition of the UI primitives, so the
// shared library keeps a clean accessibility baseline (labelled controls,
// alert role on errors, valid roles/ARIA). `region` and `color-contrast` are
// disabled: jsdom has no layout, and these primitives render outside a landmark
// here by design.
describe("UI primitives accessibility", () => {
  it("a representative composition has no axe violations", async () => {
    const { container } = render(
      <main>
        <SectionHeader
          title="Reports"
          level="h2"
          meta={<StatusChip tone="ok">3</StatusChip>}
          actions={<Button variant="primary">Add</Button>}
        />
        <Hint>Helper text for the section.</Hint>
        <TextField label="Ticker" defaultValue="CDR" />
        <SelectField label="Exchange" defaultValue="GPW">
          <option value="GPW">GPW</option>
          <option value="NC">NewConnect</option>
        </SelectField>
        <TextareaField label="Notes" defaultValue="hi" />
        <Checkbox label="Enabled" defaultChecked />
        <ErrorText>Something failed.</ErrorText>
        <p>
          <StatusPill tone="warn">Pending</StatusPill>
        </p>
        <ul>
          <ListRow
            title="report.pdf"
            href="https://example.com/r.pdf"
            meta="Bankier"
            trailing={<StatusChip>Stored</StatusChip>}
          />
        </ul>
      </main>,
    );

    const results = await axe(container, {
      rules: { region: { enabled: false }, "color-contrast": { enabled: false } },
    });

    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
