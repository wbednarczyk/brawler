import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { DriftDiff, parseDrift } from "./DriftDiff";

describe("parseDrift", () => {
  it("parses added/removed labels and a unit change from a DriftReport blob", () => {
    const drift = parseDrift(
      JSON.stringify({
        added_labels: ["Net profit from continuing operations"],
        removed_labels: ["Net profit"],
        unit_changed: ["PLN", "PLN thousand"],
      }),
    );

    expect(drift).toEqual({
      addedLabels: ["Net profit from continuing operations"],
      removedLabels: ["Net profit"],
      unitChanged: ["PLN", "PLN thousand"],
    });
  });

  it("returns null for a null blob", () => {
    expect(parseDrift(null)).toBeNull();
  });

  it("returns null for an undefined blob", () => {
    expect(parseDrift(undefined)).toBeNull();
  });

  it("returns null for malformed JSON rather than throwing", () => {
    expect(parseDrift("{not json")).toBeNull();
  });

  it("returns null for a blob with no actual diff (empty arrays, no unit change)", () => {
    expect(parseDrift(JSON.stringify({ added_labels: [], removed_labels: [] }))).toBeNull();
  });
});

describe("DriftDiff", () => {
  it("renders new-line and missing-line chip groups", () => {
    render(
      <DriftDiff
        drift={{
          addedLabels: ["New line A"],
          removedLabels: ["Missing line B"],
          unitChanged: null,
        }}
      />,
    );

    expect(screen.getByText("New line A")).toBeInTheDocument();
    expect(screen.getByText("Missing line B")).toBeInTheDocument();
    expect(screen.getByText("New lines")).toBeInTheDocument();
    expect(screen.getByText("Missing lines")).toBeInTheDocument();
  });

  it("renders the reporting-unit-changed line when present", () => {
    render(<DriftDiff drift={{ addedLabels: [], removedLabels: [], unitChanged: ["PLN", "PLN thousand"] }} />);

    expect(screen.getByText(/Reporting unit changed/)).toBeInTheDocument();
    expect(screen.getByText(/PLN thousand/)).toBeInTheDocument();
  });
});
