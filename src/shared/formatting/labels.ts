// Signal-category code → its human display name, matching the backend's
// `signal_categories.display_name` seed EXACTLY (migrations 0041 buyback→
// 0044 own_shares rename, 0079 auditor_opinion, 0080 short_position_change,
// 0085 major_holdings_change, 0092 report_delay/fund_exit/score_deterioration)
// so `text()` resolves the SAME locale entries `text(signal.categoryDisplayName)`
// already relies on elsewhere (`SignalBadges`, `InboxDetailPane`) — never a
// second, drifting copy of the same names. Used where only the raw category
// code is available (an `AlertRule.signalCategory`), not a full `CompanySignal`
// carrying its own `categoryDisplayName` from the backend.
export function formatSignalCategoryDisplayName(category: string) {
  const labels: Record<string, string> = {
    insider_transaction: "Insider transaction",
    dividend: "Dividend",
    profit_warning: "Profit warning / estimate",
    significant_contract: "Significant contract",
    own_shares: "Own-share transactions",
    guidance_change: "Guidance change",
    general_meeting: "General meeting",
    auditor_opinion: "Auditor opinion",
    short_position_change: "Short position",
    major_holdings_change: "Major holdings change",
    report_delay: "Report delay",
    fund_exit: "Fund exit",
    score_deterioration: "Score deterioration",
    other: "Other official filing",
  };

  return labels[category] ?? formatEnumLabel(category);
}

export function formatEnumLabel(value: string) {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1).toLowerCase()}`)
    .join(" ");
}

export function formatCompanyEventType(value: string) {
  const labels: Record<string, string> = {
    periodic_report: "Periodic Report",
    corporate_action: "Corporate Action",
    dividend: "Dividend",
    shareholder_meeting: "Shareholder Meeting",
    conference_call: "Conference Call",
    investor_conference: "Investor Conference",
    market_making: "Market Making",
    listing_change: "Listing Change",
    other_market_event: "Market Event",
    custom: "Custom",
  };

  return labels[value] ?? formatEnumLabel(value);
}

export function formatCompanyEventStatus(value: string) {
  return formatEnumLabel(value);
}

export function formatCompanyEventSourceType(value: string) {
  const labels: Record<string, string> = {
    manual: "Manual",
    official_calendar: "Official Calendar",
    official_report: "Official Report",
    public_calendar: "Public Calendar",
    public_media: "Public Media",
    notebook_entry: "Notebook",
    feed_item: "Feed Item",
  };

  return labels[value] ?? formatEnumLabel(value);
}

export function formatAiProvider(value: string | null | undefined) {
  const labels: Record<string, string> = {
    provider_gemini: "Gemini",
    provider_anthropic: "Claude (Anthropic)",
    provider_openai: "OpenAI (ChatGPT)",
    provider_openai_compatible: "OpenAI-compatible (custom)",
    // Sentinel for jobs routed through the capability map at run time
    // (ADR 0060): the concrete provider is resolved when the job executes.
    capability_routed: "Routed by capability",
  };

  if (!value) {
    return "Not configured";
  }

  return labels[value] ?? value;
}

export function formatGeminiModel(value: string | null | undefined) {
  const labels: Record<string, string> = {
    "gemini-2.5-flash-lite": "Gemini 2.5 Flash-Lite",
    "gemini-2.5-flash": "Gemini 2.5 Flash",
    "gemini-3.1-flash-lite": "Gemini 3.1 Flash-Lite",
    "gemini-3.5-flash": "Gemini 3.5 Flash",
  };

  if (!value) {
    return "Gemini 2.5 Flash";
  }

  return labels[value] ?? value;
}

export function formatCredentialStorage(value: string | null | undefined) {
  const labels: Record<string, string> = {
    os_keychain: "OS keychain",
    development_environment: "Development environment",
    not_configured: "Not configured",
    os_keychain_unavailable: "OS keychain unavailable",
  };

  if (!value) {
    return "Not configured";
  }

  return labels[value] ?? formatEnumLabel(value);
}

export function formatCredentialConfigured(status: { configured: boolean } | null) {
  if (!status) {
    return "Unknown";
  }

  return status.configured ? "Configured" : "Not configured";
}

export function formatCredentialKind(value: string | null | undefined) {
  const labels: Record<string, string> = {
    api_key: "API Key",
    username_password: "Username/password",
    session_token: "Session token",
    oauth_token: "OAuth token",
  };

  if (!value) {
    return "API Key";
  }

  return labels[value] ?? formatEnumLabel(value);
}
