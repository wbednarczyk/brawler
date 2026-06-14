-- Unified global full-text search index (ADR 0032).
-- search_index is DERIVED STATE, not a source of truth: it is kept in sync by the
-- per-source triggers below and is fully rebuildable from the source tables.
-- Coverage: companies, watchlists, feed items, notebook entries, transcript
-- segments, company events, research briefs, and digests. parent_id is the
-- navigational container when an item's own id is not the navigation target
-- (a transcript segment carries its owning transcript job). The tokenizer folds
-- case and diacritics for the Polish-primary, English-mixed corpus.

CREATE VIRTUAL TABLE search_index USING fts5(
    title,
    body,
    content_type UNINDEXED,
    source_id UNINDEXED,
    company_id UNINDEXED,
    parent_id UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- ===================== companies =====================
CREATE TRIGGER search_index_companies_ai AFTER INSERT ON companies BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.display_name, new.ticker || ' ' || new.qualified_ticker, 'company', new.id, new.id, NULL);
END;

CREATE TRIGGER search_index_companies_ad AFTER DELETE ON companies BEGIN
    DELETE FROM search_index WHERE content_type = 'company' AND source_id = old.id;
END;

CREATE TRIGGER search_index_companies_au
AFTER UPDATE OF display_name, ticker, qualified_ticker ON companies BEGIN
    DELETE FROM search_index WHERE content_type = 'company' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.display_name, new.ticker || ' ' || new.qualified_ticker, 'company', new.id, new.id, NULL);
END;

-- ===================== watchlists =====================
CREATE TRIGGER search_index_watchlists_ai AFTER INSERT ON watchlists BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.name, COALESCE(new.description, ''), 'watchlist', new.id, NULL, NULL);
END;

CREATE TRIGGER search_index_watchlists_ad AFTER DELETE ON watchlists BEGIN
    DELETE FROM search_index WHERE content_type = 'watchlist' AND source_id = old.id;
END;

CREATE TRIGGER search_index_watchlists_au
AFTER UPDATE OF name, description ON watchlists BEGIN
    DELETE FROM search_index WHERE content_type = 'watchlist' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.name, COALESCE(new.description, ''), 'watchlist', new.id, NULL, NULL);
END;

-- ===================== feed_items =====================
-- company_id is NULL: feed items map to companies many-to-many via feed_item_companies.
CREATE TRIGGER search_index_feed_items_ai AFTER INSERT ON feed_items BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, COALESCE(new.summary, '') || ' ' || COALESCE(new.body_text, ''), 'feed_item', new.id, NULL, NULL);
END;

CREATE TRIGGER search_index_feed_items_ad AFTER DELETE ON feed_items BEGIN
    DELETE FROM search_index WHERE content_type = 'feed_item' AND source_id = old.id;
END;

CREATE TRIGGER search_index_feed_items_au
AFTER UPDATE OF title, summary, body_text ON feed_items BEGIN
    DELETE FROM search_index WHERE content_type = 'feed_item' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, COALESCE(new.summary, '') || ' ' || COALESCE(new.body_text, ''), 'feed_item', new.id, NULL, NULL);
END;

-- ===================== notebook_entries =====================
CREATE TRIGGER search_index_notebook_entries_ai AFTER INSERT ON notebook_entries BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.body, 'notebook_entry', new.id, new.company_id, NULL);
END;

CREATE TRIGGER search_index_notebook_entries_ad AFTER DELETE ON notebook_entries BEGIN
    DELETE FROM search_index WHERE content_type = 'notebook_entry' AND source_id = old.id;
END;

CREATE TRIGGER search_index_notebook_entries_au
AFTER UPDATE OF title, body, company_id ON notebook_entries BEGIN
    DELETE FROM search_index WHERE content_type = 'notebook_entry' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.body, 'notebook_entry', new.id, new.company_id, NULL);
END;

-- ===================== transcript_segments =====================
-- segment text is immutable; only company_id/speaker can change after insert.
-- parent_id is the owning transcript job, the navigation target for a segment.
CREATE TRIGGER search_index_transcript_segments_ai AFTER INSERT ON transcript_segments BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (COALESCE(new.speaker, ''), new.text, 'transcript_segment', new.id, new.company_id, new.transcript_job_id);
END;

CREATE TRIGGER search_index_transcript_segments_ad AFTER DELETE ON transcript_segments BEGIN
    DELETE FROM search_index WHERE content_type = 'transcript_segment' AND source_id = old.id;
END;

CREATE TRIGGER search_index_transcript_segments_au
AFTER UPDATE OF company_id, speaker ON transcript_segments BEGIN
    DELETE FROM search_index WHERE content_type = 'transcript_segment' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (COALESCE(new.speaker, ''), new.text, 'transcript_segment', new.id, new.company_id, new.transcript_job_id);
END;

-- ===================== company_events =====================
CREATE TRIGGER search_index_company_events_ai AFTER INSERT ON company_events BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.event_type, 'event', new.id, new.company_id, NULL);
END;

CREATE TRIGGER search_index_company_events_ad AFTER DELETE ON company_events BEGIN
    DELETE FROM search_index WHERE content_type = 'event' AND source_id = old.id;
END;

CREATE TRIGGER search_index_company_events_au
AFTER UPDATE OF title, event_type, company_id ON company_events BEGIN
    DELETE FROM search_index WHERE content_type = 'event' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.event_type, 'event', new.id, new.company_id, NULL);
END;

-- ===================== ai_research_briefs =====================
-- company_id derived from scope when the brief is company-scoped.
CREATE TRIGGER search_index_research_briefs_ai AFTER INSERT ON ai_research_briefs BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.summary || ' ' || new.content_markdown, 'research_brief', new.id,
            CASE WHEN new.scope_type = 'company' THEN new.scope_id ELSE NULL END, NULL);
END;

CREATE TRIGGER search_index_research_briefs_ad AFTER DELETE ON ai_research_briefs BEGIN
    DELETE FROM search_index WHERE content_type = 'research_brief' AND source_id = old.id;
END;

CREATE TRIGGER search_index_research_briefs_au
AFTER UPDATE OF title, summary, content_markdown, scope_type, scope_id ON ai_research_briefs BEGIN
    DELETE FROM search_index WHERE content_type = 'research_brief' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.summary || ' ' || new.content_markdown, 'research_brief', new.id,
            CASE WHEN new.scope_type = 'company' THEN new.scope_id ELSE NULL END, NULL);
END;

-- ===================== ai_research_digests =====================
CREATE TRIGGER search_index_research_digests_ai AFTER INSERT ON ai_research_digests BEGIN
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.summary || ' ' || new.content_markdown, 'digest', new.id,
            CASE WHEN new.scope_type = 'company' THEN new.scope_id ELSE NULL END, NULL);
END;

CREATE TRIGGER search_index_research_digests_ad AFTER DELETE ON ai_research_digests BEGIN
    DELETE FROM search_index WHERE content_type = 'digest' AND source_id = old.id;
END;

CREATE TRIGGER search_index_research_digests_au
AFTER UPDATE OF title, summary, content_markdown, scope_type, scope_id ON ai_research_digests BEGIN
    DELETE FROM search_index WHERE content_type = 'digest' AND source_id = old.id;
    INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
    VALUES (new.title, new.summary || ' ' || new.content_markdown, 'digest', new.id,
            CASE WHEN new.scope_type = 'company' THEN new.scope_id ELSE NULL END, NULL);
END;

-- ===================== backfill existing rows =====================
INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT display_name, ticker || ' ' || qualified_ticker, 'company', id, id, NULL FROM companies;

INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT name, COALESCE(description, ''), 'watchlist', id, NULL, NULL FROM watchlists;

INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT title, COALESCE(summary, '') || ' ' || COALESCE(body_text, ''), 'feed_item', id, NULL, NULL FROM feed_items;

INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT title, body, 'notebook_entry', id, company_id, NULL FROM notebook_entries;

INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT COALESCE(speaker, ''), text, 'transcript_segment', id, company_id, transcript_job_id FROM transcript_segments;

INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT title, event_type, 'event', id, company_id, NULL FROM company_events;

INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT title, summary || ' ' || content_markdown, 'research_brief', id,
       CASE WHEN scope_type = 'company' THEN scope_id ELSE NULL END, NULL
FROM ai_research_briefs;

INSERT INTO search_index(title, body, content_type, source_id, company_id, parent_id)
SELECT title, summary || ' ' || content_markdown, 'digest', id,
       CASE WHEN scope_type = 'company' THEN scope_id ELSE NULL END, NULL
FROM ai_research_digests;
