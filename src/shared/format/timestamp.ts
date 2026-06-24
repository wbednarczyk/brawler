// Friendly, deterministic timestamp for feed/inspector display. Real feed data
// carries raw ISO strings ("2026-06-23T18:00:24", sometimes with an offset like
// "+01:00"); rendering those verbatim is noisy. Sample/seed data already carries
// friendly strings ("Today 09:12") that are NOT ISO — those pass through
// unchanged. Output is locale-independent ("YYYY-MM-DD HH:MM", local time) so it
// stays stable in tests and across machines.
export function formatFeedTimestamp(value: string | null | undefined): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value; // already a friendly, non-ISO label — leave it alone
  }
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}`
  );
}
