import { AnimatePresence, m } from "motion/react";
import { useEffect, useId, useRef, useState, type ReactElement, type ReactNode } from "react";
import { popoverTransition } from "@/lib/motion";

/** `top-end`: above, right edges aligned, for triggers near a panel's right edge (values). */
type Side = "right" | "top" | "top-end" | "bottom";

interface TooltipProps {
  content: ReactNode;
  side?: Side;
  /** When false the tooltip is not rendered at all (e.g. expanded sidebar labels). */
  enabled?: boolean;
  children: ReactElement;
  className?: string;
  /** Wraps long content (a sentence) at 16rem instead of keeping it on one line. */
  multiline?: boolean;
}

const OFFSET: Record<Side, { pos: string; from: { x?: number; y?: number } }> = {
  right: { pos: "left-full top-1/2 ml-2 -translate-y-1/2", from: { x: -4 } },
  top: { pos: "bottom-full left-1/2 mb-2 -translate-x-1/2", from: { y: 4 } },
  "top-end": { pos: "bottom-full right-0 mb-2", from: { y: 4 } },
  bottom: { pos: "top-full left-1/2 mt-2 -translate-x-1/2", from: { y: -4 } },
};

const OPEN_DELAY_MS = 350;

/**
 * Small label tooltip: hover after a short delay, keyboard focus immediately, Esc hides.
 * Supplementary only: the trigger must already have an accessible name.
 */
export function Tooltip({ content, side = "right", enabled = true, children, className = "", multiline = false }: TooltipProps) {
  const [open, setOpen] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const id = useId();

  useEffect(() => () => window.clearTimeout(timer.current), []);
  useEffect(() => {
    if (!enabled) setOpen(false);
  }, [enabled]);

  if (!enabled) return children;

  const show = (delay: number) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setOpen(true), delay);
  };
  const hide = () => {
    window.clearTimeout(timer.current);
    setOpen(false);
  };
  const o = OFFSET[side];

  return (
    <span
      className={`relative inline-flex ${className}`}
      onPointerEnter={() => show(OPEN_DELAY_MS)}
      onPointerLeave={hide}
      onPointerDown={hide}
      onFocus={(e) => {
        if ((e.target as HTMLElement).matches(":focus-visible")) show(0);
      }}
      onBlur={hide}
      onKeyDown={(e) => {
        if (e.key === "Escape") hide();
      }}
    >
      {children}
      <AnimatePresence>
        {open && (
          <m.span
            id={id}
            role="tooltip"
            initial={{ opacity: 0, ...o.from }}
            animate={{ opacity: 1, x: 0, y: 0 }}
            exit={{ opacity: 0, transition: { duration: 0.08 } }}
            transition={popoverTransition}
            className={`pointer-events-none absolute z-50 ${o.pos}`}
          >
            <span className={`block rounded-md border border-line-strong bg-surface-3 px-2 py-1 text-xs text-fg shadow-md ${multiline ? "w-max max-w-64 text-left whitespace-normal" : "whitespace-nowrap"}`}>
              {content}
            </span>
          </m.span>
        )}
      </AnimatePresence>
    </span>
  );
}
