import { describe, expect, it } from "vitest";
import { commitLevel, memoryLevel, volumeLevel, vramLevel } from "./levels";

const GIB = 1024 ** 3;

describe("memoryLevel", () => {
  it("follows the available-memory rule (10 % / 5 %)", () => {
    expect(memoryLevel(50, 100)).toBe("normal");
    expect(memoryLevel(10, 100)).toBe("normal");
    expect(memoryLevel(9.9, 100)).toBe("warn");
    expect(memoryLevel(5, 100)).toBe("warn");
    expect(memoryLevel(4.9, 100)).toBe("bad");
  });
  it("stays normal when a value is missing", () => {
    expect(memoryLevel(null, 100)).toBe("normal");
    expect(memoryLevel(1, null)).toBe("normal");
    expect(memoryLevel(1, 0)).toBe("normal");
  });
});

describe("commitLevel and vramLevel", () => {
  it("warn from 90 %, never critical", () => {
    expect(commitLevel(89, 100)).toBe("normal");
    expect(commitLevel(90, 100)).toBe("warn");
    expect(commitLevel(100, 100)).toBe("warn");
    expect(vramLevel(89.9, 100)).toBe("normal");
    expect(vramLevel(99, 100)).toBe("warn");
    expect(vramLevel(null, 100)).toBe("normal");
  });
});

describe("volumeLevel", () => {
  it("uses the free-space percentages", () => {
    expect(volumeLevel(20, 100)).toBe("normal");
    expect(volumeLevel(9, 100)).toBe("warn");
    expect(volumeLevel(4, 100)).toBe("bad");
  });
  it("applies the 15 GiB rule only to volumes of at least 30 GiB", () => {
    // 1 TB volume, 14 GiB free (1.4 %, already critical by percentage).
    expect(volumeLevel(14 * GIB, 1000 * GIB)).toBe("bad");
    // 200 GiB volume, 14 GiB free = 7 %: warn by percentage anyway.
    expect(volumeLevel(14 * GIB, 200 * GIB)).toBe("warn");
    // 100 GiB volume, 14 GiB free = 14 %: warn only by the absolute rule.
    expect(volumeLevel(14 * GIB, 100 * GIB)).toBe("warn");
    // 25 GiB volume, 14 GiB free: below the minimum size, the absolute rule does not apply.
    expect(volumeLevel(14 * GIB, 25 * GIB)).toBe("normal");
  });
});
