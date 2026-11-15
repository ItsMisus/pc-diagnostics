import { useEffect, useState } from "react";
import type { CoolingSystemKind } from "@/types/generated/CoolingSystemKind";
import { coolingApi } from "./api";

/** Read once per session: how a computer is cooled does not change while the app runs. */
let cached: Promise<CoolingSystemKind> | undefined;

function load(): Promise<CoolingSystemKind> {
  cached ??= coolingApi.state().then(
    (s) => s.system.kind,
    (e: unknown) => {
      // A failed read is retried next time instead of being remembered.
      cached = undefined;
      throw e;
    },
  );
  return cached;
}

/**
 * How this computer is cooled (fans, passive, unknown), for pages other than Cooling (the
 * Overview's airflow). null while loading; "unknown" when it cannot be read.
 */
export function useCoolingKind(): CoolingSystemKind | null {
  const [kind, setKind] = useState<CoolingSystemKind | null>(null);
  useEffect(() => {
    let alive = true;
    load().then(
      (k) => alive && setKind(k),
      () => alive && setKind("unknown"),
    );
    return () => {
      alive = false;
    };
  }, []);
  return kind;
}

/** The airflow the Overview draws for a cooling kind: none when it is not known. */
export function airflowFor(kind: CoolingSystemKind | null): "active" | "passive" | null {
  return kind === "fans" ? "active" : kind === "passive" ? "passive" : null;
}
