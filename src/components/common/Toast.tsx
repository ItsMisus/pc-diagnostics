import { AnimatePresence, m } from "motion/react";
import { useEffect, useRef } from "react";
import { create } from "zustand";
import { spring } from "@/lib/motion";
import { Icon, type IconName } from "./Icon";
import { useT } from "@/i18n";

/**
 * Lightweight, non-blocking confirmations ("Report salvato", "Chiave salvata"). A toast
 * never replaces the inline state of the screen that produced it: it only confirms an
 * action the user just took. Errors stay inline, next to what failed.
 */
export type ToastTone = "success" | "info" | "error";

interface ToastItem {
  id: number;
  tone: ToastTone;
  title: string;
  description?: string;
}

interface ToastStore {
  items: ToastItem[];
  push: (t: Omit<ToastItem, "id">) => void;
  dismiss: (id: number) => void;
}

let nextId = 1;
const MAX_VISIBLE = 3;

export const useToasts = create<ToastStore>((set) => ({
  items: [],
  push: (t) => set((s) => ({ items: [...s.items, { ...t, id: nextId++ }].slice(-MAX_VISIBLE) })),
  dismiss: (id) => set((s) => ({ items: s.items.filter((i) => i.id !== id) })),
}));

/** Imperative helper usable from stores and event handlers. */
export const toast = {
  success: (title: string, description?: string) =>
    useToasts.getState().push({ tone: "success", title, ...(description ? { description } : {}) }),
  info: (title: string, description?: string) =>
    useToasts.getState().push({ tone: "info", title, ...(description ? { description } : {}) }),
  error: (title: string, description?: string) =>
    useToasts.getState().push({ tone: "error", title, ...(description ? { description } : {}) }),
};

const TONE: Record<ToastTone, { icon: IconName; className: string }> = {
  success: { icon: "check", className: "text-ok" },
  info: { icon: "info", className: "text-accent-text" },
  error: { icon: "alert", className: "text-bad" },
};

/** Mounted once in App. Bottom-right stack, polite live region. */
export function Toaster() {
  const items = useToasts((s) => s.items);
  return (
    <div
      aria-live="polite"
      aria-relevant="additions"
      className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-[min(22rem,calc(100vw-2rem))] flex-col items-end gap-2"
    >
      <AnimatePresence initial={false}>
        {items.map((t) => (
          <ToastView key={t.id} item={t} />
        ))}
      </AnimatePresence>
    </div>
  );
}

const LIFETIME_MS = 4500;

function ToastView({ item }: { item: ToastItem }) {
  const dismiss = useToasts((s) => s.dismiss);
  const t = useT("common");
  const timer = useRef<number | undefined>(undefined);
  const start = () => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => dismiss(item.id), item.tone === "error" ? LIFETIME_MS * 2 : LIFETIME_MS);
  };
  const stop = () => window.clearTimeout(timer.current);

  useEffect(() => {
    start();
    return stop;
    // One timer per toast, started on mount.
  }, []);

  const tone = TONE[item.tone];
  return (
    <m.div
      layout
      role={item.tone === "error" ? "alert" : "status"}
      initial={{ opacity: 0, y: 12, scale: 0.97 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.97, transition: { duration: 0.12 } }}
      transition={spring.gentle}
      onPointerEnter={stop}
      onPointerLeave={start}
      onFocus={stop}
      onBlur={start}
      className="pointer-events-auto flex w-full items-start gap-3 rounded-md border border-line-strong bg-surface-2 px-3.5 py-3 shadow-lg"
    >
      <Icon name={tone.icon} className={`mt-0.5 shrink-0 ${tone.className}`} />
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium text-fg">{item.title}</p>
        {item.description && <p className="mt-0.5 text-xs break-words text-fg-muted">{item.description}</p>}
      </div>
      <button
        type="button"
        aria-label={t("closeNotification")}
        onClick={() => dismiss(item.id)}
        className="pressable -mt-0.5 -mr-1 rounded p-1 text-fg-subtle hover:bg-surface-3 hover:text-fg"
      >
        <Icon name="close" size={14} />
      </button>
    </m.div>
  );
}
