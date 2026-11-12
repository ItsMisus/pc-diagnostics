/**
 * Focus management for modal surfaces (`Modal`), as pure functions so they can be tested
 * without a DOM.
 */

/** Elements that can take keyboard focus inside a dialog. */
export const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type='hidden'])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "summary",
  "[tabindex]:not([tabindex='-1'])",
  "[contenteditable='true']",
].join(", ");

/**
 * Where Tab should go inside a focus trap of `count` focusable items, given the index of the
 * focused one (`-1`: focus is on the container or outside the items) and the direction.
 *
 * - `null`: let the browser move focus (it stays inside the trap).
 * - `-1`: there is nothing focusable; keep focus where it is (the caller prevents default).
 * - an index: the caller prevents default and focuses that item (wrap-around, or entering
 *   the list from the container).
 */
export function nextTrapFocus(count: number, current: number, backwards: boolean): number | null {
  if (count <= 0) return -1;
  if (current < 0 || current >= count) return backwards ? count - 1 : 0;
  if (backwards && current === 0) return count - 1;
  if (!backwards && current === count - 1) return 0;
  return null;
}

/** Minimal shape of the element to return focus to (an `HTMLElement` in the app). */
export interface FocusReturnTarget {
  readonly isConnected: boolean;
  focus: (options?: { preventScroll?: boolean }) => void;
}

/**
 * Returns focus to the element that had it before a dialog opened, if it is still in the
 * document (the trigger may have been removed meanwhile). Returns whether it did.
 */
export function restoreFocus(target: FocusReturnTarget | null | undefined): boolean {
  if (!target || !target.isConnected) return false;
  target.focus({ preventScroll: true });
  return true;
}
