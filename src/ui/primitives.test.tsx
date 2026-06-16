import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { ErrorText, Hint, ListRow } from "./index";

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
