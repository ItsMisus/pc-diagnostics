import type it from "../it/errors";
import type { Translation } from "../types";

/** English texts for the "errors" namespace: src/lib (error titles, reasons, formats). */
export default {
  // lib/format.ts: technical reason a value is missing ("Advanced").
  reason_notSupported: "Not exposed by the hardware or operating system",
  reason_providerMissing: "Provider not available: {detail}",
  reason_permissionDenied: "Requires elevated privileges",
  reason_notImplemented: "Provider not implemented yet",
  reason_failed: "Read failed: {message}",

  // lib/plain.ts: the same reasons in everyday words ("Simple").
  simpleReason_notSupported: "This PC does not provide this information",
  simpleReason_providerMissing: "Something needed to read it is missing",
  simpleReason_permissionDenied: "The app needs to run as administrator",
  simpleReason_notImplemented: "Not supported by the app yet",
  simpleReason_failed: "Couldn't read it, try again later",
  reasonTooltip: "{plain}. Technical detail: {technical}",

  // lib/format.ts: errorTitle ("Advanced").
  title_invalidInput: "Invalid value",
  title_notFound: "Item not found",
  title_permissionDenied: "Insufficient permissions",
  title_notSupported: "Feature not supported",
  title_platform: "Operating system error",
  title_database: "Local database error",
  title_busy: "Operation already in progress",
  title_cancelled: "Operation cancelled",
  title_consentRequired: "Consent required",
  title_network: "Network error",
  title_internal: "Internal error",

  // lib/plain.ts: simpleError ("Simple"): title, explanation, "What you can do".
  simpleTitle_invalidInput: "Value not accepted",
  simpleExplanation_invalidInput: "The app received a value it can't use.",
  simpleAction_invalidInput: "Check the choice you made and try again.",
  simpleTitle_notFound: "Not found",
  simpleExplanation_notFound: "What you were looking for isn't available, or doesn't exist yet.",
  simpleAction_notFound: "Try again; if needed, refresh the data on the page.",
  simpleTitle_permissionDenied: "Administrator permissions needed",
  simpleExplanation_permissionDenied: "Windows doesn't let the app read this with the current permissions.",
  simpleAction_permissionDenied: "Close the app and reopen it with a right-click → “Run as administrator”.",
  simpleTitle_notSupported: "Not available on this PC",
  simpleExplanation_notSupported: "This PC or this version of Windows doesn't offer this feature. It isn't a fault.",
  simpleAction_notSupported: "Nothing to do.",
  simpleTitle_platform: "Windows didn't respond as expected",
  simpleExplanation_platform: "The request to Windows didn't succeed.",
  simpleAction_platform: "Try again in a moment; if it happens again, restart the PC.",
  simpleTitle_database: "Problem with the app's saved data",
  simpleExplanation_database: "The app couldn't read or save its own data.",
  simpleAction_database: "Close and reopen the app; if it happens again, check that the disk isn't full.",
  simpleTitle_busy: "Already in progress",
  simpleExplanation_busy: "The app is already doing this.",
  simpleAction_busy: "Wait for it to finish, then try again.",
  simpleTitle_cancelled: "Stopped",
  simpleExplanation_cancelled: "The operation was stopped before it finished.",
  simpleAction_cancelled: "You can start it again whenever you like.",
  simpleTitle_consentRequired: "Your consent is needed",
  simpleExplanation_consentRequired: "This feature uses the Internet and is turned off by choice.",
  simpleAction_consentRequired: "If you want to use it, turn on “Allow online checks” in Settings.",
  simpleTitle_network: "Couldn't connect",
  simpleExplanation_network: "The app couldn't reach the service on the Internet.",
  simpleAction_network: "Check your Internet connection and try again in a moment.",
  simpleTitle_internal: "Something went wrong in the app",
  simpleExplanation_internal: "Something unexpected went wrong in the app.",
  simpleAction_internal: "Try again; if it happens again, close and reopen the app.",

  // lib/format.ts: source labels that are words, not product names.
  source_d3dkmt: "graphics driver (WDDM)",

  // lib/format.ts: units and date layout (US: month/day/year).
  durationDaysHours: "{days} d {hours} h",
  isoDate: "{month}/{day}/{year}",
} satisfies Translation<typeof it>;
