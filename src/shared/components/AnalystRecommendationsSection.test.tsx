import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import type { Company } from "../../api/types";
import type { AnalystRecommendationsView } from "../../api/analystRecommendations";
import { AnalystRecommendationsSection } from "./AnalystRecommendationsSection";

// v0.58 A3 (ADR 0073). Component proof per the experience-contract test-layer
// plan: empty / loading / error / partial / populated, the direction-badge
// mapping, and attribution rendered inseparably from every target.

const company = {
  id: "company_gpw_cdr",
  qualifiedTicker: "GPW:CDR",
  displayName: "CD PROJEKT S.A.",
} as Company;

const source = "https://www.biznesradar.pl/rekomendacje-spolki/CDR";

const populated: AnalystRecommendationsView = {
  companyId: "company_gpw_cdr",
  entries: [
    {
      firm: "Noble Securities",
      analyst: "Mateusz Chrzanowski",
      rating: "akumuluj",
      ratingPrev: "trzymaj",
      direction: "upgrade",
      targetPrice: "250.00",
      targetCurrency: "PLN",
      targetPrev: "230.00",
      priceAtIssue: "232.00",
      publishedAt: "2026-06-18T08:40:00",
      reportUrl: "https://static.example/rec/noble.pdf",
      sourceUrl: source,
    },
    {
      firm: "BM mBank",
      analyst: null,
      rating: "trzymaj",
      ratingPrev: null,
      direction: "initiate",
      targetPrice: null,
      targetCurrency: null,
      targetPrev: null,
      priceAtIssue: null,
      publishedAt: "2025-11-26T00:00:00",
      reportUrl: null,
      sourceUrl: source,
    },
  ],
  latestTarget: {
    firm: "Noble Securities",
    targetPrice: "250.00",
    targetCurrency: "PLN",
    publishedAt: "2026-06-18T08:40:00",
  },
  lastRefreshedAt: "2026-07-19T08:12:00Z",
};

const base = { company, error: null, loading: false, onRetry: vi.fn() };

describe("AnalystRecommendationsSection (v0.58 A3, ADR 0073)", () => {
  it("renders the attributed history, summary, and direction badges", () => {
    render(<AnalystRecommendationsSection {...base} view={populated} />);

    // ADR 0073 hard rule: the not-advice attribution is stated inline.
    expect(
      screen.getByText(/Brokerage opinions — not investment advice/),
    ).toBeInTheDocument();

    // Summary: latest target with its firm+date attribution inseparable from it.
    expect(screen.getByText("Latest target price")).toBeInTheDocument();
    expect(screen.getByText(/Noble Securities · 18\.06\.2026/)).toBeInTheDocument();
    expect(screen.getByText("Entries in local history")).toBeInTheDocument();

    // Verbatim ratings.
    expect(screen.getByText("akumuluj")).toBeInTheDocument();
    expect(screen.getByText("trzymaj")).toBeInTheDocument();

    // Direction sub-labels: upgrade carries the same-firm prior rating; the
    // firm's first entry is an initiate ("new").
    expect(screen.getByText(/▲ from trzymaj/)).toBeInTheDocument();
    expect(screen.getByText("new")).toBeInTheDocument();

    // Local wall-clock date rendered without UTC conversion (summary + row).
    expect(screen.getAllByText("18.06.2026").length).toBeGreaterThan(0);

    // Broker PDF link present on the entry that has one.
    expect(screen.getByRole("link", { name: /Broker PDF/ })).toBeInTheDocument();

    // Footer honesty line + refresh timestamp.
    expect(screen.getByText(/Ratings quoted verbatim from the source/)).toBeInTheDocument();
    expect(screen.getByText(/Last refresh/)).toBeInTheDocument();
  });

  it("shows the vs-price upside per row when a close is available", () => {
    const { container } = render(
      <AnalystRecommendationsSection {...base} view={populated} lastClose={232.2} currency="PLN" />,
    );
    // (250 - 232.2) / 232.2 ≈ +7.66% — the delta span renders sign, percent, label.
    const delta = container.querySelector(".analyst-recs-delta");
    expect(delta).not.toBeNull();
    expect(delta?.textContent).toMatch(/\+7\.6/);
    expect(delta?.textContent).toMatch(/vs price/);
  });

  it("renders partial entries without a bare or blank row (no target, no PDF)", () => {
    render(<AnalystRecommendationsSection {...base} view={populated} />);
    // The mBank entry has no target and no PDF — its row still renders the firm.
    expect(screen.getByText("BM mBank")).toBeInTheDocument();
    // A dash stands in for the missing target and the missing PDF (≥2 dashes).
    expect(screen.getAllByText("—").length).toBeGreaterThanOrEqual(2);
  });

  it("renders the empty state message and no rows", () => {
    render(
      <AnalystRecommendationsSection
        {...base}
        view={{ companyId: "company_gpw_cdr", entries: [] }}
      />,
    );
    expect(
      screen.getByText("No analyst recommendations for this company yet"),
    ).toBeInTheDocument();
    expect(screen.queryByText("akumuluj")).not.toBeInTheDocument();
  });

  it("renders a loading skeleton with no rows and no empty message", () => {
    const { container } = render(
      <AnalystRecommendationsSection {...base} loading view={null} />,
    );
    expect(container.querySelector(".analyst-recs-skeleton")).not.toBeNull();
    expect(
      screen.queryByText("No analyst recommendations for this company yet"),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("akumuluj")).not.toBeInTheDocument();
  });

  it("surfaces a read error with retry while keeping stale data visible", () => {
    const onRetry = vi.fn();
    render(
      <AnalystRecommendationsSection
        {...base}
        onRetry={onRetry}
        error="boom"
        view={populated}
      />,
    );
    expect(screen.getByText(/Could not load analyst recommendations/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
    // Stale-but-shown: the previously-loaded entries stay on screen.
    expect(screen.getByText("akumuluj")).toBeInTheDocument();
  });
});
