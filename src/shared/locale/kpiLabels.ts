import type { LocaleCode } from "./index";

// Canonical/sector KPI definitions are seeded with English labels in the
// database. This maps their stable metric_key to a Polish label so the
// fundamentals UI shows localized names instead of English (or an internal id).
// Falls back to the stored label for any key not listed here.
const PL_KPI_LABELS: Record<string, string> = {
  // Core P&L / balance sheet / cash flow.
  revenue: "Przychody",
  gross_profit: "Zysk brutto ze sprzedaży",
  operating_profit: "Zysk operacyjny (EBIT)",
  ebitda: "EBITDA",
  net_profit: "Zysk netto",
  total_assets: "Aktywa razem",
  total_equity: "Kapitał własny",
  net_debt: "Dług netto",
  cash: "Środki pieniężne",
  operating_cash_flow: "Przepływy operacyjne",
  investing_cash_flow: "Przepływy inwestycyjne",
  financing_cash_flow: "Przepływy finansowe",
  capex: "Nakłady inwestycyjne (CAPEX)",
  free_cash_flow: "Wolne przepływy pieniężne",
  // Per-share.
  eps_basic: "Zysk na akcję (podstawowy)",
  eps_diluted: "Zysk na akcję (rozwodniony)",
  shares_outstanding: "Liczba akcji",
  dividend_per_share: "Dywidenda na akcję",
  // Margins / returns.
  gross_margin: "Marża brutto",
  operating_margin: "Marża operacyjna",
  net_margin: "Marża netto",
  roe: "Rentowność kapitału własnego (ROE)",
  roa: "Rentowność aktywów (ROA)",
  roic: "Rentowność zainwestowanego kapitału (ROIC)",
  roce: "Rentowność zaangażowanego kapitału (ROCE)",
  net_debt_to_ebitda: "Dług netto / EBITDA",
  fcf_conversion: "Konwersja FCF",
  // Insurance.
  gross_written_premium: "Składka przypisana brutto",
  net_earned_premium: "Składka zarobiona netto",
  gross_insurance_revenue: "Przychody z ubezpieczeń brutto",
  claims_ratio: "Wskaźnik szkodowości",
  combined_ratio: "Wskaźnik mieszany (combined ratio)",
  technical_result: "Wynik techniczny",
  investment_result: "Wynik z inwestycji",
  // Banking.
  net_interest_income: "Wynik odsetkowy netto",
  net_fee_commission_income: "Wynik z opłat i prowizji netto",
  operating_income: "Przychody operacyjne",
  operating_expenses: "Koszty operacyjne",
  total_loans: "Kredyty",
  total_deposits: "Depozyty",
  nim: "Marża odsetkowa netto (NIM)",
  cost_income_ratio: "Wskaźnik koszty / dochody (C/I)",
  npl_ratio: "Wskaźnik NPL",
  cost_of_risk: "Koszt ryzyka",
  tcr: "Łączny współczynnik kapitałowy (TCR)",
  // Debt collection.
  recoveries: "Odzyski",
  erc: "Szacowane pozostałe odzyski (ERC)",
  cash_ebitda: "Gotówkowa EBITDA",
  portfolio_purchases: "Zakupy portfeli",
  // REIT / property.
  ffo: "Środki z operacji (FFO)",
  affo: "Skorygowane środki z operacji (AFFO)",
  affo_payout_ratio: "Wskaźnik wypłaty AFFO",
  occupancy: "Obłożenie",
  properties_count: "Nieruchomości",
  same_store_noi: "NOI porównywalny (same-store)",
  walt: "Średni ważony okres najmu (WALT)",
};

/** Localized display name for a KPI definition; falls back to its stored label. */
export function localizedKpiLabel(
  definition: { metricKey: string; label: string },
  locale: LocaleCode,
): string {
  if (locale === "pl") return PL_KPI_LABELS[definition.metricKey] ?? definition.label;
  return definition.label;
}
