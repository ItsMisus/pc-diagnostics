import { afterEach, describe, expect, it } from "vitest";
import { useBenchView } from "./viewStore";

describe("benchmark view store", () => {
  afterEach(() => useBenchView.setState({ component: null, runInView: false }));

  it("starts with no open screen and the run block not in view", () => {
    expect(useBenchView.getState().component).toBeNull();
    expect(useBenchView.getState().runInView).toBe(false);
  });

  it("records whether the run block is visible", () => {
    useBenchView.getState().setRunInView(true);
    expect(useBenchView.getState().runInView).toBe(true);
    useBenchView.getState().setRunInView(false);
    expect(useBenchView.getState().runInView).toBe(false);
  });
});
