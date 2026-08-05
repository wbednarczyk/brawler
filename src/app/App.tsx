import { useCallback, useState } from "react";
import { AppStateRoot } from "./AppStateRoot";
import type { Section } from "./navigation";
import type { LicenseStatus } from "../api/types";
import { ToastProvider } from "../ui";

type AppProps = {
  initialLicenseStatus?: LicenseStatus | null;
  /** Override the starting section. Defaults to the Cockpit (ADR 0053 phase 6).
   *  Used for deep links, and by tests to land directly on a screen not
   *  reachable from the top-nav (hosted as a Cockpit panel instead). */
  initialSection?: Section;
};

type PersistentOverflowConfig = {
  label: (hiddenCount: number) => string;
  onClick: () => void;
};

const noopPersistentOverflow: PersistentOverflowConfig = {
  label: (hiddenCount) => `+${hiddenCount} more`,
  onClick: () => {},
};

export function App({ initialLicenseStatus, initialSection }: AppProps = {}) {
  // ToastProvider mounts above AppStateRoot so the state controllers (called in
  // AppStateRoot's body) can consume useToast for the undo-vs-confirm contract
  // (ADR 0076 Decision 5). Its viewport renders locale-agnostic, pre-translated
  // strings, so sitting above LocaleContext is fine.
  //
  // The persistent-toast overflow summary ("+N more") needs a translated label
  // AND `setActiveSection("Today")` — both live inside AppStateRoot (locale +
  // navigation state), below ToastProvider. Rather than restructure the mount
  // order, AppStateRoot binds the real label/onClick up here once it has
  // mounted (mirrors the `openPaletteRef` bridge in AppShell.tsx for the same
  // "provider needs a callback from a descendant" shape) — but as STATE, not a
  // ref (D1 fix, v0.57 fix wave 2: a live-defect screenshot showed the English
  // "+9 more" literal surviving in the Polish app). `AppStateRoot` is a
  // *sibling* of `ToastViewport` under `ToastProvider`, not its ancestor, so
  // AppStateRoot re-rendering (e.g. once its `locale` state finishes loading)
  // never cascades to `ToastViewport` — only a state update owned by an
  // ancestor `ToastViewport` shares (here, `App`) does. A ref write is
  // invisible to React's render scheduler, so a translated rebind that lands
  // after the toasts already rendered would never reach the DOM without a
  // fresh toast/dismiss to force it. State makes every rebind trigger the
  // re-render that actually updates the summary row.
  const [persistentOverflow, setPersistentOverflow] =
    useState<PersistentOverflowConfig>(noopPersistentOverflow);
  const bindPersistentOverflow = useCallback((config: PersistentOverflowConfig) => {
    setPersistentOverflow(config);
  }, []);

  return (
    <ToastProvider
      onPersistentOverflowClick={persistentOverflow.onClick}
      persistentOverflowLabel={persistentOverflow.label}
    >
      <AppStateRoot
        initialLicenseStatus={initialLicenseStatus}
        initialSection={initialSection}
        onBindPersistentOverflow={bindPersistentOverflow}
      />
    </ToastProvider>
  );
}
