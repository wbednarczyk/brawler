# ADR 0072: Ownership Structure — Shareholder Stakes, History, and Classification

Status: Accepted

Who owns a company — and how that changes — is first-order investor information (founder selling, funds entering/exiting, treasury buybacks, state control), and on the GPW it is all public disclosure. The app already stores the documents that contain it. This ADR makes ownership a first-class, automatically gathered, time-aware entity.

## Context

- Periodic reports carry a mandatory "shareholders holding ≥5% of votes" section; those report documents are already persisted (ADR 0036, ~3-year backfill).
- ESPI major-holdings notifications (threshold crossings under the public-offering act: 5%, 10%, …) are formulaic filings — ideal for the deterministic rule classifier (ADR 0034), and they keep the picture fresh between reports.
- Aggregator pages (BiznesRadar/Bankier "Akcjonariat") can serve as a second witness, mirroring the ADR 0061 witness pattern.
- Free float (audit gap P7) is derivable from disclosed stakes rather than needing its own source.

## Decision

1. **`ownership_stakes` entity**: company, normalized holder name, holder type, **% of capital and % of votes separately** (preferred-vote shares are common on the GPW and the gap is itself informative), as-of date, source (report document / ESPI filing / aggregator / manual), provenance links. Stored as append-only snapshots per (source, as-of) — history is the product, matching the financial-facts philosophy.
2. **Three ingestion streams**: (a) extraction of the shareholders table from already-stored periodic reports via the layered extraction pipeline (deterministic parse first, AI fallback with mandatory confirmation — ADR 0061 semantics); (b) a new typed signal `major_holdings_change` from ESPI threshold notifications, extending the ADR 0034 taxonomy, which also updates stakes; (c) aggregator ownership pages as a routine witness, never the source of truth.
3. **Holder-type classification**: `founder_insider` | `family_foundation` | `tfi` | `ofe_pension` | `state_treasury` | `parent_company` | `treasury_shares` | `other_institutional` | `free_float_rest`. A deterministic name dictionary (TFI/OFE/PFR registries are enumerable) classifies most; the residual goes through AI with confirm-before-apply.
4. **Derived free float** = 100% − Σ disclosed stakes, flagged with an uncertainty note (disclosure threshold means small institutional stakes hide in the float).
5. **Presentation**: a company-workspace ownership section — current-state donut/pie by holder (grouped by type) + a stake-over-time chart per holder with threshold-crossing events as markers. Chart styling follows the dataviz skill at implementation time.

## Consequences

- Fund exits/founder moves feed the red-flags panel and insider-sentiment view (v0.57) and become alertable (ADR 0068).
- Free float closes without a dedicated source; v0.53's registry work stays lean.
- New extraction target joins the real-data validation loop (hand-labeled ground truth from the maintainer's tracked companies before completion — docs/testing.md rule).
- Owner-durable: import/export bundle coverage; migrations follow append-only rules.
