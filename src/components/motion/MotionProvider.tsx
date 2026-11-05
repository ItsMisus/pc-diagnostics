import { LazyMotion, MotionConfig, domMax } from "motion/react";
import type { ReactNode } from "react";

/**
 * App-wide Motion setup. `LazyMotion strict` forces the lightweight `m.*` components
 * (a stray `motion.*` import throws in development). `reducedMotion="user"` disables
 * transform and layout animations when Windows "Show animations" is off; opacity and
 * color changes remain, as recommended.
 */
export function MotionProvider({ children }: { children: ReactNode }) {
  return (
    <LazyMotion features={domMax} strict>
      <MotionConfig reducedMotion="user">{children}</MotionConfig>
    </LazyMotion>
  );
}
