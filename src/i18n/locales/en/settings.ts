import type it from "../it/settings";
import type { Translation } from "../types";

/** English texts for the "settings" namespace. */
export default {
  language: "Language",
  languageHint: "“System” follows the Windows display language.",
  "languageHint@macos": "“System” follows the macOS language.",
  languageSystem: "System",

  // Page
  seconds: "{n} s",
  saveFailed: "Setting not saved",
  saving: "Saving…",
  saved: "Changes are saved automatically.",

  // Appearance
  appearance: "Appearance",
  theme: "Theme",
  theme_system: "System",
  theme_light: "Light",
  theme_dark: "Dark",
  viewMode: "How problems and errors are shown",
  viewModeHint: "{simple}: plain explanations and what to do. {advanced}: every technical detail (sources, codes, full errors).",

  // Monitoring
  monitoring: "Monitoring",
  samplingInterval: "Sampling interval",
  samplingIntervalHint: "CPU, RAM, GPU and disk; slow sources keep their own pace.",
  topN: "Processes in the ranking",
  topNHint: "Top N by Impact Score. Used by the Processes page.",

  // Privacy
  privacy: "Privacy and network",
  onlineChecks: "Allow online checks",
  onlineChecksHint: "Needed to look for driver updates through Windows Update. No driver is installed.",
  onlineChecksWindowsOnly:
    "On this system the app makes no online checks: the only one, the driver update search through Windows Update, exists only on Windows.",

  // App overhead card
  overhead: "App resource usage",
  overheadCaption: "Resources used by the app",
  colComponent: "Component",
  colCpu: "CPU",
  colMemory: "Memory (working set)",
  "colMemory@macos": "Memory (resident)",
  colProcesses: "Processes",
  rowCore: "Core (Rust)",
  rowWebview: "WebView2 (interface)",
  "rowWebview@macos": "WebKit (interface)",
  overheadCpuNote: "Share of the whole machine, averaged over the last {window}",
  historyCapacity: "History capacity",
  historyCapacityValue: "{count} samples (about {duration})",
  samplesCollected: "Samples collected",

  // About card
  about: "About",
  version: "Version",
  build: "Build",
  dbSchema: "Database schema",
  data: "Data",
  dataValue: "Local only. No telemetry.",

  // "Fix it" assistant card (the Italian name "Risolvi" means "fix it")
  assistantTitle: "Fix it assistant (AI)",
  assistantIntro:
    "When you press “Fix it” on a problem, the app can ask Claude, Anthropic's artificial intelligence, how to solve it. It needs your personal Anthropic key: {cost}.",
  assistantIntroCost: "the cost of each request is charged to your Anthropic account",
  assistantPointConsent: "Data is sent only when you press “Fix it” and, after seeing it, agree to send it.",
  assistantPointData:
    "Only the chosen problem is sent, with the technical data about it (for example memory or disk usage, names of the programs involved). PC name, user name and file paths are removed.",
  assistantPointConfirm:
    "Nothing runs without your confirmation, one step at a time, and only from a short list of safe operations built into the app.",
  assistantPointKey: "The key is kept in Windows Credential Manager and is never shown again.",
  "assistantPointKey@macos": "The key is kept in the macOS Keychain and is never shown again.",
  assistantEnable: "Turn on the Fix it assistant",
  offByDefault: "Off by default.",
  apiKey: "Anthropic API key",
  keyChecking: "Checking…",
  keyStatusUnavailable: "Status not available",
  keySaved: "Key saved",
  keySavedDetail: "Stored in Windows Credential Manager.",
  "keySavedDetail@macos": "Stored in the macOS Keychain.",
  keyRemoved: "Key removed",
  noKey: "No key",
  keyReplacePlaceholder: "Paste a new key to replace it",
  saveKey: "Save key",
  removeKey: "Remove key",
  keyHint: "Create one at console.anthropic.com. The field clears after saving.",
  keyNotSaved: "Key not saved",
  assistantStatusUnavailable: "Assistant status not available",
  model: "Model",
  modelHint: "Approximate cost per request, at Anthropic list prices.",

  // Device (3D model on the Overview)
  device: "Device",
  deviceModel: "Device model",
  deviceModelHint: "The 3D model shown on the Overview.",
  shape_auto: "Automatic",
  shape_tower: "Tower",
  shape_laptop: "Laptop",
  shape_mini: "Mini",
  detecting: "Detecting the device…",
  detectedAs: "Detected: {shape} ({basis})",
  detectedAsNoBasis: "Detected: {shape}",
  basis_productName: "from the product name",
  basis_modelIdentifier: "from the model identifier",
  basis_chassisType: "from the chassis type",
  evidence_productName: "Product name: {value}",
  evidence_modelIdentifier: "Model identifier: {value}",
  evidence_chassisType: "Chassis type (SMBIOS): {value}",
  noModel_allInOne: "Detected: all-in-one computer, which has no 3D model. Pick the one you prefer.",
  noModel_other: "The detected form factor has no 3D model. Pick the one you prefer.",
  notDetected: "Not detected: {reason}. Pick the model here.",
  retryDetection: "Try again",
} satisfies Translation<typeof it>;
