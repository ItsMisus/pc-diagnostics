import { describe, expect, it } from "vitest";
import { appendSample, mergeSamples } from "./liveBuffer";
import { sample } from "./testing/fixtures";

const seqs = (xs: readonly { seq: number }[]) => xs.map((x) => x.seq);

describe("appendSample", () => {
  it("appends a newer sample without mutating the input", () => {
    const buf = [sample(1)];
    const next = appendSample(buf, sample(2), 10);
    expect(next && seqs(next)).toEqual([1, 2]);
    expect(seqs(buf)).toEqual([1]);
  });

  it("ignores duplicates and out-of-order samples", () => {
    const buf = [sample(1), sample(5)];
    expect(appendSample(buf, sample(5), 10)).toBeNull();
    expect(appendSample(buf, sample(3), 10)).toBeNull();
  });

  it("accepts gaps in seq (missed events)", () => {
    const next = appendSample([sample(1)], sample(9), 10);
    expect(next && seqs(next)).toEqual([1, 9]);
  });

  it("never grows past capacity", () => {
    let buf: readonly ReturnType<typeof sample>[] = [];
    for (let i = 1; i <= 50; i++) buf = appendSample(buf, sample(i), 8) ?? buf;
    expect(buf).toHaveLength(8);
    expect(seqs(buf)).toEqual([43, 44, 45, 46, 47, 48, 49, 50]);
  });

  it("handles capacity 1", () => {
    const next = appendSample([sample(1)], sample(2), 1);
    expect(next && seqs(next)).toEqual([2]);
  });
});

describe("mergeSamples", () => {
  it("sorts by seq, removes duplicates and keeps the newest", () => {
    const merged = mergeSamples(
      [
        [sample(3), sample(1)],
        [sample(2), sample(3), sample(4)],
      ],
      3,
    );
    expect(seqs(merged)).toEqual([2, 3, 4]);
  });

  it("returns an empty buffer for zero capacity", () => {
    expect(mergeSamples([[sample(1)]], 0)).toEqual([]);
  });
});
