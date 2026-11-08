import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import type { Metric } from "@/types/generated/Metric";
import { stateKindForError } from "./errors";
import {
  errorTitle,
  formatBytes,
  formatBytesPerSec,
  formatCelsius,
  formatDateTime,
  formatDuration,
  formatElapsed,
  formatFlopsPerSec,
  formatIsoDate,
  formatMetric,
  formatMhz,
  formatMib,
  formatMts,
  formatPercent,
  formatPerSec,
  formatRpm,
  formatUnitValue,
  formatWatts,
  getByteUnits,
  niceCeil,
  setByteUnits,
  sourceLabel,
  unavailabilityText,
} from "./format";

describe("niceCeil", () => {
  it("rounds up to 1, 2, 2.5 or 5 times a power of ten", () => {
    expect(niceCeil(1)).toBe(1);
    expect(niceCeil(1.01)).toBe(2);
    expect(niceCeil(2.2)).toBe(2.5);
    expect(niceCeil(3)).toBe(5);
    expect(niceCeil(7)).toBe(10);
    expect(niceCeil(42)).toBe(50);
    expect(niceCeil(250)).toBe(250);
    expect(niceCeil(1001)).toBe(2000);
    expect(niceCeil(0.3)).toBe(0.5);
  });

  it("never returns less than the input", () => {
    for (const x of [0.07, 0.1, 0.123, 9.99, 99.5, 12_345, 1_048_576]) {
      expect(niceCeil(x)).toBeGreaterThanOrEqual(x);
    }
  });

  it("returns 0 for non-positive or invalid input", () => {
    expect(niceCeil(0)).toBe(0);
    expect(niceCeil(-5)).toBe(0);
    expect(niceCeil(Number.NaN)).toBe(0);
  });
});

describe("unit formatters", () => {
  it("formats throughput with binary units", () => {
    expect(formatBytesPerSec(0)).toBe("0 B/s");
    expect(formatBytesPerSec(1024 * 1024 * 3.5)).toBe("3.5 MB/s");
    expect(formatBytesPerSec(1024 * 1024 * 3.5, 1, "iec")).toBe("3.5 MiB/s");
    expect(formatBytesPerSec(-1)).toBe("—");
  });

  it("formats frequency, temperature, power, MT/s and rates", () => {
    expect(formatMhz(3599.6)).toBe("3600 MHz");
    expect(formatMhz(Number.NaN)).toBe("—");
    expect(formatCelsius(64.4)).toBe("64 °C");
    expect(formatWatts(12.34)).toBe("12.3 W");
    expect(formatMts(3200)).toBe("3200 MT/s");
    expect(formatMts(0)).toBe("—");
    expect(formatPerSec(3.25)).toBe("3.3/s");
    expect(formatPerSec(120.4)).toBe("120/s");
  });

  it("labels every source", () => {
    expect(sourceLabel("pdh")).toBe("PDH");
    expect(sourceLabel("nvml")).toBe("NVML");
  });
});

describe("stateKindForError", () => {
  const err = (code: "notSupported" | "permissionDenied" | "platform" | "consentRequired" | "busy") => ({
    code,
    message: "m",
    detail: null,
    retriable: true,
  });

  it("maps codes onto state kinds", () => {
    expect(stateKindForError(err("notSupported"))).toBe("unsupported");
    expect(stateKindForError(err("permissionDenied"))).toBe("permission");
    expect(stateKindForError(err("platform"))).toBe("error");
  });

  it("treats consentRequired as a user choice, busy as a retriable error", () => {
    expect(stateKindForError(err("consentRequired"))).toBe("consent");
    expect(stateKindForError(err("busy"))).toBe("error");
  });
});

describe("formatBytes", () => {
  it("scales by 1024 with the labels Windows uses (simple view, the default)", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GB");
    expect(formatBytes(1.8 * 1024 ** 4)).toBe("1.8 TB");
  });

  it("uses the IEC labels when asked or when the view mode is advanced", () => {
    expect(formatBytes(16 * 1024 ** 3, 1, "iec")).toBe("16.0 GiB");
    setByteUnits("iec");
    try {
      expect(getByteUnits()).toBe("iec");
      expect(formatBytes(1024)).toBe("1.0 KiB");
      expect(formatBytesPerSec(3 * 1024 ** 2)).toBe("3.0 MiB/s");
      expect(formatMib(256)).toBe("256 MiB");
    } finally {
      setByteUnits("windows");
    }
    expect(formatMib(256)).toBe("256 MB");
    expect(formatMib(4096)).toBe("4 GB");
  });

  it("rejects invalid input", () => {
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytes(Number.NaN)).toBe("—");
  });
});

describe("formatPercent", () => {
  it("formats with fixed digits", () => {
    expect(formatPercent(42.456, 1)).toBe("42.5%");
    expect(formatPercent(Number.POSITIVE_INFINITY)).toBe("—");
  });
});

describe("formatDuration", () => {
  it("picks the two most significant units", () => {
    expect(formatDuration(42)).toBe("42 s");
    expect(formatDuration(125)).toBe("2 min");
    expect(formatDuration(3_600 * 2 + 300)).toBe("2 h 5 min");
    expect(formatDuration(86_400 * 3 + 3_600 * 4)).toBe("3 g 4 h");
  });
});

describe("metric helpers", () => {
  it("formats available metrics only", () => {
    const ok: Metric<number> = { status: "available", value: 50, unit: "percent", source: "pdh", at: 0 };
    const missing: Metric<number> = { status: "unavailable", reason: { kind: "permissionDenied" } };
    expect(formatMetric(ok, (v: number) => formatPercent(v))).toBe("50%");
    expect(formatMetric(missing, (v: number) => formatPercent(v))).toBeNull();
  });

  it("explains every unavailability kind", () => {
    expect(unavailabilityText({ kind: "providerMissing", detail: "nvml.dll" })).toContain("nvml.dll");
    expect(
      unavailabilityText({
        kind: "failed",
        detail: { code: "platform", message: "PDH error", detail: null, retriable: true },
      }),
    ).toContain("PDH error");
  });
});

describe("formatElapsed", () => {
  it("keeps second precision past one minute", () => {
    expect(formatElapsed(0)).toBe("0 s");
    expect(formatElapsed(8_900)).toBe("8 s");
    expect(formatElapsed(65_000)).toBe("1 min 05 s");
    expect(formatElapsed(-1)).toBe("—");
  });
});

describe("formatIsoDate", () => {
  it("reformats the calendar date without time zone shifts", () => {
    expect(formatIsoDate("2024-05-01")).toBe("01/05/2024");
    expect(formatIsoDate("2006-06-21T00:00:00Z")).toBe("21/06/2006");
  });

  it("returns unparseable input unchanged", () => {
    expect(formatIsoDate("sconosciuta")).toBe("sconosciuta");
  });
});

describe("formatDateTime", () => {
  it("rejects invalid timestamps", () => {
    expect(formatDateTime(Number.NaN)).toBe("—");
    expect(formatDateTime(0)).toMatch(/1970/);
  });
});

describe("GPU compute and fan formatters", () => {
  it("formats FLOP/s with SI prefixes", () => {
    expect(formatFlopsPerSec(21_770_330_229_964.8)).toBe("21.8 TFLOP/s");
    expect(formatFlopsPerSec(1_234_000_000)).toBe("1.23 GFLOP/s");
    expect(formatFlopsPerSec(45_600_000)).toBe("45.6 MFLOP/s");
    expect(formatFlopsPerSec(1500)).toBe("1.50 kFLOP/s");
    expect(formatFlopsPerSec(950)).toBe("950 FLOP/s");
    expect(formatFlopsPerSec(-1)).toBe("—");
    expect(formatFlopsPerSec(Number.NaN)).toBe("—");
  });

  it("routes the flopsPerSecond unit through the shared result formatter", () => {
    expect(formatUnitValue(21_770_330_229_964.8, "flopsPerSecond")).toBe("21.8 TFLOP/s");
    expect(formatUnitValue(4_560_000, "operationsPerSecond")).toBe("4.56 Mop/s");
  });

  it("formats fan speed, keeping 0 rpm as a real reading", () => {
    expect(formatRpm(0)).toBe("0 rpm");
    expect(formatRpm(1450.6)).toBe("1451 rpm");
    expect(formatRpm(-5)).toBe("—");
  });

  it("labels the WDDM driver source", () => {
    expect(sourceLabel("d3dkmt")).toBe("driver grafico (WDDM)");
  });
});

describe("English formatting", () => {
  afterEach(() => setLang("it"));

  it("translates words, keeps product names and follows the language at call time", () => {
    setLang("en");
    expect(sourceLabel("d3dkmt")).toBe("graphics driver (WDDM)");
    expect(sourceLabel("pdh")).toBe("PDH");
    expect(formatDuration(86_400 * 3 + 3_600 * 4)).toBe("3 d 4 h");
    expect(formatDuration(3_600 * 2 + 300)).toBe("2 h 5 min");
    expect(formatIsoDate("2024-05-01")).toBe("05/01/2024");
    expect(formatIsoDate("unknown")).toBe("unknown");
    expect(errorTitle("network")).toBe("Network error");
    expect(unavailabilityText({ kind: "permissionDenied" })).toBe("Requires elevated privileges");
    expect(unavailabilityText({ kind: "providerMissing", detail: "nvml.dll" })).toBe("Provider not available: nvml.dll");
    setLang("it");
    expect(errorTitle("network")).toBe("Errore di rete");
    expect(formatIsoDate("2024-05-01")).toBe("01/05/2024");
  });
});
