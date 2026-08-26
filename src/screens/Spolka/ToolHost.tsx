import { useCallback, useRef, useState } from "react";
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
  /** Runs `next` immediately when the active tool is clean; otherwise opens
   * the stay/discard dialog and runs it only after a confirmed discard. Every
   * cross-screen navigation (AppShell `setActiveSection`, window close) is
   * wrapped in this — see `docs/plans/frontend-v2-f3a.md` § S2. */
  guardNavigation: (next: () => void) => void;
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
  const handleRef = useRef<ToolHandle | null>(null);

  const register = useCallback((handle: ToolHandle) => {
    handleRef.current = handle;
    return () => {
      if (handleRef.current === handle) handleRef.current = null;
    };
  }, []);

  const requestUnmount = useCallback((next: () => void) => {
    const handle = handleRef.current;
    if (!handle || !handle.isDirty()) {
      next();
      return;
    }
    setPendingNext(() => next);
  }, []);

  const openTool = useCallback(
    (companyId: string, nextTool: Tool) => {
      requestUnmount(() => {
        handleRef.current = null;
        setTool(nextTool);
        setToolCompanyId(companyId);
      });
    },
    [requestUnmount],
  );

  const closeTool = useCallback(() => {
    requestUnmount(() => {
      handleRef.current = null;
      setTool(null);
      setToolCompanyId(null);
    });
  }, [requestUnmount]);

  const guardNavigation = useCallback(
    (next: () => void) => {
      requestUnmount(() => {
        handleRef.current = null;
        setTool(null);
        setToolCompanyId(null);
        next();
      });
    },
    [requestUnmount],
  );

  const stay = useCallback(() => setPendingNext(null), []);

  const discardAndProceed = useCallback(() => {
    handleRef.current?.discard();
    setPendingNext((current) => {
      current?.();
      return null;
    });
  }, []);

  const isDirty = useCallback(() => Boolean(tool && handleRef.current?.isDirty()), [tool]);

  return {
    tool,
    toolCompanyId,
    openTool,
    closeTool,
    guardNavigation,
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
