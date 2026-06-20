import { createContext, useContext, type ReactNode } from "react";

import type { SourcesScreenProps } from "../../screens/Sources/sourceTypes";

/**
 * Sources-domain feature-scoped state (Architecture v2 / ADR 0050).
 *
 * The Sources screen's view-model (adapter catalog, refresh/registry state, the
 * action callbacks) is assembled once in `AppStateRoot` and provided here, so
 * `SourcesScreen` reads it from context instead of receiving ~30 props by
 * prop-drilling. The view-model shape is the former `SourcesScreenProps`.
 */
export type SourcesViewModel = SourcesScreenProps;

const SourcesContext = createContext<SourcesViewModel | null>(null);

export function SourcesProvider({
  value,
  children,
}: {
  value: SourcesViewModel;
  children: ReactNode;
}) {
  return <SourcesContext.Provider value={value}>{children}</SourcesContext.Provider>;
}

/** The sources view-model. Throws if used outside a {@link SourcesProvider}. */
export function useSourcesViewModel(): SourcesViewModel {
  const value = useContext(SourcesContext);
  if (value === null) {
    throw new Error("useSourcesViewModel must be used within a SourcesProvider");
  }
  return value;
}
