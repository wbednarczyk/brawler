import { useCallback, useRef, useState, type MutableRefObject } from "react";
import { Button, Modal } from "../../ui";
import { useLocale } from "../../shared/locale";
import { ToolHostContext, type ToolHandle } from "../../shared/toolHost";
import type { Tool } from "./route";

export { useToolHost, type ToolHandle } from "../../shared/toolHost";

// The Spółka workshop's tool-host state (F3a S2, ADR 0107): ONE `activeTool`
// for the whole screen, gated by the SAME stay/discard dialog on every
// unmount path — closing the tool (✕), opening another tool, switching
// company, navigating away (Dziś/Inbox/global screens/named views), and the
// window close request (`handleCloseRequested.ts`). `toolCompanyId` keys the
// open tool to the company it was opened for so a late `get_company_view`
// response, or a switch away and back, can never reopen/leak across
// companies (plan §11 "Późna odpowiedź").
export type SpolkaToolHostApi = {
  tool: Tool | null;
  toolCompanyId: string | null;
  openTool: (companyId: string, tool: Tool) => void;
  closeTool: () => void;
  /** Runs `next` immediately when NO registered draft handle is dirty;
   * otherwise opens the stay/discard dialog and runs it only after a
   * confirmed discard. Every cross-screen navigation (AppShell
   * `setActiveSection`, window close) is wrapped in this — see
   * `docs/plans/frontend-v2-f3a.md` § S2. A second `guardNavigation`/
   * `openTool`/`closeTool` call while the dialog is already open is IGNORED
   * (sol R1 finding 3) — Stay/Discard resolves the FIRST request; overwriting
   * `pendingNext` would silently drop it. */
  guardNavigation: (next: () => void) => void;
  /** Low-level, UNGUARDED tool commit — clears every registered draft handle
   * and sets `tool`/`toolCompanyId` directly, skipping the dirty check. Only
   * safe from inside an ALREADY guarded transition (`guardNavigation`, or an
   * app-level atomic `navigate` built from it, sol R1 finding 3) — calling it
   * on its own would silently discard a dirty draft. Also records
   * `lastGuardedCompanyIdRef` so an independent sync effect can tell "this
   * `selectedCompanyId` change already passed the guard" from one that
   * didn't (`useSpolkaScreenWiring.tsx`). */
  commitTool: (companyId: string | null, tool: Tool | null) => void;
  lastGuardedCompanyIdRef: MutableRefObject<string | null>;
  register: (handle: ToolHandle) => () => void;
  isDirty: () => boolean;
  confirming: boolean;
  stay: () => void;
  discardAndProceed: () => void;
};

export function useSpolkaToolHost(): SpolkaToolHostApi {
  const [tool, setTool] = useState<Tool | null>(null);
  const [toolCompanyId, setToolCompanyId] = useState<string | null>(null);
  const [pendingNext, setPendingNext] = useState<(() => void) | null>(null);
  // A keyed Set, not one overwriteable slot (sol R1 finding 1): every draft-
  // owning subform hosted under a tool (notebook/journal/claims composers,
  // sector/IR-URL fields, ownership retyping, …) registers its OWN handle —
  // dirty if ANY handle is dirty, discard clears ALL of them.
  const handlesRef = useRef<Set<ToolHandle>>(new Set());
  const lastGuardedCompanyIdRef = useRef<string | null>(null);

  const register = useCallback((handle: ToolHandle) => {
    handlesRef.current.add(handle);
    return () => {
      handlesRef.current.delete(handle);
    };
  }, []);

  const commitTool = useCallback((companyId: string | null, nextTool: Tool | null) => {
    handlesRef.current.clear();
    lastGuardedCompanyIdRef.current = companyId;
    setTool(nextTool);
    setToolCompanyId(companyId);
  }, []);

  const requestUnmount = useCallback((next: () => void) => {
    const dirty = Array.from(handlesRef.current).some((handle) => handle.isDirty());
    if (!dirty) {
      next();
      return;
    }
    setPendingNext((current) => (current !== null ? current : next));
  }, []);

  const openTool = useCallback(
    (companyId: string, nextTool: Tool) => {
      requestUnmount(() => commitTool(companyId, nextTool));
    },
    [requestUnmount, commitTool],
  );

  const closeTool = useCallback(() => {
    requestUnmount(() => commitTool(null, null));
  }, [requestUnmount, commitTool]);

  const guardNavigation = useCallback(
    (next: () => void) => {
      requestUnmount(() => {
        commitTool(null, null);
        next();
      });
    },
    [requestUnmount, commitTool],
  );

  const stay = useCallback(() => setPendingNext(null), []);

  const discardAndProceed = useCallback(() => {
    for (const handle of handlesRef.current) handle.discard();
    handlesRef.current.clear();
    setPendingNext((current) => {
      current?.();
      return null;
    });
  }, []);

  const isDirty = useCallback(
    () => Array.from(handlesRef.current).some((handle) => handle.isDirty()),
    [],
  );

  return {
    tool,
    toolCompanyId,
    openTool,
    closeTool,
    guardNavigation,
    commitTool,
    lastGuardedCompanyIdRef,
    register,
    isDirty,
    confirming: pendingNext !== null,
    stay,
    discardAndProceed,
  };
}

export function SpolkaToolHostProvider({
  host,
  children,
}: {
  host: SpolkaToolHostApi;
  children: React.ReactNode;
}) {
  return <ToolHostContext.Provider value={{ register: host.register }}>{children}</ToolHostContext.Provider>;
}

// The ONE stay/discard dialog for every unmount path (deliverable 2). Primitive
// `Modal` — never `window.confirm` (lint-banned).
export function ToolHostConfirmModal({ host }: { host: SpolkaToolHostApi }) {
  const { text } = useLocale();
  return (
    <Modal
      open={host.confirming}
      onClose={host.stay}
      title={text("Unsaved changes in this tool")}
      ariaLabel={text("Unsaved changes in this tool")}
      footer={
        <>
          <Button variant="secondary" onClick={host.stay} autoFocus>
            {text("Stay")}
          </Button>
          <Button variant="danger" onClick={host.discardAndProceed}>
            {text("Discard")}
          </Button>
        </>
      }
    >
      {text("This tool has a draft in progress. Stay to keep it, or discard it to continue.")}
    </Modal>
  );
}
