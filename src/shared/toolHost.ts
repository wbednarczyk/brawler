import { createContext, useContext } from "react";

// A draft's dirty check + discard action, registered by the panel that owns
// the draft. `isDirty()` is read at unmount-time (tool close, another tool
// opening, company switch, navigating away, window close); `discard()` clears
// the draft when the user confirms losing it.
export type ToolHandle = {
  isDirty(): boolean;
  discard(): void;
};

type ToolHostContextValue = {
  register(handle: ToolHandle): () => void;
};

const noopHost: ToolHostContextValue = {
  register: () => () => {},
};

// The seam a draft-carrying panel (notebook editor, decision-journal composer,
// claims composer) registers into so the Spółka workshop's shared stay/discard
// gate (`src/screens/Spolka/ToolHost.tsx`, F3a S2, ADR 0107) can intercept
// every unmount. These panels are shared verbatim with hosts that provide no
// dirty gate of their own (e.g. the Companies screen), where registration is
// a no-op — kept in `src/shared` (not `src/screens/Spolka`) so panels outside
// the Spółka screen can import it without a shared→screens boundary violation.
export const ToolHostContext = createContext<ToolHostContextValue>(noopHost);

export function useToolHost(): ToolHostContextValue {
  return useContext(ToolHostContext);
}
