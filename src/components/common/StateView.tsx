import type { ReactNode } from "react";
import { useIsSimple } from "@/hooks/useViewMode";
import { errorTitle } from "@/lib/format";
import { simpleError } from "@/lib/plain";
import type { AppError } from "@/types/generated/AppError";
import { Button } from "./Button";
import { Disclosure } from "./Disclosure";
import { Icon, type IconName } from "./Icon";
import { type Translator, useT } from "@/i18n";

/**
 * Uniform non-data states. Every screen renders one of these instead of an empty box or a
 * made-up value:
 * - `loading`: first load in progress (for content-shaped placeholders use `Skeleton`).
 * - `empty`: the source answered, there is nothing (no runs yet, no problems found).
 * - `unavailable`: a value or block the system did not provide right now (give the reason,
 *   usually from `Metric::Unavailable` via `useUnavailabilityText`).
 * - `unsupported`: this machine or OS has no such sensor/feature (the machine's limit).
 * - `notImplemented`: our gap, not the machine's.
 * - `disconnected`: a live connection dropped (monitoring stream); usually with `onRetry`.
 * - `permission` / `consent`: blocked by OS permissions / by a user choice (settings).
 * - `error`: a command failed; pass the `AppError` for the friendly text and details.
 */
export type StateKind =
  | "loading"
  | "empty"
  | "unavailable"
  | "unsupported"
  | "disconnected"
  | "permission"
  | "consent"
  | "notImplemented"
  | "error";

const ICONS: Record<Exclude<StateKind, "loading">, IconName> = {
  empty: "info",
  unavailable: "info",
  unsupported: "minus",
  disconnected: "network",
  permission: "lock",
  consent: "lock",
  notImplemented: "info",
  error: "error",
};

export type StateLayout = "inline" | "block" | "hero";

interface StateViewProps {
  kind: StateKind;
  title?: string;
  description?: ReactNode;
  error?: AppError;
  onRetry?: () => void;
  /** Extra action(s) next to Retry, e.g. a primary "Esegui diagnosi" button for `empty`. */
  action?: ReactNode;
  /** Shorthand for `layout="inline"` (one row, no vertical padding). */
  compact?: boolean;
  /**
   * `inline`: a row inside a panel. `block` (default): a left-aligned block with padding.
   * `hero`: centered with a glyph, for a whole page or a large empty panel.
   */
  layout?: StateLayout;
}

/**
 * In the "Semplice" view an error gets a friendly sentence and a "Cosa puoi fare" hint per
 * `ErrorCode`; message, detail and code stay in "Dettagli tecnici". "Avanzata" is unchanged.
 */
export function StateView({ kind, title, description, error, onRetry, action, compact = false, layout }: StateViewProps) {
  const simple = useIsSimple();
  const t = useT("common");
  const plain = simple && error ? simpleError(error.code) : null;
  const resolvedTitle = title ?? (error ? (plain ? plain.title : errorTitle(error.code)) : defaultTitle(kind, t));
  const resolvedDescription = description ?? plain?.explanation;
  const mode: StateLayout = layout ?? (compact ? "inline" : "block");
  const isError = kind === "error";
  const showRetry = onRetry && (isError ? error?.retriable !== false : true);
  const glyph =
    kind === "loading" ? (
      <Spinner size={mode === "hero" ? 20 : 16} />
    ) : (
      <Icon name={ICONS[kind]} size={mode === "hero" ? 20 : 16} className={isError ? "text-bad" : kind === "disconnected" ? "text-warn" : "text-fg-subtle"} />
    );

  const details = error && (
    <Disclosure summary={t("technicalDetails")} className="text-xs text-fg-subtle">
      <div className="surface-inset px-3 py-2 font-mono text-2xs text-fg-muted">
        <p>{error.message}</p>
        {error.detail && <p className="mt-1 break-all">{error.detail}</p>}
        {simple && <p className="mt-1">{t("errorCode", { code: error.code })}</p>}
      </div>
    </Disclosure>
  );
  const actions = (showRetry || action) && (
    <div className="flex flex-wrap items-center gap-2">
      {showRetry && (
        <Button variant="secondary" size="sm" icon="refresh" onClick={onRetry}>
          {t("retry")}
        </Button>
      )}
      {action}
    </div>
  );

  if (mode === "hero") {
    return (
      <div role={isError ? "alert" : "status"} aria-busy={kind === "loading"} className="flex flex-col items-center gap-3 px-6 py-12 text-center">
        <span aria-hidden="true" className="satin grid size-11 place-items-center rounded-full">
          {glyph}
        </span>
        <p className="text-md font-semibold text-fg">{resolvedTitle}</p>
        {resolvedDescription && <div className="max-w-md text-sm text-fg-muted">{resolvedDescription}</div>}
        {plain && (
          <p className="max-w-md text-sm text-fg-muted">
            <span className="font-medium text-fg">{t("whatYouCanDo")}</span> {plain.action}
          </p>
        )}
        {actions}
        {details}
      </div>
    );
  }

  return (
    <div
      role={isError ? "alert" : "status"}
      aria-busy={kind === "loading"}
      className={`flex flex-col items-start gap-2 ${mode === "inline" ? "py-1" : "py-6"}`}
    >
      <div className="flex items-center gap-2 text-sm">
        {glyph}
        <span className={isError ? "font-medium text-fg" : "text-fg-muted"}>{resolvedTitle}</span>
      </div>
      {resolvedDescription && <div className="max-w-prose text-sm text-fg-subtle">{resolvedDescription}</div>}
      {plain && (
        <p className="max-w-prose text-sm text-fg-muted">
          <span className="font-medium text-fg">{t("whatYouCanDo")}</span> {plain.action}
        </p>
      )}
      {details}
      {actions && <div className="mt-1">{actions}</div>}
    </div>
  );
}

/** Indeterminate spinner: a quarter arc rotating; static with reduced motion. */
export function Spinner({ size = 16, className = "" }: { size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
      className={`shrink-0 animate-spin-slow text-fg-subtle ${className}`}
    >
      <circle cx="8" cy="8" r="6" stroke="currentColor" strokeOpacity={0.25} strokeWidth={1.75} />
      <path d="M14 8a6 6 0 0 0-6-6" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" />
    </svg>
  );
}

function defaultTitle(kind: StateKind, t: Translator<"common">): string {
  switch (kind) {
    case "loading":
      return t("stateLoading");
    case "empty":
      return t("stateEmpty");
    case "unavailable":
      return t("stateUnavailable");
    case "unsupported":
      return t("stateUnsupported");
    case "disconnected":
      return t("stateDisconnected");
    case "permission":
      return t("statePermission");
    case "consent":
      return t("stateConsent");
    case "notImplemented":
      return t("stateNotImplemented");
    case "error":
      return t("stateError");
  }
}
