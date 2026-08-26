# Analyst recommendations: what the brokers say, tracked over time

The **Analyst recommendations** panel collects sell-side recommendations for a
company — who rated it, what the rating says, the target price, and when — as
**attributed third-party opinions**. The app quotes them; it never turns them
into its own advice, and they never leak into scores or valuation.

The [Spółka screen's](company-view.md) core has a compact Recommendations
card at rest; open the full history with its **Open recommendations** button
or the matching workshop-bar tool.

## What each entry shows

- The **rating exactly as the source printed it** ("kupuj", "akumuluj",
  "trzymaj", "redukuj", "sprzedaj" — never translated or normalized away),
  with a direction marker versus the **same firm's** previous entry:
  upgrade ▲, downgrade ▼, new coverage, or reiterated.
- The **target price** (with % versus the current close, when price data is
  available) — always with the **firm and date right next to the number**.
- The issuing **firm and analyst**, the publication date, and a link to the
  **broker's PDF report** when the source provides one.

A summary strip on top shows the latest target (with its attribution), how many
entries your local history holds, and when it last changed.

## Where the data comes from — and why history grows slowly

Entries come from BiznesRadar's public per-company recommendations page, checked
daily. The free page only ever shows the **few most recent** recommendations —
so Brawler **accumulates its own history from the day tracking starts**, entry
by entry, append-only. The panel footer says this plainly. Revision history
cannot be backfilled; the earlier you track a company, the richer its history.

## Recommendations reach your alerts

Every new or changed recommendation raises a `recommendation_change` signal:
it shows up as a feed badge, in the morning briefing, and any
[alert rule](attention-and-briefing.md) pointed at the signal category fires
like for any other signal.

## Vs target, beside the price

The company's price context shows a compact **"vs target"** readout — the latest
target price and the distance from the current close — always naming the firm
and date beneath the number, with a jump into the full panel. A number without
its author never appears.
