import { describe, expect, it } from "vitest";
import type { LightingDevice } from "@/types/generated/LightingDevice";
import { toggleTarget } from "./DevicesPanel";
import { lampColors, modelForKind, modelLayout, STAGE_MODELS } from "./scene/layouts";
import { DEFAULT_SETUP, effectOfKind, reachedDevices } from "./store";

const sup = { state: "supported" as const, reason: null };
const uns = { state: "unsupported" as const, reason: "x" };
const device = (id: string, colorCapable: boolean, ok = true): LightingDevice => ({
  id,
  name: id,
  kind: colorCapable ? "keyboard" : "keyboardBacklight",
  vendor: null,
  lampCount: 1,
  colorCapable,
  color: colorCapable && ok ? sup : uns,
  brightness: ok ? sup : uns,
  effects: ok ? sup : uns,
  source: colorCapable ? "lampArray" : "macKeyboardBacklight",
});

describe("targets and reach", () => {
  it("switches devices in and out, back to all", () => {
    const all = ["a", "b"];
    const off = toggleTarget({ kind: "all" }, all, "a", false);
    expect(off).toEqual({ kind: "devices", ids: ["b"] });
    expect(toggleTarget(off, all, "a", true)).toEqual({ kind: "all" });
    expect(toggleTarget(off, all, "b", false)).toEqual({ kind: "devices", ids: [] });
  });

  it("reaches only devices that can show the effect", () => {
    const devices = [device("rgb", true), device("kb", false), device("vendor", true, false)];
    expect(reachedDevices(devices, DEFAULT_SETUP).map((d) => d.id)).toEqual(["rgb", "kb"]);
    const rainbow = { ...DEFAULT_SETUP, effect: effectOfKind("rainbow", DEFAULT_SETUP.effect) };
    expect(reachedDevices(devices, rainbow).map((d) => d.id)).toEqual(["rgb"]);
    expect(reachedDevices(devices, { ...DEFAULT_SETUP, targets: { kind: "devices", ids: ["kb"] } }).map((d) => d.id)).toEqual(["kb"]);
  });

  it("keeps colors when switching effects", () => {
    const red = { r: 255, g: 0, b: 0 };
    const wave = effectOfKind("wave", { kind: "static", color: red });
    expect(wave.kind === "wave" && wave.colors.length).toBe(3);
    const back = effectOfKind("breathing", { kind: "gradient", colors: [red, red] });
    expect(back).toEqual({ kind: "breathing", color: red, period: 4000 });
  });
});

describe("stage models", () => {
  it("lays every model out with positions in 0..1", () => {
    for (const m of STAGE_MODELS) {
      const l = modelLayout(m);
      expect(l.lamps.length, m).toBeGreaterThan(10);
      for (const lamp of l.lamps) expect(lamp.u >= 0 && lamp.u <= 1, m).toBe(true);
    }
    expect(modelForKind("keyboardBacklight")).toBe("backlight");
    expect(modelForKind("headset")).toBe("strip");
  });

  it("shows the MacBook backlight in white at the effect's level", () => {
    const l = modelLayout("backlight");
    const colors = lampColors("backlight", l, { kind: "static", color: { r: 255, g: 0, b: 0 } }, 100, 0);
    expect(colors[0]).toEqual({ r: 255, g: 255, b: 255 });
    expect(lampColors("backlight", l, { kind: "off" }, 100, 0)[0]).toEqual({ r: 0, g: 0, b: 0 });
  });
});
