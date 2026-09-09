import "@testing-library/jest-dom/vitest";
import { configure } from "@testing-library/react";
import { beforeEach, vi } from "vitest";

// Per-test isolation for browser storage (ADR 0048 decision 2 — "no cross-test
// bleed" — which the runtime reset honored but `localStorage` did not). jsdom
// keeps one `localStorage` for the whole FILE, so any persisted UI state
// (e.g. a stored pane width) would otherwise bleed across tests.
beforeEach(() => {
  window.localStorage.clear();
  window.sessionStorage.clear();
});

// Testing Library's `waitFor`/`findBy*` default budget is 1 SECOND, which is
// marginal on a loaded runner rendering the whole app. Raised once here so a
// slow-but-correct wait is not reported as a wrong result.
//
// It MUST stay well under vitest's per-test `testTimeout` (15s, vitest.config.ts).
// When the two are equal, a wait that needs its full budget kills the test with a
// bare "timed out in 5000ms" instead of the assertion's own message — which is
// exactly how PR #317's failure read after #316 set this to the then-default 5s.
configure({ asyncUtilTimeout: 5_000 });

// Tauri module mocks for the whole frontend test run. These live here (the
// configured `setupFiles` entry in vitest.config.ts) rather than in
// appWorkflowHarness: since vitest 3, a `vi.mock` call is only honored from the
// test file itself or a setup file, not from a transitively-imported helper.
// The harness imports/re-exports the mocked members and resets them per-test in
// its own `beforeEach`.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: vi.fn(() => Promise.resolve("/home/test/Downloads")),
  join: vi.fn((...paths: string[]) => Promise.resolve(paths.join("/"))),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(() => Promise.resolve("/tmp/brawler-export.json")),
}));

// Click actionability (testing.md § Frontend test responsibilities rule 7):
// user-event no-ops on a disabled or detached target, so `click`/`dblClick`/
// `tripleClick` wait for enabled and fail BY NAME on a target that stays
// disabled, is detached, or never receives the click while connected (#493).
vi.mock("@testing-library/user-event", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@testing-library/user-event")>();

  // ponytail: native `:disabled` semantics only (no `aria-disabled`); the
  // first-`<legend>` exemption of a disabled `<fieldset>` isn't modeled (no
  // app code uses a disabled fieldset). `type`/`keyboard`/`paste` on a
  // disabled input stay unguarded — user-event's internal `this.click`
  // bypasses this public-API wrapper. Under `vi.useFakeTimers` the disabled
  // wait's timeout timer is faked too, so a target that never enables dies
  // as vitest's bare `testTimeout` instead of by name.
  function isDisabled(el: Element): boolean {
    return el.closest(":disabled") !== null;
  }

  function describeTarget(el: Element): string {
    const tag = el.tagName.toLowerCase();
    const raw = el.getAttribute("aria-label") ?? el.textContent?.trim() ?? "";
    const label = raw.length > 40 ? `${raw.slice(0, 40)}…` : raw;
    return `<${tag}${label ? ` "${label}"` : ""}>`;
  }

  function detachedError(el: Element): Error {
    return new Error(
      `clicked a detached ${describeTarget(el)} — the element was replaced by a re-render; await the ready state and re-query (e.g. findEnabledButton)`,
    );
  }

  function guardClick<Args extends unknown[], R>(
    fn: (element: Element, ...args: Args) => Promise<R>,
  ): (element: Element, ...args: Args) => Promise<R> {
    return async (element, ...args) => {
      if (!(element instanceof Element)) {
        return fn(element, ...args);
      }
      if (!element.isConnected) {
        throw detachedError(element);
      }
      if (isDisabled(element)) {
        const { waitFor, getConfig } = await import("@testing-library/dom");
        // Detachment ends the wait at once (a thrown error would only be
        // retried until the timeout).
        let detached = false;
        await waitFor(() => {
          detached = !element.isConnected;
          if (!detached && isDisabled(element)) {
            throw new Error(
              `clicked ${describeTarget(element)} but it stayed disabled for ${getConfig().asyncUtilTimeout} ms — await the state that enables it`,
            );
          }
        });
        if (detached) {
          throw detachedError(element);
        }
      }
      // user-event dispatches over several macrotasks; a target replaced or
      // disabled meanwhile takes no click, or a native click React ignores.
      // Witness at the window (earliest capture point): a click counts only
      // if the target is connected and enabled when it arrives.
      let receivedWhileConnected = false;
      const witness = (event: Event) => {
        const target = event.target;
        if (target instanceof Node && (target === element || element.contains(target))) {
          receivedWhileConnected ||= element.isConnected && !isDisabled(element);
        }
      };
      const scope = element.ownerDocument.defaultView ?? element.ownerDocument;
      scope.addEventListener("click", witness, true);
      let result: R;
      try {
        result = await fn(element, ...args);
      } finally {
        scope.removeEventListener("click", witness, true);
      }
      if (!receivedWhileConnected) {
        throw element.isConnected
          ? new Error(
              `clicked ${describeTarget(element)} but it received no click — it became disabled or unclickable while the click was dispatching; await the settled state first`,
            )
          : detachedError(element);
      }
      return result;
    };
  }

  function wrapUser(user: ReturnType<typeof actual.userEvent.setup>): ReturnType<typeof actual.userEvent.setup> {
    return {
      ...user,
      click: guardClick(user.click),
      dblClick: guardClick(user.dblClick),
      tripleClick: guardClick(user.tripleClick),
      setup: (...args: Parameters<typeof user.setup>) => wrapUser(user.setup(...args)),
    };
  }

  function wrapDirectApi(api: typeof actual.userEvent): typeof actual.userEvent {
    return {
      ...api,
      click: guardClick(api.click),
      dblClick: guardClick(api.dblClick),
      tripleClick: guardClick(api.tripleClick),
      setup: (...args: Parameters<typeof api.setup>) => wrapUser(api.setup(...args)),
    };
  }

  const guarded = wrapDirectApi(actual.userEvent);
  return { ...actual, default: guarded, userEvent: guarded };
});

// Several density-tier hooks (QualityPanel, TodayScreen, EventsScreen — ADR
// 0076 D6) construct a ResizeObserver at mount; jsdom does not implement it.
// This minimal stub lets those views render in the jsdom harness.
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
