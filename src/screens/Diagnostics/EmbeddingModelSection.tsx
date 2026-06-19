import { ChevronDown, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import * as feedApi from "../../api/feed";
import * as interpretationApi from "../../api/interpretation";
import type { FindSimilarContentResult } from "../../api/interpretation";
import type { FeedItem } from "../../api/types";
import { ActionRow, Button, EmptyState, ErrorText, SelectField, StatusPill } from "../../ui";
import { useLocale } from "../../shared/locale";

// Developer-mode similarity *tester* (ADR 0035). Configuration (download, strategy)
// lives in Settings → AI; this only runs "find similar" over real feed items with
// whatever strategy is active, to eyeball the ranking.
export function EmbeddingModelSection() {
  const { text } = useLocale();
  const [open, setOpen] = useState(false);
  const [feedItems, setFeedItems] = useState<FeedItem[]>([]);
  const [similarId, setSimilarId] = useState("");
  const [similar, setSimilar] = useState<FindSimilarContentResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [inFlight, setInFlight] = useState(false);

  // Load feed items only once the panel is opened (the picker for "find similar").
  useEffect(() => {
    if (!open || feedItems.length > 0) {
      return;
    }
    feedApi
      .listFeedItems()
      .then((items) => {
        setFeedItems(items);
        if (items.length > 0) {
          setSimilarId((current) => current || items[0].id);
        }
      })
      .catch((cause) => setError(String(cause)));
  }, [open, feedItems.length]);

  const feedTitleById = new Map(feedItems.map((item) => [item.id, item.title]));

  function runFindSimilar() {
    if (!similarId.trim()) {
      return;
    }
    setInFlight(true);
    interpretationApi
      .findSimilarContent({ contentType: "feed_item", contentId: similarId.trim() })
      .then((result) => {
        setSimilar(result);
        setError(null);
      })
      .catch((cause) => {
        setSimilar(null);
        setError(String(cause));
      })
      .finally(() => setInFlight(false));
  }

  return (
    <section className="diagnostics-section" aria-labelledby="diagnostics-similarity-title">
      <button
        aria-expanded={open}
        className="diagnostics-section-header"
        onClick={() => setOpen((value) => !value)}
        type="button"
      >
        <span>
          <h2 id="diagnostics-similarity-title">{text("Similarity check")}</h2>
          <small>
            {text("Test similarity ranking over your feed. Configure the model in Settings → AI.")}
          </small>
        </span>
        <ChevronDown
          className={open ? "section-chevron section-chevron-open" : "section-chevron"}
          size={16}
        />
      </button>
      {open ? (
        <div className="diagnostics-section-body">
          <div className="diagnostics-section-toolbar">
            <SelectField
              label={text("Find similar to feed item")}
              disabled={inFlight || feedItems.length === 0}
              value={similarId}
              onChange={(event) => setSimilarId(event.target.value)}
            >
              {feedItems.length === 0 ? (
                <option value="">{text("No feed items")}</option>
              ) : (
                feedItems.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.title}
                  </option>
                ))
              )}
            </SelectField>
            <ActionRow className="diagnostics-actions">
              <Button
                className="compact-button"
                disabled={inFlight || !similarId.trim()}
                onClick={runFindSimilar}
              >
                <Sparkles size={15} />
                {text("Find similar")}
              </Button>
            </ActionRow>
          </div>

          {error ? <ErrorText>{error}</ErrorText> : null}

          {similar ? (
            <div className="diagnostics-backups-list" aria-label={text("Similar content")}>
              <p className="settings-note">
                {text("Ranked by")} {similar.strategyId}
              </p>
              {similar.items.length > 0 ? (
                similar.items.map((item) => (
                  <article className="diagnostics-backup-row" key={item.contentId}>
                    <div>
                      <h3>{feedTitleById.get(item.contentId) ?? item.contentId}</h3>
                      <small>{item.contentType}</small>
                    </div>
                    <StatusPill tone="neutral">{item.score.toFixed(3)}</StatusPill>
                  </article>
                ))
              ) : (
                <EmptyState>{text("No similar items found.")}</EmptyState>
              )}
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
