/**
 * The 3D device stage. Public API:
 *
 * - `<DeviceStage kind theme? paused? ambient? indicators? aspectRatio? className? onModeChange? />`
 * - `preloadDeviceStage()` to fetch the 3D chunk ahead of time.
 * - Types: `DeviceKind`, `StageTheme`, `StageMode`, `IndicatorSlot`, `DeviceIndicator`.
 *
 * Nothing here imports three.js: the scene lives in a lazy chunk (`scene/DeviceScene`).
 */
export { DeviceStage, preloadDeviceStage, type DeviceStageProps } from "./DeviceStage";
export { DEVICE_KINDS, INDICATOR_SLOTS, type DeviceIndicator, type DeviceKind, type IndicatorSlot, type StageMode, type StageTheme } from "./types";
