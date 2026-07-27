import { callCommand } from "./tauri";
import type { SectorPercentiles } from "./generated/SectorPercentiles";

// GENERATED DTOs from src-tauri/src/fundamentals/sector_percentiles.rs via ts-rs
// (ADR 0048). Sector-percentile read model (ADR 0089 dec. 3; contracts.md
// § Sector Percentiles). Peers are derived at read time from `companies.sector`
// among tracked companies; percentiles cover the level-0 market ratios and
// selected canonical KPIs, computed from confirmed data only.
export type { SectorPercentiles } from "./generated/SectorPercentiles";
export type { SectorPercentileMetric } from "./generated/SectorPercentileMetric";
export type { SectorMetricKind } from "./generated/SectorMetricKind";
export type { SectorPercentileEmptyReason } from "./generated/SectorPercentileEmptyReason";
export type { MetricAbsentReason } from "./generated/MetricAbsentReason";

/// Where a company stands against its tracked sector peers: rank-based inclusive
/// percentiles for the level-0 market ratios and selected canonical KPIs. Always
/// returns `peerCount`, flags `thin` when N < 4, and returns a typed
/// `emptyReason` when the company has no sector. Offloaded to `spawn_blocking` on
/// the Rust side; reads confirmed data only. Decision support only.
export function getSectorPercentiles(companyId: string) {
  return callCommand<SectorPercentiles>("get_sector_percentiles", { companyId });
}
