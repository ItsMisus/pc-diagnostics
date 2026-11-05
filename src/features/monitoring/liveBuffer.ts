import type { LiveSample } from "@/types/generated/LiveSample";

/** UI-side history bound: about 5 minutes at the default 1 s interval. */
export const LIVE_CAPACITY = 300;

/**
 * Appends one live event. Returns `null` (buffer unchanged) for duplicates and
 * out-of-order samples (`seq <= last seq`), otherwise a new array of at most `capacity`.
 */
export function appendSample(
  buffer: readonly LiveSample[],
  sample: LiveSample,
  capacity: number,
): readonly LiveSample[] | null {
  const last = buffer[buffer.length - 1];
  if (last && sample.seq <= last.seq) return null;
  const keep = Math.max(0, capacity - 1);
  const next = buffer.length > keep ? buffer.slice(buffer.length - keep) : buffer.slice();
  next.push(sample);
  return next;
}

/**
 * Merges batches (seed history, events buffered while it loaded, existing samples) into
 * one array sorted by `seq`, without duplicates, keeping the newest `capacity` samples.
 */
export function mergeSamples(
  batches: readonly (readonly LiveSample[])[],
  capacity: number,
): readonly LiveSample[] {
  const bySeq = new Map<number, LiveSample>();
  for (const batch of batches) {
    for (const s of batch) if (!bySeq.has(s.seq)) bySeq.set(s.seq, s);
  }
  const merged = [...bySeq.values()].sort((a, b) => a.seq - b.seq);
  return capacity > 0 ? merged.slice(-capacity) : [];
}
