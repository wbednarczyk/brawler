import { useCallback, useRef, useState, type MutableRefObject } from "react";
import { Button, Modal } from "../../ui";
import { focusScreenHeadingIfBody } from "../../shared/focus/focusScreenHeading";
import { useLocale } from "../../shared/locale";
import { ToolHostContext, type ToolHandle } from "../../shared/toolHost";
import type { Tool } from "./route";

export { useToolHost, type ToolHandle } from "../../shared/toolHost";

// The Spółka workshop's tool-host state (F3a S2, ADR 0107): ONE `activeTool`
// for the whole screen, gated by the SAME stay/discard dialog on every
// unmount path — closing the tool (✕), opening another tool, switching
// company, navigating away (Dziś/Inbox/global screens), and the
// window close request (`handleCloseRequested.ts`). `toolCompanyId` keys the
// open tool to the company it was opened for so a late `get_company_view`
// response, or a switch away and back, can never reopen/leak across
// companies (plan §11 "Późna odpowiedź").
// Explicit focus intent per transition (F3c S1, plan § Design 3) — exactly
// one owner acts on each: `ToolFrame` moves focus to the tool `h2` on
// "heading"; `SpolkaScreen` moves it to a bar entry on "entry"/"overview" and
// to the company picker on "company"; "none" moves nothing (a plain section
// switch through `guardNavigation`).
export type FocusIntent = "heading" | "entry" | "overview" | "company" | "none";

export type SpolkaToolHostApi = {
  tool: Tool | null;
  toolCompanyId: string | null;
  /** Increments only when a NON-NULL tool is committed — the "heading" focus
   * effect's dependency (`ToolFrame`), so a same-kind payload change (e.g. a
   * different `documentId`) still re-focuses the heading. */
  openSeq: number;
  /** Increments on EVERY commit (open or close), so a consumer can key an
   * effect off it even when `openSeq` does not move (e.g. closing a tool).
   * `closedKind` is the kind of the tool that was JUST closed (set only when
   * this commit closes one), for the "entry" intent's bar-entry lookup. */
  focus: { seq: number; intent: FocusIntent; closedKind: Tool["t"] | null };
  openTool: (companyId: string, tool: Tool) => void;
  /** Closes the open tool. `intent` defaults to "entry" (focus returns to the
   * closed tool's own bar entry — the ✕ button and the Escape-in-frame path);
   * the summary ticker / Overview tab / palette "Open overview" pass
   * "overview" instead (focus goes to the Overview entry). */
  closeTool: (intent?: FocusIntent) => void;
  /** Runs `next` immediately when NO registered draft handle is dirty;
   * otherwise opens the stay/discard dialog and runs it only after a
   * confirmed discard. Every cross-screen navigation (AppShell
   * `setActiveSection`, window close) is wrapped in this — see
   * `docs/plans/frontend-v2-f3a.md` § S2. A second `guardNavigation`/
   * `openTool`/`closeTool` call while the dialog is already open is IGNORED
   * (sol R1 finding 3) — Stay/Discard resolves the FIRST request; overwriting
   * `pendingNext` would silently drop it. `closeIntent` (default "none")
   * covers the guard's OWN close commit — a plain section switch moves no
   * focus; `useSpolkaNavigate.navigate` follows up with its own `commitTool`
   * call (the SAME tick, so this one's intent never actually renders). */
  guardNavigation: (next: () => void, closeIntent?: FocusIntent) => void;
  /** Low-level, UNGUARDED tool commit — clears every registered draft handle
   * and sets `tool`/`toolCompanyId` directly, skipping the dirty check. Only
   * safe from inside an ALREADY guarded transition (`guardNavigation`, or an
   * app-level atomic `navigate` built from it, sol R1 finding 3) — calling it
   * on its own would silently discard a dirty draft. Also records
   * `lastGuardedCompanyIdRef` so an independent sync effect can tell "this
   * `selectedCompanyId` change already passed the guard" from one that
   * didn't (`useSpolkaScreenWiring.tsx`). `focusIntent` defaults to
   * "heading" (every direct `openTool` caller wants it). */
  commitTool: (companyId: string | null, tool: Tool | null, focusIntent?: FocusIntent) => void;
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
  const [openSeq, setOpenSeq] = useState(0);
  const [focus, setFocus] = useState<{ seq: number; intent: FocusIntent; closedKind: Tool["t"] | null }>({
    seq: 0,
    intent: "none",
    closedKind: null,
  });
  // A keyed Set, not one overwriteable slot (sol R1 finding 1): every draft-
  // owning subform hosted under a tool (notebook/journal/claims composers,
  // sector/IR-URL fields, ownership retyping, …) registers its OWN handle —
  // dirty if ANY handle is dirty, discard clears ALL of them.
  const handlesRef = useRef<Set<ToolHandle>>(new Set());
  const lastGuardedCompanyIdRef = useRef<string | null>(null);
  // Mirrors `tool` synchronously (state updates are not visible until the
  // next render) so `commitTool` can read the tool it is REPLACING even when
  // two commits land in the same tick (`guardNavigation`'s own close,
  // immediately followed by `navigate`'s reopen).
  const toolRef = useRef<Tool | null>(null);

  const register = useCallback((handle: ToolHandle) => {
    handlesRef.current.add(handle);
    return () => {
      handlesRef.current.delete(handle);
    };
  }, []);

  const commitTool = useCallback(
    (companyId: string | null, nextTool: Tool | null, focusIntent: FocusIntent = "heading") => {
      const previousTool = toolRef.current;
      toolRef.current = nextTool;
      handlesRef.current.clear();
      lastGuardedCompanyIdRef.current = companyId;
      setTool(nextTool);
      setToolCompanyId(companyId);
      if (nextTool !== null) {
        setOpenSeq((seq) => seq + 1);
      }
      setFocus((current) => ({
        seq: current.seq + 1,
        intent: focusIntent,
        closedKind: nextTool === null ? (previousTool?.t ?? null) : null,
      }));
    },
    [],
  );

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

  const closeTool = useCallback(
    (intent: FocusIntent = "entry") => {
      requestUnmount(() => commitTool(null, null, intent));
    },
    [requestUnmount, commitTool],
  );

  const guardNavigation = useCallback(
    (next: () => void, closeIntent: FocusIntent = "none") => {
      requestUnmount(() => {
        commitTool(null, null, closeIntent);
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
    // The pending transition may leave Spółka entirely (a dirty palette hop
    // to another screen): its `none` intent then never runs, the Modal's
    // invoker (the Discard button) is gone, and focus would strand on
    // `<body>` — land on the new screen's heading instead (sol diff R2).
    requestAnimationFrame(() => {
      focusScreenHeadingIfBody();
    });
  }, []);

  const isDirty = useCallback(
    () => Array.from(handlesRef.current).some((handle) => handle.isDirty()),
    [],
  );

  return {
    tool,
    toolCompanyId,
    openSeq,
    focus,
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
// `Modal` — never `window.confirm` (lint-banned). Stay's initial focus goes
// through `Modal`'s `initialFocusRef` (F3c S1) — an `autoFocus` descendant
// would steal focus before `Modal` captures the real invoker.
export function ToolHostConfirmModal({ host }: { host: SpolkaToolHostApi }) {
  const { text } = useLocale();
  const stayRef = useRef<HTMLButtonElement>(null);
  return (
    <Modal
      open={host.confirming}
      onClose={host.stay}
      title={text("Unsaved changes in this tool")}
      ariaLabel={text("Unsaved changes in this tool")}
      initialFocusRef={stayRef}
      footer={
        <>
          <Button variant="secondary" onClick={host.stay} ref={stayRef}>
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
