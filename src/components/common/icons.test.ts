import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ICON_NAMES, ICON_PATHS, isIconName } from "./iconPaths";

// Type-checked by tsconfig.node.json (Node types), not by the app's tsconfig.
const SRC = fileURLToPath(new URL("../../", import.meta.url));

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) return entry === "generated" ? [] : sources(path);
    return /\.tsx?$/.test(entry) && !/\.test\.tsx?$/.test(entry) ? [path] : [];
  });
}

/** Icon names written as literals in the app: `<Icon name="x"`, `icon="x"`, `icon: "x"`. */
function usedIconNames(): Map<string, string> {
  const used = new Map<string, string>();
  for (const file of sources(SRC)) {
    const text = readFileSync(file, "utf8");
    const patterns = [/<Icon\b[^>]*?\bname="([A-Za-z]+)"/g, /\bicon=\{?"([A-Za-z]+)"/g, /\bicon:\s*"([A-Za-z]+)"/g];
    for (const re of patterns) {
      for (const m of text.matchAll(re)) used.set(m[1]!, file.slice(SRC.length));
    }
  }
  return used;
}

describe("icon family", () => {
  it("draws every icon as a valid stroke path", () => {
    for (const name of ICON_NAMES) {
      const d = ICON_PATHS[name];
      expect(d.length, name).toBeGreaterThan(0);
      expect(d, name).toMatch(/^M/);
      // Path commands and numbers only (no NaN from a helper, no stray characters).
      expect(d, name).toMatch(/^[MmLlHhVvCcSsQqTtAaZz0-9 .,-]+$/);
      expect(d, name).not.toMatch(/NaN|Infinity/);
    }
  });

  it("resolves every icon name used in the app", () => {
    const used = usedIconNames();
    expect(used.size).toBeGreaterThan(10);
    for (const [name, file] of used) {
      expect(isIconName(name), `"${name}" in ${file} is not in iconPaths.ts`).toBe(true);
    }
  });

  it("keeps the navigation and component glyphs the shell and pages rely on", () => {
    for (const name of ["overview", "hardware", "benchmarks", "processes", "drivers", "diagnostics", "history", "settings"]) {
      expect(isIconName(name), name).toBe(true);
    }
    for (const name of ["cpu", "gpu", "memory", "storage", "network", "temperature", "fan", "power", "desktop", "laptop", "miniPc"]) {
      expect(isIconName(name), name).toBe(true);
    }
  });

  it("rejects unknown names", () => {
    expect(isIconName("toString")).toBe(false);
    expect(isIconName("nope")).toBe(false);
  });
});
