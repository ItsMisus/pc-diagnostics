import type { ReactNode } from "react";
import { useLive } from "@/features/monitoring/liveStore";
import { WaitingForSample } from "@/features/monitoring/StatusViews";
import { useT } from "@/i18n";

/**
 * Wraps a live block: renders it only once a sample exists. When the subscription has
 * failed the page shows the reason once (header area); here only a short note. Until the
 * first sample arrives, `placeholder` (a skeleton shaped like the block) stands in for it.
 */
export function LiveSlot({ children, placeholder }: { children: ReactNode; placeholder?: ReactNode }) {
  const t = useT("hardware");
  const failed = useLive((s) => s.status === "unsupported" || s.status === "permission" || s.status === "error");
  const hasSample = useLive((s) => s.latest !== null);
  if (failed) return <p className="text-sm text-fg-subtle">{t("liveUnavailable")}</p>;
  if (!hasSample) return <WaitingForSample>{placeholder}</WaitingForSample>;
  return children;
}

/** Section label inside a hardware card. `level` keeps the outline right under a GPU name (h3). */
export function SubHeading({ children, level = 3 }: { children: ReactNode; level?: 3 | 4 }) {
  const Tag = level === 4 ? "h4" : "h3";
  return <Tag className="eyebrow mb-2.5">{children}</Tag>;
}
