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
 * Uniform non-data states. Every screen renders one of these instead of an empty box.
 * `notImplemented` is distinct from `unsupported`: the first is our gap, the second the
 * machine's.
 */
export type StateKind = "loading" | "empty" | "unsupported" | "permission" | "consent" | "notImplemented" | "error";

const ICONS: Record<StateKind, IconName> = {
  loading: "refresh",
  empty: "info",
  unsupported: "info",
  permission: "lock",
  consent: "lock",
  notImplemented: "info",
  error: "alert",
};

interface StateViewProps {
  kind: StateKind;
  title?: string;
  description?: ReactNode;
  error?: AppError;
  onRetry?: () => void;
  compact?: boolean;
}

/**
 * In the "Semplice" view an error gets a friendly sentence and a "Cosa puoi fare" hint per
 * `ErrorCode`; message, detail and code stay in "Dettagli tecnici". "Avanzata" is unchanged.
 */
export function StateView({ kind, title, description, error, onRetry, compact = false }: StateViewProps) {
  const simple = useIsSimple();
  const t = useT("common");
  const plain = simple && error ? simpleError(error.code) : null;
  const resolvedTitle = title ?? (error ? (plain ? plain.title : errorTitle(error.code)) : defaultTitle(kind, t));
  const resolvedDescription = description ?? plain?.explanation;
  return (
    <div
      role={kind === "error" ? "alert" : "status"}
      aria-busy={kind === "loading"}
      className={`flex flex-col items-start gap-2 ${compact ? "py-1" : "py-6"}`}
    >
      <div className="flex items-center gap-2 text-sm">
        {kind === "loading" ? (
          <Spinner />
        ) : (
          <Icon name={ICONS[kind]} className={kind === "error" ? "text-bad" : "text-fg-subtle"} />
        )}
        <span className={kind === "error" ? "text-fg" : "text-fg-muted"}>{resolvedTitle}</span>
      </div>
      {resolvedDescription && <div className="max-w-prose text-sm text-fg-subtle">{resolvedDescription}</div>}
      {plain && (
        <p className="max-w-prose text-sm text-fg-muted">
          <span className="font-medium text-fg">{t("whatYouCanDo")}</span> {plain.action}
        </p>
      )}
      {error && (
        <Disclosure summary={t("technicalDetails")} className="text-xs text-fg-subtle">
          <div className="rounded-md border border-line bg-surface-2/60 px-2.5 py-2 font-mono text-2xs">
            <p>{error.message}</p>
            {error.detail && <p className="mt-1 break-all">{error.detail}</p>}
            {simple && <p className="mt-1">{t("errorCode", { code: error.code })}</p>}
          </div>
        </Disclosure>
      )}
      {onRetry && (kind === "error" ? error?.retriable !== false : true) && (
        <Button variant="secondary" size="sm" onClick={onRetry} className="mt-1">
          <Icon name="refresh" />
          {t("retry")}
        </Button>
      )}
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
    case "unsupported":
      return t("stateUnsupported");
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
