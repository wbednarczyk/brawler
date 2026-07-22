# Attention alerts and the morning briefing

Brawler can tell you **what deserves a look** instead of leaving you to
re-scan everything. Two things work together: **alert rules** you set up once,
and the **morning briefing** at the top of Today that sums up what changed. Both
are **decision support only** — they state facts and link to the evidence, and
never tell you to buy, sell, or hold.

## The morning briefing

Open **Today**. At the very top sits the **morning briefing** — a short answer
to "what changed in my companies, and what needs doing?". It pulls together, for
your tracked companies:

- **new typed signals** since your last briefing (insider trades, dividends,
  profit warnings…),
- **autopilot runs** that fetched and read a new report,
- **claims due** for verification,
- **upcoming report dates**, and
- **alerts that fired**.

Each line links straight to its evidence — click it to jump to the signal, the
run, or the price context behind it.

**The briefing is deterministic** — a structured list composed from your data (new reports, signals, due claims), no AI involved and no keys needed.

**Refreshing it.** Brawler composes one **automatically once a day** while the
app is open. You can also press **Generate briefing** any time to recompose it
on the spot.

## Alert rules

An **alert rule** says *what you want to be told about*. Set them up in
**Alerts** (left sidebar, Library group). Each rule has three parts:

1. **Trigger** — one of:
   - a **signal category** (e.g. any insider transaction, any profit warning),
   - an **autopilot run completing** for a company, or
   - a **price condition**: the price **enters a range you set** (a low and a
     high), or reaches a **52-week low**.
2. **Scope** — a **single company** or a whole **watchlist**.
3. **On / off** — disable a rule to silence it without deleting it.

Start from the **preset rule chips** for the common cases, pick the scope, and
for a price range type or drag the **min/max** values. Price rules are checked
against the daily prices on each pull. You can delete a rule (with an undo), and
review recently **fired alerts** on the same screen.

## Where fired alerts show up

When a rule fires, Brawler records an **attention event** tied to the exact
thing that set it off. You see it in two places:

- A **pop-up alert** (toast) in the corner, which you can **click through** to
  the evidence, or dismiss.
- The **Today attention list**, where fired alerts sit in the stream **grouped
  by company**. Each has a **Review** action (marks it seen and opens the
  evidence) and a **Dismiss**.

An alert **never fires twice for the same thing** — re-fetching the same filing
won't re-alert you — and each rule alerts at most **once a day**.

## A note on scope

Attention routing is **in-app only** for now: alerts live inside Brawler, not as
operating-system notifications (that may come later). Everything stays **local**
on your machine, and every alert is a **fact with a link** — never advice.

---

### A small quality-of-life note: confirmations

Around the app, when you kick off something that runs in the background — a
**source refresh**, an **import**, or starting an **AI digest or brief** —
Brawler now confirms it with a brief **pop-up** in the corner that fades on its
own. It's just reassurance that the action started or finished; the detailed
result still appears where it always did.
