import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanySectorField } from "./CompanySectorField";
import { getCompanySector, listCompanySectors, setCompanySector } from "../../api/companySector";

vi.mock("../../api/companySector", () => ({
  getCompanySector: vi.fn(),
  setCompanySector: vi.fn(),
  listCompanySectors: vi.fn(),
}));

const getMock = vi.mocked(getCompanySector);
const setMock = vi.mocked(setCompanySector);
const listSectorsMock = vi.mocked(listCompanySectors);

describe("CompanySectorField", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listSectorsMock.mockResolvedValue([]);
  });

  it("renders the current (registry-sourced) sector", async () => {
    getMock.mockResolvedValue("Technology");

    render(<CompanySectorField companyId="company_gpw_cdr" />);

    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Sector" })).toHaveValue("Technology");
    });
  });

  it("saves a manual override", async () => {
    const user = userEvent.setup();
    getMock.mockResolvedValue("Technology");
    setMock.mockResolvedValue("Gaming");

    render(<CompanySectorField companyId="company_gpw_cdr" />);
    const input = screen.getByRole("textbox", { name: "Sector" });
    await waitFor(() => expect(input).toHaveValue("Technology"));

    await user.clear(input);
    await user.type(input, "Gaming");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(setMock).toHaveBeenCalledWith("company_gpw_cdr", "Gaming");
    });
    await waitFor(() => expect(input).toHaveValue("Gaming"));
  });

  it("clears the manual override, reverting to the registry-sourced value", async () => {
    const user = userEvent.setup();
    getMock.mockResolvedValue("Gaming");
    setMock.mockResolvedValue("Technology");

    render(<CompanySectorField companyId="company_gpw_cdr" />);
    const input = screen.getByRole("textbox", { name: "Sector" });
    await waitFor(() => expect(input).toHaveValue("Gaming"));

    await user.click(screen.getByRole("button", { name: "Clear override" }));

    await waitFor(() => {
      expect(setMock).toHaveBeenCalledWith("company_gpw_cdr", null);
    });
    await waitFor(() => expect(input).toHaveValue("Technology"));
  });

  // v0.53 M2 T3, reworked 2026-07-14 (owner report: a ~90-chip taxonomy wall
  // is unusable): suggestions are type-to-filter — no chips render until the
  // typed value narrows the taxonomy, matches are capped, and an exact-only
  // match (the registry-filled common case) suggests nothing.
  it("renders no suggestion wall on load, even with a large taxonomy", async () => {
    getMock.mockResolvedValue(null);
    listSectorsMock.mockResolvedValue(
      Array.from({ length: 90 }, (_, i) => `Sector ${i}`),
    );

    render(<CompanySectorField companyId="company_gpw_cdr" />);
    await waitFor(() => expect(listSectorsMock).toHaveBeenCalled());

    expect(screen.queryByRole("group", { name: "Registry sectors" })).not.toBeInTheDocument();
  });

  it("typing filters the taxonomy into capped suggestions", async () => {
    const user = userEvent.setup();
    getMock.mockResolvedValue(null);
    listSectorsMock.mockResolvedValue([
      "banki komercyjne",
      "Biotechnologia",
      "budownictwo ogólne",
      "Gry",
      ...Array.from({ length: 20 }, (_, i) => `usługi ${i}`),
    ]);

    render(<CompanySectorField companyId="company_gpw_cdr" />);
    const input = screen.getByRole("textbox", { name: "Sector" });

    await user.type(input, "b");
    const group = await screen.findByRole("group", { name: "Registry sectors" });
    expect(group).toBeInTheDocument();
    // Case-insensitive substring: three sectors contain "b".
    expect(screen.getByRole("button", { name: "banki komercyjne" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Biotechnologia" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "budownictwo ogólne" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Gry" })).not.toBeInTheDocument();

    // The cap keeps a broad match from becoming a wall.
    await user.clear(input);
    await user.type(input, "usługi");
    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /usługi/ }).length).toBeLessThanOrEqual(12);
    });
  });

  it("an exact taxonomy value suggests nothing (the registry-filled common case)", async () => {
    getMock.mockResolvedValue("Gry");
    listSectorsMock.mockResolvedValue(["Gry", "banki komercyjne"]);

    render(<CompanySectorField companyId="company_gpw_cdr" />);
    const input = screen.getByRole("textbox", { name: "Sector" });
    await waitFor(() => expect(input).toHaveValue("Gry"));

    expect(screen.queryByRole("group", { name: "Registry sectors" })).not.toBeInTheDocument();
  });

  it("clicking a suggestion fills the input, and Save persists it", async () => {
    const user = userEvent.setup();
    getMock.mockResolvedValue(null);
    listSectorsMock.mockResolvedValue(["banki komercyjne", "Gry"]);
    setMock.mockResolvedValue("banki komercyjne");

    render(<CompanySectorField companyId="company_gpw_cdr" />);
    const input = screen.getByRole("textbox", { name: "Sector" });

    await user.type(input, "bank");
    await user.click(await screen.findByRole("button", { name: "banki komercyjne" }));
    expect(input).toHaveValue("banki komercyjne");

    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(setMock).toHaveBeenCalledWith("company_gpw_cdr", "banki komercyjne");
    });
  });
});
