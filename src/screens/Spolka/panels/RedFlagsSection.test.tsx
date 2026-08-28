import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { Company } from "../../../api/types";
import type { RedFlagsView } from "../../../api/redFlags";
import { RedFlagsSection } from "./RedFlagsSection";

const company = { id: "c1", qualifiedTicker: "GPW:RFT", displayName: "Red Flag Test S.A." } as Company;

const populated: RedFlagsView = {
  active: [
    {
      flagId: "rf:auditor_red_flag:c1:fi-aud",
      flagType: "auditor_red_flag",
      severity: "high",
      title: "Opinia z zastrzeżeniem biegłego rewidenta",
      raisedDate: "2026-05-01",
      evidenceUrl: "https://example.test/audit",
      evidenceFeedItemId: "fi-aud",
      ackedAt: null,
    },
    {
      flagId: "rf:fund_exit:c1:beta@2025-12-31",
      flagType: "fund_exit",
      severity: "medium",
      title: "Wyjście z akcjonariatu: Fundusz Beta OFE",
      raisedDate: "2025-12-31",
      evidenceUrl: null,
      evidenceFeedItemId: null,
      ackedAt: null,
    },
  ],
  history: [
    {
      flagId: "rf:report_delay:c1:evt-1",
      flagType: "report_delay",
      severity: "high",
      title: "Opóźniony raport okresowy: Raport roczny",
      raisedDate: "2026-04-01",
      evidenceUrl: null,
      evidenceFeedItemId: null,
      ackedAt: "2026-04-05T10:00:00Z",
    },
  ],
};

const emptyView: RedFlagsView = { active: [], history: [] };

describe("RedFlagsSection (v0.57 T7, ADR 0083 D8)", () => {
  it("renders active flags with severity chips and type labels", () => {
    render(
      <RedFlagsSection company={company} view={populated} error={null} onAcknowledge={() => {}} />,
    );
    expect(screen.getByText("Auditor red flag")).toBeInTheDocument();
    expect(screen.getByText("Fund exit")).toBeInTheDocument();
    expect(screen.getByText("Opinia z zastrzeżeniem biegłego rewidenta")).toBeInTheDocument();
    // Severity chips: one high, one medium.
    expect(screen.getByText("High")).toBeInTheDocument();
    // Two rows have a Medium chip (active fund_exit + history report_delay is high),
    // so at least the fund_exit one shows Medium.
    expect(screen.getAllByText("Medium").length).toBeGreaterThan(0);
  });

  it("shows a calm explicit empty state, never blank", () => {
    render(
      <RedFlagsSection company={company} view={emptyView} error={null} onAcknowledge={() => {}} />,
    );
    expect(screen.getByText("No active warning signals")).toBeInTheDocument();
    expect(
      screen.getByText(/Nothing to flag right now/),
    ).toBeInTheDocument();
  });

  it("acknowledges a flag through the inline confirm", async () => {
    const onAcknowledge = vi.fn();
    render(
      <RedFlagsSection
        company={company}
        view={populated}
        error={null}
        onAcknowledge={onAcknowledge}
      />,
    );
    const user = userEvent.setup();
    // Each active row has an Acknowledge button; confirm the first.
    const ackButtons = screen.getAllByRole("button", { name: "Acknowledge" });
    await user.click(ackButtons[0]);
    // Inline confirm appears; confirm with Yes.
    await user.click(screen.getByRole("button", { name: "Yes" }));
    expect(onAcknowledge).toHaveBeenCalledWith("rf:auditor_red_flag:c1:fi-aud");
  });

  it("opens the evidence item when a feed item backs the flag", async () => {
    const onOpenEvidence = vi.fn();
    render(
      <RedFlagsSection
        company={company}
        view={populated}
        error={null}
        onAcknowledge={() => {}}
        onOpenEvidence={onOpenEvidence}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Open" }));
    expect(onOpenEvidence).toHaveBeenCalledWith("fi-aud");
  });

  it("reveals acknowledged history on expand", async () => {
    render(
      <RedFlagsSection company={company} view={populated} error={null} onAcknowledge={() => {}} />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Acknowledged history" }));
    expect(screen.getByText("Opóźniony raport okresowy: Raport roczny")).toBeInTheDocument();
  });
});
