# Autopilot (the autonomous report pipeline)

**Autopilot** closes the loop on a periodic report: when a company you track
publishes a new report, Brawler can detect it, fetch it, read out the figures,
work out what changed, and check it against your open claims and research —
with no manual steps. The result shows up as one notification on **Today**.

It's opt-in **per company**, and it's still decision support only: the result
tells you *what changed* and *what to verify*, never buy/sell/hold.

## Turning it on

Autopilot is **off by default** for every company. Open a company's dashboard
→ the **Fundamentals** panel to set its mode for that one company, or use
**Companies → Manage settings** to set the same mode across several companies
at once (see [Per-company settings](company-settings.md)).

There are three modes:

- **Off** — nothing automatic. This is today's manual flow: you launch KPI
  extraction yourself and confirm every value.
- **Assist** — on detecting a new report, Brawler fetches it and extracts the
  figures for you automatically, but nothing is saved until **you** confirm
  it. The proposals land exactly where manual extraction would put them —
  review them in Fundamentals as usual (confirm, edit, or reject each one).
- **Autopilot** — the full loop. Extracted figures are saved automatically,
  flagged as **not yet reviewed by you**, fully cited, and easy to undo (see
  below). Use this once you trust a company's extraction quality and just
  want the numbers waiting for you.

Switching a company's mode never changes facts or runs it already produced.

## Where results show up

Every run appears as an **Autopilot card** on **Today**: a summary of the
report, a **Structure changed** note if the report's line items shifted from
what's on file (new or missing lines, a different reporting unit), and a
**Review** button that opens the company. If a run couldn't finish, the card
says so honestly rather than pretending nothing happened.

## Reviewing proposals

- **Assist-mode** runs land their proposals `pending`, exactly like a manual
  extraction — open Fundamentals and confirm, edit, or reject each one.
- **Autopilot-mode** runs skip that step: the figures are already saved. The
  Today card instead offers **Undo** (a two-step confirm) instead of
  confirm/reject.

### Undo

**Undo** reverts exactly the facts that one run produced — nothing from any
other run or manual entry. It's a two-step confirm to avoid a stray click
undoing real work; once done, the card shows **"Reverted N facts."** Undo is
idempotent — undoing an already-undone run does nothing.

Undo only appears on **autopilot-mode** runs that actually produced facts. An
assist-mode run has nothing to undo yet (its facts are still `pending`, so you
reject them the normal way instead), and a run that failed before producing
anything has nothing to revert either.

## What "provenance" means on a fact

Every fact carries a note on **how it got into Brawler**:

- **Structured** — read directly out of the report's own machine-readable
  data (the official filing format, or a PDF Brawler can map with certainty).
  This is the most reliable path: a clean structured read auto-confirms
  outright in **both** Assist and Autopilot modes, because there's no
  judgment call involved — it's a direct read, not a guess.
- **AI** — read out of the document's text by an AI model, used when a
  structured read isn't possible. AI proposals still follow the mode you
  picked (confirm/reject in Assist, auto-saved-but-flagged in Autopilot).

Either way, every fact you see keeps its source — click through to see the
snippet or filing section it came from.

## What Autopilot needs

- **AI access**, for the extraction step — with no AI provider configured,
  Assist/Autopilot still fetch the report and diff it against the last one,
  but flag extraction as unavailable rather than looping.
- **The app open.** Autopilot runs while Brawler is running, checking as new
  reports come in — it does not fetch or analyze anything while the app is
  closed.

## A note on trust and advice

Autopilot never tells you to buy, sell, or hold. It tells you what a new
report changed and what's worth double-checking — sourced, flagged when
unreviewed, and always reversible.
