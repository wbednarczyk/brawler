# Quality frameworks

A **quality framework** is your own checklist of criteria that a company should
meet. Criteria come in two kinds:

- **Quantitative** — measurable checks like high return on equity, conservative
  debt, positive free cash flow. Brawler evaluates these against the company's
  reported fundamentals, fully deterministic and offline — no AI, no network.
- **Qualitative** — judgment questions like *does the company have a moat?* or
  *is management's capital allocation disciplined?* These are assessed by an AI
  agent **using only the evidence already stored in your app**, with citations
  you can check (see [Qualitative criteria](#qualitative-criteria-agent-assessed)).

Either way the result is a **scorecard**: pass / partial / fail / no-data (or
*insufficient evidence*) per criterion. And it's decision support only: a
framework measures facts and evidence, it never tells you to buy or sell.

> New to the expression syntax? See the **[DSL reference](dsl-reference.md)**.

## Where to find it

Open a company, then the **Quality** tab (next to *Fundamentals*).

## Frameworks that ship with Brawler

Brawler comes with a **Quality (Kroeze-style)** template — a general quality
checklist covering durable returns, healthy margins, conservative leverage, cash
generation, and growth. It's marked as a *Template*.

You can use a template in two ways:

- **Edit it in place.** Every framework is editable, templates included. Change a
  threshold, add or remove a criterion — it's yours to shape.
- **Clone it.** *Clone* makes a personal copy so the original stays untouched and
  you can keep several variants.

If you've edited a template and want the shipped defaults back, use **Reset**
(only shown for templates). Reset restores the template's original criteria;
your edits to that framework are replaced.

## Building your own framework

1. Click **New** and give it a name (e.g. "My quality screen").
2. Under **Add criterion**, give each check a **label** (e.g. "Strong return on
   equity") and an **expression** (e.g. `roe >= 15%`).
3. As you type the expression, Brawler validates it live and shows which metrics
   it uses. A red message means the expression can't be understood yet.
4. Click **Add**. Repeat for each criterion.

Criteria can be simple (`fcf > 0`) or combine several tests
(`net_debt_to_ebitda < 2.5 AND fcf > 0`). See the
[DSL reference](dsl-reference.md) for everything available.

## Qualitative criteria (agent-assessed)

Some things that matter — moat, pricing power, recurring revenue, capital
allocation — can't be written as a formula. For those, add a **qualitative**
criterion (*Add qualitative criterion*): instead of an expression it carries
**assessment guidance** — your description of what the agent should look at and
what strong evidence would look like.

How it works:

- Click **Assess** (whole framework) or **Assess this criterion** (one row). The
  assessment runs as a background job; results appear when it completes.
- The agent reads **only what your app already holds** for that company —
  claims, notes, typed signals, transcript segments, and stored report documents.
  It never searches the web, and every verdict must cite the evidence it used.
  You can open each citation and judge it yourself.
- The verdict is pass / partial / fail — or **insufficient evidence** when the
  stored material simply doesn't answer the question. That's an honest "can't
  tell yet", not a fail.
- Answers are written in the app's current language.
- **Re-run assessment** regenerates a verdict whenever you want (e.g. after new
  reports arrive). When [Autopilot](autopilot.md) processes a new report, it
  re-assesses qualitative criteria automatically, and a **changed verdict** is
  surfaced in your research digest so shifts don't slip by silently.
- Asking for an assessment while one is already running is safe — the request
  is parked and runs right after, so nothing is lost and nothing runs twice.

Qualitative assessment needs an AI provider: route the **Qualitative
assessment** capability to a provider (and optionally a failover pool) in
Settings → AI — see [AI provider pools](ai-provider-presets.md). Everything
else on this page stays fully offline.

## Running an evaluation

Pick the framework in the **Framework** dropdown and click **Evaluate**. Brawler
computes every metric your criteria need from the company's latest confirmed
financial period and shows a scorecard:

- **Pass** — the criterion is met.
- **Partial** — not met, but within the *partial band* you set (a softer
  threshold; optional).
- **Fail** — not met.
- **No data** — a metric the criterion needs isn't available yet (a missing
  fact). This is different from *Fail*: the company didn't fail the test, Brawler
  just couldn't compute it.

Each row shows the **measured value** next to its verdict, so you see *why* — not
just pass/fail, but "ROE was 18%".

## History is kept

Every evaluation is saved as an immutable snapshot. The measured values are
**pinned to the moment you ran it**, so when newer figures arrive (e.g. a
preliminary number is replaced by the audited final), past scorecards still show
what they showed then. The **Evaluation history** list lets you see how a
company has tracked against the framework over time. **Click a run to expand it**
and see that run's full per-criterion detail — each criterion's verdict and the
measured value as it stood then. Remove a run with its delete button to prune the
history.

## Which metrics can I use?

Brawler ships a broad library of computed metrics out of the box — margins,
returns (ROE, ROIC, ROCE), leverage (net debt / EBITDA, debt / equity),
liquidity (current ratio, quick ratio), cash flow (FCF, FCF margin, FCF
conversion), and more. The criteria editor shows the metrics each expression
uses, and a metric simply reads as **No data** for a company until the
underlying facts exist. The full list and the formula syntax are in the
[DSL reference](dsl-reference.md).

## Sharing frameworks

Frameworks and their criteria are part of your data export/import bundle, so a
framework you build travels with your data (including any custom metrics it
relies on). Evaluation results are reproducible, so they aren't required in the
bundle.
