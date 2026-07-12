# The MCP server — let an AI assistant read your research

Brawler can expose a small, **read-only** connector that an AI assistant (Claude
Code, Claude Desktop, or any other MCP client) can call to answer questions
about **your own research** — the companies you track, the facts you've
confirmed, the management claims you're watching, and your quality assessments.

It uses **MCP** (the Model Context Protocol), the standard way local AI
assistants plug into tools. Turn it on and your assistant can pull from Brawler
directly — no copy-paste, no bespoke integration.

Two things to be clear about up front:

- It is **read-only**. The assistant can *look up* your research; it cannot
  change anything, add companies, or run jobs.
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

## What the assistant can ask for

Four read-only tools, each backed by the same data you see in the app:

- **`get_company_dossier`** — a company's identity, fundamentals coverage, a
  slice of confirmed facts, and its scorecard summary.
- **`search_research`** — full-text search across your research (optionally
  scoped to one company).
- **`list_claims_due`** — the management claims that are due or overdue to
  verify.
- **`get_quality_assessment`** — your qualitative assessment and quality-
  framework evaluations for a company.

All four are **decision support**, never advice: they return your sourced facts
and computed analysis, and — like the rest of Brawler — they never tell the
assistant to buy, sell, or hold.

## Security posture

- **Localhost only.** The server binds `127.0.0.1` and nothing else. Nothing on
  your network or the internet can reach it; the bind address is not
  configurable.
- **Token required.** Every request must carry your bearer token; anything else
  is refused. The token lives in your OS keychain and is shown only once.
- **Off by default, app-open-only.** It runs only when you enable it and only
  while Brawler is open. Closing Brawler stops it.
- **Read-only.** There is no tool that writes, deletes, or triggers work.

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
