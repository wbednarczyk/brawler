# Report comparison (report-over-report diff)

**Report comparison** shows you what changed between a company's two most recent
financial statements of the same kind — section by section. When a company files
this quarter's report, you don't have to read it cover-to-cover against last
quarter's: Brawler lines the two up and highlights the sections that **changed**,
were **added**, or were **removed**.

It's fully **local and offline** — no AI, no network, no API key. The comparison
is deterministic: the same two reports always produce the same result. And it's
decision support only — it shows you *what moved*, never whether to buy or sell.

## Where to find it

Open a company, then the **Fundamentals** tab. Below the stored report documents
you'll find the **Report comparison** panel.

## What it compares

Brawler compares **consecutive statements of the same type**, and never mixes
the two types:

- **Consolidated** statements (the group — Polish *skonsolidowane*, "SSF") are
  compared only against the previous consolidated statement.
- **Standalone** statements (the parent company alone — Polish *jednostkowe*,
  "JSF") are compared only against the previous standalone one.

This keeps the comparison meaningful: you're always looking at like-for-like.

The panel lists the comparable pairs it found from the reports you've already
fetched for that company, newest first, each labelled with its period (e.g.
*2026 Q1*). Click **Compare** on a pair to see the diff.

## Reading the diff

Each section of the statement is matched to its counterpart in the older report
and tagged:

- **Changed** — the section exists in both, but its text differs.
- **Added** — the section is new in the later report.
- **Removed** — the section was in the older report but is gone in the later one.
- **Unchanged** — identical in both (shown for context).

The header tells you how many sections were **aligned** (matched between the two
reports) so you can gauge how cleanly the two filings line up.

## Before you can compare

The comparison works on the **text** of reports you've already downloaded, so two
things have to be in place:

1. **The reports must be fetched, not just listed.** A report that's only been
   detected (metadata) has no file to read yet. Fetch it from the report
   documents panel first.
2. **The text must be extractable.** If you see *Sections are still being
   extracted*, use **Extract & compare** to run the one-time text extraction,
   then compare.

### When a report can't be compared

Some filings are **scanned images** with no real text layer (common for small
NewConnect issuers). Brawler can't read text out of an image, so it tells you
plainly: *Can't compare — no extractable text (scanned report).* This is a
limitation of the source document, not a bug — there's nothing reliable to diff.

If two statements are genuinely identical, you'll see *No changes between these
statements.*

## What's compared — and what isn't (yet)

This release compares the **structured financial statements** — the balance
sheet, income statement, cash-flow statement, and their notes — which line up
cleanly between filings.

The **narrative management report** (the discussion-and-analysis prose) is **not**
compared yet: across issuers its headings don't line up reliably enough to give a
trustworthy diff, so it's deferred to a later release rather than shown
half-working. A short AI-written "what changed" summary is also planned for a
future release. For now, the comparison is the deterministic, offline,
section-level text diff described above.

## Privacy

The reports, the extracted text, and every comparison stay **local on your
machine**. Nothing is uploaded.
