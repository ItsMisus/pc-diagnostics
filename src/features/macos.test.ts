import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { setPlatform } from "@/lib/platform";
import type { FixPlan } from "@/types/generated/FixPlan";
import type { FixStep } from "@/types/generated/FixStep";
import type { RemediationAction } from "@/types/generated/RemediationAction";
import { canExecuteStep, type FixPhase } from "./assistant/fixFlow";
import { actionRunsOn, describeAction } from "./assistant/model";
import { defaultVolume } from "./benchmarks/model";
import { volume as benchVolume } from "./benchmarks/testing/fixtures";
import { coverageSummary, platformCoverage } from "./diagnostics/model";
import { simpleSummary } from "./diagnostics/simple";
import { coverage } from "./diagnostics/testing/fixtures";
import { gpuVendorLabel } from "./monitoring/labels";
import { hasUnifiedMemory, isSystemMount, systemVolume } from "./monitoring/selection";
import { gpuInfo, volume } from "./monitoring/testing/fixtures";

/**
 * macOS data shapes and gating, with values as a Mac reports them: POSIX mount points, an
 * Apple silicon GPU without dedicated memory, no driver inventory.
 */

afterEach(() => {
  setPlatform("windows");
  setLang("it");
});

const GIB = 1024 ** 3;

describe("volumes as mount points", () => {
  it("treats / as the system volume and /Volumes/* as others", () => {
    expect(isSystemMount("/")).toBe(true);
    expect(isSystemMount(" / ")).toBe(true);
    expect(isSystemMount("/Volumes/Backup")).toBe(false);
    expect(isSystemMount("/System/Volumes/Data")).toBe(false);
    // Windows spellings still work.
    expect(isSystemMount("C:\\")).toBe(true);
  });

  it("prefers / even when an external volume is larger", () => {
    const vols = [volume("/Volumes/Archivio", 4000 * GIB, true), volume("/Volumes/Dati", 2000 * GIB), volume("/", 460 * GIB)];
    expect(systemVolume(vols)?.mountPoint).toBe("/");
    expect(defaultVolume([benchVolume("/Volumes/USB", { removable: true }), benchVolume("/")])).toBe("/");
    // Without /, the first fixed volume, never a removable one first.
    expect(defaultVolume([benchVolume("/Volumes/USB", { removable: true }), benchVolume("/Volumes/Dati")])).toBe("/Volumes/Dati");
  });
});

describe("Apple silicon GPU", () => {
  const apple = { ...gpuInfo("gpu0"), name: "Apple M5", vendor: "other" as const, vendorId: 0 };

  it("has unified memory instead of a VRAM size", () => {
    expect(hasUnifiedMemory(apple)).toBe(true);
    expect(hasUnifiedMemory({ ...apple, name: "AMD Radeon Pro 5500M", vendorId: 0x1002 })).toBe(false);
    // A reported dedicated size always wins over the inference.
    expect(hasUnifiedMemory({ ...apple, dedicatedVramBytes: 8 * GIB })).toBe(false);
    expect(hasUnifiedMemory({ ...apple, name: "GPU", vendorId: 0x106b })).toBe(true);
  });

  it("vendor label: Apple id named, a missing id not shown as a number", () => {
    expect(gpuVendorLabel("other", 0x106b)).toBe("Apple");
    expect(gpuVendorLabel("other", 0)).toBe("Non riportato");
    expect(gpuVendorLabel("other", 0x1234)).toBe("Altro (0x1234)");
  });
});

describe("driver coverage", () => {
  const items = [coverage("cpu", "evaluated"), coverage("drivers", "notEvaluated", "inventario dei dispositivi non raccolto")];

  it("is left out on macOS when the inventory does not exist there", () => {
    expect(platformCoverage(items, "macos").map((c) => c.component)).toEqual(["cpu"]);
    expect(coverageSummary(platformCoverage(items, "macos")).complete).toBe(true);
    expect(simpleSummary([], platformCoverage(items, "macos"))).not.toContain(
      "Una parte del PC è stata controllata solo in parte o non è stata controllata: vedi sotto.",
    );
  });

  it("is kept on Windows, and anywhere when it was evaluated", () => {
    expect(platformCoverage(items, "windows")).toHaveLength(2);
    expect(platformCoverage([coverage("drivers", "partial")], "macos")).toHaveLength(1);
  });
});

describe("assistant actions on macOS", () => {
  const ACTIONS: RemediationAction[] = [
    { kind: "openSettingsPage", page: "storageSense" },
    { kind: "openDeviceManager" },
    { kind: "openDiskCleanup", drive: "C:" },
    { kind: "runSystemFileCheck" },
    { kind: "emptyRecycleBin" },
    { kind: "cleanTempFiles", olderThanDays: 7 },
    { kind: "closeProgram", pid: 812, name: "Safari" },
  ];
  const WINDOWS_TOOLS = new Set(["openSettingsPage", "openDeviceManager", "openDiskCleanup", "runSystemFileCheck"]);

  it("Windows tools never run on macOS; the other actions may (the backend validates them)", () => {
    for (const a of ACTIONS) {
      expect(actionRunsOn(a, "windows"), a.kind).toBe(true);
      expect(actionRunsOn(a, "macos"), a.kind).toBe(!WINDOWS_TOOLS.has(a.kind));
    }
  });

  it("descriptions say so on macOS, and keep the Windows wording on Windows", () => {
    setPlatform("macos");
    for (const a of ACTIONS.filter((x) => WINDOWS_TOOLS.has(x.kind))) {
      expect(describeAction(a), a.kind).toContain("esiste solo su Windows");
    }
    expect(describeAction({ kind: "emptyRecycleBin" })).toBe("Svuoterà il Cestino. Non si può annullare.");
    expect(describeAction({ kind: "closeProgram", pid: 1, name: "Safari" })).toBe(
      "Chiederà a «Safari» di chiudersi. Salva prima il tuo lavoro.",
    );
    setLang("en");
    expect(describeAction({ kind: "emptyRecycleBin" })).toBe("It will empty the Trash. This can't be undone.");
    setPlatform("windows");
    expect(describeAction({ kind: "emptyRecycleBin" })).toBe(
      "It will empty the Recycle Bin. Windows will ask you to confirm. This can't be undone.",
    );
  });

  it("a Windows-only step is never executable on macOS", () => {
    const step = (id: string, action: RemediationAction): FixStep => ({
      id,
      title: id,
      explanation: "",
      risk: "low",
      requiresAdmin: false,
      reversible: true,
      action,
      manualInstructions: null,
      validationNote: null,
    });
    const plan: FixPlan = {
      sessionId: "s",
      ruleId: "storage.low_space",
      model: "m",
      servedBy: "m",
      summary: "",
      confidence: "medium",
      warnings: [],
      usage: { inputTokens: 1, outputTokens: 1 },
      createdAt: 0,
      steps: [step("cleanup", { kind: "openDiskCleanup", drive: "C:" }), step("temp", { kind: "cleanTempFiles", olderThanDays: 7 })],
    };
    const phase: FixPhase = { kind: "plan", plan, steps: {}, manualDone: {} };
    expect(canExecuteStep(phase, "cleanup", "windows")).toBe(true);
    expect(canExecuteStep(phase, "cleanup", "macos")).toBe(false);
    expect(canExecuteStep(phase, "temp", "macos")).toBe(true);
  });
});
