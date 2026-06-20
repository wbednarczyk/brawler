import { createContext, useContext, type ReactNode } from "react";

/**
 * Factory for a per-screen view-model context (Architecture v2 / ADR 0050).
 *
 * Each screen's view-model is assembled once in `AppStateRoot` and provided
 * through one of these contexts, so the screen reads it from context instead of
 * receiving its whole bundle by prop-drilling. Returns a `[Provider, useViewModel]`
 * pair; the hook throws if used outside its provider (a wiring bug).
 */
export function createScreenContext<T>(name: string): [
  (props: { value: T; children: ReactNode }) => ReactNode,
  () => T,
] {
  const Context = createContext<T | null>(null);

  function Provider({ value, children }: { value: T; children: ReactNode }) {
    return <Context.Provider value={value}>{children}</Context.Provider>;
  }

  function useViewModel(): T {
    const value = useContext(Context);
    if (value === null) {
      throw new Error(`${name} view-model used outside its provider`);
    }
    return value;
  }

  return [Provider, useViewModel];
}
