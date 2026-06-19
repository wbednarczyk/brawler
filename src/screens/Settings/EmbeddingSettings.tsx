import { Download, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import * as interpretationApi from "../../api/interpretation";
import type { EmbeddingModelStatus } from "../../api/interpretation";
import { Button, ErrorText, FieldRow, SelectField } from "../../ui";
import { useLocale } from "../../shared/locale";

// On-device embedding model configuration (ADR 0035). Lives under the AI settings
// tab; the diagnostics panel only *tests* it. Configuration is just: pick the
// similarity method, and download the model when needed — with one bundled
// status+action line rather than a wall of read-only boxes.
export function EmbeddingSettings() {
  const { text } = useLocale();
  const [status, setStatus] = useState<EmbeddingModelStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [inFlight, setInFlight] = useState(false);

  function refresh() {
    interpretationApi
      .getEmbeddingModelStatus()
      .then((next) => {
        setStatus(next);
        setError(null);
      })
      .catch((cause) => setError(String(cause)));
  }

  useEffect(() => {
    refresh();
  }, []);

  // Advance the status while a download is in flight.
  useEffect(() => {
    if (status?.weightsState !== "downloading") {
      return;
    }
    const handle = window.setInterval(refresh, 1500);
    return () => window.clearInterval(handle);
  }, [status?.weightsState]);

  function run(action: () => Promise<EmbeddingModelStatus>) {
    setInFlight(true);
    action()
      .then((next) => {
        setStatus(next);
        setError(null);
      })
      .catch((cause) => setError(String(cause)))
      .finally(() => setInFlight(false));
  }

  const featureCompiled = status?.featureCompiled ?? false;
  const weightsState = status?.weightsState;
  const ready = weightsState === "ready";
  const downloading = weightsState === "downloading";
  const usingEmbedding = status?.activeSimilarityStrategy === "embedding";
  const embeddedTotal = status?.embeddedCounts.reduce((sum, entry) => sum + entry.count, 0) ?? 0;

  // One bundled status+action line per state.
  function statusCaption() {
    if (!ready) {
      if (status?.downloadError) {
        return status.downloadError;
      }
      return downloading
        ? text("Downloading the model (~450 MB)…")
        : text("Model not downloaded (~450 MB).");
    }
    return usingEmbedding
      ? `${embeddedTotal} ${text("feed items indexed")}`
      : text("Model ready.");
  }

  function statusAction() {
    if (!ready) {
      return downloading ? null : (
        <Button disabled={inFlight} onClick={() => run(interpretationApi.downloadEmbeddingModel)}>
          <Download size={15} />
          {text("Download model")}
        </Button>
      );
    }
    if (usingEmbedding) {
      return (
        <Button
          variant="secondary"
          disabled={inFlight}
          onClick={() => run(interpretationApi.rebuildEmbeddingIndex)}
        >
          <RefreshCw size={15} />
          {inFlight ? text("Reindexing") : text("Reindex")}
        </Button>
      );
    }
    return null;
  }

  return (
    <section
      className="settings-group settings-group--divided"
      aria-labelledby="settings-embedding-title"
    >
      <h2 id="settings-embedding-title">{text("Semantic similarity")}</h2>
      <p className="settings-note">
        {text("Match feed items by meaning, not just keywords. Optional, on-device, no API key.")}
      </p>

      {!featureCompiled ? (
        <p className="settings-note">{text("Not available in this build.")}</p>
      ) : (
        <>
          <FieldRow>
            <SelectField
              label={text("Similarity matching")}
              disabled={inFlight}
              value={status?.activeSimilarityStrategy ?? "static"}
              onChange={(event) =>
                run(() =>
                  interpretationApi.setSimilarityStrategy(
                    event.target.value === "embedding" ? "embedding" : "static",
                  ),
                )
              }
            >
              <option value="static">{text("Static (keywords)")}</option>
              <option value="embedding" disabled={!ready}>
                {text("On-device model")}
              </option>
            </SelectField>
          </FieldRow>

          <div className="embedding-status">
            <span className="settings-note">{statusCaption()}</span>
            {statusAction()}
          </div>
          {error ? <ErrorText>{error}</ErrorText> : null}
        </>
      )}
    </section>
  );
}
