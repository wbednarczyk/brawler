import { describe, expect, it } from "vitest";

import { buildFactMatrix, groupFactMatrixRows, periodSortKey } from "./factMatrix";
import type { FactMatrixRow } from "./factMatrix";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";

function period(id: string, fiscalYear: number, periodType: string, periodEndDate: string | null): FinancialPeriod {
  return {
    id,
    companyId: "co1",
    fiscalYear,
    periodType,
    periodEndDate,
    reportEvidenceRef: null,
    createdAt: "",
    updatedAt: "",
  };
}

function definition(
  id: string,
  metricKey: string,
  label: string,
  overrides: Partial<Pick<KpiDefinition, "scope" | "statementGroup">> = {},
): KpiDefinition {
  return {
    id,
    scope: overrides.scope ?? "global",
    companyId: null,
    sector: null,
    metricKey,
    label,
    valueKind: "monetary",
    unit: null,
    computation: "reported",
    formula: null,
    displayFormat: null,
    origin: "seed",
    statementGroup: overrides.statementGroup ?? "other",
    createdAt: "",
    updatedAt: "",
  };
}

function fact(id: string, periodId: string, definitionId: string, metricKey = definitionId): FinancialFact {
  return {
    id,
    companyId: "co1",
    periodId,
    definitionId,
    metricKey,
    valueNumeric: "1",
    currency: "PLN",
    statementBasis: "consolidated",
    attribution: "total",
    variant: "actual",
    measureWindow: "period",
    dataQuality: "final",
    asReportedValue: null,
    asReportedScale: null,
    reportingStandard: null,
    extractionMethod: "manual",
    confidence: null,
    confirmationState: "confirmed",
    supersedesId: null,
    sourceDocumentRef: null,
    annotation: null,
    createdAt: "",
    updatedAt: "",
  };
}

describe("buildFactMatrix", () => {
  it("orders periods oldest-to-newest with annual after the quarters", () => {
    const periods = [
      period("p_q3", 2025, "q3", "2025-09-30"),
      period("p_2024", 2024, "annual", "2024-12-31"),
      period("p_q1", 2025, "q1", "2025-03-31"),
    ];
    const matrix = buildFactMatrix(periods, [], []);
    expect(matrix.periods.map((p) => p.id)).toEqual(["p_2024", "p_q1", "p_q3"]);
  });

  it("derives an order when period end dates are missing", () => {
    expect(periodSortKey(period("x", 2025, "q4", null))).toBe("2025-12");
    expect(periodSortKey(period("x", 2025, "annual", null)) > periodSortKey(period("y", 2025, "q4", null))).toBe(true);
  });

  it("builds one row per KPI that has facts, with a sparse period->fact map", () => {
    const periods = [period("p1", 2025, "q1", "2025-03-31"), period("p2", 2025, "q2", "2025-06-30")];
    const definitions = [
      definition("d_rev", "revenue", "Revenue"),
      definition("d_np", "net_profit", "Net profit"),
      definition("d_unused", "ebitda", "EBITDA"),
    ];
    const facts = [
      fact("f1", "p1", "d_rev"),
      fact("f2", "p2", "d_rev"),
      fact("f3", "p2", "d_np"),
    ];

    const matrix = buildFactMatrix(periods, facts, definitions);

    expect(matrix.rows.map((row) => row.definition.id)).toEqual(["d_rev", "d_np"]);
    expect(matrix.rows[0].cells.p1?.id).toBe("f1");
    expect(matrix.rows[0].cells.p2?.id).toBe("f2");
    expect(matrix.rows[1].cells.p1).toBeUndefined();
    expect(matrix.rows[1].cells.p2?.id).toBe("f3");
  });

  it("never drops a fact whose definition is not loaded (synthesizes a row)", () => {
    const periods = [period("p1", 2026, "h1", "2026-06-30")];
    // Definitions list is empty (e.g. wrong scope loaded) but a confirmed fact
    // references a canonical definition id.
    const facts = [fact("f1", "p1", "kpidef_revenue")];

    const matrix = buildFactMatrix(periods, facts, []);

    expect(matrix.rows).toHaveLength(1);
    expect(matrix.rows[0].definition.id).toBe("kpidef_revenue");
    expect(matrix.rows[0].definition.label).toBe("Revenue");
    expect(matrix.rows[0].cells.p1?.id).toBe("f1");
  });

  // ADR 0093 dec. 2: preliminary and final coexist in the same slot (same
  // period + definition, distinguished only by `dataQuality`). The matrix
  // shows ONE fact per period, and it must be the final one — mirrors the
  // backend's final-preferred idiom (`metric_history`,
  // `stored_fact_set_filtered`) instead of the old "last object in the facts
  // array wins" behavior.
  describe("final-preferred cell selection (ADR 0093 dec. 2)", () => {
    it("prefers the final sibling when the preliminary fact comes LAST in the array (facts arrive created_at DESC — final first)", () => {
      const periods = [period("p1", 2025, "annual", "2025-12-31")];
      const definitions = [definition("d_rev", "revenue", "Revenue")];
      const finalFact = { ...fact("f_final", "p1", "d_rev"), dataQuality: "final" };
      const preliminaryFact = { ...fact("f_prelim", "p1", "d_rev"), dataQuality: "preliminary" };
      // Real ordering: created_at DESC, and the final fact is created LATER
      // (the audited report lands after the preliminary release) — so it
      // sorts first, and the preliminary sibling comes after it.
      const facts = [finalFact, preliminaryFact];

      const matrix = buildFactMatrix(periods, facts, definitions);

      expect(matrix.rows[0].cells.p1?.id).toBe("f_final");
      expect(matrix.rows[0].cells.p1?.dataQuality).toBe("final");
    });

    it("prefers the final sibling even when the preliminary fact comes FIRST in the array (last-wins would shadow it)", () => {
      const periods = [period("p1", 2025, "annual", "2025-12-31")];
      const definitions = [definition("d_rev", "revenue", "Revenue")];
      const finalFact = { ...fact("f_final", "p1", "d_rev"), dataQuality: "final" };
      const preliminaryFact = { ...fact("f_prelim", "p1", "d_rev"), dataQuality: "preliminary" };
      const facts = [preliminaryFact, finalFact];

      const matrix = buildFactMatrix(periods, facts, definitions);

      expect(matrix.rows[0].cells.p1?.id).toBe("f_final");
    });

    it("still shows a preliminary-only fact (no final sibling), with its quality exposed", () => {
      const periods = [period("p1", 2026, "h1", "2026-06-30")];
      const definitions = [definition("d_rev", "revenue", "Revenue")];
      const preliminaryFact = { ...fact("f_prelim", "p1", "d_rev"), dataQuality: "preliminary" };

      const matrix = buildFactMatrix(periods, [preliminaryFact], definitions);

      expect(matrix.rows[0].cells.p1?.id).toBe("f_prelim");
      expect(matrix.rows[0].cells.p1?.dataQuality).toBe("preliminary");
    });

    it("prefers final over an estimated sibling too (any non-final rank is equal, final always wins)", () => {
      const periods = [period("p1", 2025, "annual", "2025-12-31")];
      const definitions = [definition("d_rev", "revenue", "Revenue")];
      const estimatedFact = { ...fact("f_est", "p1", "d_rev"), dataQuality: "estimated" };
      const finalFact = { ...fact("f_final", "p1", "d_rev"), dataQuality: "final" };

      const matrix = buildFactMatrix(periods, [estimatedFact, finalFact], definitions);

      expect(matrix.rows[0].cells.p1?.id).toBe("f_final");
    });
  });
});

// Grouped matrix rows (card #307): rows collapse under a display group driven
// by the KPI definition's `statementGroup` (backend catalog field, migration
// `0130`) — income / balance / cash_flow / per_share / company / other, in
// that fixed order, with a company-scoped definition winning the "KPI
// operacyjne spółki" group ahead of `statementGroup`, and any group with no
// rows omitted entirely.
function rowFor(def: KpiDefinition): FactMatrixRow {
  return { definition: def, cells: {} };
}

describe("groupFactMatrixRows", () => {
  it("groups rows in the fixed approved order: income, balance, cash_flow, per_share, company, other", () => {
    // Deliberately supplied out of order to prove the grouper — not the
    // caller — imposes the display order.
    const rows = [
      rowFor(definition("d_other", "roe", "ROE", { statementGroup: "other" })),
      rowFor(definition("d_ps", "eps_basic", "EPS", { statementGroup: "per_share" })),
      rowFor(definition("d_bal", "total_assets", "Total assets", { statementGroup: "balance" })),
      rowFor(definition("d_inc", "revenue", "Revenue", { statementGroup: "income" })),
      rowFor(definition("d_cf", "operating_cash_flow", "OCF", { statementGroup: "cash_flow" })),
      rowFor(
        definition("d_co", "broker_client_count", "Broker clients", {
          scope: "company",
          statementGroup: "other",
        }),
      ),
    ];

    const groups = groupFactMatrixRows(rows);

    expect(groups.map((g) => g.key)).toEqual([
      "income",
      "balance",
      "cash_flow",
      "per_share",
      "company",
      "other",
    ]);
    expect(groups.find((g) => g.key === "income")?.rows[0].definition.id).toBe("d_inc");
    expect(groups.find((g) => g.key === "company")?.rows[0].definition.id).toBe("d_co");
  });

  it("routes a company-scoped definition into the company group regardless of its statementGroup default", () => {
    const rows = [
      rowFor(
        definition("d_co", "cfd_lots_traded", "CFD lots", {
          scope: "company",
          statementGroup: "other",
        }),
      ),
    ];

    const groups = groupFactMatrixRows(rows);

    expect(groups).toHaveLength(1);
    expect(groups[0].key).toBe("company");
  });

  it("sorts a company-scoped definition into its OWN statement group when it carries an explicit non-other value", () => {
    const rows = [
      rowFor(
        definition("d_co_income", "own_revenue_concept", "Own revenue concept", {
          scope: "company",
          statementGroup: "income",
        }),
      ),
    ];

    const groups = groupFactMatrixRows(rows);

    expect(groups).toHaveLength(1);
    expect(groups[0].key).toBe("income");
  });

  it("omits every group with no rows", () => {
    const rows = [rowFor(definition("d_inc", "revenue", "Revenue", { statementGroup: "income" }))];

    const groups = groupFactMatrixRows(rows);

    expect(groups).toHaveLength(1);
    expect(groups[0].key).toBe("income");
  });

  it("returns no groups for an empty row set", () => {
    expect(groupFactMatrixRows([])).toEqual([]);
  });

  it("is exposed on buildFactMatrix's result, staying in sync with rows", () => {
    const periods = [period("p1", 2025, "annual", "2025-12-31")];
    const definitions = [
      definition("d_rev", "revenue", "Revenue", { statementGroup: "income" }),
      definition("d_co", "broker_clients", "Broker clients", { scope: "company" }),
    ];
    const facts = [fact("f1", "p1", "d_rev"), fact("f2", "p1", "d_co")];

    const matrix = buildFactMatrix(periods, facts, definitions);

    expect(matrix.groups.map((g) => g.key)).toEqual(["income", "company"]);
  });
});
