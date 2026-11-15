import type it from "../it/common";
import type { Translation } from "../types";

/** English texts for the "common" namespace: shared components. */
export default {
  stateLoading: "Loading…",
  stateEmpty: "No data available",
  stateUnsupported: "Not supported on this system",
  statePermission: "Insufficient permissions",
  stateConsent: "Consent required",
  stateNotImplemented: "Not implemented yet",
  stateError: "Error",
  stateUnavailable: "Not available",
  stateDisconnected: "Connection lost",
  whatYouCanDo: "What you can do:",
  technicalDetails: "Technical details",
  errorCode: "code: {code}",
  retry: "Try again",
  close: "Close",
  closeNotification: "Close notification",
  notAvailable: "n/a",
  notReported: "not reported by the system",
  source: "source: {source}",
  viewModeLabel: "View",
  viewModeSimple: "Simple",
  viewModeSwitchTo: "switch to {mode}",
  viewModeAdvanced: "Advanced",
  viewModeHint: "Simple: explanations in plain words. Advanced: every technical detail. Also available in Settings.",
  supportSupported: "Available",
  supportReadOnly: "Read only",
  supportUnsupported: "Not supported",
} satisfies Translation<typeof it>;
