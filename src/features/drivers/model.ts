import type { Tone } from "@/components/common/StatusBadge";
import type { DeviceCategory } from "@/types/generated/DeviceCategory";
import type { DeviceHealth } from "@/types/generated/DeviceHealth";
import type { DeviceRecord } from "@/types/generated/DeviceRecord";
import type { DriverSummary } from "@/types/generated/DriverSummary";
import type { UpdateSource } from "@/types/generated/UpdateSource";
import type { UpdateStatus } from "@/types/generated/UpdateStatus";
import { localeTag, type Translator, translatorFor } from "@/i18n";

/**
 * Labels follow the current language. Every function takes an optional translator: components
 * pass their `useT("drivers")` (so memoized results recompute on a language change); other
 * callers get the current language.
 */
type T = Translator<"drivers">;
const drivers = (): T => translatorFor("drivers");

export interface Labelled {
  label: string;
  tone: Tone;
}

/** OS-reported device state. "Funzionante" says nothing about the driver being current. */
const HEALTH_TONES: Record<DeviceHealth, Tone> = {
  working: "ok",
  problemDetected: "bad",
  driverMissing: "bad",
  disabled: "neutral",
};

export function healthLabel(health: DeviceHealth, t: T = drivers()): Labelled {
  return { label: t(`health_${health}`), tone: HEALTH_TONES[health] };
}

const UPDATE_TONES: Record<UpdateStatus["kind"], Tone> = {
  updateAvailable: "warn",
  verified: "ok",
  notVerifiable: "neutral",
  notChecked: "neutral",
};

export function updateLabel(update: UpdateStatus, t: T = drivers()): Labelled {
  return { label: t(`update_${update.kind}`), tone: UPDATE_TONES[update.kind] };
}

/** Product names: identical in every language. */
const SOURCE_LABELS: Record<UpdateSource, string> = { windowsUpdate: "Windows Update" };

export function updateSourceLabel(source: UpdateSource): string {
  return SOURCE_LABELS[source];
}

/** Display order of the groups: the components users ask about most come first. */
export const CATEGORY_ORDER: readonly DeviceCategory[] = [
  "gpu",
  "display",
  "chipset",
  "storageController",
  "disk",
  "ethernet",
  "wifi",
  "bluetooth",
  "audio",
  "usb",
  "input",
  "other",
];

export function categoryLabel(category: DeviceCategory, t: T = drivers()): string {
  return t(`category_${category}`);
}

export type DeviceFilter = "all" | "problems" | "updates" | "notVerifiable";

export const isProblem = (d: DeviceRecord) => d.health === "problemDetected" || d.health === "driverMissing";

const FILTER_TESTS: Record<DeviceFilter, (d: DeviceRecord) => boolean> = {
  all: () => true,
  problems: isProblem,
  updates: (d) => d.update.kind === "updateAvailable",
  notVerifiable: (d) => d.update.kind === "notVerifiable",
};

export function filterLabel(filter: DeviceFilter, t: T = drivers()): string {
  return t(`filter_${filter}`);
}

export function matchesFilter(device: DeviceRecord, filter: DeviceFilter): boolean {
  return FILTER_TESTS[filter](device);
}

/** Chip counts, computed on the full list (independent of search). */
export function filterCounts(devices: readonly DeviceRecord[]): Record<DeviceFilter, number> {
  const counts: Record<DeviceFilter, number> = { all: 0, problems: 0, updates: 0, notVerifiable: 0 };
  for (const d of devices) {
    for (const f of Object.keys(FILTER_TESTS) as DeviceFilter[]) if (FILTER_TESTS[f](d)) counts[f] += 1;
  }
  return counts;
}

export interface DeviceQuery {
  filter: DeviceFilter;
  /** null = every category. */
  category: DeviceCategory | null;
  search: string;
}

/** Case-insensitive name search combined with the chip and category filters. */
export function filterDevices(devices: readonly DeviceRecord[], q: DeviceQuery): DeviceRecord[] {
  const tag = localeTag();
  const needle = q.search.trim().toLocaleLowerCase(tag);
  return devices.filter(
    (d) =>
      matchesFilter(d, q.filter) &&
      (q.category === null || d.category === q.category) &&
      (!needle || d.name.toLocaleLowerCase(tag).includes(needle)),
  );
}

export interface DeviceGroup {
  category: DeviceCategory;
  label: string;
  devices: DeviceRecord[];
}

const collators = new Map<string, Intl.Collator>();

/** Name collator of the current language (cached per locale). */
function collator(): Intl.Collator {
  const tag = localeTag();
  let c = collators.get(tag);
  if (!c) {
    c = new Intl.Collator(tag, { sensitivity: "base", numeric: true });
    collators.set(tag, c);
  }
  return c;
}

/**
 * Groups by category in `CATEGORY_ORDER`, omitting empty groups. Within a group, devices
 * that need attention (problems, then available updates) come first, then by name.
 */
export function groupDevices(devices: readonly DeviceRecord[], t: T = drivers()): DeviceGroup[] {
  const names = collator();
  const rank = (d: DeviceRecord) => (isProblem(d) ? 0 : d.update.kind === "updateAvailable" ? 1 : 2);
  const byCategory = new Map<DeviceCategory, DeviceRecord[]>();
  for (const d of devices) {
    const list = byCategory.get(d.category);
    if (list) list.push(d);
    else byCategory.set(d.category, [d]);
  }
  return CATEGORY_ORDER.flatMap((category) => {
    const list = byCategory.get(category);
    if (!list) return [];
    list.sort((a, b) => rank(a) - rank(b) || names.compare(a.name, b.name));
    return [{ category, label: categoryLabel(category, t), devices: list }];
  });
}

export interface SummaryCard {
  id: "total" | "working" | "problems" | "updates" | "unverified";
  label: string;
  value: number;
  /** Status tone only when the number carries meaning (problems > 0, updates > 0). */
  tone: Tone;
  hint: string;
}

export function summaryCards(s: DriverSummary, t: T = drivers()): SummaryCard[] {
  const problems = s.problems + s.driverMissing;
  const unverified = s.notVerifiable + s.notChecked;
  return [
    { id: "total", label: t("card_total"), value: s.total, tone: "neutral", hint: t("cardHint_total") },
    {
      id: "working",
      label: t("card_working"),
      value: s.working,
      tone: "neutral",
      hint: t("cardHint_working"),
    },
    {
      id: "problems",
      label: t("card_problems"),
      value: problems,
      tone: problems > 0 ? "bad" : "neutral",
      hint: t("cardHint_problems", { problems: s.problems, missing: s.driverMissing }),
    },
    {
      id: "updates",
      label: t("card_updates"),
      value: s.updatesAvailable,
      tone: s.updatesAvailable > 0 ? "warn" : "neutral",
      hint: t("cardHint_updates"),
    },
    {
      id: "unverified",
      label: t("card_unverified"),
      value: unverified,
      tone: "neutral",
      hint: t("cardHint_unverified", { notChecked: s.notChecked, notVerifiable: s.notVerifiable }),
    },
  ];
}
