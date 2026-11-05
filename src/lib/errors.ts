import type { AppError } from "@/types/generated/AppError";

export type ErrorStateKind = "unsupported" | "permission" | "consent" | "error";

/**
 * Maps a command failure onto a `StateView` kind. `notSupported` covers both "running
 * outside the desktop app" and "this machine cannot do it": neither is a fault to retry.
 * `consentRequired` is a user choice (e.g. online checks disabled), not a fault either.
 */
export function stateKindForError(error: AppError): ErrorStateKind {
  switch (error.code) {
    case "notSupported":
      return "unsupported";
    case "permissionDenied":
      return "permission";
    case "consentRequired":
      return "consent";
    default:
      return "error";
  }
}
