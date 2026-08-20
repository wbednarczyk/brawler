import { describe, it, expect, vi } from "vitest";
import { createRef } from "react";
import { render, screen, fireEvent } from "@testing-library/react";

import { Button, Checkbox, DateField, ErrorText, ExpandableRow, FilterToolbar, Hint, ListRow, PanelHeader, ProvenanceFigure, RangeBarChart, SectionHeader, StatusChip, StatusPill, TextareaField } from "./index";
import { PrimitiveGallery } from "./PrimitiveGallery";

// ADR 0081 Q4: Button emits stable data-ui-button-variant metadata so scoped
// interaction-contract helpers (tests/browser/helpers/interactionContracts.ts)
// can find primitive icon buttons without falling back to a CSS class selector.
describe("Button", () => {
  it("emits a stable data-ui-button-variant attribute matching the variant prop", () => {
    render(<Button variant="primary">Save</Button>);
    expect(screen.getByRole("button", { name: "Save" })).toHaveAttribute("data-ui-button-variant", "primary");
  });

  it("emits the attribute for every variant, including the default", () => {
    const { rerender } = render(<Button>Default</Button>);
    expect(screen.getByRole("button", { name: "Default" })).toHaveAttribute("data-ui-button-variant", "secondary");

    rerender(
      <Button variant="icon" aria-label="icon action">
        x
      </Button>,
    );
    expect(screen.getByRole("button", { name: "icon action" })).toHaveAttribute("data-ui-button-variant", "icon");
  });

  it("forwards an explicit experience-contract primary-action marker without inferring it", () => {
    render(<Button data-ux-primary-action="true">Continue</Button>);
    expect(screen.getByRole("button", { name: "Continue" })).toHaveAttribute("data-ux-primary-action", "true");
  });
});

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

describe("ProvenanceFigure", () => {
  it("renders the label, figure value, and source ticket", () => {
    render(
      <ProvenanceFigure label="Zysk na akcję" value="3,49 zł" sourceTicket="ESPI · PSr 2026 · dziś" />,
    );
    expect(screen.getByText("Zysk na akcję")).toBeInTheDocument();
    expect(screen.getByText("3,49 zł")).toHaveClass("ui-provenance-figure-value");
    expect(screen.getByText("ESPI · PSr 2026 · dziś")).toHaveClass("ui-provenance-figure-ticket");
  });

  it("omits the ticket when no source is known yet", () => {
    render(<ProvenanceFigure label="Wartość księgowa" value="39,89 zł" />);
    expect(screen.getByText("39,89 zł")).toBeInTheDocument();
    expect(document.querySelector(".ui-provenance-figure-ticket")).toBeNull();
  });
});

describe("DateField", () => {
  it("renders a styled native date input carrying the design-system box + date classes", () => {
    render(<DateField label="Decided on" defaultValue="2026-06-01" />);
    const input = screen.getByLabelText("Decided on");
    expect(input).toHaveAttribute("type", "date");
    expect(input).toHaveClass("ui-text-input");
    expect(input).toHaveClass("ui-date-input");
    expect(input).toHaveValue("2026-06-01");
  });

  it("emits changes and forwards a ref like the other field primitives", () => {
    const onChange = vi.fn();
    const ref = createRef<HTMLInputElement>();
    render(<DateField ref={ref} aria-label="Decided on" value="" onChange={onChange} />);
    const input = screen.getByLabelText("Decided on");
    expect(ref.current).toBe(input);
    fireEvent.change(input, { target: { value: "2026-06-02" } });
    expect(onChange).toHaveBeenCalledTimes(1);
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

  // ADR 0076 Decision 6 (compact header, K3 double panel chrome): a panel's
  // leading header opts into the pane-lead hook so the shared `.cockpit-pane`
  // rule can visually compact its title. The class only tags the header — the
  // title must stay in the accessible tree so `aria-labelledby` /
  // `getByRole("heading")` keep resolving (the compaction is CSS clip-path, not
  // removal), and it is inert (no compaction) outside a `.cockpit-pane`.
  it("tags the leading header with the pane-lead hook while keeping the title accessible", () => {
    const { container } = render(<SectionHeader title="Quality" paneLead />);
    expect(container.querySelector(".ui-pane-lead-header")).not.toBeNull();
    expect(screen.getByRole("heading", { level: 2, name: "Quality" })).toBeInTheDocument();
  });

  it("does not add the pane-lead hook by default", () => {
    const { container } = render(<SectionHeader title="Quality" />);
    expect(container.querySelector(".ui-pane-lead-header")).toBeNull();
  });
});

describe("PanelHeader", () => {
  it("tags the leading header with the pane-lead hook while keeping the title accessible", () => {
    const { container } = render(<PanelHeader title="Research" paneLead titleId="r-title" />);
    expect(container.querySelector(".ui-pane-lead-header")).not.toBeNull();
    // The title stays an addressable h1 so `<section aria-labelledby>` and
    // heading queries still resolve it under the compact rule.
    const heading = screen.getByRole("heading", { level: 1, name: "Research" });
    expect(heading).toHaveAttribute("id", "r-title");
  });

  it("does not add the pane-lead hook by default", () => {
    const { container } = render(<PanelHeader title="Research" />);
    expect(container.querySelector(".ui-pane-lead-header")).toBeNull();
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

describe("ExpandableRow", () => {
  it("renders a disclosure chevron whose rotation reflects expansion (ADR 0076 D9)", () => {
    const { container, rerender } = render(
      <ExpandableRow label="Row" isExpanded={false} onToggle={() => {}} detail={<p>body</p>}>
        <span>header</span>
      </ExpandableRow>,
    );

    // The primitive renders the chevron itself — disclosure is never ARIA-only.
    const chevron = container.querySelector(".expandable-row-chevron");
    expect(chevron).not.toBeNull();
    // Decorative: the row already carries aria-expanded, so the icon is hidden.
    expect(chevron).toHaveAttribute("aria-hidden", "true");

    // Collapsed: the article advertises the collapsed state, chevron points right.
    const article = screen.getByRole("button", { name: "Row" });
    expect(article).toHaveAttribute("aria-expanded", "false");
    expect(article).not.toHaveClass("expandable-row-open");

    // Expanded: the open modifier drives the CSS rotation to ▾.
    rerender(
      <ExpandableRow label="Row" isExpanded={true} onToggle={() => {}} detail={<p>body</p>}>
        <span>header</span>
      </ExpandableRow>,
    );
    const openArticle = screen.getByRole("button", { name: "Row" });
    expect(openArticle).toHaveAttribute("aria-expanded", "true");
    expect(openArticle).toHaveClass("expandable-row-open");
  });

  it("toggles from keyboard without the chevron stealing the accessible name", () => {
    const onToggle = vi.fn();
    render(
      <ExpandableRow label="Keyboard row" isExpanded={false} onToggle={onToggle}>
        <span>header</span>
      </ExpandableRow>,
    );
    const article = screen.getByRole("button", { name: "Keyboard row" });
    fireEvent.keyDown(article, { key: "Enter" });
    fireEvent.keyDown(article, { key: " " });
    expect(onToggle).toHaveBeenCalledTimes(2);
  });

  it("renders action controls OUTSIDE the clickable article (no nested interactive)", () => {
    // A button nested inside the row's role="button" article is an axe
    // nested-interactive (WCAG 4.1.2) violation — action controls must be
    // siblings of the article, via the `actions` slot (card 02e991e #3).
    render(
      <ExpandableRow
        label="Row with actions"
        isExpanded={false}
        onToggle={() => {}}
        actions={<button type="button">Delete</button>}
      >
        <span>header</span>
      </ExpandableRow>,
    );
    const article = screen.getByRole("button", { name: "Row with actions" });
    const deleteButton = screen.getByRole("button", { name: "Delete" });
    expect(article.contains(deleteButton)).toBe(false);
  });
});

describe("FilterToolbar", () => {
  // jsdom has no container queries, so the *tier* (when the disclosure appears)
  // is asserted in the Playwright density matrix. Here we assert the disclosure
  // TOGGLE semantics the primitive owns so every consumer inherits the S-tier
  // "Filtry" collapse (ADR 0076 D6): search stays visible; the rest fold behind
  // an aria-expanded disclosure with a rotating chevron (D9).
  it("keeps the search slot visible and folds the controls behind a Filters disclosure", () => {
    render(
      <FilterToolbar ariaLabel="Test filters" search={<input aria-label="Search box" />}>
        <select aria-label="Kind filter" />
      </FilterToolbar>,
    );

    // Search is always rendered outside the collapsible region.
    expect(screen.getByLabelText("Search box")).toBeVisible();

    // The disclosure is a real button (never ARIA-only) with a rotating chevron.
    const disclosure = screen.getByRole("button", { name: "Filters" });
    const chevron = disclosure.querySelector(".filter-toolbar-chevron");
    expect(chevron).not.toBeNull();
    expect(chevron).toHaveAttribute("aria-hidden", "true");

    // Collapsed by default: the controls region advertises the folded state.
    expect(disclosure).toHaveAttribute("aria-expanded", "false");
    const controls = document.querySelector(".filter-toolbar-controls");
    expect(controls).toHaveAttribute("data-collapsed", "true");
    // The children live inside the collapsible region.
    expect(controls).toContainElement(screen.getByLabelText("Kind filter"));
  });

  it("expands and collapses the controls region on toggle", () => {
    render(
      <FilterToolbar ariaLabel="Test filters">
        <select aria-label="Kind filter" />
      </FilterToolbar>,
    );
    const disclosure = screen.getByRole("button", { name: "Filters" });
    const controls = document.querySelector(".filter-toolbar-controls");

    fireEvent.click(disclosure);
    expect(disclosure).toHaveAttribute("aria-expanded", "true");
    expect(controls).toHaveAttribute("data-collapsed", "false");

    fireEvent.click(disclosure);
    expect(disclosure).toHaveAttribute("aria-expanded", "false");
    expect(controls).toHaveAttribute("data-collapsed", "true");
  });
});

describe("RangeBarChart (football field)", () => {
  const rows = [
    { key: "pe", label: "P/E × median", low: 96, base: 122, high: 148, rangeText: "96–148 zł" },
    { key: "evebitda", label: "EV/EBITDA × median", low: 108, base: 131, high: 154, rangeText: "108–154 zł" },
    { key: "pbv", label: "P/BV × median", absentText: "too few peers" },
  ];

  it("is a labelled group exposing every method row label + range text", () => {
    render(
      <RangeBarChart
        ariaLabel="Implied fair-value ranges"
        rangeLegendLabel="implied range"
        rows={rows}
      />,
    );
    const group = screen.getByRole("group", { name: "Implied fair-value ranges" });
    expect(group).toBeInTheDocument();
    expect(screen.getByText("P/E × median")).toBeInTheDocument();
    expect(screen.getByText("EV/EBITDA × median")).toBeInTheDocument();
    expect(screen.getByText("96–148 zł")).toBeInTheDocument();
    expect(screen.getByText("108–154 zł")).toBeInTheDocument();
  });

  it("draws a bar per drawable row and positions the fill within the shared domain", () => {
    const { container } = render(
      <RangeBarChart ariaLabel="ff" rangeLegendLabel="implied range" rows={rows} />,
    );
    // Two drawable rows → two range fills; the absent row draws no track.
    const fills = container.querySelectorAll(".ui-range-bar-fill");
    expect(fills).toHaveLength(2);
    for (const fill of fills) {
      const left = parseFloat((fill as HTMLElement).style.left);
      const width = parseFloat((fill as HTMLElement).style.width);
      expect(left).toBeGreaterThanOrEqual(0);
      expect(left + width).toBeLessThanOrEqual(100.01);
    }
    expect(container.querySelectorAll(".ui-range-bar-track")).toHaveLength(2);
  });

  it("renders a typed-absence row as its reason text, never a zero-width bar", () => {
    render(<RangeBarChart ariaLabel="ff" rangeLegendLabel="implied range" rows={rows} />);
    expect(screen.getByText("too few peers")).toBeInTheDocument();
  });

  it("draws the shared marker line across every drawable row + a legend entry", () => {
    const { container } = render(
      <RangeBarChart
        ariaLabel="ff"
        rangeLegendLabel="implied range"
        markerLegendLabel="current price"
        marker={{ value: 132, label: "132 zł" }}
        rows={rows}
      />,
    );
    // One marker per drawable row (2), positioned inside the domain.
    const markers = container.querySelectorAll(".ui-range-bar-marker");
    expect(markers).toHaveLength(2);
    expect(screen.getByText(/current price — 132 zł/)).toBeInTheDocument();
  });
});

describe("Toast gallery entry", () => {
  it("renders the transient action-feedback queue and no alert region (ADR 0097)", () => {
    render(<PrimitiveGallery />);
    expect(screen.getByText("Sources refreshed")).toBeInTheDocument();
    const statuses = screen.getAllByRole("status");
    expect(statuses.some((node) => node.textContent?.includes("Sources refreshed"))).toBe(true);
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
    // The persistent (role=alert) toast variant is retired — no toast may be an
    // alert region (ErrorText elsewhere in the gallery legitimately uses one).
    const viewport = document.querySelector(".ui-toast-viewport");
    expect(viewport).not.toBeNull();
    expect(viewport?.querySelector('[role="alert"]')).toBeNull();
  });
});
