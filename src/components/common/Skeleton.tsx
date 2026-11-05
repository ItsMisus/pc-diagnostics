/**
 * Discreet placeholder for content that is about to arrive (first sample, first
 * inventory). Purely decorative: the surrounding element carries the "loading" status.
 * The pulse is slow and low-contrast and stops entirely with reduced motion.
 */
export function Skeleton({ className = "" }: { className?: string }) {
  return <span aria-hidden="true" className={`block animate-skeleton rounded-md bg-surface-3 ${className}`} />;
}

/** A few text-like lines, for card bodies. */
export function SkeletonLines({ lines = 3, className = "" }: { lines?: number; className?: string }) {
  const widths = ["w-2/3", "w-5/6", "w-1/2", "w-3/4"];
  return (
    <span aria-hidden="true" className={`flex flex-col gap-2 ${className}`}>
      {Array.from({ length: lines }, (_, i) => (
        <Skeleton key={i} className={`h-3 ${widths[i % widths.length]}`} />
      ))}
    </span>
  );
}
