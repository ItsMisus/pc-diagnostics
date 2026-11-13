import { StatusBadge } from "@/components/common/StatusBadge";
import { Tooltip } from "@/components/common/Tooltip";
import { useT } from "@/i18n";
import type { Support } from "@/types/generated/Support";

/**
 * Whether a control works on this computer, in words: "Disponibile", "Sola lettura" or "Non
 * supportato", with the backend's reason as a tooltip. Every control center marks each
 * control with it, so nothing that cannot act looks like it can.
 */
export function SupportBadge({ support, size = "sm" }: { support: Support; size?: "sm" | "md" }) {
  const t = useT("common");
  const badge = (
    <StatusBadge
      size={size}
      glyph
      tone={support.state === "supported" ? "ok" : support.state === "readOnly" ? "neutral" : "warn"}
      label={t(support.state === "supported" ? "supportSupported" : support.state === "readOnly" ? "supportReadOnly" : "supportUnsupported")}
    />
  );
  return support.reason ? (
    <Tooltip content={support.reason} multiline>
      <span tabIndex={0} className="rounded-full">
        {badge}
      </span>
    </Tooltip>
  ) : (
    badge
  );
}

/** The reason under a control that cannot act, as one muted line (null when supported). */
export function SupportReason({ support }: { support: Support }) {
  if (support.state === "supported" || !support.reason) return null;
  return <p className="mt-1 text-xs text-fg-muted">{support.reason}</p>;
}
