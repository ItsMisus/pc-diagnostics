import { Icon, type IconName } from "./Icon";

export type Tone = "ok" | "warn" | "bad" | "neutral";

const STYLE: Record<Tone, { dot: string; chip: string; icon: IconName; iconClass: string }> = {
  ok: { dot: "bg-ok", chip: "border-ok/25 bg-ok-soft", icon: "success", iconClass: "text-ok" },
  warn: { dot: "bg-warn", chip: "border-warn/30 bg-warn-soft", icon: "warning", iconClass: "text-warn" },
  bad: { dot: "bg-bad", chip: "border-bad/30 bg-bad-soft", icon: "error", iconClass: "text-bad" },
  neutral: { dot: "bg-fg-subtle", chip: "border-line bg-control", icon: "info", iconClass: "text-fg-subtle" },
};

/**
 * Status label: a tinted capsule with a dot (or a status glyph) and the status in words.
 * Color is never the only signal: the text always states the status, and `glyph` adds a
 * shape cue (check / triangle / cross) where several statuses sit side by side.
 */
export function StatusBadge({
  tone,
  label,
  glyph = false,
  size = "md",
}: {
  tone: Tone;
  label: string;
  /** Shape cue instead of the dot. */
  glyph?: boolean;
  size?: "sm" | "md";
}) {
  const s = STYLE[tone];
  return (
    <span
      className={`inline-flex shrink-0 items-center gap-1.5 rounded-full border font-medium whitespace-nowrap text-fg-muted ${s.chip} ${
        size === "sm" ? "h-5 px-1.5 text-2xs" : "h-[22px] px-2 text-2xs"
      }`}
    >
      {glyph ? <Icon name={s.icon} size={12} strokeWidth={1.75} className={s.iconClass} /> : <span className={`size-1.5 rounded-full ${s.dot}`} aria-hidden="true" />}
      {label}
    </span>
  );
}
