import { describe, it, expect, vi } from "vitest";
import { createRef } from "react";
import { render, screen, fireEvent } from "@testing-library/react";

import { Checkbox, ErrorText, Hint, ListRow, SectionHeader, StatusChip, StatusPill, TextareaField } from "./index";

describe("ErrorText", () => {
  it("renders an alert with the error-text class", () => {
    render(<ErrorText>boom</ErrorText>);
    const node = screen.getByRole("alert");
    expect(node).toHaveTextContent("boom");
    expect(node).toHaveClass("error-text");
  });
});

describe("Hint", () => {
  it("renders muted helper text", () => {
    const { container } = render(<Hint>do the thing</Hint>);
    const node = container.querySelector(".ui-hint");
    expect(node).not.toBeNull();
    expect(node).toHaveTextContent("do the thing");
  });
});

describe("ListRow", () => {
  it("renders an external link with a truncating title and trailing content", () => {
    render(
      <ul>
        <ListRow
          title="very_long_report_filename.pdf"
          titleAttr="very_long_report_filename.pdf"
          href="https://example.com/r.pdf"
          meta="Bankier.pl"
          trailing={<span data-testid="badge">Stored</span>}
        />
      </ul>,
    );
    const link = screen.getByRole("link", { name: /very_long_report_filename/ });
    expect(link).toHaveAttribute("href", "https://example.com/r.pdf");
    expect(link).toHaveAttribute("target", "_blank");
    expect(screen.getByText("Bankier.pl")).toHaveClass("ui-list-row-meta");
    expect(screen.getByTestId("badge")).toBeInTheDocument();
  });

  it("renders a non-link row when no href is given", () => {
    render(
      <ul>
        <ListRow title="plain row" />
      </ul>,
    );
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.getByText("plain row")).toHaveClass("ui-list-row-title");
  });
});

describe("SectionHeader", () => {
  it("renders the title as an h2 by default", () => {
    render(<SectionHeader title="Overview" />);
    expect(screen.getByRole("heading", { level: 2, name: "Overview" })).toBeInTheDocument();
  });

  it("renders the title at the requested level with meta and actions (preserves document outline)", () => {
    render(
      <SectionHeader
        title="Reports"
        level="h3"
        meta={<span>3</span>}
        actions={
          <button type="button">Add</button>
        }
      />,
    );
    expect(screen.getByRole("heading", { level: 3, name: "Reports" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add" })).toBeInTheDocument();
  });
});

describe("Checkbox", () => {
  it("renders a labelled checkbox and fires onChange when toggled", () => {
    const onChange = vi.fn();
    render(<Checkbox label="Enabled" checked={false} onChange={onChange} />);
    const box = screen.getByRole("checkbox", { name: "Enabled" });
    expect(box).not.toBeChecked();
    fireEvent.click(box);
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("keeps a site className on the wrapping label and supports disabled", () => {
    const { container } = render(<Checkbox label="Off" className="x-toggle" disabled />);
    expect(container.querySelector("label.ui-checkbox.x-toggle")).not.toBeNull();
    expect(screen.getByRole("checkbox")).toBeDisabled();
  });
});

describe("TextareaField", () => {
  it("renders a bare textarea with the shared input classes and forwards a ref", () => {
    const ref = createRef<HTMLTextAreaElement>();
    render(<TextareaField ref={ref} aria-label="Notes" defaultValue="hi" />);
    const textarea = screen.getByLabelText("Notes");
    expect(textarea.tagName).toBe("TEXTAREA");
    expect(textarea).toHaveClass("ui-text-input", "ui-textarea-input");
    expect(ref.current).toBe(textarea);
  });

  it("renders a labelled field when given a label", () => {
    const { container } = render(<TextareaField label="Body" />);
    expect(container.querySelector("label.ui-text-field")).not.toBeNull();
  });
});

describe("StatusChip", () => {
  it("renders a toned chip", () => {
    render(<StatusChip tone="warn">Pending</StatusChip>);
    expect(screen.getByText("Pending")).toHaveClass("ui-status-chip", "ui-status-chip-warn");
  });
});

describe("StatusPill", () => {
  it("renders a toned pill, and neutral carries no tone modifier", () => {
    const { rerender } = render(<StatusPill tone="ok">Live</StatusPill>);
    expect(screen.getByText("Live")).toHaveClass("ui-status-pill", "ui-status-pill-ok");

    rerender(<StatusPill>Idle</StatusPill>);
    const idle = screen.getByText("Idle");
    expect(idle).toHaveClass("ui-status-pill");
    expect(idle.className).not.toContain("ui-status-pill-");
  });
});
