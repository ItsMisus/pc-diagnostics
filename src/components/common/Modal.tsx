import { AnimatePresence, m } from "motion/react";
import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { useT } from "@/i18n";
import { backdropVariants, dialogVariants, sheetVariants } from "@/lib/motion";
import { Button } from "./Button";
import { FOCUSABLE_SELECTOR, nextTrapFocus, restoreFocus } from "./focusTrap";
import { Icon, type IconName } from "./Icon";
import { Tooltip } from "./Tooltip";

export type ModalSize = "sm" | "md" | "lg";
export type ModalPresentation = "dialog" | "sheet";

const WIDTH: Record<ModalSize, string> = {
  sm: "w-[min(26rem,calc(100vw-2rem))]",
  md: "w-[min(34rem,calc(100vw-2rem))]",
  lg: "w-[min(44rem,calc(100vw-2rem))]",
};

const SHEET_WIDTH: Record<ModalSize, string> = {
  sm: "w-[min(22rem,calc(100vw-1rem))]",
  md: "w-[min(28rem,calc(100vw-1rem))]",
  lg: "w-[min(36rem,calc(100vw-1rem))]",
};

export interface ModalProps {
  open: boolean;
  /** Called on Esc and the close button (and the backdrop with `closeOnBackdrop`). The parent sets `open` to false. */
  onClose: () => void;
  /** Title in the default header; also the dialog's accessible name. */
  title?: ReactNode;
  /** Short line under the title (the subject: "Temperatura CPU alta"). */
  description?: ReactNode;
  /** Glyph before the title. */
  icon?: IconName;
  /** Replaces the default header entirely (then pass `labelledBy`). */
  header?: ReactNode;
  /** id of the element naming the dialog when `header` is custom. */
  labelledBy?: string;
  /** Actions row, right-aligned, at most one primary. Ignored with `bare`. */
  footer?: ReactNode;
  children: ReactNode;
  size?: ModalSize;
  /** `dialog`: centered. `sheet`: a panel from the trailing edge, full height (details, inspectors). */
  presentation?: ModalPresentation;
  /** Esc closes (default true). Set false while closing would lose work (a request in flight). */
  dismissible?: boolean;
  /** A click outside the panel closes (default false: dialogs here hold work). */
  closeOnBackdrop?: boolean;
  /** Disables the header's close button (e.g. while a step runs); `closeHint` says why. */
  closeDisabled?: boolean;
  closeHint?: string;
  /** Hides the header's close button (the footer then must offer a way out). */
  hideClose?: boolean;
  /** Renders `children` directly in the panel's column (they bring their own `ModalBody` / `ModalFooter`). */
  bare?: boolean;
  /** Element to focus on open. Default: the first `autoFocus` element, else the first focusable one. */
  initialFocusRef?: RefObject<HTMLElement | null>;
  /** After the close animation, once focus is back on the trigger. */
  onExitComplete?: () => void;
}

/**
 * Accessible modal dialog and side sheet, on the native `<dialog>` (`showModal`): the rest of
 * the window is inert, it sits in the top layer, Esc arrives as `cancel`.
 * - Focus: moves into the dialog on open (first `autoFocus` element or `initialFocusRef`), Tab
 *   and Shift+Tab wrap inside it, and on close it returns to the element that had it.
 * - Motion: backdrop fades, the panel rises (dialog) or slides in (sheet) on a spring; the exit
 *   is faster. Reduced motion keeps only the fade (MotionProvider).
 * - App shortcuts are ignored while it is open (AppShell checks `dialog[open]`).
 */
export function Modal({
  open,
  onClose,
  title,
  description,
  icon,
  header,
  labelledBy,
  footer,
  children,
  size = "md",
  presentation = "dialog",
  dismissible = true,
  closeOnBackdrop = false,
  closeDisabled = false,
  closeHint,
  hideClose = false,
  bare = false,
  initialFocusRef,
  onExitComplete,
}: ModalProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const returnTo = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const descriptionId = useId();
  const t = useT("common");
  // Latest values for the native event handlers below.
  const state = useRef({ open, dismissible, onClose });
  state.current = { open, dismissible, onClose };

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog || !open || dialog.open) return;
    const active = document.activeElement;
    returnTo.current = active instanceof HTMLElement && active !== document.body ? active : null;
    dialog.showModal();
    const target = initialFocusRef?.current;
    if (target) target.focus();
    else if (!panelRef.current?.contains(document.activeElement)) firstFocusable(panelRef.current)?.focus();
    // `initialFocusRef` is read once, at open.
  }, [open]);

  // Unmounted while open: the browser closes the dialog with the element; give focus back.
  useEffect(
    () => () => {
      if (ref.current?.open) restoreFocus(returnTo.current);
    },
    [],
  );

  const finishClose = () => {
    const dialog = ref.current;
    if (dialog?.open && !state.current.open) dialog.close();
    if (!state.current.open) {
      restoreFocus(returnTo.current);
      returnTo.current = null;
      onExitComplete?.();
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDialogElement>) => {
    if (e.key !== "Tab") return;
    const items = focusables(panelRef.current);
    const current = items.indexOf(document.activeElement as HTMLElement);
    const next = nextTrapFocus(items.length, current, e.shiftKey);
    if (next === null) return;
    e.preventDefault();
    if (next >= 0) items[next]?.focus();
  };

  const sheet = presentation === "sheet";
  const named = labelledBy ?? (title ? titleId : undefined);

  const closeButton = !hideClose && (
    <Tooltip content={closeHint ?? t("close")} side="bottom-end" enabled={Boolean(closeHint) && closeDisabled}>
      <Button variant="ghost" size="sm" iconOnly icon="close" aria-label={t("close")} disabled={closeDisabled} onClick={onClose} className="-mt-0.5 -mr-2" />
    </Tooltip>
  );

  return (
    <dialog
      ref={ref}
      aria-labelledby={named}
      aria-describedby={description && !header ? descriptionId : undefined}
      onKeyDown={onKeyDown}
      onCancel={(e) => {
        // Esc: the browser would close the dialog by itself; the owner decides.
        e.preventDefault();
        if (state.current.dismissible) state.current.onClose();
      }}
      onClose={() => {
        // The browser may force-close (e.g. a repeated Esc). Keep the dialog while its owner
        // wants it open and it may not be dismissed; otherwise report the close.
        const s = state.current;
        if (!s.open) return;
        if (s.dismissible) s.onClose();
        else ref.current?.showModal();
      }}
      className="fixed inset-0 m-0 h-full max-h-none w-full max-w-none overflow-hidden border-0 bg-transparent p-0 text-fg outline-none"
    >
      <AnimatePresence onExitComplete={finishClose}>
        {open && (
          <m.div key="modal" className="absolute inset-0" initial="hidden" animate="shown" exit="hidden">
            <m.div variants={backdropVariants} aria-hidden="true" className="absolute inset-0 bg-[var(--backdrop)]" />
            <div
              className={`relative flex h-full w-full ${sheet ? "items-stretch justify-end p-2" : "items-center justify-center p-6"}`}
              onClick={(e) => {
                if (e.target === e.currentTarget && closeOnBackdrop && dismissible) onClose();
              }}
            >
              <m.div
                ref={panelRef}
                variants={sheet ? sheetVariants : dialogVariants}
                className={`glass-panel flex flex-col overflow-hidden ${
                  sheet ? `${SHEET_WIDTH[size]} h-full rounded-xl` : `${WIDTH[size]} max-h-[min(85vh,52rem)] rounded-xl`
                }`}
              >
                {header ??
                  ((title || !hideClose) && (
                    <header className="flex shrink-0 items-start gap-3 border-b border-line px-6 pt-5 pb-4">
                      {icon && (
                        <span aria-hidden="true" className="satin grid size-8 shrink-0 place-items-center rounded-full text-fg-muted">
                          <Icon name={icon} />
                        </span>
                      )}
                      <div className="min-w-0 flex-1 self-center">
                        {title && (
                          <h2 id={titleId} className="font-display text-md font-semibold text-fg">
                            {title}
                          </h2>
                        )}
                        {description && (
                          <p id={descriptionId} className="mt-0.5 truncate text-xs text-fg-muted">
                            {description}
                          </p>
                        )}
                      </div>
                      {closeButton}
                    </header>
                  ))}
                {bare ? (
                  children
                ) : (
                  <>
                    <ModalBody>{children}</ModalBody>
                    {footer && <ModalFooter>{footer}</ModalFooter>}
                  </>
                )}
              </m.div>
            </div>
          </m.div>
        )}
      </AnimatePresence>
    </dialog>
  );
}

/** Scrolling content area of a modal. */
export function ModalBody({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`min-h-0 flex-1 overflow-y-auto px-6 py-5 ${className}`}>{children}</div>;
}

/** Footer of a modal: actions on the right, at most one primary, always visible below the body. */
export function ModalFooter({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-line px-6 py-4 ${className}`}>{children}</div>;
}

/** Class names of `ModalBody` / `ModalFooter`, for owners that animate their own wrappers. */
export const MODAL_BODY_CLASS = "min-h-0 flex-1 overflow-y-auto px-6 py-5";
export const MODAL_FOOTER_CLASS = "flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-line px-6 py-4";

function focusables(root: HTMLElement | null): HTMLElement[] {
  if (!root) return [];
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter((el) => el.getClientRects().length > 0);
}

function firstFocusable(root: HTMLElement | null): HTMLElement | undefined {
  if (!root) return undefined;
  return root.querySelector<HTMLElement>("[autofocus]") ?? focusables(root)[0];
}
