import { invoke } from "@tauri-apps/api/core";

import type { PriceContext } from "./generated/PriceContext";

export type { PriceContext } from "./generated/PriceContext";
export type { PriceContextRatios } from "./generated/PriceContextRatios";
export type { PriceContextHistoryPoint } from "./generated/PriceContextHistoryPoint";

// Price context for the company workspace (v0.53 T5, ADR 0067/0082): latest
// close + change, 52-week range, level-0 market ratios, and chart-ready close
// history. Read-only computed model — the workspace reloads it on companyId
// change. `emptyReason` distinguishes "not mapped to a market-data provider
// yet" from "mapped, but no quotes fetched yet".
export async function getPriceContext(companyId: string): Promise<PriceContext> {
  return invoke("get_price_context", { companyId });
}
