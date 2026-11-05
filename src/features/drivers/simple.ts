import type { Tone } from "@/components/common/StatusBadge";
import { type Translator, translatorFor } from "@/i18n";
import type { DeviceHealth } from "@/types/generated/DeviceHealth";
import type { DeviceRecord } from "@/types/generated/DeviceRecord";
import type { DriverSummary } from "@/types/generated/DriverSummary";
import type { UpdateStatus } from "@/types/generated/UpdateStatus";

/**
 * Plain-language helpers for the "Semplice" view of Drivers. Pure, unit-tested.
 *
 * Suggested actions are deliberately limited to safe steps the user takes in Windows:
 * this app never installs drivers, and the copy never suggests registry edits, firmware
 * settings, turning off parts of Windows or buying hardware.
 *
 * Texts live in the "drivers" namespace. Every function takes an optional translator:
 * components pass their `useT("drivers")`; other callers get the current language.
 */

type T = Translator<"drivers">;
const drivers = (): T => translatorFor("drivers");

/** Safe steps the user can take, referenced by id so the text follows the language. */
export type SimpleAction = "restart" | "reconnect" | "update" | "wait" | "reenable" | "noneIfUnused";

/** Text of a suggested action in the current language. */
export function actionText(action: SimpleAction, t: T = drivers()): string {
  return t(`action_${action}`);
}

export interface ProblemExplanation {
  /** What Windows is saying, in one short sentence. */
  title: string;
  /** What it means for the user. */
  meaning: string;
  /** Safe steps, in the order to try them. */
  actions: string[];
}

/**
 * Windows Device Manager problem codes (the numbers shown as "Codice N") with a dedicated
 * explanation, and the safe steps for each. Titles and meanings are `problem<code>_*` keys.
 */
const PROBLEM_ACTIONS = {
  1: ["update", "restart"],
  3: ["restart", "update"],
  10: ["restart", "reconnect", "update"],
  12: ["restart", "reconnect", "update"],
  14: ["restart"],
  18: ["update", "restart"],
  19: ["update", "restart", "reconnect"],
  21: ["wait", "restart"],
  22: ["noneIfUnused", "reenable"],
  24: ["reconnect", "update", "restart"],
  28: ["update", "restart"],
  31: ["restart", "update"],
  32: ["update", "restart"],
  37: ["restart", "update"],
  39: ["update", "restart"],
  41: ["reconnect", "update", "restart"],
  43: ["reconnect", "restart", "update"],
  45: ["noneIfUnused", "reconnect"],
  47: ["reconnect", "restart"],
  48: ["update", "restart"],
  52: ["update", "restart"],
  54: ["restart"],
} as const satisfies Record<number, readonly SimpleAction[]>;

type ProblemCode = keyof typeof PROBLEM_ACTIONS;

const GENERIC_ACTIONS: readonly SimpleAction[] = ["restart", "reconnect", "update"];

/** Codes with a dedicated explanation (exported for tests). */
export const KNOWN_PROBLEM_CODES: readonly number[] = Object.keys(PROBLEM_ACTIONS).map(Number);

const isKnownCode = (code: number): code is ProblemCode => Object.hasOwn(PROBLEM_ACTIONS, code);

interface ExplanationTable {
  codes: Map<ProblemCode, ProblemExplanation>;
  generic: ProblemExplanation;
}

/**
 * Explanations built once per translator (i.e. per language), so the same code always
 * returns the same object within a language and texts follow a language change.
 */
const tables = new WeakMap<T, ExplanationTable>();

function table(t: T): ExplanationTable {
  let tbl = tables.get(t);
  if (!tbl) {
    tbl = {
      codes: new Map(),
      generic: {
        title: t("problemGeneric_title"),
        meaning: t("problemGeneric_meaning"),
        actions: GENERIC_ACTIONS.map((a) => actionText(a, t)),
      },
    };
    tables.set(t, tbl);
  }
  return tbl;
}

function explanationFor(code: ProblemCode, t: T): ProblemExplanation {
  const tbl = table(t);
  let e = tbl.codes.get(code);
  if (!e) {
    e = {
      title: t(`problem${code}_title`),
      meaning: t(`problem${code}_meaning`),
      actions: PROBLEM_ACTIONS[code].map((a) => actionText(a, t)),
    };
    tbl.codes.set(code, e);
  }
  return e;
}

/**
 * Name to show in the "Semplice" view. Devices without a driver often have no friendly
 * name and Windows shows their hardware id (e.g. ACPI\AMDI0204\2&DABA3FF&0) instead; that id
 * means nothing to a non-technical user, so it is replaced with Device Manager's wording
 * and stays visible under "Mostra dettagli tecnici".
 */
export function simpleDeviceName(d: Pick<DeviceRecord, "name" | "instanceId">, t: T = drivers()): string {
  const n = d.name.trim();
  const looksLikeId =
    n === "" || n.toLowerCase() === d.instanceId.trim().toLowerCase() || (n.includes("\\") && !n.includes(" "));
  return looksLikeId ? t("unknownDevice") : n;
}

/** Plain explanation of a Windows problem code; unknown or missing codes get a generic one. */
export function problemCodeExplanation(code: number | null, t: T = drivers()): ProblemExplanation {
  return code !== null && isKnownCode(code) ? explanationFor(code, t) : table(t).generic;
}

/**
 * Explanation for a device that needs attention, or null when Windows reports nothing.
 * A device without a driver and without a code is explained as "Manca il driver".
 */
export function deviceProblemExplanation(d: DeviceRecord, t: T = drivers()): ProblemExplanation | null {
  if (d.problemCode !== null && d.problemCode !== 0) return problemCodeExplanation(d.problemCode, t);
  switch (d.health) {
    case "working":
      return null;
    case "driverMissing":
      return explanationFor(28, t);
    case "disabled":
      return explanationFor(22, t);
    case "problemDetected":
      return table(t).generic;
  }
}

export interface Labelled {
  label: string;
  tone: Tone;
}

const HEALTH_TONES: Record<DeviceHealth, Tone> = {
  working: "ok",
  problemDetected: "bad",
  driverMissing: "bad",
  disabled: "neutral",
};

/** Device state in everyday words (same tones as `healthLabel`). */
export function simpleHealthLabel(health: DeviceHealth, t: T = drivers()): Labelled {
  return { label: t(`simpleHealth_${health}`), tone: HEALTH_TONES[health] };
}

const UPDATE_TONES: Record<UpdateStatus["kind"], Tone> = {
  updateAvailable: "warn",
  verified: "ok",
  notVerifiable: "neutral",
  notChecked: "neutral",
};

/** Update state in everyday words (same tones as `updateLabel`). */
export function simpleUpdateLabel(update: UpdateStatus, t: T = drivers()): Labelled {
  return { label: t(`simpleUpdate_${update.kind}`), tone: UPDATE_TONES[update.kind] };
}

/** Plain summary of the inventory. "No problems" never claims the drivers are up to date. */
export function simpleDriverSummary(s: DriverSummary, t: T = drivers()): string[] {
  const problems = s.problems + s.driverMissing;
  const lines: string[] = [];
  if (s.total === 0) return [t("summaryEmpty")];
  if (problems === 0) {
    lines.push(t.plural("summaryNoProblems", s.total));
    lines.push(t("summaryNotUpToDate"));
  } else {
    lines.push(t.plural("summaryProblems", problems, { total: s.total }));
  }
  if (s.updatesAvailable > 0) lines.push(t.plural("summaryUpdates", s.updatesAvailable));
  if (s.disabled > 0) lines.push(t.plural("summaryDisabled", s.disabled));
  return lines;
}

/** Reminder shown next to every suggested action list. */
export function simpleNoInstallNote(t: T = drivers()): string {
  return t("noInstallNote");
}
