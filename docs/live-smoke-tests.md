# Live Smoke Tests

Live smoke tests validate real external providers and sources. They are not part of the default local check set or default CI because they require credentials, network access, and external service availability.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Engineering Workflow](engineering-workflow.md), [Project Practices](project-practices.md), and [Kanban](kanban.md).

## OS Keyring Persistence

Purpose: prove that the runtime OS credential backend can persist the Gemini transcription API-key credential target used by the app.

Command:

```bash
make smoke-keyring
```

Expected result:

- The ignored Rust smoke test `live_keyring_persists_gemini_transcription_secret` passes.
- The test writes a temporary secret to the real OS credential store, reads it back, clears it, verifies it is gone, and restores any pre-existing Gemini key.

Failure interpretation:

- A failure means the current app runtime cannot rely on the configured OS keyring backend for Gemini credentials.
- This test must be run on the same OS/runtime whose credential persistence is being validated. A WSL run validates the WSL/Linux keyring path; it does not validate the packaged Windows app's Windows Credential Manager path.
- If the packaged Windows app cannot persist credentials while this test passes only in WSL, M10 must add a Windows-runtime keyring validation path or replace the Windows credential backend before closure.

## Gemini YouTube Transcription

Purpose: prove that the configured Gemini transcription model can process a real supported public YouTube URL and return transcript segments.

Default model:

- `gemini-2.5-flash`

Required environment:

- `GEMINI_API_KEY`: Gemini API key from Google AI Studio.
- `BRAWLER_GEMINI_SMOKE_YOUTUBE_URL`: public YouTube URL to transcribe.
- `BRAWLER_GEMINI_SMOKE_MODEL`: optional model override when validating alternatives.
- `BRAWLER_GEMINI_REQUEST_TIMEOUT_SECONDS`: optional request timeout override. Use a shorter value such as `45` for fail-fast provider smoke attempts; keep the app setting default of `300` seconds, or a longer configured value, for real conference videos.

Recommended first validation URL:

- `https://www.youtube.com/watch?v=9hE5-98ZeCg`

This is the public YouTube URL used by Google's own Gemini video-understanding documentation examples. Use it first to validate provider wiring with a short known input before testing longer investor conference videos.

Command:

```bash
GEMINI_API_KEY=... \
BRAWLER_GEMINI_SMOKE_YOUTUBE_URL='https://www.youtube.com/watch?v=9hE5-98ZeCg' \
make smoke-gemini-transcript
```

Fail-fast command while checking provider capacity:

```bash
BRAWLER_GEMINI_REQUEST_TIMEOUT_SECONDS=45 \
BRAWLER_GEMINI_SMOKE_YOUTUBE_URL='https://www.youtube.com/watch?v=9hE5-98ZeCg' \
make smoke-gemini-transcript
```

Expected result:

- The ignored Rust smoke test `live_gemini_transcribes_youtube_url` passes.
- Output includes the model name and number of transcript segments created.

Failure interpretation:

- Missing credentials: create a Gemini API key in Google AI Studio and set `GEMINI_API_KEY`.
- Provider limit: retry later or choose a smaller/cheaper model if available.
- Provider unavailable: Gemini accepted the request shape but the selected model is temporarily unavailable or overloaded. Retry later or rerun with `BRAWLER_GEMINI_SMOKE_MODEL=gemini-2.5-flash`, `BRAWLER_GEMINI_SMOKE_MODEL=gemini-3.1-flash-lite`, or `BRAWLER_GEMINI_SMOKE_MODEL=gemini-3.5-flash`.
- Network timeout: retry with the recommended short validation URL first. If the short URL passes but a longer conference video times out, increase the app's provider timeout/configuration in the next implementation slice instead of treating Gemini as non-working.
- Provider rejection: the selected YouTube URL or model is not accepted by Gemini. Try another public YouTube URL first. If the default model rejects supported public URLs, M10 must change the default to the cheapest model that passes the live smoke test.
- Parse error: Gemini returned output that did not match the requested JSON transcript shape; fix provider prompting/parsing before closing M10.

M10 closure rule:

- M10 cannot close until this smoke test has passed at least once on the milestone branch.
- The smoke test result must not require storing the API key in the repository, Nix files, `.envrc`, logs, or exported settings.
