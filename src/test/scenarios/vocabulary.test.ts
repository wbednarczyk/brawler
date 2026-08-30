import { describe, expect, it } from "vitest";
import { createMockRuntime } from "./runtime";
import type { ScenarioName } from "./scenarios";
import type { CompanyEvent, TranscriptJob } from "../../api/types";

// F4b S1 guardrail: the mock/backend vocabulary must stay a SUBSET of what
// the Rust storage layer actually validates — a mock value the real INSERT
// would reject is worse than no coverage, because it silently teaches the
// frontend a shape that can never occur. Sets copied verbatim from the
// `validate_allowed_*_value` calls they enforce (line refs point at the
// literal `&[...]` arrays, not the whole function).

// src-tauri/src/storage/transcripts.rs:322 (validate_allowed_transcript_value("source_type", ...))
const TRANSCRIPT_SOURCE_TYPES = new Set(["youtube_url"]);
// src-tauri/src/storage/transcripts.rs:326-332 (company_resolution_status)
const TRANSCRIPT_COMPANY_RESOLUTION_STATUSES = new Set([
  "provided",
  "recognized",
  "unresolved",
  "needs_user_selection",
]);
// src-tauri/src/storage/transcripts.rs:334-337 (status)
const TRANSCRIPT_STATUSES = new Set(["queued", "running", "completed", "failed"]);

// src-tauri/src/storage/events.rs:146-157 (event_type)
const EVENT_TYPES = new Set([
  "periodic_report",
  "corporate_action",
  "dividend",
  "shareholder_meeting",
  "conference_call",
  "investor_conference",
  "market_making",
  "listing_change",
  "other_market_event",
  "custom",
]);
// src-tauri/src/storage/events.rs:159-171 (status)
const EVENT_STATUSES = new Set([
  "scheduled",
  "confirmed",
  "tentative",
  "changed",
  "cancelled",
  "completed",
  "proposed",
]);
// src-tauri/src/storage/events.rs:173-183 (source_type)
const EVENT_SOURCE_TYPES = new Set([
  "manual",
  "official_calendar",
  "official_report",
  "public_media",
  "notebook_entry",
  "feed_item",
  "derived_signal",
]);

function expectTranscriptVocab(job: TranscriptJob, label: string) {
  expect(TRANSCRIPT_SOURCE_TYPES.has(job.sourceType), `${label}: sourceType "${job.sourceType}"`).toBe(
    true,
  );
  expect(
    TRANSCRIPT_COMPANY_RESOLUTION_STATUSES.has(job.companyResolutionStatus),
    `${label}: companyResolutionStatus "${job.companyResolutionStatus}"`,
  ).toBe(true);
  expect(TRANSCRIPT_STATUSES.has(job.status), `${label}: status "${job.status}"`).toBe(true);
}

function expectEventVocab(event: CompanyEvent, label: string) {
  expect(EVENT_TYPES.has(event.eventType), `${label}: eventType "${event.eventType}"`).toBe(true);
  expect(EVENT_STATUSES.has(event.status), `${label}: status "${event.status}"`).toBe(true);
  expect(EVENT_SOURCE_TYPES.has(event.sourceType), `${label}: sourceType "${event.sourceType}"`).toBe(
    true,
  );
}

const SCENARIOS: ScenarioName[] = ["empty", "minimal", "rich"];

describe("mock vocabulary guard (F4b S1) — transcript jobs stay inside the Rust-validated sets", () => {
  for (const scenario of SCENARIOS) {
    it(`every transcript job under "${scenario}"`, async () => {
      const runtime = createMockRuntime(scenario);
      const jobs = (await runtime.invoke("list_video_transcript_jobs", {})) as TranscriptJob[];
      for (const job of jobs) expectTranscriptVocab(job, `${scenario}/${job.id}`);
    });
  }

  it("create_video_transcript_job output stays inside the sets, with and without a company", async () => {
    const runtime = createMockRuntime("minimal");
    const withCompany = (await runtime.invoke("create_video_transcript_job", {
      input: { companyId: "company_gpw_cdr", sourceUrl: "https://www.youtube.com/watch?v=x", sourceLabel: "X" },
    })) as TranscriptJob;
    expectTranscriptVocab(withCompany, "create_video_transcript_job (with company)");
    expect(withCompany.companyResolutionStatus).toBe("provided");

    const withoutCompany = (await runtime.invoke("create_video_transcript_job", {
      input: { sourceUrl: "https://www.youtube.com/watch?v=y", sourceLabel: "Y" },
    })) as TranscriptJob;
    expectTranscriptVocab(withoutCompany, "create_video_transcript_job (no company)");
    expect(withoutCompany.companyResolutionStatus).toBe("unresolved");
  });

  it("resolve_transcript_job_company output stays inside the sets", async () => {
    const runtime = createMockRuntime("minimal");
    const jobs = (await runtime.invoke("list_video_transcript_jobs", {})) as TranscriptJob[];
    const unresolved = jobs.find((job) => job.companyResolutionStatus === "unresolved");
    expect(unresolved, "a minimal-scenario job with no company").toBeTruthy();
    const resolved = (await runtime.invoke("resolve_transcript_job_company", {
      input: { jobId: unresolved!.id, companyId: "company_gpw_cdr" },
    })) as TranscriptJob;
    expectTranscriptVocab(resolved, "resolve_transcript_job_company");
    expect(resolved.companyResolutionStatus).toBe("provided");
  });
});

describe("mock vocabulary guard (F4b S1) — company events stay inside the Rust-validated sets", () => {
  for (const scenario of SCENARIOS) {
    it(`every company event under "${scenario}"`, async () => {
      const runtime = createMockRuntime(scenario);
      const events = (await runtime.invoke("list_company_events", {})) as CompanyEvent[];
      for (const event of events) expectEventVocab(event, `${scenario}/${event.id}`);
    });
  }

  it("create_company_event output stays inside the sets", async () => {
    const runtime = createMockRuntime("minimal");
    const created = (await runtime.invoke("create_company_event", {
      input: { companyId: "company_gpw_cdr", eventType: "dividend", title: "Dividend", eventDate: "2026-09-01" },
    })) as CompanyEvent;
    expectEventVocab(created, "create_company_event");
    expect(created.sourceType).toBe("manual");
  });

  it("the rich scenario seeds one `derived_signal` `proposed` event (decision 5's confirmProposed state)", async () => {
    const runtime = createMockRuntime("rich");
    const events = (await runtime.invoke("list_company_events", {})) as CompanyEvent[];
    const proposed = events.filter((event) => event.status === "proposed");
    expect(proposed.length).toBeGreaterThanOrEqual(1);
    expect(proposed.every((event) => event.sourceType === "derived_signal")).toBe(true);
  });

  it("confirm_derived_event: confirm moves a proposed event to confirmed; reject removes it", async () => {
    const confirmRuntime = createMockRuntime("rich");
    const before = (await confirmRuntime.invoke("list_company_events", {})) as CompanyEvent[];
    const proposed = before.find((event) => event.status === "proposed");
    expect(proposed, "a seeded proposed event").toBeTruthy();

    await confirmRuntime.invoke("confirm_derived_event", {
      input: { eventId: proposed!.id, action: "confirm" },
    });
    const afterConfirm = (await confirmRuntime.invoke("list_company_events", {})) as CompanyEvent[];
    const confirmed = afterConfirm.find((event) => event.id === proposed!.id);
    expect(confirmed?.status).toBe("confirmed");

    const rejectRuntime = createMockRuntime("rich");
    const beforeReject = (await rejectRuntime.invoke("list_company_events", {})) as CompanyEvent[];
    const toReject = beforeReject.find((event) => event.status === "proposed");
    expect(toReject, "a seeded proposed event").toBeTruthy();

    await rejectRuntime.invoke("confirm_derived_event", {
      input: { eventId: toReject!.id, action: "reject" },
    });
    const afterReject = (await rejectRuntime.invoke("list_company_events", {})) as CompanyEvent[];
    expect(afterReject.find((event) => event.id === toReject!.id)).toBeUndefined();
  });
});
