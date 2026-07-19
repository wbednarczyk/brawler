export const gpwRegistryAdapterId = "gpw-company-registry";
export const eventSourceAdapterIds = ["gpw-market-events-rss", "bankier-kalendarium-html"];
// Retention window (days) for the MANUAL feed cleanup action. Automatic timed
// pruning was disabled by owner decision 2026-07-19 (it silently deleted
// researched reports); the constants that drove the retired auto-prune timer
// (interval / initial delay / start jitter) were removed with it.
export const feedPruneRetentionDays = 30;
