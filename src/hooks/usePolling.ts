import { useCallback, useEffect, useState } from "react";
import { toAppError } from "@/services/ipc";
import type { AsyncState } from "./useAsync";

/**
 * Runs `fn` on mount and then every `intervalMs` while mounted. Calls never overlap (the
 * next one is scheduled after the previous settles). Polling stops on error until
 * `retry`; the timer is cleared on unmount. `fn` must be referentially stable.
 */
export function usePolling<T>(fn: () => Promise<T>, intervalMs: number) {
  const [state, setState] = useState<AsyncState<T>>({ status: "loading" });
  const [nonce, setNonce] = useState(0);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = () => {
      fn().then(
        (data) => {
          if (!active) return;
          setState({ status: "ready", data });
          timer = setTimeout(tick, intervalMs);
        },
        (e: unknown) => {
          if (active) setState({ status: "error", error: toAppError(e) });
        },
      );
    };
    setState({ status: "loading" });
    tick();
    return () => {
      active = false;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [fn, intervalMs, nonce]);

  const retry = useCallback(() => setNonce((n) => n + 1), []);
  return [state, retry] as const;
}
