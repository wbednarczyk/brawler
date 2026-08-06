# AI in Brawler: transcripts only (BYOA)

Since v0.59 the in-app AI analysis layer is **retired**: no capability routing,
no provider pools, no in-app KPI/claim extraction or summaries. Brawler is a
deterministic research substrate; **intelligence comes from your own agent**
talking to Brawler over the local MCP port (BYOA — bring your own agent). See
[MCP server](mcp-server.md).

The **one** AI feature that remains in-app is **YouTube transcription** — data
acquisition, not interpretation: it turns a recording of an earnings call into
searchable text in the company notebook.

## Setting up transcription

1. **Settings → AI → Transcript provider** — Gemini is the supported provider.
2. **Settings → Credentials** — save your Gemini API key (the free tier is
   sufficient for occasional transcriptions).

No other keys are used anywhere in the app. Outputs from the pre-v0.59 AI
features (old analyses, briefs, digests) were deleted, not archived, when the
layer retired — a deliberate clean cut, not a data-loss bug.

## Where the old AI features went

- **KPI extraction** → deterministic readers + the BiznesRadar-primary daily
  pull ([Fundamentals & Coverage](fundamentals-coverage.md)). No AI, no keys.
- **Claims, verdicts, narratives** → manual entry, plus agent-assisted writes
  via MCP write-tools with mandatory provenance ([The MCP server](mcp-server.md)) —
  off by default, opt in from Settings.
- **Talking to your research** → any MCP-capable agent (Claude, etc.) connected
  to the local MCP server.
