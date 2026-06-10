import { useCallback, useState } from "react";

type AsyncActionState = "idle" | "running" | "done" | "error";

export function useAsyncAction<TArgs extends unknown[]>(
  action: (...args: TArgs) => Promise<unknown>,
) {
  const [state, setState] = useState<AsyncActionState>("idle");
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(
    async (...args: TArgs) => {
      setState("running");
      setError(null);

      try {
        const result = await action(...args);
        setState("done");
        return result;
      } catch (caught) {
        const message = caught instanceof Error ? caught.message : String(caught);
        setError(message);
        setState("error");
        throw caught;
      }
    },
    [action],
  );

  return { error, run, state };
}
