import { describe, expect, it, vi } from "vitest";

// Forces a fixed IANA zone for every Date constructed in this file so the
// local-midnight-boundary assertions below are deterministic regardless of
// the host/CI machine's own timezone (vitest isolates each test file into its
// own module/worker context, so this does not leak into sibling suites).
vi.stubEnv("TZ", "Europe/Warsaw");

import type { AttentionEvent } from "../../api/attention";
import type { TodayDeltaSummary } from "../../api/generated/TodayDeltaSummary";
import type { TodayItem } from "../../api/generated/TodayItem";
import { makeAttentionEvent } from "../../test/scenarios/entities";
import {
  allSeen,
  bucketByLocalDay,
  capDayRows,
  formatDeltaHeadline,
  isItemSeen,
  pickPrimary,
  splitDisplayBuckets,
} from "./dayQueueModel";

function filing(overrides: Partial<Extract<TodayItem, { kind: "filing" }>> = {}): TodayItem {
  return {
    kind: "filing",
    feedItemId: "feed_1",
    companyId: "company_1",
    qualifiedTicker: "GPW:BFT",
    title: "Wyniki finansowe",
    publishedAt: "2026-08-20T17:19:00Z",
    read: false,
    presentationKind: "report",
    ...overrides,
  };
}

function mediaCluster(
  overrides: Partial<Extract<TodayItem, { kind: "mediaCluster" }>> = {},
): TodayItem {
  return {
    kind: "mediaCluster",
    companyId: "company_2",
    qualifiedTicker: "GPW:KGH",
    day: "2026-08-20",
    count: 5,
    earliestPublishedAt: "2026-08-20T12:54:00Z",
    latestPublishedAt: "2026-08-20T14:40:00Z",
    topTitles: ["Sierra Gorda"],
    feedItemIds: ["feed_2", "feed_3"],
    ...overrides,
  };
}

function nonArrival(
  overrides: Partial<Extract<TodayItem, { kind: "nonArrival" }>> = {},
): TodayItem {
  return {
    kind: "nonArrival",
    eventKey: "event_1",
    companyId: "company_3",
    qualifiedTicker: "GPW:DNP",
    eventDate: "2026-08-20",
    title: "Raport H1 zapowiedziany wczoraj",
    ...overrides,
  };
}

function calendar(overrides: Partial<Extract<TodayItem, { kind: "calendar" }>> = {}): TodayItem {
  return {
    kind: "calendar",
    eventKey: "cal_1",
    eventDate: "2026-08-21",
    eventType: "periodic_report",
    title: "Publikacja raportu skonsolidowanego",
    companyId: "company_4",
    qualifiedTicker: "GPW:BDX",
    ...overrides,
  };
}

function attention(overrides: Partial<AttentionEvent> = {}): AttentionEvent {
  return { ...makeAttentionEvent("attn_1", "rule_1", "company_1"), ...overrides };
}

const NOW = new Date("2026-08-21T07:25:00Z");

describe("bucketByLocalDay", () => {
  it("splits items either side of LOCAL midnight into different day buckets", () => {
    // Both timestamps fall on UTC calendar day 2026-08-20, but Warsaw is
    // UTC+2 in August: 22:30Z is already 2026-08-21 00:30 local.
    const late = filing({ feedItemId: "late", publishedAt: "2026-08-20T22:30:00Z" });
    const early = filing({ feedItemId: "early", publishedAt: "2026-08-20T21:00:00Z" });

    const buckets = bucketByLocalDay([late, early], [], NOW);

    expect(buckets.map((b) => b.day)).toEqual(["2026-08-21", "2026-08-20"]);
    expect(buckets[0].items).toEqual([late]);
    expect(buckets[1].items).toEqual([early]);
  });

  it("orders buckets newest day first and sorts rows newest-first within a day", () => {
    const older = filing({ feedItemId: "older", publishedAt: "2026-08-19T09:00:00Z" });
    const newer = filing({ feedItemId: "newer", publishedAt: "2026-08-19T18:00:00Z" });
    const yesterday = filing({ feedItemId: "y", publishedAt: "2026-08-20T10:00:00Z" });

    const buckets = bucketByLocalDay([older, newer, yesterday], [], NOW);

    expect(buckets.map((b) => b.day)).toEqual(["2026-08-20", "2026-08-19"]);
    expect(buckets[1].items.map((i) => (i as { feedItemId: string }).feedItemId)).toEqual([
      "newer",
      "older",
    ]);
  });

  it("buckets a bare-date item (nonArrival/calendar) by its own date, no UTC round-trip", () => {
    const missed = nonArrival({ eventDate: "2026-08-20" });
    const buckets = bucketByLocalDay([missed], [], NOW);
    expect(buckets.map((b) => b.day)).toEqual(["2026-08-20"]);
  });

  it("clamps ANY future-dated item into TODAY's bucket — nothing spawns a section above DZIŚ", () => {
    // Main.dc.html: the DZIŚ section carries the calendar announcements
    // ("2 zapowiedzi z kalendarza"); and a future-stamped run (clock skew /
    // anomalous seed) creating its own topmost section would eat one of the
    // two visible display slots and push the real queue into "Earlier".
    const futureEvent = calendar({ eventKey: "far", eventDate: "2026-08-26" });
    const futureCluster = mediaCluster({ day: "2026-08-30", latestPublishedAt: "2026-08-30T09:00:00Z" });
    const todayFiling = filing({ feedItemId: "f1", publishedAt: "2026-08-21T06:00:00Z" });

    const buckets = bucketByLocalDay([futureEvent, futureCluster, todayFiling], [], NOW);

    expect(buckets.map((b) => b.day)).toEqual(["2026-08-21"]);
    expect(buckets[0].relativeDay).toBe("today");
    expect(buckets[0].items).toHaveLength(3);
  });

  it("attaches attention events to the day they fired on, and drops dismissed ones", () => {
    const fired = attention({ id: "a1", firedAt: "2026-08-21T06:00:00Z", dismissed: false });
    const archived = attention({ id: "a2", firedAt: "2026-08-21T06:05:00Z", dismissed: true });

    const buckets = bucketByLocalDay([], [fired, archived], NOW);

    expect(buckets).toHaveLength(1);
    expect(buckets[0].attention).toEqual([fired]);
    expect(buckets[0].total).toBe(1);
  });

  it("labels the injected `now`'s own local day as today/yesterday, never Date.now()", () => {
    const todayItem = filing({ feedItemId: "t", publishedAt: "2026-08-21T06:00:00Z" });
    const yesterdayItem = filing({ feedItemId: "y", publishedAt: "2026-08-20T06:00:00Z" });

    const buckets = bucketByLocalDay([todayItem, yesterdayItem], [], NOW);

    expect(buckets[0].relativeDay).toBe("today");
    expect(buckets[1].relativeDay).toBe("yesterday");

    // A different injected clock shifts the labels for the SAME item data —
    // proof the function reads `now`, not the real wall clock.
    const laterNow = new Date("2026-08-22T07:25:00Z");
    const shifted = bucketByLocalDay([todayItem, yesterdayItem], [], laterNow);
    expect(shifted[0].relativeDay).toBe("yesterday");
    expect(shifted[1].relativeDay).toBe("earlier");
  });

  // F2 S4 harvest: the row/day/attention comparators used a bare
  // `a < b ? 1 : -1`, which returns -1 for EVERY tied pair and so never
  // actually preserves original (insertion) order on a genuine tie — two
  // filings published in the same second, a real possibility, sorted by
  // implementation-defined comparator internals instead of staying stable.
  it("keeps same-timestamp rows in their original (insertion) order — a real tie, not just distinct times", () => {
    const first = filing({ feedItemId: "first", publishedAt: "2026-08-20T09:00:00Z" });
    const second = filing({ feedItemId: "second", publishedAt: "2026-08-20T09:00:00Z" });

    const buckets = bucketByLocalDay([first, second], [], NOW);

    expect(buckets[0].items.map((item) => (item as { feedItemId: string }).feedItemId)).toEqual([
      "first",
      "second",
    ]);
  });

  describe("counters and allSeen", () => {
    it("counts an unread filing and an unseen attention event as unseen", () => {
      const buckets = bucketByLocalDay(
        [filing({ read: false, publishedAt: "2026-08-21T06:10:00Z" })],
        [attention({ firedAt: "2026-08-21T06:00:00Z", seen: false })],
        NOW,
      );
      expect(buckets[0].total).toBe(2);
      expect(buckets[0].unseen).toBe(2);
      expect(allSeen(buckets[0])).toBe(false);
    });

    it("is allSeen once every filing is read and every attention event is seen", () => {
      const buckets = bucketByLocalDay(
        [filing({ read: true, publishedAt: "2026-08-21T06:10:00Z" })],
        [attention({ firedAt: "2026-08-21T06:00:00Z", seen: true })],
        NOW,
      );
      expect(buckets[0].unseen).toBe(0);
      expect(allSeen(buckets[0])).toBe(true);
    });

    it("a mediaCluster/nonArrival/calendar/autopilotRun row has no seen signal — always unseen", () => {
      expect(isItemSeen(mediaCluster())).toBe(false);
      expect(isItemSeen(nonArrival())).toBe(false);
      expect(isItemSeen(calendar())).toBe(false);
      const buckets = bucketByLocalDay([mediaCluster({ latestPublishedAt: "2026-08-21T06:00:00Z" })], [], NOW);
      expect(allSeen(buckets[0])).toBe(false);
    });
  });
});

describe("pickPrimary", () => {
  it("returns null on a clean morning (no buckets)", () => {
    expect(pickPrimary([])).toBeNull();
  });

  it("never picks a calendar row (no landing destination) even when it is the only item", () => {
    const buckets = bucketByLocalDay([calendar()], [], NOW);
    expect(pickPrimary(buckets)).toBeNull();
  });

  it("tier 1: an unseen urgent attention event beats everything else", () => {
    const urgent = attention({ id: "urgent", firedAt: "2026-08-21T06:00:00Z", severity: "urgent", seen: false });
    const buckets = bucketByLocalDay(
      [filing({ read: false, presentationKind: "report" })],
      [urgent],
      NOW,
    );
    expect(pickPrimary(buckets)).toEqual({ kind: "attention", event: urgent });
  });

  it("tier 2: an unread report filing beats a non-arrival", () => {
    const report = filing({ feedItemId: "report", read: false, presentationKind: "report" });
    const buckets = bucketByLocalDay([report, nonArrival()], [], NOW);
    expect(pickPrimary(buckets)).toEqual({ kind: "item", item: report });
  });

  it("tier 3: a non-arrival beats the chronological rest", () => {
    const missed = nonArrival();
    const media = mediaCluster({ latestPublishedAt: "2026-08-21T06:30:00Z" });
    const buckets = bucketByLocalDay([media, missed], [], NOW);
    expect(pickPrimary(buckets)).toEqual({ kind: "item", item: missed });
  });

  it("tier 4: falls back to the newest unseen actionable row", () => {
    const older = mediaCluster({ feedItemIds: ["a"], latestPublishedAt: "2026-08-21T05:00:00Z" });
    const newer = mediaCluster({ feedItemIds: ["b"], latestPublishedAt: "2026-08-21T06:00:00Z" });
    const buckets = bucketByLocalDay([older, newer], [], NOW);
    expect(pickPrimary(buckets)).toEqual({ kind: "item", item: newer });
  });

  it("does not pick an already-read/seen row", () => {
    const buckets = bucketByLocalDay(
      [filing({ read: true, presentationKind: "report" })],
      [attention({ firedAt: "2026-08-21T06:00:00Z", seen: true })],
      NOW,
    );
    expect(pickPrimary(buckets)).toBeNull();
  });
});

describe("formatDeltaHeadline", () => {
  const text = (value: string) => value;

  it("joins report and filing counts", () => {
    const summary: TodayDeltaSummary = { reportCount: 1, filingCount: 3, mediaCount: 26 };
    expect(formatDeltaHeadline(summary, "en", text)).toBe(
      "Since your last visit: 1 report, 3 filings",
    );
  });

  it("uses the Polish plural forms", () => {
    const summary: TodayDeltaSummary = { reportCount: 1, filingCount: 5, mediaCount: 0 };
    expect(formatDeltaHeadline(summary, "pl", text)).toBe(
      "Since your last visit: 1 raport, 5 komunikatów",
    );
  });

  it("falls back to the clean-morning copy when nothing changed", () => {
    const summary: TodayDeltaSummary = { reportCount: 0, filingCount: 0, mediaCount: 0 };
    expect(formatDeltaHeadline(summary, "en", text)).toBe("Nothing new since your last visit");
  });
});

// The "Wcześniej" rollup grouping helper (F2 S5, plan decision 5 / contract
// §5): buckets beyond the two freshest-with-content display slots (DZIŚ +
// WCZORAJ) merge into one rollup — Delta.dc.html "WCZEŚNIEJ W TYM TYGODNIU".
describe("splitDisplayBuckets", () => {
  it("keeps every bucket visible and returns no rollup when there are 2 or fewer", () => {
    const buckets = bucketByLocalDay(
      [
        filing({ feedItemId: "today", publishedAt: "2026-08-21T06:00:00Z" }),
        filing({ feedItemId: "yesterday", publishedAt: "2026-08-20T06:00:00Z" }),
      ],
      [],
      NOW,
    );

    const { visible, earlier } = splitDisplayBuckets(buckets);

    expect(visible.map((b) => b.day)).toEqual(["2026-08-21", "2026-08-20"]);
    expect(earlier).toBeNull();
  });

  it("rolls every bucket beyond the freshest two into one summary (count + unseen + day range)", () => {
    const buckets = bucketByLocalDay(
      [
        filing({ feedItemId: "today", publishedAt: "2026-08-21T06:00:00Z", read: false }),
        filing({ feedItemId: "yesterday", publishedAt: "2026-08-20T06:00:00Z", read: false }),
        filing({ feedItemId: "mon", publishedAt: "2026-08-17T06:00:00Z", read: true }),
        filing({ feedItemId: "tue1", publishedAt: "2026-08-18T06:00:00Z", read: false }),
        filing({ feedItemId: "tue2", publishedAt: "2026-08-18T09:00:00Z", read: true }),
      ],
      [],
      NOW,
    );

    const { visible, earlier } = splitDisplayBuckets(buckets);

    expect(visible.map((b) => b.day)).toEqual(["2026-08-21", "2026-08-20"]);
    expect(earlier).not.toBeNull();
    expect(earlier!.buckets.map((b) => b.day)).toEqual(["2026-08-18", "2026-08-17"]);
    // 3 rolled-up items total (tue1, tue2, mon); only "tue1" is unseen.
    expect(earlier!.count).toBe(3);
    expect(earlier!.unseen).toBe(1);
    // Oldest→newest of the rolled-up range (not the whole bucket list).
    expect(earlier!.oldestDay).toBe("2026-08-17");
    expect(earlier!.newestDay).toBe("2026-08-18");
  });
});

// S-tier row cap (F2 S5, plan Dense row "S top-3+zwiń") — the pure decision;
// the width MEASUREMENT itself is browser-only (`density-matrix.spec.ts`).
describe("capDayRows", () => {
  const rows = ["a", "b", "c", "d", "e"];

  it("never caps when not narrow", () => {
    expect(capDayRows(rows, false, false)).toEqual({ visible: rows, hidden: 0 });
  });

  it("never caps once the day has been manually expanded", () => {
    expect(capDayRows(rows, true, true)).toEqual({ visible: rows, hidden: 0 });
  });

  it("never caps a day with 3 or fewer rows", () => {
    const short = ["a", "b", "c"];
    expect(capDayRows(short, true, false)).toEqual({ visible: short, hidden: 0 });
  });

  it("caps to the newest 3 rows and reports the hidden count", () => {
    expect(capDayRows(rows, true, false)).toEqual({ visible: ["a", "b", "c"], hidden: 2 });
  });
});
