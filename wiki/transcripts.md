# Transcripts

YouTube recordings turned into text — results calls, management interviews.
Paste a link; Gemini turns speech into text. Segments you select become a
company notebook note, with a link back to the minute in the recording.

## The screen

- **Composer**: a recording link (required) and an optional company. `Fetch
  transcript` is the one filled action when nothing else is going on. A
  transcript without a company is fine — link it later.
- **List**: one row per transcript — a human title (or "Recording from
  YouTube" until it has one), the link, one status chip (`Queued` · `In
  progress` · `Ready` · `Transcript failed`), the linked company (or `—` with
  a `Link company` action), and when it's finished, when it was fetched.
- **Row actions**: `Fetch again` (queued/failed rows), `Link company` (until
  one is set), `Remove` (asks to confirm — it also drops the stored
  segments).

## Reading a transcript

Open a row to see its segments: search them, tick the ones worth keeping,
`Add to notebook`. That opens a note draft — title, tags, kind, status,
dates — with the segments already quoted, each carrying its origin (the
recording and the minute). `Save note` writes it to the company's notebook;
`Discard` throws the draft away. Segments themselves are read-only — the
recording said what it said.

On a narrow window the segments fold behind `Show segments` so the row list
stays reachable; nothing is hidden, just collapsed.

## When Gemini isn't set up

Transcription needs a Gemini API key (Settings → Credentials — the only AI
this app uses). With no transcripts yet and no key, the screen is one
invitation: `Open settings`. If you already have transcripts, the list stays
visible — new fetches and retries just wait for the key.

## Empty states

No transcripts yet → the composer *is* the invitation: paste a link, `Fetch
transcript`. A search inside a transcript's segments with no match → a quiet
"no segment contains that" (no action — clear the search yourself). The list
failed to load → an error line with `Refresh transcripts`.
