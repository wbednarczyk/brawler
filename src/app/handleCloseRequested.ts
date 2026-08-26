// Window-close interceptor (F3a S2, ADR 0107): a pure function so the dirty
// gate on a native OS close request is testable without a real Tauri window.
// Wired in `useAppLifecycleEffects.ts` via `getCurrentWindow().onCloseRequested`
// — Tauri-only; a no-op in the browser test/dev harness.
export type CloseRequestedEvent = { preventDefault(): void };
export type CloseRequestHost = { isDirty(): boolean; ask(): void };

export function handleCloseRequested(event: CloseRequestedEvent, host: CloseRequestHost): void {
  if (!host.isDirty()) return;
  event.preventDefault();
  host.ask();
}
