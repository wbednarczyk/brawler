# The MCP server — let an AI assistant work with your research

Brawler can expose a connector that an AI assistant (Claude Code, Claude
Desktop, or any other MCP client) calls to work with **your own research** — the
companies you track, the facts you've confirmed, the management claims you're
watching, your quality assessments, and more. It can **read everything** and,
when you allow it, **write research back** (notes, claims, facts, verdicts) —
always with a source.

It uses **MCP** (the Model Context Protocol), the standard way local AI
assistants plug into tools. Turn it on and your assistant pulls from Brawler
directly — no copy-paste, no bespoke integration.

This page is the **reference** (enabling, security, troubleshooting). For the
step-by-step *connect an agent and let it write* walk-through, see
**[Connecting an AI agent to Brawler](mcp-agent-guide.md)**.

Two tiers, and one thing to be clear about up front:

- **Read tools** are live whenever the server is on. The assistant can look up
  anything you can see in the app.
- **Act tools** (write, mark, trigger jobs) are **off by default** and only work
  after you turn on **Allow write tools**. Deletes, undo, and
  settings/credentials are **never** exposed — those stay UI-only.
- It is **local and off by default**. The server only ever listens on your own
  machine (`127.0.0.1`), only while Brawler is open, only when you enable it,
  and only for a caller holding your token.

## Turning it on (Settings → MCP server)

1. Open **Settings → MCP server**.
2. **Generate a token.** Brawler shows the token **exactly once** — copy it
   right then. It is stored in your OS keychain; Brawler never shows it again
   and never writes it to a log. Lost it? Just **regenerate** (the old one stops
   working immediately) or **revoke** to switch the connector off entirely.
3. **Enable the server.** The **status pill** turns to *running* and shows the
   port (default **8317**).

Enabling and disabling take effect live — no restart. Changing the **port**
takes effect the next time the server starts (toggle it off and on again).

## Where to add the server (Windows vs WSL)

**Add the connector to a Claude that runs on the same machine as Brawler.** Brawler
is a Windows app and its server listens only on Windows' own loopback
(`127.0.0.1`) — so the client has to reach *that* loopback:

- **Works:** Claude Desktop, or **Claude Code in a Windows terminal**
  (PowerShell / Windows Terminal). Same machine, same loopback — it connects.
- **Does *not* work (by default):** Claude Code running **inside WSL**. Under
  WSL2's default (NAT) networking, WSL has its *own* `127.0.0.1` that is a
  different loopback from Windows', and Brawler's server is deliberately
  loopback-only and not reachable across that boundary — so the connection is
  simply **refused**. This is the security posture working as intended, not a
  bug.
- **The WSL exception — mirrored networking.** WSL2's **mirrored** networking
  mode shares `localhost` in both directions, so Windows and WSL see the same
  loopback. With it on, Claude Code in WSL can reach the server. Enable it in
  `C:\Users\<you>\.wslconfig`:

  ```
  [wsl2]
  networkingMode=mirrored
  ```

  then run `wsl --shutdown` (or reboot) so WSL restarts with the new mode. After
  that the same `http://127.0.0.1:<port>/mcp` command works from WSL too.

If in doubt, add the connector from Claude on Windows — that always matches where
the app is running.

## Connecting your assistant

You need two things: the **port** (default 8317) and the **token** you copied.

### Claude Code (HTTP — recommended)

Claude Code speaks MCP's HTTP transport directly, so it connects to the
endpoint with no helper process:

```
claude mcp add --transport http brawler http://127.0.0.1:8317/mcp \
  --header "Authorization: Bearer <your-token>"
```

Swap `8317` for your port and `<your-token>` for the copied token. That's it —
ask Claude something like *"use the brawler tools to pull the dossier for
<ticker>"* and it will call through.

### The stdio adapter (for stdio-only clients)

Some MCP clients only speak the **stdio** transport (they launch a process and
talk over its stdin/stdout). Brawler ships a tiny bridge for them,
`brawler-mcp-stdio`, that forwards each line to the running HTTP server:

```
brawler-mcp-stdio --port 8317 --token <your-token>
```

It reads `BRAWLER_MCP_PORT` / `BRAWLER_MCP_TOKEN` from the environment if you
omit the flags. Point the client's stdio-server command at this executable
(it sits next to `brawler.exe` in the portable folder). The bridge does no
thinking of its own — it just pipes your assistant's requests to the same local
server, so Brawler must be **open with the server enabled** for it to work.

## What the assistant can do

The surface mirrors the app: **read tools** cover the whole workspace
(companies, watchlists, feed, financial facts with their provenance, ownership
and insiders, health and red flags, reports and diffs, transcripts, notes,
claims, expectations, the journal, quality frameworks, the calendar, autopilot
runs, attention events, and the morning briefing); **act tools** let the
assistant record research and run jobs once you allow writes. The complete,
always-current tool list lives in the
[agent guide's catalog](mcp-agent-guide.md#the-full-tool-catalog).

Everything is **decision support**, never advice: tools return your sourced
facts and computed analysis, and — like the rest of Brawler — never tell the
assistant to buy, sell, or hold.

## Letting the assistant write (Allow write tools)

Write tools are off until you turn on **Settings → MCP server → Allow write
tools**. Once on, the assistant can create and update notes, claims, facts,
verdicts, expectations, journal entries, and more, plus manage watchlists and
trigger jobs. Two guardrails always hold:

- **Every write must cite a source.** If the assistant tries to save a note,
  claim, fact, or verdict without saying where it came from, **Brawler refuses
  the write** and tells the assistant which source field is missing — nothing is
  stored. (The typed refusals are `provenance_required` for a missing source and
  `writes_disabled` when the toggle is off.)
- **The assistant can't switch its own writes on.** The setting is UI-only —
  only you can flip it, here. Turn it off any time and writes stop immediately.

What stays **UI-only, always**: deleting anything, undoing a run, and changing
settings, tokens, or credentials. Per-write citation rules and worked examples
are in the [agent guide](mcp-agent-guide.md#step-3--enabling-writes-optional).

## Security posture

- **Localhost only.** The server binds `127.0.0.1` and nothing else. Nothing on
  your network or the internet can reach it; the bind address is not
  configurable.
- **Token required.** Every request must carry your bearer token; anything else
  is refused. The token lives in your OS keychain and is shown only once.
- **Off by default, app-open-only.** It runs only when you enable it and only
  while Brawler is open. Closing Brawler stops it.
- **Writes off by default.** No tool writes, marks, or triggers work until you
  turn on *Allow write tools*; deletes/undo/settings are never exposed at all.

## Troubleshooting

The **status pill** in Settings tells you what's wrong:

- **"auth token is not configured"** — you enabled the server without generating
  a token. Generate one, then it will start.
- **"failed to bind … port … in use"** — another program (or a second Brawler)
  already holds that port. Pick a different port and re-enable.
- **Assistant hangs or says it can't reach the tool** — check the status pill
  reads *running*, that the port in your client matches, and that the token is
  current (regenerating invalidates the old one). With the stdio adapter,
  confirm Brawler is open and the server enabled — the adapter returns a clear
  error naming the port if it can't connect.
