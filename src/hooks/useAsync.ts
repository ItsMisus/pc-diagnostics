import { useCallback, useEffect, useState } from "react";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";

export type AsyncState<T> =
  | { status: "loading" }
  | { status: "ready"; data: T }
  | { status: "error"; error: AppError };

/** Runs `fn` on mount (and on `reload`). Stale responses are discarded. */
export function useAsync<T>(fn: () => Promise<T>) {
  const [state, setState] = useState<AsyncState<T>>({ status: "loading" });
  const [nonce, setNonce] = useState(0);

  useEffect(() => {
    let active = true;
    setState({ status: "loading" });
    fn().then(
      (data) => active && setState({ status: "ready", data }),
      (e: unknown) => active && setState({ status: "error", error: toAppError(e) }),
    );
    return () => {
      active = false;
    };
    // `fn` must be stable (module-level command wrapper); only `reload` re-runs it.
  }, [nonce]);

  const reload = useCallback(() => setNonce((n) => n + 1), []);
  return [state, reload] as const;
}
