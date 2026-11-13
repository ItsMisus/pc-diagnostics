import type { Support } from "@/types/generated/Support";

/** Order of the "what you can control" lists: what works, then what is missing, then read-only. */
const SUPPORT_RANK: Record<Support["state"], number> = { supported: 0, unsupported: 1, readOnly: 2 };

/** A copy of `items` grouped by support (available → unsupported → read-only), stable within a group. */
export function bySupport<T>(items: readonly T[], support: (item: T) => Support): T[] {
  return [...items].sort((a, b) => SUPPORT_RANK[support(a).state] - SUPPORT_RANK[support(b).state]);
}
