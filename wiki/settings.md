# Settings

App-wide preferences, grouped into tabs. Most changes take effect immediately;
a few (marked below) apply the next time you restart Brawler.

## The tabs

- **Appearance** — theme (dark/light/system), accent palette, and interface
  language.
- **Sources** — `Check every` (how often sources poll automatically) and
  `How far back to fetch` (years of company history a fetch pulls in, 1–10).
- **Transcripts** — `Transcription quality` (the Gemini model used for YouTube
  transcription) and `Give up after` (how long to wait before a transcription
  attempt times out).
- **Credentials** — your Gemini API key (needed for transcription): whether
  one's configured, where it's stored, paste a new one, or clear it.
- **Import And Export** — save your research data or settings to a file, or
  load them back in, with a preview before anything is applied.
- **Keyboard shortcuts** — every shortcut, its keys, and a per-shortcut
  enable/reset.
- **Logs** — activity records stay on this computer, nothing is sent
  anywhere; `Detail level` controls how much gets recorded, and `History kept`
  shows how many log files are kept and how large each can grow.
- **Data storage** — `Parallel work` / `Wait when busy` / `Wait to start`
  govern how the app talks to its local database at once (applies after
  restart); **Background work** governs how many source refreshes and
  autopilot tasks run at once (applies after restart).
- **MCP server** — let an AI assistant read your research over the Model
  Context Protocol; see [The MCP server](mcp-server.md) for the full
  reference.
- **License** — your license status and key.

## Everyday moves

- **Change how far back a fetch reaches** — Sources → `How far back to
  fetch`, a preset or the slider (1–10 years).
- **Fix a slow transcription** — Transcripts → raise `Give up after`.
- **Rotate your Gemini key** — Credentials → paste a new key → `Save`, or
  `Clear` to remove it.
- **Back up before a risky change** — Import And Export → `Export` on
  Research data and/or Settings.
- **Quiet the activity log** — Logs → `Detail level` → `Errors only`.
