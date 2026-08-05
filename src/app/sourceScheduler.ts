export const gpwRegistryAdapterId = "gpw-company-registry";
export const eventSourceAdapterIds = ["gpw-market-events-rss", "bankier-kalendarium-html"];
// Retention window (days) for the MANUAL feed cleanup action. Automatic timed
// pruning is disabled by design (it silently deleted researched reports;
// product-spec.md § Ingestion).
export const feedPruneRetentionDays = 30;
