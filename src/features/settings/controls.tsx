import { type ReactNode, useId } from "react";
import { Surface, type SurfaceVariant } from "@/components/common/Panel";
import { Switch } from "@/components/common/Switch";

/** Vertical rhythm shared by every row of a Settings panel: hairline between rows, none at the edges. */
export const ROW_FRAME = "border-t border-line py-3.5 first:border-t-0 first:pt-0 last:pb-0";

/** Row label and hint. Hints use the muted tone (not the caption tone) so they stay readable. */
export const ROW_LABEL = "text-sm leading-5 font-medium text-fg";
export const ROW_HINT = "mt-0.5 max-w-lg text-xs leading-[18px] text-fg-muted";

/**
 * One setting: label + one short hint on the left, the control on the right. On narrow widths
 * the control wraps under the label and stays right-aligned. `hintId` lets the control point
 * at the hint (`aria-describedby`).
 */
export function Row({
  label,
  hint,
  hintId,
  children,
}: {
  label: string;
  hint?: ReactNode;
  hintId?: string;
  children: ReactNode;
}) {
  return (
    <div className={`flex flex-wrap items-center gap-x-6 gap-y-2.5 ${ROW_FRAME}`}>
      <div className="min-w-[min(100%,14rem)] flex-1">
        <div className={ROW_LABEL}>{label}</div>
        {hint && (
          <div id={hintId} className={ROW_HINT}>
            {hint}
          </div>
        )}
      </div>
      <div className="ml-auto flex max-w-full shrink-0 items-center">{children}</div>
    </div>
  );
}

/** Kept for existing callers; the shared, animated switch lives in `components/common/Switch`. */
export function Toggle({
  label,
  checked,
  onChange,
  describedBy,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  describedBy?: string;
}) {
  return <Switch label={label} checked={checked} onChange={onChange} {...(describedBy ? { describedBy } : {})} />;
}

/**
 * A titled group of the Settings page (Aspetto, Dispositivo, ...): the heading sits above
 * its panels, as in a grouped list. Panels inside are 16 px apart; groups 32 px.
 */
export function SettingsGroup({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="flex min-w-0 flex-col gap-2.5">
      <h2 id={id} className="px-1 font-display text-md font-semibold text-fg">
        {title}
      </h2>
      <div className="flex min-w-0 flex-col gap-4">{children}</div>
    </section>
  );
}

/** A solid panel of rows. */
export function RowsPanel({ children, variant = "secondary" }: { children: ReactNode; variant?: SurfaceVariant }) {
  return (
    <Surface variant={variant} className="min-w-0">
      {children}
    </Surface>
  );
}
