import type { ReactNode } from "react";
import { Switch } from "@/components/common/Switch";

/** Vertical rhythm shared by every row of a Settings card: hairline between rows, none at the edges. */
export const ROW_FRAME = "border-t border-line py-3.5 first:border-t-0 first:pt-0 last:pb-0";

/** Row label and hint. Hints use the muted tone (not the caption tone) so they stay readable. */
export const ROW_LABEL = "text-[13px] leading-5 font-medium text-fg";
export const ROW_HINT = "mt-0.5 max-w-lg text-xs leading-[18px] text-fg-muted";

/**
 * Label + hint on the left, control on the right. Shared by the Settings cards. On narrow
 * widths the control wraps under the label and stays right-aligned.
 */
export function Row({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className={`flex flex-wrap items-center gap-x-6 gap-y-2.5 ${ROW_FRAME}`}>
      <div className="min-w-[min(100%,14rem)] flex-1">
        <div className={ROW_LABEL}>{label}</div>
        {hint && <div className={ROW_HINT}>{hint}</div>}
      </div>
      <div className="ml-auto flex shrink-0 items-center">{children}</div>
    </div>
  );
}

/** Kept for existing callers; the shared, animated switch lives in `components/common/Switch`. */
export function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return <Switch label={label} checked={checked} onChange={onChange} />;
}
