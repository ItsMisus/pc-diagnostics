import { Fragment, type ReactNode } from "react";
import { Skeleton, SkeletonLines } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { useInventory } from "./inventoryStore";
import { useLive } from "./liveStore";

/**
 * Page-level state of the live subscription when it is not delivering data.
 * Renders nothing once live (individual cards then handle "waiting for first sample").
 */
export function LiveConnectionState({ compact = false }: { compact?: boolean }) {
  const status = useLive((s) => s.status);
  const error = useLive((s) => s.error);
  const retry = useLive((s) => s.retry);
  const t = useT("monitoring");

  if (status === "live") return null;
  if (status === "idle" || status === "connecting") {
    return <StateView compact={compact} kind="loading" title={t("liveConnecting")} />;
  }
  if (status === "unsupported") {
    return (
      <StateView
        compact={compact}
        kind="unsupported"
        title={t("liveUnsupported")}
        description={t("liveUnsupportedDesc")}
        {...(error ? { error } : {})}
      />
    );
  }
  return (
    <StateView
      compact={compact}
      kind={status}
      title={status === "permission" ? t("livePermission") : t("liveUnavailable")}
      {...(error ? { error } : {})}
      onRetry={retry}
    />
  );
}

/** True when cards should render live content (or a "waiting" state), not a failure. */
export function useLiveUsable(): boolean {
  return useLive((s) => s.status === "live");
}

/**
 * A first load in progress: a placeholder shaped like the content, with the status
 * announced to assistive technology (the skeleton itself is decorative).
 */
function Pending({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div role="status" aria-busy="true">
      <span className="sr-only">{label}</span>
      {children}
    </div>
  );
}

/**
 * The first live sample has not arrived yet. `children` is a skeleton shaped like the
 * block it stands in for (default: a few text lines). Only for the first sample: once a
 * value is on screen, later samples update it in place and never bring this back.
 */
export function WaitingForSample({ children }: { children?: ReactNode }) {
  const t = useT("monitoring");
  return <Pending label={t("waitingFirstSample")}>{children ?? <SkeletonLines />}</Pending>;
}

const LABEL_W = ["w-24", "w-32", "w-28", "w-20"];
const VALUE_W = ["w-16", "w-24", "w-20", "w-14"];

/** Placeholder with the geometry of a `DefinitionList` (same row height, gaps, columns). */
export function DefinitionSkeleton({ rows }: { rows: number }) {
  return (
    <span aria-hidden="true" className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5">
      {Array.from({ length: rows }, (_, i) => (
        <Fragment key={i}>
          <span className="flex h-5 items-center">
            <Skeleton className={`h-2.5 ${LABEL_W[i % LABEL_W.length]}`} />
          </span>
          <span className="flex h-5 items-center">
            <Skeleton className={`h-2.5 ${VALUE_W[i % VALUE_W.length]}`} />
          </span>
        </Fragment>
      ))}
    </span>
  );
}

/** Small header badge; text always states the status (color is secondary). */
export function LiveBadge() {
  const status = useLive((s) => s.status);
  const t = useT("monitoring");
  const tc = useT("common");
  switch (status) {
    case "live":
      return <StatusBadge tone="ok" label={t("badgeLive")} />;
    case "idle":
    case "connecting":
      return <StatusBadge tone="neutral" label={t("badgeConnecting")} />;
    case "unsupported":
      return <StatusBadge tone="neutral" label={t("liveUnsupported")} />;
    case "permission":
      return <StatusBadge tone="warn" label={tc("statePermission")} />;
    case "error":
      return <StatusBadge tone="bad" label={t("badgeStopped")} />;
  }
}

/**
 * Non-data states of the hardware inventory. Renders nothing when ready. With a
 * `placeholder`, the first load shows it (a skeleton of the content) instead of a spinner.
 */
export function InventoryState({ compact = false, placeholder }: { compact?: boolean; placeholder?: ReactNode }) {
  const status = useInventory((s) => s.status);
  const error = useInventory((s) => s.error);
  const reload = useInventory((s) => s.reload);
  const t = useT("monitoring");

  if (status === "ready") return null;
  if (status === "idle" || status === "loading" || !error) {
    return placeholder ? (
      <Pending label={t("detectingHardware")}>{placeholder}</Pending>
    ) : (
      <StateView compact={compact} kind="loading" title={t("detectingHardware")} />
    );
  }
  const kind = stateKindForError(error);
  return (
    <StateView
      compact={compact}
      kind={kind}
      error={error}
      {...(kind === "unsupported"
        ? {
            title: t("inventoryUnsupported"),
            description: t("inventoryUnsupportedDesc"),
          }
        : { onRetry: reload })}
    />
  );
}
