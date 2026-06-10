export const gpwRegistryAdapterId = "gpw-company-registry";
export const eventSourceAdapterIds = ["gpw-market-events-rss", "bankier-kalendarium-html"];
export const feedPruneRetentionDays = 30;
export const feedPruneIntervalMs = 24 * 60 * 60 * 1000;
export const feedPruneInitialDelayMs = 2 * 60 * 1000;

export function schedulerStartJitterMs(intervalMs: number) {
  const jitterWindowMs = Math.min(Math.max(Math.floor(intervalMs * 0.1), 1000), 60000);

  if (jitterWindowMs < 2000) {
    return Math.floor(Math.random() * jitterWindowMs);
  }

  const minimumJitterMs = Math.min(15000, Math.floor(jitterWindowMs / 2));

  return minimumJitterMs + Math.floor(Math.random() * (jitterWindowMs - minimumJitterMs + 1));
}
