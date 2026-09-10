import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: unknown) => invokeMock(command, args),
}));

import {
  getCompanySector,
  listCompanySectors,
  setCompanySector,
} from "./companySector";
import { getCompanyIrReportsUrl, setCompanyIrReportsUrl } from "./ir";

// The thin company-config wrappers (ADR 0067 dec. 3 sector override, the IR
// reports URL) carry the exact command names contracts.md pins — a renamed
// command or a dropped argument reddens here before it reaches a screen.
describe("company config api wrappers", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
  });

  it("sector read/write/list name their commands and arguments", async () => {
    await getCompanySector("company_gpw_cdr");
    expect(invokeMock).toHaveBeenCalledWith("get_company_sector", {
      companyId: "company_gpw_cdr",
    });

    await setCompanySector("company_gpw_cdr", "Gaming");
    expect(invokeMock).toHaveBeenCalledWith("set_company_sector", {
      companyId: "company_gpw_cdr",
      sector: "Gaming",
    });

    await setCompanySector("company_gpw_cdr", null);
    expect(invokeMock).toHaveBeenLastCalledWith("set_company_sector", {
      companyId: "company_gpw_cdr",
      sector: null,
    });

    invokeMock.mockResolvedValueOnce(["Banking", "Gaming"]);
    await expect(listCompanySectors()).resolves.toEqual(["Banking", "Gaming"]);
    expect(invokeMock).toHaveBeenLastCalledWith(
      "list_company_sectors",
      undefined,
    );
  });

  it("IR reports URL read/write name their commands and arguments", async () => {
    await getCompanyIrReportsUrl("company_gpw_cdr");
    expect(invokeMock).toHaveBeenCalledWith("get_company_ir_reports_url", {
      companyId: "company_gpw_cdr",
    });

    await setCompanyIrReportsUrl(
      "company_gpw_cdr",
      "https://example.invalid/ir",
    );
    expect(invokeMock).toHaveBeenLastCalledWith("set_company_ir_reports_url", {
      companyId: "company_gpw_cdr",
      url: "https://example.invalid/ir",
    });
  });
});
