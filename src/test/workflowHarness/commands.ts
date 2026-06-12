import type { AiAnalysisJob, AccentPalette, AppLocale, ShortcutBindingSetting, Theme, Watchlist } from "../../api/types";
import type { EvidenceLink, ResearchBriefJob, ResearchDigestJob, ResearchEvidenceInput, ResearchQuestion, ResearchReminder } from "../../api/researchTypes";
import {
  initialFeedItems,
  initialGeminiCredentialStatus,
  initialLicenseStatus,
  initialNotebookEntry,
  initialTranscriptSegmentsByJobId,
  initialUnmatchedSourceItems,
  invalidLicenseStatus,
  missingLicenseStatus,
  type CreateCompanyArgs,
} from "./testData";
import { appTestState, buildResearchTimeline } from "./state";

export function handleAppCommand(command: string, args?: unknown): Promise<unknown> {

    if (command === "health") {
      return Promise.resolve({ status: "ok", version: "0.3.0" });
    }

    if (command === "database_status") {
      return Promise.resolve({
        appliedMigrations: 29,
        companies: 0,
        sourceAdapters: 12,
        settings: 11,
      });
    }

    if (command === "list_companies") {
      return Promise.resolve(appTestState.companiesResponse);
    }

    if (command === "lookup_company") {
      const input = (args as {
        input: {
          exchange: string;
          ticker: string | null;
          displayName: string | null;
          isin: string | null;
        };
      }).input;
      const exchange = input.exchange.trim().toUpperCase();
      const ticker = input.ticker?.trim().toUpperCase();
      const isin = input.isin?.trim().toUpperCase();
      const displayName = input.displayName?.trim().toUpperCase();
      const match = appTestState.companyRegistryEntriesResponse
        .filter((entry) =>
          (ticker && entry.ticker.toUpperCase() === ticker) ||
          (isin && entry.isin?.toUpperCase() === isin) ||
          (displayName && displayName.length >= 3 && entry.displayName.toUpperCase().includes(displayName)),
        )
        .sort((left, right) => {
          const leftPreferred = left.exchange.toUpperCase() === exchange ? 0 : 1;
          const rightPreferred = right.exchange.toUpperCase() === exchange ? 0 : 1;
          return leftPreferred - rightPreferred || left.qualifiedTicker.localeCompare(right.qualifiedTicker);
        })[0];

      return Promise.resolve(
        match
          ? {
              exchange: match.exchange,
              ticker: match.ticker,
              qualifiedTicker: match.qualifiedTicker,
              displayName: match.displayName,
              isin: match.isin ?? "",
              source: "company_directory",
            }
          : null,
      );
    }

    if (command === "create_company") {
      const { input } = args as CreateCompanyArgs;
      const created = {
        id: `company_${input.exchange.toLowerCase()}_${input.ticker.toLowerCase()}`,
        exchange: input.exchange,
        ticker: input.ticker,
        qualifiedTicker: `${input.exchange}:${input.ticker}`,
        displayName: input.displayName,
        isin: input.isin,
        cik: null,
        lei: null,
      };
      appTestState.companiesResponse = [...appTestState.companiesResponse, created];
      appTestState.companyRegistryEntriesResponse = appTestState.companyRegistryEntriesResponse.map((entry) =>
        entry.exchange === input.exchange && entry.ticker === input.ticker
          ? { ...entry, tracked: true }
          : entry,
      );

      return Promise.resolve(created);
    }

    if (command === "delete_company") {
      return Promise.resolve();
    }

    if (command === "list_research_evidence") {
      const { input } = args as { input: ResearchEvidenceInput };
      return Promise.resolve(buildResearchTimeline(input));
    }

    if (command === "list_company_timeline") {
      const { companyId } = args as { companyId: string };
      return Promise.resolve(buildResearchTimeline({ companyId }));
    }

    if (command === "mark_research_scope_reviewed") {
      const { input } = args as {
        input: { scopeType: "company" | "watchlist"; scopeId: string; reviewedAt?: string | null };
      };
      const reviewedAt = input.reviewedAt ?? "2026-06-05T10:00:00Z";
      appTestState.researchReviewCheckpointResponse = {
        id: `research_review_${input.scopeType}_${input.scopeId}`,
        scopeType: input.scopeType,
        scopeId: input.scopeId,
        reviewedAt,
        createdAt: reviewedAt,
        updatedAt: reviewedAt,
      };

      return Promise.resolve(appTestState.researchReviewCheckpointResponse);
    }

    if (command === "list_research_questions") {
      const { input } = args as {
        input: { scopeType?: string | null; scopeId?: string | null; status?: string | null };
      };
      return Promise.resolve(
        appTestState.researchQuestionsResponse.filter((question) => {
          const scopeMatches = !input.scopeType || question.scopeType === input.scopeType;
          const idMatches = !input.scopeId || question.scopeId === input.scopeId;
          const statusMatches = !input.status || question.status === input.status;
          return scopeMatches && idMatches && statusMatches;
        }),
      );
    }

    if (command === "create_research_question") {
      const { input } = args as {
        input: { scopeType: "company" | "watchlist"; scopeId: string; title: string; body?: string | null };
      };
      const now = "2026-06-05T10:00:00Z";
      const created: ResearchQuestion = {
        id: `research_question_${input.scopeType}_${input.scopeId}_${appTestState.researchQuestionsResponse.length + 1}`,
        scopeType: input.scopeType,
        scopeId: input.scopeId,
        title: input.title,
        body: input.body ?? "",
        status: "open",
        closedAt: null,
        createdAt: now,
        updatedAt: now,
      };
      appTestState.researchQuestionsResponse = [...appTestState.researchQuestionsResponse, created];
      appTestState.researchEvidenceItemsResponse = [
        {
          id: `evidence_research_question_${created.id}`,
          evidenceType: "research_question",
          sourceDomain: "research",
          sourceId: created.id,
          companyId: created.scopeId,
          occurredAt: created.updatedAt,
          title: created.title,
          summary: created.body || null,
          sourceUrl: null,
          attribution: null,
          trustCategory: "user_note",
          reviewState: {
            changedSinceCompanyReview: true,
            changedSinceWatchlistReview: true,
          },
        },
        ...appTestState.researchEvidenceItemsResponse,
      ];
      return Promise.resolve(created);
    }

    if (command === "update_research_question") {
      const { input } = args as {
        input: { id: string; title?: string | null; body?: string | null; status?: "open" | "answered" | "closed" | null };
      };
      const updatedAt = "2026-06-05T10:10:00Z";
      appTestState.researchQuestionsResponse = appTestState.researchQuestionsResponse.map((question) =>
        question.id === input.id
          ? {
              ...question,
              title: input.title ?? question.title,
              body: input.body ?? question.body,
              status: input.status ?? question.status,
              closedAt: input.status && input.status !== "open" ? updatedAt : input.status === "open" ? null : question.closedAt,
              updatedAt,
            }
          : question,
      );
      return Promise.resolve(appTestState.researchQuestionsResponse.find((question) => question.id === input.id));
    }

    if (command === "delete_research_question") {
      const { id } = args as { id: string };
      appTestState.researchQuestionsResponse = appTestState.researchQuestionsResponse.filter(
        (question) => question.id !== id,
      );
      appTestState.researchEvidenceItemsResponse = appTestState.researchEvidenceItemsResponse.filter(
        (item) => !(item.evidenceType === "research_question" && item.sourceId === id),
      );
      appTestState.evidenceLinksResponse = appTestState.evidenceLinksResponse.filter(
        (link) =>
          !(link.fromType === "research_question" && link.fromId === id) &&
          !(link.toType === "research_question" && link.toId === id),
      );
      return Promise.resolve();
    }

    if (command === "list_evidence_links") {
      const { input } = args as { input: { endpointType: string; endpointId: string } };
      return Promise.resolve(
        appTestState.evidenceLinksResponse.filter(
          (link) =>
            (link.fromType === input.endpointType && link.fromId === input.endpointId) ||
            (link.toType === input.endpointType && link.toId === input.endpointId),
        ),
      );
    }

    if (command === "create_evidence_link") {
      const { input } = args as {
        input: Omit<EvidenceLink, "id" | "createdAt">;
      };
      const existing = appTestState.evidenceLinksResponse.find(
        (link) =>
          link.fromType === input.fromType &&
          link.fromId === input.fromId &&
          link.toType === input.toType &&
          link.toId === input.toId &&
          link.relationType === input.relationType,
      );
      if (existing) {
        return Promise.resolve(existing);
      }
      const created: EvidenceLink = {
        ...input,
        id: `evidence_link_${appTestState.evidenceLinksResponse.length + 1}`,
        createdAt: "2026-06-05T10:15:00Z",
      };
      appTestState.evidenceLinksResponse = [...appTestState.evidenceLinksResponse, created];
      return Promise.resolve(created);
    }

    if (command === "delete_evidence_link") {
      const { id } = args as { id: string };
      appTestState.evidenceLinksResponse = appTestState.evidenceLinksResponse.filter((link) => link.id !== id);
      return Promise.resolve();
    }

    if (command === "list_research_briefs") {
      const { input } = args as { input: { scopeType: "company" | "watchlist"; scopeId: string } };
      return Promise.resolve(
        appTestState.researchBriefJobsResponse.filter(
          (job) => job.scopeType === input.scopeType && job.scopeId === input.scopeId,
        ),
      );
    }

    if (command === "start_research_brief") {
      const { input } = args as { input: { scopeType: "company" | "watchlist"; scopeId: string } };
      const now = "2026-06-05T10:20:00Z";
      const id = `research_brief_job_${input.scopeType}_${input.scopeId}_${appTestState.researchBriefJobsResponse.length + 1}`;
      const created: ResearchBriefJob = {
        id,
        scopeType: input.scopeType,
        scopeId: input.scopeId,
        providerId: "test_sample",
        model: "test-sample-analysis-v1",
        promptVersion: "m30.research_brief.v1",
        evidenceCollectorVersion: "m30.collector.v1",
        rendererVersion: "m30.renderer.v1",
        status: "succeeded",
        errorCode: null,
        error: null,
        createdAt: now,
        startedAt: now,
        finishedAt: now,
        brief: {
          id: `research_brief_${input.scopeType}_${input.scopeId}_${appTestState.researchBriefJobsResponse.length + 1}`,
          jobId: id,
          scopeType: input.scopeType,
          scopeId: input.scopeId,
          providerId: "test_sample",
          model: "test-sample-analysis-v1",
          promptVersion: "m30.research_brief.v1",
          evidenceCollectorVersion: "m30.collector.v1",
          rendererVersion: "m30.renderer.v1",
          title: "Generated research brief",
          summary: "Source-grounded brief summary.",
          contentMarkdown: "## What changed\n\nReview cited evidence together. [E1]",
          language: "en",
          generatedAt: now,
          createdAt: now,
          citations: [
            {
              id: "research_brief_citation_1",
              briefId: `research_brief_${input.scopeType}_${input.scopeId}_1`,
              citationKey: "E1",
              evidenceType: "feed_item",
              evidenceId: "feed_sample_cdr_report",
              label: "Current report placeholder for watchlist company",
              snippet: "Sample official report used to validate research timeline rendering.",
              createdAt: now,
            },
          ],
        },
      };
      appTestState.researchBriefJobsResponse = [created, ...appTestState.researchBriefJobsResponse];
      return Promise.resolve(created);
    }

    if (command === "list_research_reminders") {
      const { input } = args as { input: { scopeType: "company" | "watchlist"; scopeId: string; status?: string | null } };
      return Promise.resolve(
        appTestState.researchRemindersResponse.filter((reminder) => {
          const scopeMatches =
            reminder.scopeType === input.scopeType && reminder.scopeId === input.scopeId;
          const companyInWatchlist =
            input.scopeType === "watchlist" &&
            appTestState.watchlistMembershipsResponse.some(
              (membership) =>
                membership.watchlistId === input.scopeId && membership.companyId === reminder.companyId,
            );
          const statusMatches = !input.status || reminder.status === input.status;
          return (scopeMatches || companyInWatchlist) && statusMatches;
        }),
      );
    }

    if (command === "update_research_reminder") {
      const { input } = args as {
        input: {
          id: string;
          status?: "open" | "completed" | "dismissed" | null;
          snoozedUntil?: string | null;
          completedAt?: string | null;
          dismissedAt?: string | null;
        };
      };
      appTestState.researchRemindersResponse = appTestState.researchRemindersResponse.map((reminder) =>
        reminder.id === input.id
          ? {
              ...reminder,
              status: input.status ?? reminder.status,
              completedAt:
                input.completedAt !== undefined
                  ? input.completedAt
                  : input.status === "completed"
                    ? "2026-06-05T10:25:00Z"
                    : reminder.completedAt,
              snoozedUntil: input.snoozedUntil === undefined ? reminder.snoozedUntil : input.snoozedUntil,
              dismissedAt: input.dismissedAt === undefined ? reminder.dismissedAt : input.dismissedAt,
              updatedAt: "2026-06-05T10:25:00Z",
            }
          : reminder,
      );
      return Promise.resolve(appTestState.researchRemindersResponse.find((reminder) => reminder.id === input.id));
    }

    if (command === "create_research_reminder") {
      const { input } = args as {
        input: {
          scopeType: "company" | "watchlist";
          scopeId: string;
          companyId?: string | null;
          reminderKind: "manual_research";
          sourceType?: ResearchReminder["sourceType"];
          sourceId?: string | null;
          title: string;
          body?: string | null;
          dueAt?: string | null;
        };
      };
      const now = "2026-06-05T10:30:00Z";
      const created: ResearchReminder = {
        id: `research_reminder_manual_${appTestState.researchRemindersResponse.length + 1}`,
        scopeType: input.scopeType,
        scopeId: input.scopeId,
        companyId: input.companyId ?? null,
        reminderKind: input.reminderKind,
        sourceType: input.sourceType ?? null,
        sourceId: input.sourceId ?? null,
        title: input.title,
        body: input.body ?? "",
        dueAt: input.dueAt ?? null,
        status: "open",
        snoozedUntil: null,
        completedAt: null,
        dismissedAt: null,
        createdAt: now,
        updatedAt: now,
      };
      appTestState.researchRemindersResponse = [created, ...appTestState.researchRemindersResponse];
      return Promise.resolve(created);
    }

    if (command === "delete_research_reminder") {
      const { id } = args as { id: string };
      appTestState.researchRemindersResponse = appTestState.researchRemindersResponse.filter(
        (reminder) => reminder.id !== id,
      );
      return Promise.resolve(undefined);
    }

    if (command === "list_research_digests") {
      const { input } = args as { input: { scopeType: "company" | "watchlist"; scopeId: string } };
      return Promise.resolve(
        appTestState.researchDigestJobsResponse.filter(
          (job) => job.scopeType === input.scopeType && job.scopeId === input.scopeId,
        ),
      );
    }

    if (command === "start_research_digest") {
      const { input } = args as { input: { scopeType: "company" | "watchlist"; scopeId: string } };
      const now = "2026-06-05T10:30:00Z";
      const id = `research_digest_job_${input.scopeType}_${input.scopeId}_${appTestState.researchDigestJobsResponse.length + 1}`;
      const created: ResearchDigestJob = {
        id,
        scopeType: input.scopeType,
        scopeId: input.scopeId,
        providerId: "test_sample",
        model: "test-sample-analysis-v1",
        promptVersion: "m31.research_digest.v1",
        evidenceCollectorVersion: "m31.digest_collector.v1",
        rendererVersion: "m31.digest_renderer.v1",
        status: "succeeded",
        errorCode: null,
        error: null,
        createdAt: now,
        startedAt: now,
        finishedAt: now,
        digest: {
          id: `research_digest_${input.scopeType}_${input.scopeId}_1`,
          jobId: id,
          scopeType: input.scopeType,
          scopeId: input.scopeId,
          providerId: "test_sample",
          model: "test-sample-analysis-v1",
          promptVersion: "m31.research_digest.v1",
          evidenceCollectorVersion: "m31.digest_collector.v1",
          rendererVersion: "m31.digest_renderer.v1",
          title: "Research digest",
          summary: "Open reminders and changed evidence to review.",
          contentMarkdown: "## Today's review\n\nStart with open reminders. [E1]",
          language: "en",
          generatedAt: now,
          createdAt: now,
          citations: [
            {
              id: "research_digest_citation_1",
              digestId: `research_digest_${input.scopeType}_${input.scopeId}_1`,
              citationKey: "E1",
              evidenceType: "feed_item",
              evidenceId: "feed_sample_cdr_report",
              label: "Current report placeholder for watchlist company",
              snippet: "Sample official report used to validate research digest rendering.",
              createdAt: now,
            },
          ],
        },
      };
      appTestState.researchDigestJobsResponse = [created, ...appTestState.researchDigestJobsResponse];
      return Promise.resolve(created);
    }

    if (command === "export_research_data") {
      return Promise.resolve({
        fileName: "brawler-research-data-2026-06-05.json",
        mediaType: "application/json",
        contents: "{\"schemaVersion\":1}",
        summary: {
          companies: appTestState.companiesResponse.length,
          watchlists: appTestState.watchlistsResponse.length,
          memberships: appTestState.watchlistMembershipsResponse.length,
          notebookEntries: appTestState.notebookEntriesResponse.length,
          researchQuestions: appTestState.researchQuestionsResponse.length,
          evidenceLinks: appTestState.evidenceLinksResponse.length,
          aiResearchBriefs: appTestState.researchBriefJobsResponse.filter((job) => job.brief).length,
          aiResearchBriefCitations: appTestState.researchBriefJobsResponse.reduce(
            (total, job) => total + (job.brief?.citations.length ?? 0),
            0,
          ),
          researchReminders: appTestState.researchRemindersResponse.length,
          aiResearchDigests: appTestState.researchDigestJobsResponse.filter((job) => job.digest).length,
          aiResearchDigestCitations: appTestState.researchDigestJobsResponse.reduce(
            (total, job) => total + (job.digest?.citations.length ?? 0),
            0,
          ),
          settings: 0,
        },
      });
    }

    if (command === "preview_research_import") {
      return Promise.resolve({
        valid: true,
        summary: {
          companiesCreated: 1,
          companiesMerged: 1,
          watchlistsCreated: 1,
          watchlistsMerged: 0,
          membershipsCreated: 1,
          notebookEntriesCreated: 1,
          notebookEntriesSkipped: 0,
          researchQuestionsCreated: 1,
          researchQuestionsMerged: 0,
          evidenceLinksCreated: 0,
          evidenceLinksSkipped: 0,
          aiResearchBriefsCreated: 0,
          aiResearchBriefsSkipped: 0,
          aiResearchBriefCitationsCreated: 0,
          aiResearchBriefCitationsSkipped: 0,
          researchRemindersCreated: 0,
          researchRemindersSkipped: 0,
          aiResearchDigestsCreated: 0,
          aiResearchDigestsSkipped: 0,
          aiResearchDigestCitationsCreated: 0,
          aiResearchDigestCitationsSkipped: 0,
          settingsUpdated: 0,
        },
        warnings: [],
        errors: [],
      });
    }

    if (command === "apply_research_import") {
      return Promise.resolve({
        summary: {
          companiesCreated: 1,
          companiesMerged: 1,
          watchlistsCreated: 1,
          watchlistsMerged: 0,
          membershipsCreated: 1,
          notebookEntriesCreated: 1,
          notebookEntriesSkipped: 0,
          researchQuestionsCreated: 1,
          researchQuestionsMerged: 0,
          evidenceLinksCreated: 0,
          evidenceLinksSkipped: 0,
          aiResearchBriefsCreated: 0,
          aiResearchBriefsSkipped: 0,
          aiResearchBriefCitationsCreated: 0,
          aiResearchBriefCitationsSkipped: 0,
          researchRemindersCreated: 0,
          researchRemindersSkipped: 0,
          aiResearchDigestsCreated: 0,
          aiResearchDigestsSkipped: 0,
          aiResearchDigestCitationsCreated: 0,
          aiResearchDigestCitationsSkipped: 0,
          settingsUpdated: 0,
        },
        warnings: [],
      });
    }

    if (command === "export_settings_data") {
      return Promise.resolve({
        fileName: "brawler-settings-2026-06-05.yaml",
        mediaType: "application/x-yaml",
        contents: "schemaVersion: 1\nsettings:\n  theme: dark\n",
        summary: {
          companies: 0,
          watchlists: 0,
          memberships: 0,
          notebookEntries: 0,
          researchQuestions: 0,
          evidenceLinks: 0,
          aiResearchBriefs: 0,
          aiResearchBriefCitations: 0,
          researchReminders: 0,
          aiResearchDigests: 0,
          aiResearchDigestCitations: 0,
          settings: 15,
        },
      });
    }

    if (command === "preview_settings_import") {
      return Promise.resolve({
        valid: true,
        summary: {
          companiesCreated: 0,
          companiesMerged: 0,
          watchlistsCreated: 0,
          watchlistsMerged: 0,
          membershipsCreated: 0,
          notebookEntriesCreated: 0,
          notebookEntriesSkipped: 0,
          settingsUpdated: 2,
        },
        warnings: [],
        errors: [],
      });
    }

    if (command === "apply_settings_import") {
      appTestState.settingsResponse = {
        ...appTestState.settingsResponse,
        theme: "light",
      };

      return Promise.resolve({
        summary: {
          companiesCreated: 0,
          companiesMerged: 0,
          watchlistsCreated: 0,
          watchlistsMerged: 0,
          membershipsCreated: 0,
          notebookEntriesCreated: 0,
          notebookEntriesSkipped: 0,
          settingsUpdated: 2,
        },
        warnings: [],
      });
    }

    if (command === "list_watchlists") {
      return Promise.resolve(appTestState.watchlistsResponse);
    }

    if (command === "list_watchlist_memberships") {
      return Promise.resolve(appTestState.watchlistMembershipsResponse);
    }

    if (command === "create_watchlist") {
      const { input } = args as { input: { name: string; description: string | null } };
      const created = {
        id: `watchlist_${input.name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "")}`,
        name: input.name,
        description: input.description,
        companyCount: 0,
      };
      appTestState.watchlistsResponse = [...appTestState.watchlistsResponse, created];

      return Promise.resolve(created);
    }

    if (command === "rename_watchlist") {
      const { input } = args as { input: { id: string; name: string; description: string | null } };
      let renamed: Watchlist | null = null;
      appTestState.watchlistsResponse = appTestState.watchlistsResponse.map((watchlist) => {
        if (watchlist.id !== input.id) {
          return watchlist;
        }

        renamed = {
          ...watchlist,
          name: input.name,
          description: input.description,
        };
        return renamed;
      });
      appTestState.watchlistMembershipsResponse = appTestState.watchlistMembershipsResponse.map((membership) =>
        membership.watchlistId === input.id
          ? { ...membership, watchlistName: input.name }
          : membership,
      );

      return Promise.resolve(renamed);
    }

    if (command === "delete_watchlist") {
      const { watchlistId } = args as { watchlistId: string };
      appTestState.watchlistsResponse = appTestState.watchlistsResponse.filter(
        (watchlist) => watchlist.id !== watchlistId,
      );
      appTestState.watchlistMembershipsResponse = appTestState.watchlistMembershipsResponse.filter(
        (membership) => membership.watchlistId !== watchlistId,
      );
      return Promise.resolve();
    }

    if (command === "add_company_to_watchlist") {
      const { input } = args as { input: { watchlistId: string; companyId: string } };
      const watchlist = appTestState.watchlistsResponse.find((entry) => entry.id === input.watchlistId);
      if (
        watchlist &&
        !appTestState.watchlistMembershipsResponse.some(
          (membership) =>
            membership.watchlistId === input.watchlistId && membership.companyId === input.companyId,
        )
      ) {
        appTestState.watchlistMembershipsResponse = [
          ...appTestState.watchlistMembershipsResponse,
          {
            watchlistId: input.watchlistId,
            watchlistName: watchlist.name,
            companyId: input.companyId,
          },
        ];
        appTestState.watchlistsResponse = appTestState.watchlistsResponse.map((entry) =>
          entry.id === input.watchlistId
            ? { ...entry, companyCount: entry.companyCount + 1 }
            : entry,
        );
      }
      return Promise.resolve();
    }

    if (command === "remove_company_from_watchlist") {
      const { input } = args as { input: { watchlistId: string; companyId: string } };
      const hadMembership = appTestState.watchlistMembershipsResponse.some(
        (membership) =>
          membership.watchlistId === input.watchlistId && membership.companyId === input.companyId,
      );
      appTestState.watchlistMembershipsResponse = appTestState.watchlistMembershipsResponse.filter(
        (membership) =>
          membership.watchlistId !== input.watchlistId || membership.companyId !== input.companyId,
      );
      if (hadMembership) {
        appTestState.watchlistsResponse = appTestState.watchlistsResponse.map((entry) =>
          entry.id === input.watchlistId
            ? { ...entry, companyCount: Math.max(0, entry.companyCount - 1) }
            : entry,
        );
      }
      return Promise.resolve();
    }

    if (command === "list_feed_items") {
      return Promise.resolve(appTestState.feedItemsResponse);
    }

    if (command === "list_company_events") {
      const input = (args as {
        input: {
          mode: string;
          companyId: string | null;
          watchlistId: string | null;
          eventType: string | null;
          status: string | null;
          dateFrom: string | null;
          dateTo: string | null;
        };
      }).input;

      return Promise.resolve(
        appTestState.companyEventsResponse.filter((event) => {
          const companyMatches = !input.companyId || event.companyId === input.companyId;
          const typeMatches = !input.eventType || event.eventType === input.eventType;
          const statusMatches = !input.status || event.status === input.status;
          const dateFromMatches = !input.dateFrom || event.eventDate >= input.dateFrom;
          const dateToMatches = !input.dateTo || event.eventDate <= input.dateTo;
          const watchlistMatches =
            !input.watchlistId ||
            (input.watchlistId === "watchlist_main_gpw" && event.companyId === "company_gpw_cdr");

          return (
            companyMatches &&
            typeMatches &&
            statusMatches &&
            dateFromMatches &&
            dateToMatches &&
            watchlistMatches
          );
        }),
      );
    }

    if (command === "create_company_event") {
      const input = (args as {
        input: {
          companyId: string;
          eventType: string;
          title: string;
          eventDate: string;
          eventTime: string | null;
          status: string;
          sourceType: string;
          sourceAdapterId: string | null;
          sourceEventKey: string | null;
          sourceUrl: string | null;
          attribution: string | null;
          fetchedAt: string | null;
        };
      }).input;
      const company = appTestState.companiesResponse.find((entry) => entry.id === input.companyId);
      const existing = appTestState.transcriptJobsResponse.find(
        (job) => job.sourceUrl === input.sourceUrl,
      );

      if (existing) {
        return Promise.resolve(existing);
      }

      const created = {
        id: "manual_event_created",
        companyId: input.companyId,
        company: company?.qualifiedTicker ?? "GPW:UNK",
        companyName: company?.displayName ?? "Unknown company",
        eventType: input.eventType,
        title: input.title,
        eventDate: input.eventDate,
        eventTime: input.eventTime,
        status: input.status,
        sourceType: input.sourceType,
        sourceAdapterId: input.sourceAdapterId,
        sourceEventKey: input.sourceEventKey,
        sourceUrl: input.sourceUrl,
        attribution: input.attribution,
        fetchedAt: input.fetchedAt,
        manual: true,
        createdAt: "2026-06-01T08:00:00Z",
        updatedAt: "2026-06-01T08:00:00Z",
      };

      appTestState.companyEventsResponse = [...appTestState.companyEventsResponse, created];

      return Promise.resolve(created);
    }

    if (command === "list_video_transcript_jobs") {
      const input = (args as { input: { companyId: string | null } }).input;

      return Promise.resolve(
        appTestState.transcriptJobsResponse.filter((job) => !input.companyId || job.companyId === input.companyId),
      );
    }

    if (command === "create_video_transcript_job") {
      const input = (args as {
        input: {
          sourceUrl: string;
          companyId: string | null;
          providerId: string | null;
          sourceLabel: string | null;
          recognizedCompanyCandidates: unknown[] | null;
        };
      }).input;
      const existing = appTestState.transcriptJobsResponse.find(
        (job) =>
          job.sourceUrl === input.sourceUrl &&
          (job.companyId ?? null) === (input.companyId ?? null),
      );

      if (existing) {
        return Promise.resolve(existing);
      }

      const company = appTestState.companiesResponse.find((entry) => entry.id === input.companyId);
      const created = {
        id: "transcript_job_created",
        companyId: input.companyId,
        company: company?.qualifiedTicker ?? null,
        companyName: company?.displayName ?? null,
        providerId: input.providerId ?? "provider_gemini",
        sourceType: "youtube_url",
        sourceUrl: input.sourceUrl,
        sourceLabel: input.sourceLabel,
        companyResolutionStatus: input.companyId ? "provided" : "unresolved",
        recognizedCompanyCandidates: input.recognizedCompanyCandidates ?? [],
        status: "queued",
        errorCode: null,
        createdAt: "2026-06-01T10:05:00Z",
        startedAt: null,
        finishedAt: null,
        error: null,
      };

      appTestState.transcriptJobsResponse = [created, ...appTestState.transcriptJobsResponse];

      return Promise.resolve(created);
    }

    if (command === "list_transcript_segments") {
      const { transcriptJobId } = args as { transcriptJobId: string };

      return Promise.resolve(initialTranscriptSegmentsByJobId[transcriptJobId] ?? []);
    }

    if (command === "delete_video_transcript_job") {
      const { jobId } = args as { jobId: string };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.filter((job) => job.id !== jobId);

      return Promise.resolve();
    }

    if (command === "update_video_transcript_job") {
      const input = (args as {
        input: {
          jobId: string;
          sourceLabel: string | null;
        };
      }).input;
      const existing = appTestState.transcriptJobsResponse.find((job) => job.id === input.jobId);

      if (!existing) {
        return Promise.reject(new Error("job not found"));
      }

      const updated = {
        ...existing,
        sourceLabel: input.sourceLabel,
      };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
        job.id === input.jobId ? updated : job,
      );

      return Promise.resolve(updated);
    }

    if (command === "run_video_transcript_job") {
      const input = (args as { input: { jobId: string; providerMode: string } }).input;
      const existing = appTestState.transcriptJobsResponse.find((job) => job.id === input.jobId);

      if (!existing) {
        return Promise.reject(new Error("job not found"));
      }

      if (input.providerMode === "provider_gemini" && !appTestState.geminiCredentialStatusResponse.configured) {
        const failed = {
          ...existing,
          status: "failed",
          startedAt: "2026-06-01T10:06:00Z",
          finishedAt: "2026-06-01T10:07:00Z",
          errorCode: "provider_not_configured",
          error: "Gemini transcription provider is not configured.",
        };
        appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
          job.id === input.jobId ? failed : job,
        );

        return Promise.resolve(failed);
      }

      const updated = {
        ...existing,
        status: "completed",
        startedAt: "2026-06-01T10:06:00Z",
        finishedAt: "2026-06-01T10:07:00Z",
        errorCode: null,
        error: null,
      };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
        job.id === input.jobId ? updated : job,
      );

      return Promise.resolve(updated);
    }

    if (command === "resolve_transcript_job_company") {
      const input = (args as { input: { jobId: string; companyId: string } }).input;
      const company = appTestState.companiesResponse.find((entry) => entry.id === input.companyId);
      const resolved = appTestState.transcriptJobsResponse.find((job) => job.id === input.jobId);

      if (!resolved) {
        return Promise.reject(new Error("job not found"));
      }

      const updated = {
        ...resolved,
        companyId: input.companyId,
        company: company?.qualifiedTicker ?? null,
        companyName: company?.displayName ?? null,
        companyResolutionStatus: "provided",
      };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
        job.id === input.jobId ? updated : job,
      );

      return Promise.resolve(updated);
    }

    if (command === "delete_unsaved_feed_items") {
      const deletedCount = appTestState.feedItemsResponse.filter((feedItem) => !feedItem.saved).length;
      appTestState.feedItemsResponse = appTestState.feedItemsResponse.filter((feedItem) => feedItem.saved);

      return Promise.resolve({
        itemsDeleted: deletedCount,
        deletedAt: "2026-05-31T12:00:00Z",
      });
    }

    if (command === "prune_old_feed_items") {
      return Promise.resolve({
        retentionDays: 30,
        itemsDeleted: 0,
        prunedAt: "2026-05-31T12:00:00Z",
      });
    }

    if (command === "list_notebook_entries") {
      const companyId = (args as { companyId: string }).companyId;

      return Promise.resolve(
        appTestState.notebookEntriesResponse.filter((entry) => entry.companyId === companyId),
      );
    }

    if (command === "create_notebook_entry") {
      const input = (args as {
        input: {
          companyId: string;
          title: string;
          body: string;
          bodyFormat: string;
          tags: string[];
          kind: string;
          claimStatus: string | null;
          eventDate: string | null;
          followUpAfter: string | null;
          followUpDate: string | null;
          origins: Array<{
            sourceType: string;
            sourceId: string | null;
            sourceUrl: string | null;
            label: string | null;
          }>;
        };
      }).input;
      const created = {
        id: `note_${input.companyId}_${input.title.toLowerCase().replace(/\s+/g, "_")}`,
        companyId: input.companyId,
        title: input.title,
        body: input.body,
        bodyFormat: input.bodyFormat,
        tags: input.tags.map((tag) => tag.toLowerCase()).sort(),
        kind: input.kind,
        claimStatus: input.claimStatus,
        eventDate: input.eventDate,
        followUpAfter: input.followUpAfter,
        followUpDate: input.followUpDate,
        createdAt: "2026-05-29T10:00:00Z",
        updatedAt: "2026-05-29T10:00:00Z",
        origins: input.origins.map((item, index) => ({
          id: `note_origin_${index}`,
          sourceType: item.sourceType,
          sourceId: item.sourceId,
          sourceUrl: item.sourceUrl,
          label: item.label,
          createdAt: "2026-05-29T10:00:00Z",
        })),
      };

      appTestState.notebookEntriesResponse = [created, ...appTestState.notebookEntriesResponse];

      return Promise.resolve(created);
    }

    if (command === "create_note_from_transcript_selection") {
      const input = (args as {
        input: {
          transcriptJobId: string;
          transcriptSegmentIds: string[];
          noteDraft: {
            title: string;
            body: string;
            tags: string[];
            kind: string;
            claimStatus: string | null;
            eventDate: string | null;
            followUpAfter: string | null;
            followUpDate: string | null;
          };
        };
      }).input;
      const job = appTestState.transcriptJobsResponse.find((entry) => entry.id === input.transcriptJobId);
      const created = {
        id: "note_from_transcript_selection",
        companyId: job?.companyId ?? "company_gpw_cdr",
        title: input.noteDraft.title,
        body: input.noteDraft.body,
        bodyFormat: "markdown",
        tags: input.noteDraft.tags.map((tag) => tag.toLowerCase()).sort(),
        kind: input.noteDraft.kind,
        claimStatus: input.noteDraft.claimStatus,
        eventDate: input.noteDraft.eventDate,
        followUpAfter: input.noteDraft.followUpAfter,
        followUpDate: input.noteDraft.followUpDate,
        createdAt: "2026-06-01T10:08:00Z",
        updatedAt: "2026-06-01T10:08:00Z",
        origins: input.transcriptSegmentIds.map((segmentId, index) => ({
          id: `note_origin_transcript_${index}`,
          sourceType: "transcript_segment",
          sourceId: segmentId,
          sourceUrl: job?.sourceUrl ?? null,
          label: `Transcript ${input.transcriptJobId} ${segmentId}`,
          createdAt: "2026-06-01T10:08:00Z",
        })),
      };

      appTestState.notebookEntriesResponse = [created, ...appTestState.notebookEntriesResponse];

      return Promise.resolve(created);
    }

    if (command === "update_notebook_entry") {
      const input = (args as {
        input: {
          id: string;
          title: string;
          body: string;
          tags: string[];
          kind: string;
          claimStatus: string | null;
          eventDate: string | null;
          followUpAfter: string | null;
          followUpDate: string | null;
        };
      }).input;
      const existing = appTestState.notebookEntriesResponse.find((entry) => entry.id === input.id);
      const updated = {
        ...(existing ?? initialNotebookEntry),
        id: input.id,
        title: input.title,
        body: input.body,
        tags: input.tags.map((tag) => tag.toLowerCase()).sort(),
        kind: input.kind,
        claimStatus: input.claimStatus,
        eventDate: input.eventDate,
        followUpAfter: input.followUpAfter,
        followUpDate: input.followUpDate,
        updatedAt: "2026-05-29T10:05:00Z",
      };

      appTestState.notebookEntriesResponse = appTestState.notebookEntriesResponse.map((entry) =>
        entry.id === updated.id ? updated : entry,
      );

      return Promise.resolve(updated);
    }

    if (command === "delete_notebook_entry") {
      const { id } = args as { id: string };
      appTestState.notebookEntriesResponse = appTestState.notebookEntriesResponse.filter(
        (entry) => entry.id !== id,
      );

      return Promise.resolve();
    }

    if (command === "list_source_adapters") {
      const input = (args as { input?: { includeDeveloperOnly?: boolean } } | undefined)?.input;
      const includeDeveloperOnly = Boolean(input?.includeDeveloperOnly);
      return Promise.resolve(
        includeDeveloperOnly
          ? appTestState.sourceAdaptersResponse
          : appTestState.sourceAdaptersResponse.filter((adapter) => adapter.visibility !== "developer"),
      );
    }

    if (command === "set_source_adapter_enabled") {
      const input = (args as { input: { adapterId: string; enabled: boolean } }).input;
      const adapter = appTestState.sourceAdaptersResponse.find((sourceAdapter) => sourceAdapter.id === input.adapterId);
      if (!adapter || !adapter.userConfigurable) {
        return Promise.reject(new Error("source is not user configurable"));
      }

      adapter.enabled = input.enabled;
      adapter.healthStatus = input.enabled ? "notRefreshed" : "off";
      return Promise.resolve(adapter);
    }

    if (command === "list_unmatched_source_items") {
      return Promise.resolve(initialUnmatchedSourceItems);
    }

    if (command === "list_company_registry_entries") {
      return Promise.resolve(appTestState.companyRegistryEntriesResponse);
    }

    if (command === "refresh_sources") {
      if (appTestState.refreshSourcesError) {
        return Promise.reject(new Error(appTestState.refreshSourcesError));
      }

      appTestState.feedItemsResponse = [
        {
          ...initialFeedItems[0],
          id: "feed_gpw_espi_ebi_refreshed_ntc",
          company: "GPW:CDR",
          title: "Refreshed GPW report from sample source",
          summary: "",
          time: "2026-05-30T17:13:31+02:00",
          publishedAt: "2026-05-30T17:13:31+02:00",
          fetchedAt: "2026-05-30T17:30:00Z",
          unread: true,
          saved: false,
          bodyText: "Official GPW body text fetched from the detail page.",
          attachments: [
            {
              id: "feed_attachment_sample_report_pdf",
              label: "report.pdf",
              url: "https://www.gpw.pl/pub/GPW/ESPI/2026/report.pdf",
            },
          ],
        },
        ...appTestState.feedItemsResponse,
      ];

      return Promise.resolve({
        adapterId: "gpw-espi-ebi",
        itemsFetched: 2,
        itemsCreated: 2,
        itemsMatched: 1,
        itemsUnmatched: 1,
        detailItemsAttempted: 1,
        detailItemsStored: 1,
        detailItemsFailed: 0,
        fetchedAt: "2026-05-30T17:30:00Z",
      });
    }

    if (command === "refresh_source") {
      const input = (args as { input: { adapterId: string } }).input;

      return Promise.resolve({
        adapterId: input.adapterId,
        itemsFetched: 2,
        itemsCreated: 1,
        itemsMatched: 1,
        itemsUnmatched: 0,
        detailItemsAttempted: 0,
        detailItemsStored: 0,
        detailItemsFailed: 0,
        fetchedAt: "2026-05-30T17:30:00Z",
      });
    }

    if (command === "refresh_gpw_company_registry") {
      return Promise.resolve({
        adapterId: "company-directories",
        entriesFetched: 750,
        entriesUpserted: 750,
        entriesDeactivated: 0,
        fetchedAt: "2026-05-31T12:00:00Z",
      });
    }

    if (command === "get_settings") {
      return Promise.resolve(appTestState.settingsResponse);
    }

    if (command === "get_license_status") {
      return Promise.resolve(appTestState.licenseStatusResponse);
    }

    if (command === "submit_license_key") {
      const input = (args as { input: { licenseKey: string } }).input;

      if (input.licenseKey.includes("valid-friend-license")) {
        appTestState.licenseStatusResponse = initialLicenseStatus;
      } else {
        appTestState.licenseStatusResponse = invalidLicenseStatus;
      }

      return Promise.resolve(appTestState.licenseStatusResponse);
    }

    if (command === "clear_license_key") {
      appTestState.licenseStatusResponse = missingLicenseStatus;

      return Promise.resolve(appTestState.licenseStatusResponse);
    }

    if (command === "get_local_metrics_snapshot") {
      return Promise.resolve(appTestState.localMetricsSnapshotResponse);
    }

    if (command === "get_gemini_transcription_credential_status") {
      return Promise.resolve(appTestState.geminiCredentialStatusResponse);
    }

    if (command === "set_gemini_transcription_api_key") {
      const input = (args as { input: { apiKey: string } }).input;

      if (!input.apiKey.trim()) {
        return Promise.reject(new Error("credential value is required"));
      }

      appTestState.geminiCredentialStatusResponse = {
        ...initialGeminiCredentialStatus,
        configured: true,
        storage: "os_keychain",
      };

      return Promise.resolve(appTestState.geminiCredentialStatusResponse);
    }

    if (command === "clear_gemini_transcription_api_key") {
      appTestState.geminiCredentialStatusResponse = initialGeminiCredentialStatus;

      return Promise.resolve(appTestState.geminiCredentialStatusResponse);
    }

    if (command === "update_settings") {
      const input = (args as {
        input: {
          theme?: Theme;
          accentPalette?: AccentPalette;
          locale?: AppLocale;
          pollIntervalSeconds?: number;
          youtubeTranscriptionModel?: string;
          youtubeTranscriptionTimeoutSeconds?: number;
          generalAnalysisProvider?: string;
          generalAnalysisModel?: string;
          generalAnalysisTimeoutSeconds?: number;
          shortcutBindings?: Record<string, ShortcutBindingSetting>;
        };
      }).input;

    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      theme: input.theme ?? appTestState.settingsResponse.theme,
      accentPalette: input.accentPalette ?? appTestState.settingsResponse.accentPalette,
      locale: input.locale ?? appTestState.settingsResponse.locale,
        pollIntervalSeconds: input.pollIntervalSeconds ?? appTestState.settingsResponse.pollIntervalSeconds,
        shortcutBindings: input.shortcutBindings ?? appTestState.settingsResponse.shortcutBindings,
        aiProviders: {
          ...appTestState.settingsResponse.aiProviders,
          youtubeTranscriptionModel:
            input.youtubeTranscriptionModel ??
            appTestState.settingsResponse.aiProviders.youtubeTranscriptionModel,
          youtubeTranscriptionTimeoutSeconds:
            input.youtubeTranscriptionTimeoutSeconds ??
            appTestState.settingsResponse.aiProviders.youtubeTranscriptionTimeoutSeconds,
          generalAnalysisProvider:
            input.generalAnalysisProvider ??
            appTestState.settingsResponse.aiProviders.generalAnalysisProvider,
          generalAnalysisModel:
            input.generalAnalysisModel ??
            appTestState.settingsResponse.aiProviders.generalAnalysisModel,
          generalAnalysisTimeoutSeconds:
            input.generalAnalysisTimeoutSeconds ??
            appTestState.settingsResponse.aiProviders.generalAnalysisTimeoutSeconds,
        },
      };

      return Promise.resolve(appTestState.settingsResponse);
    }

    if (command === "list_ai_analysis") {
      const input = (args as { input: { feedItemId: string } }).input;

      return Promise.resolve(appTestState.aiAnalysisJobsResponse[input.feedItemId] ?? []);
    }

    if (command === "start_ai_analysis") {
      const input = (args as {
        input: { feedItemId: string; promptPresetId?: string; customQuestion?: string };
      }).input;
      const item =
        appTestState.feedItemsResponse.find((feedItem) => feedItem.id === input.feedItemId) ??
        initialFeedItems.find((feedItem) => feedItem.id === input.feedItemId) ??
        initialFeedItems[0];
      const createdAt = new Date("2026-01-01T10:00:00.000Z").toISOString();
      const job: AiAnalysisJob = {
        id: `ai_job_${input.feedItemId}_${appTestState.aiAnalysisJobsResponse[input.feedItemId]?.length ?? 0}`,
        feedItemId: input.feedItemId,
        promptPresetId: input.promptPresetId ?? "default_summary",
        customQuestion: input.customQuestion ?? null,
        providerId: appTestState.settingsResponse.aiProviders.generalAnalysisProvider ?? "provider_gemini",
        model: appTestState.settingsResponse.aiProviders.generalAnalysisModel,
        promptVersion: "analysis_v1",
        status: "succeeded",
        errorCode: null,
        error: null,
        createdAt,
        startedAt: createdAt,
        finishedAt: createdAt,
        result: {
          id: `ai_result_${input.feedItemId}`,
          aiAnalysisJobId: null,
          feedItemId: input.feedItemId,
          providerId: appTestState.settingsResponse.aiProviders.generalAnalysisProvider ?? "provider_gemini",
          model: appTestState.settingsResponse.aiProviders.generalAnalysisModel,
          promptVersion: "analysis_v1",
          summary: `AI summary for ${item.title}`,
          significance: "medium",
          reasoning: "Grounded in the selected feed item summary and source metadata.",
          language: item.language,
          tags: ["analysis", "feed"],
          sourceReferences: [
            {
              id: `ai_source_${input.feedItemId}`,
              sourceUrl: item.sourceUrl,
              label: item.source,
              createdAt,
            },
          ],
          createdAt,
        },
      };

      appTestState.aiAnalysisJobsResponse[input.feedItemId] = [
        job,
        ...(appTestState.aiAnalysisJobsResponse[input.feedItemId] ?? []),
      ];

      return Promise.resolve(job);
    }

    if (command === "retry_ai_analysis") {
      const { jobId } = args as { jobId: string };
      const feedItemId =
        Object.entries(appTestState.aiAnalysisJobsResponse).find(([, jobs]) =>
          jobs.some((job) => job.id === jobId),
        )?.[0] ?? initialFeedItems[0].id;
      const job = appTestState.aiAnalysisJobsResponse[feedItemId]?.find((candidate) => candidate.id === jobId);

      if (!job) {
        return Promise.reject(new Error("AI analysis job not found"));
      }

      const retriedJob: AiAnalysisJob = {
        ...job,
        status: "succeeded",
        errorCode: null,
        error: null,
      };
      appTestState.aiAnalysisJobsResponse[feedItemId] = [retriedJob];

      return Promise.resolve(retriedJob);
    }

    if (command === "update_feed_item_state") {
      const input = (args as { input: { id: string; read?: boolean; saved?: boolean } }).input;
      const item =
        appTestState.feedItemsResponse.find((feedItem) => feedItem.id === input.id) ??
        initialFeedItems.find((feedItem) => feedItem.id === input.id) ??
        initialFeedItems[0];

      return Promise.resolve({
        ...item,
        unread: input.read === undefined ? item.unread : !input.read,
        saved: input.saved ?? item.saved,
      });
    }

  return Promise.reject(new Error(`Unexpected command: ${command}`));
}
