import { AnimatePresence, m } from "motion/react";
import type { ReactNode } from "react";
import { pageVariants } from "@/lib/motion";

/**
 * Cross-fades between sections. `mode="wait"` keeps one page mounted at a time (each page
 * owns store subscriptions and polling), with a 90 ms exit so navigation never feels
 * blocked. `onExitComplete` lets the shell reset the scroll position between pages.
 */
export function PageTransition({
  pageKey,
  children,
  onExitComplete,
}: {
  pageKey: string;
  children: ReactNode;
  onExitComplete?: () => void;
}) {
  return (
    <AnimatePresence mode="wait" initial={false} {...(onExitComplete ? { onExitComplete } : {})}>
      <m.div key={pageKey} variants={pageVariants} initial="initial" animate="enter" exit="exit">
        {children}
      </m.div>
    </AnimatePresence>
  );
}
