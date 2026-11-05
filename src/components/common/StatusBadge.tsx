export type Tone = "ok" | "warn" | "bad" | "neutral";

const STYLE: Record<Tone, { dot: string; chip: string }> = {
  ok: { dot: "bg-ok", chip: "border-ok/25 bg-ok-soft" },
  warn: { dot: "bg-warn", chip: "border-warn/25 bg-warn-soft" },
  bad: { dot: "bg-bad", chip: "border-bad/25 bg-bad-soft" },
  neutral: { dot: "bg-fg-subtle", chip: "border-line bg-surface-2/60" },
};

/** Status label. Color is never the only signal: the text always states the status. */
export function StatusBadge({ tone, label }: { tone: Tone; label: string }) {
  const s = STYLE[tone];
  return (
    <span
      className={`inline-flex h-[22px] shrink-0 items-center gap-1.5 rounded-full border px-2 text-2xs font-medium whitespace-nowrap text-fg-muted ${s.chip}`}
    >
      <span className={`size-1.5 rounded-full ${s.dot}`} aria-hidden="true" />
      {label}
    </span>
  );
}
