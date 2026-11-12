import { describe, expect, it } from "vitest";
import { nextTrapFocus, restoreFocus } from "./focusTrap";

describe("nextTrapFocus", () => {
  it("lets the browser move focus between inner items", () => {
    expect(nextTrapFocus(3, 0, false)).toBeNull();
    expect(nextTrapFocus(3, 1, false)).toBeNull();
    expect(nextTrapFocus(3, 2, true)).toBeNull();
  });

  it("wraps from the last item to the first and back", () => {
    expect(nextTrapFocus(3, 2, false)).toBe(0);
    expect(nextTrapFocus(3, 0, true)).toBe(2);
  });

  it("enters the list when focus is on the container", () => {
    expect(nextTrapFocus(3, -1, false)).toBe(0);
    expect(nextTrapFocus(3, -1, true)).toBe(2);
  });

  it("keeps focus in place when nothing is focusable", () => {
    expect(nextTrapFocus(0, -1, false)).toBe(-1);
    expect(nextTrapFocus(0, -1, true)).toBe(-1);
  });

  it("wraps a single item onto itself", () => {
    expect(nextTrapFocus(1, 0, false)).toBe(0);
    expect(nextTrapFocus(1, 0, true)).toBe(0);
  });
});

describe("restoreFocus", () => {
  const target = (isConnected: boolean) => {
    const calls: unknown[] = [];
    return { isConnected, focus: (o?: unknown) => void calls.push(o), calls };
  };

  it("focuses the previous element without scrolling", () => {
    const t = target(true);
    expect(restoreFocus(t)).toBe(true);
    expect(t.calls).toEqual([{ preventScroll: true }]);
  });

  it("skips an element that left the document", () => {
    const t = target(false);
    expect(restoreFocus(t)).toBe(false);
    expect(t.calls).toEqual([]);
  });

  it("does nothing without a target", () => {
    expect(restoreFocus(null)).toBe(false);
    expect(restoreFocus(undefined)).toBe(false);
  });
});
