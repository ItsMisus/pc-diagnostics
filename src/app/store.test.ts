import { afterEach, describe, expect, it } from "vitest";
import { formatBytes, getByteUnits } from "@/lib/format";
import { useAppStore } from "./store";

describe("view mode and byte labels", () => {
  afterEach(() => useAppStore.getState().setViewMode("simple"));

  it("switches between Windows and IEC labels with the view mode", () => {
    expect(getByteUnits()).toBe("windows");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GB");

    useAppStore.getState().setViewMode("advanced");
    expect(getByteUnits()).toBe("iec");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GiB");

    useAppStore.getState().setViewMode("simple");
    expect(getByteUnits()).toBe("windows");
  });
});
