import { describe, expect, it } from "vitest";
import { call, isAppError, toAppError } from "./ipc";

describe("isAppError", () => {
  it("accepts a backend error shape", () => {
    expect(
      isAppError({ code: "busy", message: "x", detail: null, retriable: true }),
    ).toBe(true);
  });

  it("rejects unknown codes and wrong shapes", () => {
    expect(isAppError({ code: "nope", message: "x", retriable: true })).toBe(false);
    expect(isAppError("boom")).toBe(false);
    expect(isAppError(null)).toBe(false);
  });

  it("accepts the network code", () => {
    expect(isAppError({ code: "network", message: "x", detail: null, retriable: true })).toBe(true);
  });
});

describe("toAppError", () => {
  it("passes through AppError and fills missing detail", () => {
    const e = toAppError({ code: "database", message: "m", retriable: true });
    expect(e).toEqual({ code: "database", message: "m", detail: null, retriable: true });
  });

  it("wraps strings and Error instances as internal", () => {
    expect(toAppError("plain").code).toBe("internal");
    expect(toAppError("plain").detail).toBe("plain");
    expect(toAppError(new Error("boom")).detail).toBe("boom");
  });
});

describe("call", () => {
  it("rejects with notSupported outside the Tauri runtime", async () => {
    await expect(call("get_settings")).rejects.toMatchObject({ code: "notSupported" });
  });
});
