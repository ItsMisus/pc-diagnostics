import type it from "../it/assistant";
import type { Translation } from "../types";

/**
 * English texts for the "assistant" namespace. The Italian assistant name "Risolvi" ("fix
 * it") is rendered as "Fix it": the button says what it does.
 */
export default {
  // Button
  button: "Fix it",
  buttonAria: "Fix it: {label}",
  buttonTitle: "Ask the AI assistant how to fix it (it first shows you what would be sent)",

  // Not ready (model.ts)
  setupStepOpenSettings: "Open Settings and find the “Fix it assistant (AI)” section.",
  setupStepEnable: "Turn on the assistant.",
  setupStepKey: "Paste your personal Anthropic key and press “Save key”.",
  disabledTitle: "The Fix it assistant is off",
  disabledExplanation:
    "Fix it asks an artificial intelligence (Anthropic's Claude) how to solve the problem. It's off by design until you decide to use it. No data has been sent.",
  noKeyTitle: "The Anthropic key is missing",
  noKeyExplanation:
    "The assistant is on, but it needs your personal Anthropic key: any cost of the requests is charged to your account. No data has been sent.",

  // Models (model.ts)
  modelRecommended: "{model} (recommended)",
  modelHintSonnet55: "Good balance of quality and cost (~$0.03 per request)",
  modelHintOpus55: "For the hardest problems (~$0.06–0.10)",
  modelHintOpus45: "Previous generation (~$0.10–0.18)",
  modelHintHaiku55: "The cheapest, for simple problems (~$0.002)",

  // Plan (model.ts)
  risk_low: "Low risk",
  risk_medium: "Medium risk",
  risk_high: "High risk",
  planConfidence_high: "The AI thinks these steps are likely to solve the problem",
  planConfidence_medium: "The AI thinks these steps might solve the problem",
  planConfidence_low: "The AI isn't sure these steps will solve the problem",
  planConfidenceShort_high: "High confidence",
  planConfidenceShort_medium: "Medium confidence",
  planConfidenceShort_low: "Low confidence",

  // Actions (model.ts)
  settingsPage_windowsUpdate: "Windows Update",
  settingsPage_windowsUpdateOptional: "Optional updates",
  settingsPage_storageSense: "Storage Sense",
  settingsPage_display: "Display",
  settingsPage_sound: "Sound",
  settingsPage_bluetooth: "Bluetooth & devices",
  settingsPage_network: "Network & internet",
  settingsPage_power: "Power",
  settingsPage_troubleshoot: "Troubleshoot",
  settingsPage_startupApps: "Startup apps",
  settingsPage_installedApps: "Installed apps",
  settingsPage_privacy: "Privacy & security",
  days_one: "{count} day",
  days_other: "{count} days",
  action_openSettingsPage:
    "It will open the “{page}” page in Windows Settings. On its own it changes nothing: you decide what to change.",
  action_openDeviceManager: "It will open Windows Device Manager. On its own it changes nothing: you decide what to do.",
  action_openDiskCleanup: "It will open Windows Disk Cleanup for drive {drive}. You choose what to delete.",
  action_emptyRecycleBin: "It will empty the Recycle Bin. Windows will ask you to confirm. This can't be undone.",
  action_cleanTempFiles:
    "It will delete temporary files older than {days} from your temporary files folder. Files still in use are skipped. This can't be undone.",
  action_closeProgram: "It will ask “{name}” to close, like clicking the X. Save your work first.",
  action_runSystemFileCheck:
    "Windows will ask for administrator permission, then a window opens that checks and repairs the Windows system files. It can take 10–20 minutes: leave the window open until it finishes.",

  // Numbers (model.ts)
  costZero: "$0",
  costBelowCent: "less than $0.01",
  costApprox: "≈ ${amount}",
  tokenUsage: "{input} tokens sent · {output} tokens received",

  // Dialog
  dialogTitle: "Fix it with the AI assistant",
  close: "Close",
  cancel: "Cancel",
  waitForStep: "Wait for the current step to finish",
  checking: "Checking the assistant…",
  preparing: "Preparing the data to send…",
  nothingSentYet: "No data has been sent yet.",
  howToEnable: "How to turn it on",
  openSettings: "Open Settings",
  whatWillBeSent: "What will be sent",
  previewIntro:
    "Check the data below. Nothing is sent until you press “Send and ask for a fix”. PC name, user name and file paths have already been removed.",
  previewRefreshed: "The previous preview had expired: the data was prepared again. Check it and press “Send” again.",
  destination: "Destination",
  model: "Model",
  estimatedCost: "Estimated cost",
  costNote: "(estimate, charged to your Anthropic account)",
  payloadSize: "Data size",
  exactDataAria: "Exact data that will be sent",
  showExactData: "Show the exact data",
  exactData: "Exact data",
  sendAndAsk: "Send and ask for a fix",
  requesting: "{model} is preparing a proposal…",
  requestingDescription:
    "This usually takes 30–60 seconds. If you cancel, the answer will be ignored; the request has already been sent, though, and any cost still applies.",
  errorReportChanged: "The diagnostic report has changed",
  errorStatus: "Couldn't check the assistant",
  errorPreview: "Couldn't prepare the data to send",
  errorRequest: "The request to the assistant failed",
  errorReportChangedDescription:
    "A new diagnosis was run in the meantime. Close this and press “Fix it” again on the updated problem.",
  errorRequestDescription: "If you try again, the data will be prepared again and shown to you before it is sent.",
  errorReportChangedDetail: "The latest report changed: expected rule {expected}, the preview is for {got}",

  // Plan view
  proposal: "Proposal",
  warnings: "Caution",
  noSteps: "The assistant didn't propose any steps to run.",
  proposedSteps: "Proposed steps",
  stepNumber: "Step {n}:",
  requiresAdmin: "Requires administrator",
  notReversible: "Not reversible",
  appNote: "App note: {note}",
  confirmStepAria: "Confirm step {n}",
  whatWillHappen: "What will happen:",
  mayAskAdmin: "Windows may ask you for administrator permission.",
  cannotUndo: "This step can't be undone.",
  confirmAndRun: "Confirm and run",
  runningStep: "Running…",
  stepDone: "Done:",
  stepFailed: "Failed:",
  stepNotRun: "The step was not run",
  runStep: "Run this step",
  manualStep: "To do by hand:",
  manualDone: "Done",
  aiDisclaimer: "These suggestions are generated by AI: check them before going ahead.",
  answeredBy: "Answer from {model}",
} satisfies Translation<typeof it>;
